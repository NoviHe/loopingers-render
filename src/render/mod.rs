pub mod audio;
pub mod batch;
pub mod effects;
pub mod encoder;
pub mod graph;
pub mod image_video;
pub mod job;
pub mod looper;
pub mod presets;
pub mod request;

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    config::AppPaths,
    error::{Error, Result},
    ffmpeg::Tools,
    hardware::HardwareInfo,
    karaoke::ass::{build_ass, AssStyle, SubtitleTrack},
    media::{
        build_timed_chapters, clean_artist, clean_proper_title, format_ffmetadata,
        format_playlist_txt, format_timestamp_txt,
    },
    util::{random_code, unique_render_comment},
};
use effects::AppliedEffect;
use encoder::Encoder;
use image_video::{estimate_tempo_bpm, ImageVideo, DEFAULT_BPM};
use job::{JobHandle, JobStatus, RenderStage, RenderTimings};
pub use looper::VisualVariationMode;
use looper::{resolve_visual_variation, LoopRender, VisualVariation};
use request::RenderRequest;

/// Per-render uniqueness envelope shared by single Generate renders and future bulk renders.
#[derive(Debug, Clone, PartialEq)]
pub struct AntiDuplicateStamp {
    pub comment: Option<String>,
    pub visual_variation: VisualVariation,
    pub visual_label: String,
}

/// Builds a fresh per-render identity (unique `comment` UUID + randomized camera motion/phase).
pub fn build_anti_duplicate_stamp(
    mode: VisualVariationMode,
    anti_duplicate: bool,
) -> AntiDuplicateStamp {
    let (visual_variation, visual_label) = resolve_visual_variation(mode, anti_duplicate);
    let comment = if anti_duplicate {
        Some(unique_render_comment())
    } else {
        None
    };
    AntiDuplicateStamp {
        comment,
        visual_variation,
        visual_label,
    }
}

/// Everything a render worker needs besides the request itself.
#[derive(Clone)]
pub struct RenderContext {
    pub paths: AppPaths,
    pub tools: Tools,
    pub hardware: HardwareInfo,
}

/// Removes the intermediate files of a render, whether it succeeded or not.
#[derive(Default)]
struct TempFiles(Vec<PathBuf>);

impl TempFiles {
    fn track(&mut self, path: &Path) {
        self.0.push(path.to_path_buf());
    }
}

impl Drop for TempFiles {
    fn drop(&mut self) {
        for path in &self.0 {
            let _ = fs::remove_file(path);
        }
    }
}

/// A snapshot of the editor settings used to create a Generate bundle thumbnail.
#[derive(Debug, Clone)]
pub struct AutoThumbnailRequest {
    pub spec: crate::thumbnail::ThumbnailSpec,
    pub timestamp: f64,
}

#[derive(Debug, Clone)]
pub struct SimpleRenderRequest {
    pub video_path: PathBuf,
    pub audio_files: Vec<PathBuf>,
    pub track_titles: Vec<String>,
    pub thumbnail_path: Option<PathBuf>,
    pub auto_thumbnail: Option<AutoThumbnailRequest>,
    pub output_dir: PathBuf,
    pub size: (u32, u32),
    pub fps: u32,
    pub encoder: Encoder,
    pub preset: String,
    pub quality: u32,
    pub audio_bitrate: String,
    pub stream_copy_video: bool,
    pub youtube_standard: bool,
    pub visual_mode: VisualVariationMode,
    pub anti_duplicate: bool,
    pub output_code: Option<String>,
    pub pre_resolved_stamp: Option<AntiDuplicateStamp>,
    pub fade_out_secs: f64,
    pub crossfade_secs: f64,
    pub normalize_volume: bool,
    pub target_duration_secs: Option<f64>,
}

/// Runs a render job to completion and records the outcome on `job`.
pub async fn execute(context: &RenderContext, job: &JobHandle, request: RenderRequest) {
    job.start_timing();
    if let Err(error) = run_pipeline(context, job, request).await {
        job.update(|job| job.fail(error.to_string()));
    }
}

/// Runs a simple Generate render combining a looping video with concatenated music tracks.
pub async fn execute_simple(
    context: &RenderContext,
    job: &JobHandle,
    request: SimpleRenderRequest,
) {
    job.start_timing();
    if let Err(error) = run_simple_pipeline(context, job, request).await {
        job.update(|job| job.fail(error.to_string()));
    }
}

