use std::{collections::VecDeque, f64::consts::TAU, path::Path, process::Stdio};

use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};

use super::{
    effects::AppliedEffect, encoder::Encoder, graph::FilterGraph, job::JobHandle,
    presets::Resolution,
};
use crate::{
    error::{Error, Result},
    ffmpeg::{tail, Tools},
};

const MAX_PROGRESS_PERCENT: f64 = 99.9;
const STDERR_LINES_KEPT: usize = 100;
const ERROR_TAIL_CHARS: usize = 400;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FitMode {
    Crop,
    Fit,
    BlurPad,
}

impl FitMode {
    pub const ALL: [Self; 3] = [Self::Crop, Self::Fit, Self::BlurPad];

    pub fn label(self) -> &'static str {
        match self {
            Self::Crop => "Crop Penuh (Fill)",
            Self::Fit => "Letterbox",
            Self::BlurPad => "Blur Estetik",
        }
    }

    fn apply(self, graph: &mut FilterGraph, width: u32, height: u32) {
        match self {
            Self::Crop => graph.chain(&format!(
                "scale={width}:{height}:force_original_aspect_ratio=increase,crop={width}:{height}"
            )),
            Self::Fit => graph.chain(&format!(
                "scale={width}:{height}:force_original_aspect_ratio=decrease,\
                 pad={width}:{height}:(ow-iw)/2:(oh-ih)/2:black"
            )),
            Self::BlurPad => graph.stage(|input| {
                format!(
                    "[{input}]split[bg][fg];\
                     [bg]scale={width}:{height}:force_original_aspect_ratio=increase,\
                     crop={width}:{height},boxblur=20:5[bgblur];\
                     [fg]scale={width}:{height}:force_original_aspect_ratio=decrease[fgfit];\
                     [bgblur][fgfit]overlay=(W-w)/2:(H-h)/2"
                )
            }),
        }
    }
}

/// User-facing visual camera motion choice before per-render anti-duplicate resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VisualVariationMode {
    #[default]
    None,
    SlowZoom,
    SlowPan,
    Random,
}

impl VisualVariationMode {
    pub fn from_index(idx: usize) -> Self {
        match idx {
            1 => Self::SlowZoom,
            2 => Self::SlowPan,
            3 => Self::Random,
            _ => Self::None,
        }
    }
}

/// Slow sinusoidal visual movement applied on looping video renders when re-encoding.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum VisualVariation {
    #[default]
    None,
    SlowZoom {
        speed: f64,
        phase: f64,
    },
    SlowPan {
        speed_x: f64,
        speed_y: f64,
        phase_x: f64,
        phase_y: f64,
    },
    SlowZoomAndPan {
        zoom_speed: f64,
        zoom_phase: f64,
        pan_speed_x: f64,
        pan_speed_y: f64,
        pan_phase_x: f64,
        pan_phase_y: f64,
    },
}

/// Peak zoom scale yielding a 94% visible crop window (`1 / 0.94`).
const ZOOM_WINDOW_SCALE: f64 = 1.0 / 0.94;
/// Overscan scale yielding an 88% panning crop window (`1 / 0.88`).
const PAN_OVERSCAN_SCALE: f64 = 1.0 / 0.88;

fn even_overscan(dim: u32, scale: f64) -> u32 {
    ((f64::from(dim) * scale).round() as u32 + 1) & !1
}

fn zoom_scale_prefix(
    width: u32,
    height: u32,
    base: f64,
    amp: f64,
    speed: f64,
    phase: f64,
) -> String {
    format!(
        "scale=w='max({width},trunc({width}*({base:.4}+{amp:.4}*sin(t*{speed:.3}+{phase:.3}))/2)*2)':\
         h='max({height},trunc({height}*({base:.4}+{amp:.4}*sin(t*{speed:.3}+{phase:.3}))/2)*2)':\
         eval=frame:flags=bilinear"
    )
}

fn phase_from_percent(pct: u32) -> f64 {
    (f64::from(pct % 100) / 100.0) * TAU
}

