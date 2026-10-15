pub mod audio;
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
    media::{clean_artist, clean_proper_title},
};
use effects::AppliedEffect;
use encoder::Encoder;
use image_video::{estimate_tempo_bpm, ImageVideo, DEFAULT_BPM};
use job::{JobHandle, JobStatus};
use looper::LoopRender;
use request::RenderRequest;

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

/// Runs a render job to completion and records the outcome on `job`.
pub async fn execute(context: &RenderContext, job: &JobHandle, request: RenderRequest) {
    if let Err(error) = run_pipeline(context, job, request).await {
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
    let RenderContext {
        paths,
        tools,
        hardware,
    } = context;
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
    let subtitle_file = format!("subtitles_{}.ass", job.id());
    let subtitle_path = paths.temp.join(&subtitle_file);
    tokio::fs::write(
        &subtitle_path,
        build_ass(&subtitle_tracks, width, height, &style),
    )
    .await?;
    temp.track(&subtitle_path);

    let master_audio = paths.temp.join(format!("master_audio_{}.m4a", job.id()));
    temp.track(&master_audio);
    let merged_duration = audio::merge_tracks(tools, &audio_files, &master_audio).await?;
    let duration = if merged_duration > 0.0 {
        merged_duration
    } else {
        request.tracks.iter().map(|track| track.duration).sum()
    };

    let encoder = Encoder::resolve(request.encoder, hardware);
    job.update(|job| {
        job.status = JobStatus::Rendering;
        job.total_time_sec = duration;
        job.encoder_name = encoder.label(&hardware.gpu_name);
    });

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
    let effect = AppliedEffect::resolve(effect_id, &paths.effects);
    LoopRender {
        tools,
        video: &video,
        audio: &master_audio,
        output: &output,
        work_dir: &paths.temp,
        subtitle_file: &subtitle_file,
        duration,
        size: (width, height),
        fit: request.fit,
        fps: request.fps,
        effect: &effect,
        encoder,
        preset: &request.preset,
        quality: request.quality,
    }
    .run(job)
    .await?;

    job.update(|job| {
        job.status = JobStatus::Completed;
        job.progress_percent = 100.0;
        job.current_time_sec = duration;
        job.eta_seconds = 0.0;
        job.output_file = Some(output);
    });
    Ok(())
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