fn require_file(path: &Path, message: &str) -> Result<()> {
    if path.is_file() {
        Ok(())
    } else {
        Err(Error::invalid(format!("{message}: {}", path.display())))
    }
}

async fn run_pipeline(
    context: &RenderContext,
    job: &JobHandle,
    request: RenderRequest,
) -> Result<()> {
    let paths = &context.paths;
    let mut temp = TempFiles::default();
    job.update(|job| job.status = JobStatus::Preparing);

    require_file(&request.video_path, "File video background tidak ditemukan")?;
    let audio_files: Vec<PathBuf> = request
        .tracks
        .iter()
        .map(|track| track.path.clone())
        .collect();
    for audio in &audio_files {
        require_file(audio, "File audio tidak ditemukan")?;
    }

    let mut video = request.video_path.clone();
    let mut effect_id = request.visual_effect.as_str();
    if let Some(baked) = bake_image_background(context, job, &request, &audio_files).await {
        temp.track(&baked);
        video = baked;
        effect_id = "none";
    }

    let (width, height) = presets::dimensions(&request.aspect_ratio, &request.resolution);
    let has_subtitles = request.style.show_lyrics == Some(true)
        || request.style.show_intro == Some(true)
        || request.style.show_tracklist == Some(true);
    let subtitle_file = if has_subtitles {
        let subtitle_tracks: Vec<SubtitleTrack> = request
            .tracks
            .iter()
            .map(|track| SubtitleTrack {
                title: clean_proper_title(&track.title),
                artist: clean_artist(&track.artist),
                duration: track.duration,
                lyrics: track.lyrics.clone(),
            })
            .collect();
        let style = AssStyle::from_config(&request.style, height);
        let file = format!("subtitles_{}.ass", job.id());
        let subtitle_path = paths.temp.join(&file);
        tokio::fs::write(
            &subtitle_path,
            build_ass(&subtitle_tracks, width, height, &style),
        )
        .await?;
        temp.track(&subtitle_path);

        if let Some(font_path) = &request.style.font_path {
            if font_path.is_file() {
                if let Some(file_name) = font_path.file_name() {
                    let dest_font = paths.temp.join(file_name);
                    let _ = tokio::fs::copy(font_path, &dest_font).await;
                    temp.track(&dest_font);
                }
            }
        }

        Some(file)
    } else {
        None
    };

    let overlay_png = if let Some(spec) = request.tracklist_overlay.clone() {
        let png_path = paths.temp.join(format!("overlay_{}.png", job.id()));
        let png_out = png_path.clone();
        tokio::task::spawn_blocking(move || {
            crate::thumbnail::render_overlay_png(&spec, width, height, &png_out)
        })
        .await
        .map_err(|error| {
            Error::internal(format!("Proses overlay daftar lagu terhenti: {error}"))
        })??;
        temp.track(&png_path);
        Some(png_path)
    } else {
        None
    };

    let duration_fallback: f64 = request.tracks.iter().map(|track| track.duration).sum();
    let file_name = if request.custom_filename.trim().is_empty() {
        format!("loopingers_render_{}.mp4", job.id())
    } else {
        let clean_prefix = request
            .custom_filename
            .trim()
            .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        format!("{clean_prefix}_loopingers_render_{}.mp4", job.id())
    };
    let output = paths.outputs.join(file_name);

    merge_and_render(
        context,
        job,
        &mut temp,
        MergeAndRender {
            video: &video,
            audio_files: &audio_files,
            output_path: Some(output),
            generate_bundle: None,
            duration_fallback,
            subtitle_file: subtitle_file.as_deref(),
            overlay_png: overlay_png.as_deref(),
            size: (width, height),
            fit: request.fit,
            fps: request.fps,
            effect_id,
            encoder: request.encoder,
            preset: &request.preset,
            quality: request.quality,
            audio_bitrate: &request.audio_bitrate,
            stream_copy_video: false,
            youtube_standard: request.youtube_standard,
            visual_variation: VisualVariation::None,
            visual_label: None,
            metadata_comment: None,
            fade_out_secs: 0.0,
            crossfade_secs: 0.0,
            normalize_volume: false,
            target_duration_secs: None,
            visualizer: request.visualizer.as_ref(),
        },
    )
    .await
}