fn slow_zoom_random(rng: &mut impl rand::Rng) -> (VisualVariation, String) {
    let phase_pct = rng.gen_range(0..100);
    (
        VisualVariation::SlowZoom {
            speed: rng.gen_range(0.18..0.30),
            phase: phase_from_percent(phase_pct),
        },
        format!("Zoom pelan · fase {phase_pct}%"),
    )
}

fn slow_pan_random(rng: &mut impl rand::Rng) -> (VisualVariation, String) {
    let phase_pct = rng.gen_range(0..100);
    let phase_y_pct = (phase_pct + rng.gen_range(17..73)) % 100;
    (
        VisualVariation::SlowPan {
            speed_x: rng.gen_range(0.18..0.28),
            speed_y: rng.gen_range(0.13..0.22),
            phase_x: phase_from_percent(phase_pct),
            phase_y: phase_from_percent(phase_y_pct),
        },
        format!("Geser pelan · fase {phase_pct}%"),
    )
}

fn slow_zoom_and_pan_random(rng: &mut impl rand::Rng) -> (VisualVariation, String) {
    let phase_pct = rng.gen_range(0..100);
    (
        VisualVariation::SlowZoomAndPan {
            zoom_speed: rng.gen_range(0.18..0.28),
            zoom_phase: phase_from_percent(phase_pct),
            pan_speed_x: rng.gen_range(0.16..0.25),
            pan_speed_y: rng.gen_range(0.12..0.20),
            pan_phase_x: rng.gen_range(0.0..TAU),
            pan_phase_y: rng.gen_range(0.0..TAU),
        },
        format!("Zoom & Geser pelan · fase {phase_pct}%"),
    )
}

fn pick_random_motion(rng: &mut impl rand::Rng) -> (VisualVariation, String) {
    match rng.gen_range(0..3) {
        0 => slow_zoom_random(rng),
        1 => slow_pan_random(rng),
        _ => slow_zoom_and_pan_random(rng),
    }
}

/// Resolves concrete camera parameters and display label for a render.
/// When `anti_duplicate` is enabled, even `None` gets a randomized camera motion and phase
/// so repeated renders of the same album never produce duplicate video frames.
pub fn resolve_visual_variation_with_rng(
    mode: VisualVariationMode,
    anti_duplicate: bool,
    rng: &mut impl rand::Rng,
) -> (VisualVariation, String) {
    match mode {
        VisualVariationMode::None => {
            if anti_duplicate {
                pick_random_motion(rng)
            } else {
                (VisualVariation::None, "Tanpa".to_owned())
            }
        }
        VisualVariationMode::SlowZoom => {
            if anti_duplicate {
                slow_zoom_random(rng)
            } else {
                (VisualVariation::slow_zoom(), "Zoom pelan".to_owned())
            }
        }
        VisualVariationMode::SlowPan => {
            if anti_duplicate {
                slow_pan_random(rng)
            } else {
                (VisualVariation::slow_pan(), "Geser pelan".to_owned())
            }
        }
        VisualVariationMode::Random => pick_random_motion(rng),
    }
}

/// Resolves concrete camera parameters using thread-local randomness.
pub fn resolve_visual_variation(
    mode: VisualVariationMode,
    anti_duplicate: bool,
) -> (VisualVariation, String) {
    let mut rng = rand::thread_rng();
    resolve_visual_variation_with_rng(mode, anti_duplicate, &mut rng)
}

impl VisualVariation {
    pub fn slow_zoom() -> Self {
        Self::SlowZoom {
            speed: 0.24,
            phase: 0.0,
        }
    }

    pub fn slow_pan() -> Self {
        Self::SlowPan {
            speed_x: 0.22,
            speed_y: 0.16,
            phase_x: 0.0,
            phase_y: 1.1,
        }
    }

    pub fn random() -> (Self, &'static str) {
        let mut rng = rand::thread_rng();
        let (variation, _) = pick_random_motion(&mut rng);
        let label = match variation {
            Self::SlowZoom { .. } => "Acak -> Zoom pelan",
            Self::SlowPan { .. } => "Acak -> Geser pelan",
            Self::SlowZoomAndPan { .. } => "Acak -> Zoom & Geser pelan",
            Self::None => "Tanpa",
        };
        (variation, label)
    }

