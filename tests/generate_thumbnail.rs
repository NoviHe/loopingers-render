//! Generate bundle checks requiring FFmpeg/FFprobe on PATH.
use std::path::PathBuf;

use loopkaraoke::{
    config::AppPaths,
    ffmpeg::{self, Tools},
    hardware::HardwareInfo,
    render::{
        self,
        encoder::Encoder,
        job::{JobHandle, JobStatus},
        AutoThumbnailRequest, RenderContext, SimpleRenderRequest, VisualVariationMode,
    },
    thumbnail::{self, ThumbnailSpec},
};

fn thumbnail_spec() -> ThumbnailSpec {
    ThumbnailSpec {
        title: "Generate Album".into(),
        subtitle: String::new(),
        track_titles: vec!["Lagu Kedua".into(), "Lagu Pertama".into()],
        column_mode: "single".into(),
        number_style: "sequential".into(),
        title_font_size: 56,
        track_font_size: 24,
        subtitle_font_size: 24,
        vignette_opacity: 0.0,
        title_color: "#ffffff".into(),
        track_color: "#ffffff".into(),
        subtitle_color: "#ffffff".into(),
        title_y_pct: 0.05,
        subtitle_y_pct: 0.82,
        title_font_path: None,
        track_font_path: None,
        subtitle_font_path: None,
    }
}