async fn run_simple_pipeline(
    context: &RenderContext,
    job: &JobHandle,
    request: SimpleRenderRequest,
) -> Result<()> {
    let mut temp = TempFiles::default();
    job.update(|job| job.status = JobStatus::Preparing);

    require_file(&request.video_path, "File video visual tidak ditemukan")?;
    if request.audio_files.is_empty() {
        return Err(Error::invalid("Playlist lagu masih kosong"));
    }
    for audio in &request.audio_files {
        require_file(audio, "File audio tidak ditemukan")?;
    }

    tokio::fs::create_dir_all(&request.output_dir).await?;
    let size = normalize_video_size(request.size);
    let fps = request.fps.clamp(1, 120);
    let stamp = request
        .pre_resolved_stamp
        .unwrap_or_else(|| build_anti_duplicate_stamp(request.visual_mode, request.anti_duplicate));
    let stream_copy_video =
        request.stream_copy_video && stamp.visual_variation == VisualVariation::None;

    merge_and_render(
        context,
        job,
        &mut temp,
        MergeAndRender {
            video: &request.video_path,
            audio_files: &request.audio_files,
            output_path: None,
            generate_bundle: Some(GenerateBundle {
                base_output_dir: request.output_dir,
                track_titles: request.track_titles,
                thumbnail_path: request.thumbnail_path,
                auto_thumbnail: request.auto_thumbnail,
                output_code: request.output_code,
            }),
            duration_fallback: request.target_duration_secs.unwrap_or(0.0),
            subtitle_file: None,
            overlay_png: None,
            size,
            fit: looper::FitMode::Crop,
            fps,
            effect_id: "none",
            encoder: request.encoder,
            preset: &request.preset,
            quality: request.quality,
            audio_bitrate: &request.audio_bitrate,
            stream_copy_video,
            youtube_standard: request.youtube_standard,
            visual_variation: stamp.visual_variation,
            visual_label: Some(stamp.visual_label),
            metadata_comment: stamp.comment,
            fade_out_secs: request.fade_out_secs,
            crossfade_secs: request.crossfade_secs,
            normalize_volume: request.normalize_volume,
            target_duration_secs: request.target_duration_secs,
            visualizer: None,
        },
    )
    .await
}

fn normalize_video_size(size: (u32, u32)) -> (u32, u32) {
    let (w, h) = size;
    if w == 0 || h == 0 {
        (1920, 1080)
    } else {
        ((w & !1).max(2), (h & !1).max(2))
    }
}

struct GenerateBundle {
    output_code: Option<String>,
    base_output_dir: PathBuf,
    track_titles: Vec<String>,
    thumbnail_path: Option<PathBuf>,
    auto_thumbnail: Option<AutoThumbnailRequest>,
}

struct RenderReport<'a> {
    encoder: &'a str,
    preset: &'a str,
    size: (u32, u32),
    fps: u32,
    duration: f64,
    stream_copy: bool,
    normalize_volume: bool,
    visual_label: &'a str,
    timings: &'a RenderTimings,
}

fn format_render_report(report: &RenderReport<'_>) -> String {
    let realtime = if report.timings.video_seconds > 0.0 {
        format!("{:.2}x", report.duration / report.timings.video_seconds)
    } else {
        "Belum terukur".to_owned()
    };
    let mode = if report.stream_copy {
        "Salin video langsung"
    } else {
        "Encode ulang"
    };
    let preset = if report.stream_copy {
        "Tidak digunakan"
    } else {
        report.preset
    };
    let normalize = if report.normalize_volume {
        "Aktif"
    } else {
        "Off"
    };
    format!(
        "LoopKaraoke Studio v{} - Laporan Render\n\
         Mode: {mode}\nEncoder aktual: {}\nPreset aktual: {preset}\n\
         Resolusi tujuan: {}x{}\nFPS tujuan: {}\nDurasi hasil: {:.3} detik\n\
         Normalisasi audio: {normalize}\nVariasi visual: {}\n\n\
         Waktu persiapan: {:.3} detik\nWaktu audio: {:.3} detik\n\
         Waktu video: {:.3} detik\nWaktu penyimpanan hasil: {:.3} detik\n\
         Total waktu: {:.3} detik\nKecepatan tahap video: {realtime}\n\n\
         Waktu di atas tidak mencakup penulisan laporan ini.\n",
        env!("CARGO_PKG_VERSION"),
        report.encoder,
        report.size.0,
        report.size.1,
        report.fps,
        report.duration,
        report.visual_label,
        report.timings.preparation_seconds,
        report.timings.audio_seconds,
        report.timings.video_seconds,
        report.timings.finalizing_seconds,
        report.timings.total_elapsed_seconds,
    )
}