    pub fn phase_percent(&self) -> Option<u32> {
        let phase = match *self {
            Self::None => return None,
            Self::SlowZoom { phase, .. } => phase,
            Self::SlowPan { phase_x, .. } => phase_x,
            Self::SlowZoomAndPan { zoom_phase, .. } => zoom_phase,
        };
        Some(((phase.rem_euclid(TAU) / TAU) * 100.0).round() as u32 % 100)
    }

    pub fn apply(&self, graph: &mut FilterGraph, fit: FitMode, width: u32, height: u32) {
        match *self {
            Self::None => fit.apply(graph, width, height),
            Self::SlowZoom { speed, phase } => {
                fit.apply(graph, width, height);
                let base = (1.0 + ZOOM_WINDOW_SCALE) * 0.5;
                let amp = (ZOOM_WINDOW_SCALE - 1.0) * 0.5;
                let scale = zoom_scale_prefix(width, height, base, amp, speed, phase);
                graph.chain(&format!(
                    "{scale},crop={width}:{height}:(iw-{width})/2:(ih-{height})/2"
                ));
            }
            Self::SlowPan {
                speed_x,
                speed_y,
                phase_x,
                phase_y,
            } => {
                let ov_w = even_overscan(width, PAN_OVERSCAN_SCALE);
                let ov_h = even_overscan(height, PAN_OVERSCAN_SCALE);
                if fit == FitMode::Crop {
                    fit.apply(graph, ov_w, ov_h);
                } else {
                    fit.apply(graph, width, height);
                    graph.chain(&format!("scale={ov_w}:{ov_h}:flags=lanczos"));
                }
                graph.chain(&format!(
                    "crop={width}:{height}:\
                     '(iw-{width})*0.5*(1+sin(t*{speed_x:.3}+{phase_x:.3}))':\
                     '(ih-{height})*0.5*(1+cos(t*{speed_y:.3}+{phase_y:.3}))'"
                ));
            }
            Self::SlowZoomAndPan {
                zoom_speed,
                zoom_phase,
                pan_speed_x,
                pan_speed_y,
                pan_phase_x,
                pan_phase_y,
            } => {
                fit.apply(graph, width, height);
                let base = (ZOOM_WINDOW_SCALE + PAN_OVERSCAN_SCALE) * 0.5;
                let amp = (PAN_OVERSCAN_SCALE - ZOOM_WINDOW_SCALE) * 0.5;
                let scale = zoom_scale_prefix(width, height, base, amp, zoom_speed, zoom_phase);
                graph.chain(&format!(
                    "{scale},crop={width}:{height}:\
                     '(iw-{width})*0.5*(1+sin(t*{pan_speed_x:.3}+{pan_phase_x:.3}))':\
                     '(ih-{height})*0.5*(1+cos(t*{pan_speed_y:.3}+{pan_phase_y:.3}))'"
                ));
            }
        }
    }
}

/// One FFmpeg run that loops the background video over the merged audio and burns subtitles.
pub struct LoopRender<'a> {
    pub tools: &'a Tools,
    pub video: &'a Path,
    pub audio: &'a Path,
    pub output: &'a Path,
    /// Directory holding the subtitle file; FFmpeg runs here so the `ass` filter needs no
    /// escaped drive-letter path.
    pub work_dir: &'a Path,
    pub subtitle_file: Option<&'a str>,
    pub overlay_png: Option<&'a Path>,
    pub duration: f64,
    pub size: Resolution,
    pub fit: FitMode,
    pub fps: u32,
    pub effect: &'a AppliedEffect,
    pub encoder: Encoder,
    pub preset: &'a str,
    pub quality: u32,
    pub stream_copy_video: bool,
    pub youtube_standard: bool,
    pub visual_variation: VisualVariation,
    pub visualizer: Option<&'a crate::visualizer::VisualizerSpec>,
    pub chapters_file: Option<&'a Path>,
    pub metadata_comment: Option<&'a str>,
}