#[tokio::test]
async fn automatic_thumbnail_uses_generate_video_and_shares_the_output_bundle() {
    let root = std::env::temp_dir().join(format!(
        "loopkaraoke-auto-thumbnail-{}",
        uuid::Uuid::new_v4()
    ));
    let paths = AppPaths::new(
        &root,
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/effects"),
    )
    .unwrap();
    let tools = Tools::locate();
    let video = paths.temp.join("red.mp4");
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
                "color=c=red:s=160x90:r=10:d=0.6",
                "-an",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&video),
        "automatic thumbnail fixture video",
    )
    .await
    .unwrap();
    let audio = paths.temp.join("song.wav");
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
                "sine=frequency=440:sample_rate=48000:duration=0.6",
                "-c:a",
                "pcm_s16le",
            ])
            .arg(&audio),
        "automatic thumbnail fixture audio",
    )
    .await
    .unwrap();
    let manual = paths.temp.join("manual-cover.png");
    image::RgbImage::from_pixel(32, 32, image::Rgb([0, 0, 255]))
        .save(&manual)
        .unwrap();
    let context = RenderContext {
        paths: paths.clone(),
        tools,
        hardware: HardwareInfo::default(),
    };
    let job = JobHandle::new();
    let mut request = SimpleRenderRequest {
        video_path: video,
        audio_files: vec![audio.clone(), audio],
        track_titles: vec!["Lagu Kedua".into(), "Lagu Pertama".into()],
        thumbnail_path: Some(manual.clone()),
        auto_thumbnail: Some(AutoThumbnailRequest {
            spec: thumbnail_spec(),
            timestamp: 999.0,
        }),
        output_dir: paths.outputs.clone(),
        size: (160, 90),
        fps: 10,
        encoder: Encoder::CpuX264,
        preset: "ultrafast".into(),
        quality: 28,
        audio_bitrate: "128k".into(),
        stream_copy_video: true,
        youtube_standard: false,
        visual_mode: VisualVariationMode::None,
        anti_duplicate: false,
        output_code: None,
        pre_resolved_stamp: None,
        fade_out_secs: 0.0,
        crossfade_secs: 0.0,
        normalize_volume: false,
        target_duration_secs: None,
    };
    render::execute_simple(&context, &job, request.clone()).await;
    let snapshot = job.snapshot();
    assert_eq!(
        snapshot.status,
        JobStatus::Completed,
        "{}",
        snapshot.error_message
    );
    assert!(
        snapshot.report_warning.is_none(),
        "{:?}",
        snapshot.report_warning
    );
    let video_output = snapshot.output_file.unwrap();
    assert!(video_output.is_file());
    let bundle = video_output.parent().unwrap();
    let video_code = video_output
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap()
        .strip_prefix("video_")
        .unwrap();
    let generated = image::open(bundle.join(format!("thumbnail_{video_code}.jpg")))
        .unwrap()
        .to_rgb8();
    assert_eq!(
        generated.dimensions(),
        (thumbnail::WIDTH, thumbnail::HEIGHT)
    );
    let corner = generated.get_pixel(20, thumbnail::HEIGHT - 20);
    assert!(
        corner[0] > 200 && corner[2] < 40,
        "thumbnail must use the red Generate video, not blue manual image: {corner:?}"
    );
    assert!(!bundle.join(manual.file_name().unwrap()).exists());
    assert!(
        manual.is_file(),
        "automatic mode must preserve the original manual cover"
    );
    let playlist = std::fs::read_to_string(bundle.join("playlist.txt")).unwrap();
    assert!(playlist.find("Lagu Kedua").unwrap() < playlist.find("Lagu Pertama").unwrap());
    assert!(bundle.join("Timestamp.txt").is_file());
    assert!(bundle.join("render_report.txt").is_file());
    assert!(!std::fs::read_dir(&paths.temp).unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with("temp_thumb_frame_")));
    request.auto_thumbnail = None;
    let manual_job = JobHandle::new();
    render::execute_simple(&context, &manual_job, request.clone()).await;
    let manual_snapshot = manual_job.snapshot();
    assert_eq!(
        manual_snapshot.status,
        JobStatus::Completed,
        "{}",
        manual_snapshot.error_message
    );
    let manual_video = manual_snapshot.output_file.unwrap();
    let manual_code = manual_video
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap()
        .strip_prefix("video_")
        .unwrap();
    let manual_output = manual_video
        .parent()
        .unwrap()
        .join(format!("thumbnail_{manual_code}.png"));
    assert_eq!(
        std::fs::read(manual_output).unwrap(),
        std::fs::read(&manual).unwrap()
    );
    assert!(!manual_video
        .parent()
        .unwrap()
        .join("manual-cover.png")
        .exists());
    let mut automatic_request = request.clone();
    automatic_request.target_duration_secs = Some(1.8);
    automatic_request.auto_thumbnail = Some(AutoThumbnailRequest {
        spec: thumbnail_spec(),
        timestamp: 0.0,
    });
    let mut failed_request = request.clone();
    failed_request.video_path = root.join("missing-video.mp4");
    let queue = render::batch::BatchHandle::new(3);
    render::batch::execute(
        context.clone(),
        queue.clone(),
        vec![
            render::batch::BatchItem {
                request: automatic_request,
            },
            render::batch::BatchItem {
                request: failed_request,
            },
            render::batch::BatchItem {
                request: request.clone(),
            },
        ],
        || {},
    )
    .await;
    let batch = queue.snapshot();
    assert_eq!(batch.status, render::batch::BatchStatus::Completed);
    assert_eq!(batch.finished.len(), 3);
    assert!(batch.finished[1].error.is_some());
    let batch_dir = batch.directory.as_ref().unwrap();
    assert_eq!(batch_dir.parent(), Some(paths.outputs.as_path()));
    assert!(batch_dir
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("Batch_"));
    let summary = std::fs::read_to_string(batch_dir.join("ringkasan_batch.txt")).unwrap();
    assert!(summary.contains(batch.finished[1].error.as_ref().unwrap()));
    for (index, extension) in [(0, "jpg"), (2, "png")] {
        let output = batch.finished[index].output.as_ref().unwrap();
        if index == 0 {
            let actual = loopkaraoke::media::probe_video(&context.tools, output)
                .await
                .unwrap();
            assert!(
                (actual.duration - 1.8).abs() < 0.25,
                "planned duration must be respected: {}",
                actual.duration
            );
        }
        let code = output
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .strip_prefix("video_")
            .unwrap();
        assert_eq!(output.parent().unwrap().parent(), Some(batch_dir.as_path()));
        assert_eq!(
            output
                .parent()
                .unwrap()
                .file_name()
                .unwrap()
                .to_string_lossy(),
            format!("{:02}_{code}", index + 1)
        );
        assert!(summary.contains(output.to_str().unwrap()));
        assert!(batch.finished[index].duration_seconds > 0.0);
        assert!(output
            .parent()
            .unwrap()
            .join(format!("thumbnail_{code}.{extension}"))
            .is_file());
    }
    assert_ne!(batch.finished[0].output, batch.finished[2].output);
    let stopped = render::batch::BatchHandle::new(2);
    let stop_signal = stopped.clone();
    render::batch::execute(
        context,
        stopped.clone(),
        vec![
            render::batch::BatchItem {
                request: request.clone(),
            },
            render::batch::BatchItem { request },
        ],
        move || {
            if stop_signal.snapshot().finished.len() == 1 {
                stop_signal.stop_after_current();
            }
        },
    )
    .await;
    assert_eq!(
        stopped.snapshot().status,
        render::batch::BatchStatus::Stopped
    );
    assert_eq!(stopped.snapshot().finished.len(), 1);
    assert_ne!(stopped.snapshot().directory, batch.directory);
    assert!(stopped
        .snapshot()
        .directory
        .unwrap()
        .join("ringkasan_batch.txt")
        .is_file());
    std::fs::remove_dir_all(&root).unwrap();
}