struct MergeAndRender<'a> {
    video: &'a Path,
    audio_files: &'a [PathBuf],
    output_path: Option<PathBuf>,
    generate_bundle: Option<GenerateBundle>,
    duration_fallback: f64,
    subtitle_file: Option<&'a str>,
    overlay_png: Option<&'a Path>,
    size: (u32, u32),
    fit: looper::FitMode,
    fps: u32,
    effect_id: &'a str,
    encoder: Encoder,
    preset: &'a str,
    quality: u32,
    audio_bitrate: &'a str,
    stream_copy_video: bool,
    youtube_standard: bool,
    visual_variation: VisualVariation,
    visual_label: Option<String>,
    metadata_comment: Option<String>,
    fade_out_secs: f64,
    crossfade_secs: f64,
    normalize_volume: bool,
    target_duration_secs: Option<f64>,
    visualizer: Option<&'a crate::visualizer::VisualizerSpec>,
}

async fn create_unique_render_dir(base_dir: &Path) -> Result<PathBuf> {
    tokio::fs::create_dir_all(base_dir).await?;
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let candidate = base_dir.join(&stamp);
    let dir = if !candidate.exists() {
        candidate
    } else {
        let mut chosen = base_dir.join(format!("{stamp}_{}", random_code(4)));
        while chosen.exists() {
            chosen = base_dir.join(format!("{stamp}_{}", random_code(4)));
        }
        chosen
    };
    tokio::fs::create_dir_all(&dir).await?;
    Ok(dir)
}

fn unique_video_filename(render_dir: &Path) -> String {
    loop {
        let candidate = format!("video_{}.mp4", random_code(6));
        if !render_dir.join(&candidate).exists() {
            return candidate;
        }
    }
}

fn resolve_raw_titles(audio_files: &[PathBuf], track_titles: &[String]) -> Vec<String> {
    audio_files
        .iter()
        .enumerate()
        .map(|(idx, path)| {
            track_titles
                .get(idx)
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .or_else(|| {
                    path.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .filter(|n| !n.is_empty())
                })
                .unwrap_or_else(|| format!("Track {}", idx + 1))
        })
        .collect()
}