impl LoopRender<'_> {
    pub async fn run(&self, job: &JobHandle) -> Result<()> {
        let (width, height) = self.size;
        let mut cmd = self.tools.ffmpeg();
        cmd.current_dir(self.work_dir)
            .args(["-hide_banner", "-nostats", "-loglevel", "warning", "-y"])
            .args(["-stream_loop", "-1", "-i"])
            .arg(self.video)
            .arg("-i")
            .arg(self.audio);

        let mut next_input_idx = 2usize;
        let mut chapters_input_idx = None;

        if self.stream_copy_video {
            if let Some(chapters) = self.chapters_file {
                cmd.args(["-f", "ffmetadata", "-i"]).arg(chapters);
                chapters_input_idx = Some(next_input_idx);
            }
            cmd.arg("-t")
                .arg(self.duration.to_string())
                .args(["-map", "0:v:0", "-map", "1:a:0", "-c:v", "copy"]);
        } else {
            let mut graph = FilterGraph::new("0:v");
            self.visual_variation
                .apply(&mut graph, self.fit, width, height);
            self.effect.apply(&mut graph, 2, width, height);
            if let Some(viz) = self.visualizer {
                if viz.enabled {
                    viz.apply_ffmpeg(&mut graph, width, height);
                }
            }
            if self.overlay_png.is_some() {
                let overlay_idx = if self.effect.input_args().is_empty() {
                    2
                } else {
                    3
                };
                graph.stage(|input| format!("[{input}][{overlay_idx}:v]overlay=0:0"));
            }
            let tail_filter = match self.subtitle_file {
                Some(sub) => format!("format=yuv420p,ass={sub}:fontsdir=."),
                None => "format=yuv420p".to_owned(),
            };
            let filter = graph.finish(&tail_filter);

            let effect_args = self.effect.input_args();
            next_input_idx += effect_args.iter().filter(|a| *a == "-i").count();
            cmd.args(&effect_args);
            if let Some(png) = self.overlay_png {
                cmd.arg("-i").arg(png);
                next_input_idx += 1;
            }
            if let Some(chapters) = self.chapters_file {
                cmd.args(["-f", "ffmetadata", "-i"]).arg(chapters);
                chapters_input_idx = Some(next_input_idx);
            }
            cmd.arg("-t")
                .arg(self.duration.to_string())
                .arg("-filter_complex")
                .arg(&filter)
                .args(["-map", "[outv]", "-map", "1:a:0"])
                .args(self.encoder.args(self.preset, self.quality))
                .arg("-r")
                .arg(self.fps.to_string());
            if self.youtube_standard && !self.encoder.is_hevc() {
                let gop = (self.fps.max(1) * 2).to_string();
                cmd.args([
                    "-profile:v",
                    "high",
                    "-g",
                    &gop,
                    "-colorspace",
                    "bt709",
                    "-color_primaries",
                    "bt709",
                    "-color_trc",
                    "bt709",
                ]);
                match self.encoder {
                    Encoder::CpuX264 => {
                        cmd.args(["-bf", "2", "-flags", "+cgop"]);
                    }
                    Encoder::NvencH264 => {
                        cmd.args(["-bf", "2"]);
                    }
                    _ => {}
                }
            }
        }

        cmd.args(metadata_chapter_args(
            chapters_input_idx,
            self.metadata_comment,
        ))
        .args([
            "-c:a",
            "copy",
            "-movflags",
            "+faststart",
            "-progress",
            "pipe:1",
        ])
        .arg(self.output)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|error| Error::Tool(format!("FFmpeg tidak dapat dijalankan: {error}")))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::internal("stdout FFmpeg tidak tersedia"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| Error::internal("stderr FFmpeg tidak tersedia"))?;
        let stderr_tail = tokio::spawn(collect_tail(stderr));

        let mut lines = BufReader::new(stdout).lines();
        let mut speed = 0.0;
        while let Some(line) = lines.next_line().await? {
            match parse_progress(&line) {
                Some(Progress::Time(seconds)) => {
                    let total = self.duration;
                    job.update(|job| {
                        job.current_time_sec = seconds.min(total);
                        job.progress_percent = (seconds / total * 100.0).min(MAX_PROGRESS_PERCENT);
                        job.eta_seconds = eta(total, job.current_time_sec, speed);
                    });
                }
                Some(Progress::Speed(value)) => {
                    speed = value;
                    job.update(|job| {
                        job.speed = format!("{value}x");
                        job.eta_seconds = eta(job.total_time_sec, job.current_time_sec, value);
                    });
                }
                Some(Progress::Fps(value)) => job.update(|job| job.fps = value),
                None => {}
            }
        }

        let status = child.wait().await?;
        let stderr_text = stderr_tail.await.unwrap_or_default();
        if status.success() {
            return Ok(());
        }
        let code = status
            .code()
            .map_or_else(|| "tidak diketahui".to_owned(), |c| c.to_string());
        Err(Error::Tool(format!(
            "FFmpeg gagal dengan kode {code}: {}",
            tail(&stderr_text, ERROR_TAIL_CHARS)
        )))
    }
}

