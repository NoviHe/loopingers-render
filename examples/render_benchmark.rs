//! Runs the real Generate pipeline on identical synthetic media and camera parameters.
//! cargo run --example render_benchmark -- <folder containing ffmpeg and ffprobe>
use std::{path::PathBuf, time::Instant};

use loopkaraoke::{
    config::AppPaths,
    ffmpeg::{self, Tools},
    hardware::HardwareInfo,
    media::probe_video,
    render::{
        self, build_anti_duplicate_stamp,
        encoder::{RenderSpeed, VideoCompression, VideoRenderSettings, VideoTarget},
        job::{JobHandle, JobStatus},
        RenderContext, SimpleRenderRequest, VisualVariationMode,
    },
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tools_dir = std::env::args_os()
        .nth(1)
        .ok_or("Pass the FFmpeg tools directory")?;
    let tools = Tools::from_directory(&PathBuf::from(tools_dir))?;
    tools.check().await?;
    let root = std::env::temp_dir().join(format!(
        "loopkaraoke-render-benchmark-{}",
        uuid::Uuid::new_v4()
    ));
    let paths = AppPaths::new(
        &root,
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/effects"),
    )?;
    let video = root.join("source.mp4");
    ffmpeg::run(
        tools
            .ffmpeg()
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=1280x720:rate=30",
                "-t",
                "2",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&video),
        "benchmark source",
    )
    .await?;
    let mut audio = Vec::new();
    for (name, seconds, frequency) in [("first.wav", "12", "440"), ("second.wav", "18", "660")] {
        let path = root.join(name);
        ffmpeg::run(
            tools
                .ffmpeg()
                .args([
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                ])
                .arg(format!("sine=frequency={frequency}:sample_rate=48000"))
                .args(["-t", seconds, "-ac", "2", "-c:a", "pcm_s16le"])
                .arg(&path),
            "benchmark audio",
        )
        .await?;
        audio.push(path);
    }
    let hardware = HardwareInfo::detect(&tools).await;
    let context = RenderContext {
        paths,
        tools,
        hardware,
    };
    let settings = VideoRenderSettings {
        target: VideoTarget::YouTubeStandard,
        compression: VideoCompression::CompactSize,
        ..VideoRenderSettings::default()
    };
    let encoder = settings.resolve_encoder(&context.hardware);
    let stamp = build_anti_duplicate_stamp(VisualVariationMode::SlowPan, true);
    println!("Benchmark folder: {}", root.display());
    println!(
        "Encoder: {}; 1080p30; pan+anti-duplicate; normalized audio",
        encoder.name()
    );
    let mut csv = String::from("mode,preset,audio_seconds,video_seconds,total_seconds,bytes\n");
    let mut elapsed = [Vec::new(), Vec::new()];
    let mut outputs = Vec::new();
    // Alternate order so the second mode does not always benefit from warm filesystem caches.
    for (round, modes) in [
        [RenderSpeed::Balanced, RenderSpeed::Fast],
        [RenderSpeed::Fast, RenderSpeed::Balanced],
    ]
    .into_iter()
    .enumerate()
    {
        for speed in modes {
            let current = VideoRenderSettings { speed, ..settings };
            let preset = current.resolve_preset(encoder);
            let job = JobHandle::new();
            let start = Instant::now();
            render::execute_simple(
                &context,
                &job,
                SimpleRenderRequest {
                    video_path: video.clone(),
                    audio_files: audio.clone(),
                    track_titles: vec!["First".into(), "Second".into()],
                    thumbnail_path: None,
                    auto_thumbnail: None,
                    output_dir: root.join(format!("{speed:?}-{round}")),
                    size: (1920, 1080),
                    fps: 30,
                    encoder,
                    preset: preset.into(),
                    quality: current.resolve_quality(encoder),
                    audio_bitrate: "192k".into(),
                    stream_copy_video: false,
                    youtube_standard: true,
                    visual_mode: VisualVariationMode::SlowPan,
                    anti_duplicate: true,
                    output_code: None,
                    pre_resolved_stamp: Some(stamp.clone()),
                    fade_out_secs: 0.0,
                    crossfade_secs: 0.0,
                    normalize_volume: true,
                    target_duration_secs: None,
                },
            )
            .await;
            let seconds = start.elapsed().as_secs_f64();
            let snapshot = job.snapshot();
            if snapshot.status != JobStatus::Completed {
                return Err(snapshot.error_message.into());
            }
            let output = snapshot.output_file.ok_or("No rendered output")?;
            if let Some(warning) = snapshot.report_warning {
                return Err(warning.into());
            }
            if !output
                .parent()
                .ok_or("Missing output directory")?
                .join("render_report.txt")
                .is_file()
            {
                return Err("Missing render timing report".into());
            }
            let info = probe_video(&context.tools, &output).await?;
            if info.width != 1920
                || info.height != 1080
                || (info.fps - 30.0).abs() > 0.01
                || (info.duration - 30.0).abs() > 0.15
                || !info.has_audio
                || info.codec != "h264"
            {
                return Err(format!("Unexpected output properties: {info:?}").into());
            }
            let chapters = ffmpeg::run(
                context
                    .tools
                    .ffprobe()
                    .args(["-v", "error", "-show_chapters", "-of", "json"])
                    .arg(&output),
                "benchmark chapter verification",
            )
            .await?;
            let chapters: serde_json::Value = serde_json::from_slice(&chapters.stdout)?;
            let second_start = chapters["chapters"][1]["start_time"]
                .as_str()
                .ok_or("Missing second chapter")?
                .parse::<f64>()?;
            if (second_start - 12.0).abs() > 0.05 {
                return Err("Playlist chapter order changed".into());
            }
            let index = usize::from(speed == RenderSpeed::Fast);
            elapsed[index].push(seconds);
            let bytes = std::fs::metadata(&output)?.len();
            csv.push_str(&format!(
                "{speed:?},{preset},{:.3},{:.3},{seconds:.3},{bytes}\n",
                snapshot.timings.audio_seconds, snapshot.timings.video_seconds
            ));
            println!("{speed:?} {preset}: {seconds:.2}s (audio {:.2}s, video {:.2}s), {bytes} bytes; output verified",
                snapshot.timings.audio_seconds, snapshot.timings.video_seconds);
            outputs.push((speed, output));
        }
    }
    let average = |values: &[f64]| values.iter().sum::<f64>() / values.len() as f64;
    let baseline = average(&elapsed[0]);
    let fast = average(&elapsed[1]);
    let summary = format!("Balanced average: {baseline:.3}s\nFast average: {fast:.3}s\nSpeedup: {:.3}x\nElapsed reduction: {:.1}%\n", baseline / fast, (1.0 - fast / baseline) * 100.0);
    println!("{summary}");
    std::fs::write(root.join("results.csv"), csv)?;
    std::fs::write(root.join("summary.txt"), summary)?;
    let baseline = &outputs
        .iter()
        .find(|(mode, _)| *mode == RenderSpeed::Balanced)
        .unwrap()
        .1;
    let fast = &outputs
        .iter()
        .find(|(mode, _)| *mode == RenderSpeed::Fast)
        .unwrap()
        .1;
    let comparison = context
        .tools
        .ffmpeg()
        .args(["-hide_banner", "-i"])
        .arg(baseline)
        .arg("-i")
        .arg(fast)
        .args(["-lavfi", "ssim", "-an", "-f", "null", "-"])
        .output()
        .await?;
    if !comparison.status.success() {
        return Err("SSIM comparison failed".into());
    }
    let comparison = String::from_utf8_lossy(&comparison.stderr);
    let score = comparison
        .lines()
        .find(|line| line.contains("SSIM Y:"))
        .ok_or("Missing SSIM score")?;
    let score = &score[score.find("SSIM Y:").unwrap()..];
    println!("{score}");
    std::fs::write(root.join("ssim.txt"), score)?;
    Ok(())
}