async fn merge_and_render(
    context: &RenderContext,
    job: &JobHandle,
    temp: &mut TempFiles,
    params: MergeAndRender<'_>,
) -> Result<()> {
    let RenderContext {
        paths,
        tools,
        hardware,
    } = context;

    let master_audio = paths.temp.join(format!("master_audio_{}.m4a", job.id()));
    temp.track(&master_audio);
    job.set_stage(RenderStage::Audio);
    let merged = audio::merge_tracks_detailed(
        tools,
        params.audio_files,
        &master_audio,
        &audio::AudioMergeOptions {
            bitrate: params.audio_bitrate.to_owned(),
            crossfade_secs: params.crossfade_secs,
            fade_out_secs: params.fade_out_secs,
            normalize_volume: params.normalize_volume,
            target_duration_secs: params.target_duration_secs,
        },
        params.generate_bundle.is_some(),
    )
    .await?;
    job.set_stage(RenderStage::Preparing);
    let duration = if merged.duration > 0.0 {
        merged.duration
    } else {
        params.duration_fallback
    };

    let (output_path, bundle_outputs) = if let Some(bundle) = params.generate_bundle {
        let render_dir = create_unique_render_dir(&bundle.base_output_dir).await?;
        let filename = match bundle.output_code {
            Some(code) if code.len() == 6 && code.bytes().all(|c| c.is_ascii_alphanumeric()) => {
                format!("video_{code}.mp4")
            }
            Some(_) => return Err(Error::invalid("Kode nama keluaran tidak valid")),
            None => unique_video_filename(&render_dir),
        };
        let output = render_dir.join(filename);
        let raw_titles = resolve_raw_titles(params.audio_files, &bundle.track_titles);
        let chapters = build_timed_chapters(
            &raw_titles,
            &merged.track_durations,
            params.crossfade_secs,
            duration,
        );
        let ffmeta_file = if chapters.is_empty() {
            None
        } else {
            let meta_path = paths.temp.join(format!("chapters_{}.ffmeta", job.id()));
            tokio::fs::write(&meta_path, format_ffmetadata(&chapters)).await?;
            temp.track(&meta_path);
            Some(meta_path)
        };
        let playlist_txt = format_playlist_txt(&raw_titles);
        let timestamp_txt = format_timestamp_txt(&chapters);
        (
            output,
            Some((
                render_dir,
                playlist_txt,
                timestamp_txt,
                bundle.thumbnail_path,
                ffmeta_file,
                bundle.auto_thumbnail,
            )),
        )
    } else {
        let output = params
            .output_path
            .ok_or_else(|| Error::internal("Path output render tidak tersedia"))?;
        (output, None)
    };

    let encoder = Encoder::resolve(params.encoder, hardware);
    let resolved_preset = if encoder.presets().contains(&params.preset) {
        params.preset
    } else {
        encoder.default_preset()
    };
    let encoder_label = if params.stream_copy_video {
        "Stream Copy (Kualitas Asli)".to_owned()
    } else {
        encoder.label(&hardware.gpu_name)
    };
    let metadata_comment = params.metadata_comment.clone();
    let visual_label = params.visual_label.clone();
    job.update(|job| {
        job.status = JobStatus::Rendering;
        job.total_time_sec = duration;
        job.encoder_name = encoder_label;
        job.metadata_comment = metadata_comment.clone();
        job.visual_label = visual_label;
    });

    let chapters_file = bundle_outputs
        .as_ref()
        .and_then(|(_, _, _, _, ffmeta, _)| ffmeta.as_deref());

    let effect = AppliedEffect::resolve(params.effect_id, &paths.effects);
    job.set_stage(RenderStage::Video);
    let render_res = LoopRender {
        tools,
        video: params.video,
        audio: &master_audio,
        output: &output_path,
        work_dir: &paths.temp,
        subtitle_file: params.subtitle_file,
        overlay_png: params.overlay_png,
        duration,
        size: params.size,
        fit: params.fit,
        fps: params.fps,
        effect: &effect,
        encoder,
        preset: resolved_preset,
        quality: params.quality,
        stream_copy_video: params.stream_copy_video,
        youtube_standard: params.youtube_standard,
        visual_variation: params.visual_variation,
        visualizer: params.visualizer,
        chapters_file,
        metadata_comment: metadata_comment.as_deref(),
    }
    .run(job)
    .await;
    job.set_stage(RenderStage::Finalizing);

    if let Err(error) = render_res {
        let _ = fs::remove_file(&output_path);
        if let Some((render_dir, ..)) = bundle_outputs {
            let _ = fs::remove_dir(&render_dir);
        }
        return Err(error);
    }

    let report_dir = bundle_outputs
        .as_ref()
        .map(|(directory, ..)| directory.clone());
    if let Some((render_dir, playlist_txt, timestamp_txt, thumbnail_path, _, auto_thumbnail)) =
        bundle_outputs
    {
        tokio::fs::write(render_dir.join("playlist.txt"), playlist_txt).await?;
        tokio::fs::write(render_dir.join("Timestamp.txt"), timestamp_txt).await?;
        if let Some(automatic) = auto_thumbnail {
            let thumbnail_output =
                thumbnail_path_for_video(&output_path, std::ffi::OsStr::new("jpg"));
            let source_timing = crate::media::probe_video(tools, params.video)
                .await
                .ok()
                .map(|video| (video.duration, video.fps));
            let timestamp = thumbnail_timestamp(automatic.timestamp, source_timing);
            if let Err(error) = crate::thumbnail::render(
                tools,
                &paths.temp,
                params.video,
                timestamp,
                automatic.spec,
                thumbnail_output.clone(),
            )
            .await
            {
                let _ = tokio::fs::remove_file(&thumbnail_output).await;
                append_report_warning(job, format!("Thumbnail otomatis gagal dibuat: {error}"));
            }
        } else if let Some(thumb_src) = thumbnail_path.filter(|p| p.is_file()) {
            let extension = thumb_src.extension().unwrap_or(std::ffi::OsStr::new("jpg"));
            let dest = thumbnail_path_for_video(&output_path, extension);
            if dest != thumb_src {
                tokio::fs::copy(&thumb_src, &dest).await?;
            }
        }
    }

    job.set_stage(RenderStage::Completed);
    if let Some(directory) = report_dir {
        let snapshot = job.snapshot();
        let report = format_render_report(&RenderReport {
            encoder: &snapshot.encoder_name,
            preset: resolved_preset,
            size: params.size,
            fps: params.fps,
            duration,
            stream_copy: params.stream_copy_video,
            normalize_volume: params.normalize_volume,
            visual_label: snapshot.visual_label.as_deref().unwrap_or("Tanpa"),
            timings: &snapshot.timings,
        });
        if let Err(error) = tokio::fs::write(directory.join("render_report.txt"), report).await {
            append_report_warning(job, format!("Laporan render gagal disimpan: {error}"));
        }
    }
    job.update(|job| {
        job.status = JobStatus::Completed;
        job.progress_percent = 100.0;
        job.current_time_sec = duration;
        job.eta_seconds = 0.0;
        job.output_file = Some(output_path);
    });
    Ok(())
}