fn metadata_chapter_args(chapters_input_idx: Option<usize>, comment: Option<&str>) -> Vec<String> {
    let mut args = if let Some(idx) = chapters_input_idx {
        let s = idx.to_string();
        vec![
            "-map_metadata".into(),
            s.clone(),
            "-map_metadata:s".into(),
            "-1".into(),
            "-map_chapters".into(),
            s,
        ]
    } else {
        vec![
            "-map_metadata".into(),
            "-1".into(),
            "-map_chapters".into(),
            "-1".into(),
        ]
    };
    if let Some(comment) = comment.filter(|c| !c.trim().is_empty()) {
        args.push("-metadata".into());
        args.push(format!("comment={comment}"));
    }
    args
}

fn eta(total: f64, current: f64, speed: f64) -> f64 {
    if speed > 0.0 && total > current {
        (total - current) / speed
    } else {
        0.0
    }
}

/// Keeps only the last lines of FFmpeg's stderr so a long render cannot fill memory.
async fn collect_tail(reader: impl AsyncRead + Unpin) -> String {
    let mut kept = VecDeque::with_capacity(STDERR_LINES_KEPT);
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if kept.len() == STDERR_LINES_KEPT {
            kept.pop_front();
        }
        kept.push_back(line);
    }
    kept.into_iter().collect::<Vec<_>>().join("\n")
}

#[derive(Debug, PartialEq)]
enum Progress {
    Time(f64),
    Speed(f64),
    Fps(f64),
}

