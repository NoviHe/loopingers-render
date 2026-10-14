use std::{collections::VecDeque, path::Path, process::Stdio};

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

/// One FFmpeg run that loops the background video over the merged audio and burns subtitles.
pub struct LoopRender<'a> {
    pub tools: &'a Tools,
    pub video: &'a Path,
    pub audio: &'a Path,
    pub output: &'a Path,
    /// Directory holding the subtitle file; FFmpeg runs here so the `ass` filter needs no
    /// escaped drive-letter path.
    pub work_dir: &'a Path,
    pub subtitle_file: &'a str,
    pub duration: f64,
    pub size: Resolution,
    pub fit: FitMode,
    pub fps: u32,
    pub effect: &'a AppliedEffect,
    pub encoder: Encoder,
    pub preset: &'a str,
    pub quality: u32,
}

impl LoopRender<'_> {
    pub async fn run(&self, job: &JobHandle) -> Result<()> {
        let (width, height) = self.size;
        let mut graph = FilterGraph::new("0:v");
        self.fit.apply(&mut graph, width, height);
        self.effect.apply(&mut graph, 2, width, height);
        let filter = graph.finish(&format!("format=yuv420p,ass={}", self.subtitle_file));

        let mut cmd = self.tools.ffmpeg();
        cmd.current_dir(self.work_dir)
            .args(["-hide_banner", "-nostats", "-loglevel", "warning", "-y"])
            .args(["-stream_loop", "-1", "-i"])
            .arg(self.video)
            .arg("-i")
            .arg(self.audio)
            .args(self.effect.input_args())
            .arg("-t")
            .arg(self.duration.to_string())
            .arg("-filter_complex")
            .arg(&filter)
            .args(["-map", "[outv]", "-map", "1:a:0"])
            .args(self.encoder.args(self.preset, self.quality))
            .arg("-r")
            .arg(self.fps.to_string())
            .args(["-c:a", "copy", "-progress", "pipe:1"])
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
}