fn thumbnail_path_for_video(video: &Path, extension: &std::ffi::OsStr) -> PathBuf {
    let stem = video.file_stem().unwrap_or_default().to_string_lossy();
    let code = stem.strip_prefix("video_").unwrap_or(&stem);
    let mut output = video.with_file_name(format!("thumbnail_{code}"));
    output.set_extension(extension);
    output
}

fn thumbnail_timestamp(timestamp: f64, source_timing: Option<(f64, f64)>) -> f64 {
    let timestamp = if timestamp.is_finite() {
        timestamp.max(0.0)
    } else {
        0.0
    };
    match source_timing.filter(|(duration, _)| duration.is_finite() && *duration > 0.0) {
        // The exact duration is EOF, not a readable frame position.
        Some((duration, fps)) => {
            let frame_duration = if fps.is_finite() && fps > 0.0 {
                1.0 / fps
            } else {
                0.1
            };
            timestamp.min((duration - frame_duration).max(0.0))
        }
        None => timestamp,
    }
}

fn append_report_warning(job: &JobHandle, warning: String) {
    job.update(|job| {
        if let Some(previous) = &mut job.report_warning {
            previous.push('\n');
            previous.push_str(&warning);
        } else {
            job.report_warning = Some(warning);
        }
    });
}

/// Image backgrounds are regenerated to match the playlist tempo and the chosen effect.
/// Returns the new video, or `None` to keep the one prepared earlier.
async fn bake_image_background(
    context: &RenderContext,
    job: &JobHandle,
    request: &RenderRequest,
    audio_files: &[PathBuf],
) -> Option<PathBuf> {
    let source = request
        .image_source
        .as_ref()
        .filter(|source| source.path.is_file())?;
    let bpm = match audio_files.first() {
        Some(first) => estimate_tempo_bpm(&context.tools, first).await,
        None => DEFAULT_BPM,
    };
    let output = context
        .paths
        .temp
        .join(format!("bg_render_{}.mp4", job.id()));
    let effect = AppliedEffect::resolve(&request.visual_effect, &context.paths.effects);
    let converted = ImageVideo {
        image: &source.path,
        output: &output,
        duration: 10.0,
        fps: request.fps,
        aspect_ratio: &request.aspect_ratio,
        resolution: &request.resolution,
        motion: source.motion,
        beat_pulse: request.beat_pulse,
        bpm,
        effect: &effect,
    }
    .render(&context.tools, &context.hardware)
    .await;

    match converted {
        Ok(_) => Some(output),
        Err(error) => {
            eprintln!("Background gambar tidak dibuat ulang, memakai video yang ada: {error}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        append_report_warning, build_anti_duplicate_stamp, format_render_report,
        normalize_video_size, resolve_raw_titles, thumbnail_path_for_video, thumbnail_timestamp,
        unique_video_filename, RenderReport, RenderTimings, VisualVariation, VisualVariationMode,
    };

    #[test]
    fn thumbnail_filename_shares_the_video_code_and_preserves_image_format() {
        let video = std::path::Path::new("output/video_a1b2c3.mp4");
        for extension in ["jpg", "png", "webp"] {
            assert_eq!(
                thumbnail_path_for_video(video, std::ffi::OsStr::new(extension)),
                std::path::PathBuf::from(format!("output/thumbnail_a1b2c3.{extension}"))
            );
        }
    }

    #[test]
    fn thumbnail_seek_avoids_eof_and_invalid_times() {
        assert_eq!(thumbnail_timestamp(f64::NAN, Some((10.0, 10.0))), 0.0);
        assert_eq!(thumbnail_timestamp(-1.0, Some((10.0, 10.0))), 0.0);
        assert_eq!(thumbnail_timestamp(f64::INFINITY, Some((10.0, 10.0))), 0.0);
        assert_eq!(thumbnail_timestamp(10.0, Some((10.0, 10.0))), 9.9);
        assert_eq!(thumbnail_timestamp(10.0, Some((10.0, 1.0))), 9.0);
        assert_eq!(thumbnail_timestamp(10.0, Some((10.0, f64::NAN))), 9.9);
        assert_eq!(thumbnail_timestamp(999.0, Some((0.05, 10.0))), 0.0);
        assert_eq!(thumbnail_timestamp(3.5, None), 3.5);
        assert_eq!(thumbnail_timestamp(3.5, Some((f64::NAN, 10.0))), 3.5);
    }

    #[test]
    fn finalization_warnings_preserve_all_failures() {
        let job = super::JobHandle::new();
        append_report_warning(&job, "Thumbnail otomatis gagal dibuat".into());
        append_report_warning(&job, "Laporan render gagal disimpan".into());
        assert_eq!(
            job.snapshot().report_warning.as_deref(),
            Some("Thumbnail otomatis gagal dibuat\nLaporan render gagal disimpan")
        );
    }

    #[test]
    fn render_report_explains_actual_encoder_timings_and_copy_mode() {
        let timings = RenderTimings {
            preparation_seconds: 1.0,
            audio_seconds: 3.0,
            video_seconds: 10.0,
            finalizing_seconds: 2.0,
            total_elapsed_seconds: 16.0,
        };
        let mut report = RenderReport {
            encoder: "NVIDIA NVENC H.264 (GPU)",
            preset: "p5",
            size: (1920, 1080),
            fps: 30,
            duration: 100.0,
            stream_copy: false,
            normalize_volume: true,
            visual_label: "Geser pelan",
            timings: &timings,
        };
        let text = format_render_report(&report);
        assert!(text.contains("Encoder aktual: NVIDIA NVENC H.264 (GPU)"));
        assert!(text.contains("Preset aktual: p5"));
        assert!(text.contains("Waktu audio: 3.000 detik"));
        assert!(text.contains("Total waktu: 16.000 detik"));
        assert!(text.contains("Kecepatan tahap video: 10.00x"));
        report.stream_copy = true;
        let text = format_render_report(&report);
        assert!(text.contains("Mode: Salin video langsung"));
        assert!(text.contains("Preset aktual: Tidak digunakan"));
    }

    #[test]
    fn normalizes_source_video_dimensions_to_even() {
        assert_eq!(normalize_video_size((1920, 1080)), (1920, 1080));
        assert_eq!(normalize_video_size((1081, 1921)), (1080, 1920));
        assert_eq!(normalize_video_size((0, 720)), (1920, 1080));
        assert_eq!(normalize_video_size((1, 1)), (2, 2));
    }

    #[test]
    fn builds_unique_video_name_and_resolves_titles() {
        let dir = std::env::temp_dir();
        let name = unique_video_filename(&dir);
        assert!(name.starts_with("video_"));
        assert!(name.ends_with(".mp4"));
        assert_eq!(name.len(), "video_xxxxxx.mp4".len());

        let paths = vec![PathBuf::from("01. senja.mp3"), PathBuf::from("dua.mp3")];
        let titles = resolve_raw_titles(&paths, &["Senja di Pelabuhan".into(), "".into()]);
        assert_eq!(titles, vec!["Senja di Pelabuhan", "dua.mp3"]);
    }

    #[test]
    fn builds_distinct_anti_duplicate_stamps_across_renders() {
        let s1 = build_anti_duplicate_stamp(VisualVariationMode::None, true);
        let s2 = build_anti_duplicate_stamp(VisualVariationMode::None, true);
        assert!(s1.comment.as_deref().unwrap().starts_with("RenderPro "));
        assert_ne!(s1.comment, s2.comment);
        assert_ne!(s1.visual_variation, VisualVariation::None);
        assert!(s1.visual_label.contains("fase "));

        let off = build_anti_duplicate_stamp(VisualVariationMode::None, false);
        assert_eq!(off.comment, None);
        assert_eq!(off.visual_variation, VisualVariation::None);
        assert_eq!(off.visual_label, "Tanpa");
    }
}