/// Parses one `key=value` line of `-progress` output.
fn parse_progress(line: &str) -> Option<Progress> {
    let (key, value) = line.trim().split_once('=')?;
    let value = value.trim();
    match key {
        // Both keys carry microseconds; "N/A" and negative values appear before the first frame.
        "out_time_us" | "out_time_ms" => value
            .parse::<i64>()
            .ok()
            .filter(|micros| *micros >= 0)
            .map(|micros| Progress::Time(micros as f64 / 1_000_000.0)),
        "speed" => value
            .trim_end_matches('x')
            .trim()
            .parse()
            .ok()
            .map(Progress::Speed),
        "fps" => value.parse().ok().map(Progress::Fps),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_lines_are_parsed() {
        assert_eq!(
            parse_progress("out_time_ms=2500000"),
            Some(Progress::Time(2.5))
        );
        assert_eq!(parse_progress("speed= 2.5x"), Some(Progress::Speed(2.5)));
        assert_eq!(parse_progress("fps=59.94"), Some(Progress::Fps(59.94)));
        assert_eq!(parse_progress("out_time_us=N/A"), None);
        assert_eq!(parse_progress("speed=N/A"), None);
        assert_eq!(parse_progress("progress=continue"), None);
    }

    #[test]
    fn eta_needs_a_positive_speed() {
        assert_eq!(eta(100.0, 50.0, 2.0), 25.0);
        assert_eq!(eta(100.0, 50.0, 0.0), 0.0);
        assert_eq!(eta(100.0, 100.0, 2.0), 0.0);
    }

    #[test]
    fn blur_pad_graph_keeps_the_input_label() {
        let mut graph = FilterGraph::new("0:v");
        FitMode::BlurPad.apply(&mut graph, 1280, 720);
        let text = graph.finish("null");
        assert!(text.starts_with("[0:v]split[bg][fg];"));
        assert!(text.ends_with("overlay=(W-w)/2:(H-h)/2[s1];[s1]null[outv]"));
    }

    #[test]
    fn visual_variation_builds_zoom_and_pan_stages() {
        let mut zoom_graph = FilterGraph::new("0:v");
        VisualVariation::slow_zoom().apply(&mut zoom_graph, FitMode::Crop, 1920, 1080);
        let zoom_text = zoom_graph.finish("format=yuv420p");
        assert!(zoom_text.contains("eval=frame:flags=bilinear"));
        assert!(zoom_text.contains("1.0319+0.0319*sin("));
        assert!(zoom_text.contains("crop=1920:1080"));

        let mut pan_graph = FilterGraph::new("0:v");
        VisualVariation::slow_pan().apply(&mut pan_graph, FitMode::Crop, 1280, 720);
        let pan_text = pan_graph.finish("format=yuv420p");
        assert!(pan_text.contains("scale=1456:818"));
        assert!(pan_text.contains("crop=1280:720"));

        let mut combo_graph = FilterGraph::new("0:v");
        VisualVariation::SlowZoomAndPan {
            zoom_speed: 0.22,
            zoom_phase: 0.0,
            pan_speed_x: 0.20,
            pan_speed_y: 0.15,
            pan_phase_x: 0.0,
            pan_phase_y: 0.5,
        }
        .apply(&mut combo_graph, FitMode::Crop, 1280, 720);
        let combo_text = combo_graph.finish("format=yuv420p");
        assert!(combo_text.contains("1.1001+0.0363*sin("));
        assert!(combo_text.contains("crop=1280:720"));
    }

    #[test]
    fn builds_metadata_and_chapter_mapping_args() {
        assert_eq!(
            metadata_chapter_args(None, None),
            vec!["-map_metadata", "-1", "-map_chapters", "-1"]
        );
        assert_eq!(
            metadata_chapter_args(Some(2), Some("RenderPro abc-123")),
            vec![
                "-map_metadata",
                "2",
                "-map_metadata:s",
                "-1",
                "-map_chapters",
                "2",
                "-metadata",
                "comment=RenderPro abc-123",
            ]
        );
    }

    #[test]
    fn resolves_anti_duplicate_visual_variation_with_phase() {
        use rand::{rngs::StdRng, SeedableRng};
        let mut rng_a = StdRng::seed_from_u64(42);
        let mut rng_b = StdRng::seed_from_u64(99);

        // Even Mode::None receives random camera motion + phase when anti-duplicate is on.
        let (var_a, label_a) =
            resolve_visual_variation_with_rng(VisualVariationMode::None, true, &mut rng_a);
        let (var_b, label_b) =
            resolve_visual_variation_with_rng(VisualVariationMode::None, true, &mut rng_b);
        assert_ne!(var_a, VisualVariation::None);
        assert_ne!(var_a, var_b);
        assert!(label_a.contains(" · fase "));
        assert!(label_b.contains(" · fase "));

        // Specific Zoom mode still randomizes phase across renders when anti-duplicate is on.
        let (z1, zl1) =
            resolve_visual_variation_with_rng(VisualVariationMode::SlowZoom, true, &mut rng_a);
        let (z2, zl2) =
            resolve_visual_variation_with_rng(VisualVariationMode::SlowZoom, true, &mut rng_b);
        assert_ne!(z1, z2);
        assert!(zl1.starts_with("Zoom pelan · fase "));
        assert!(zl2.starts_with("Zoom pelan · fase "));
        assert!(z1.phase_percent().is_some());

        // When anti-duplicate is off, Mode::None remains static for stream-copy.
        let (static_var, static_label) =
            resolve_visual_variation_with_rng(VisualVariationMode::None, false, &mut rng_a);
        assert_eq!(static_var, VisualVariation::None);
        assert_eq!(static_label, "Tanpa");
    }
}
