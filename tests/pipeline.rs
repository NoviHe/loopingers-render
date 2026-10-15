//! End-to-end checks that need the FFmpeg binaries on `PATH`.

use std::path::PathBuf;

use loopkaraoke::{
    config::AppPaths,
    demo,
    ffmpeg::Tools,
    hardware::HardwareInfo,
    karaoke::ass::StyleConfig,
    media::probe_video,
    render::{
        self,
        effects::AppliedEffect,
        encoder::Encoder,
        image_video::{ImageVideo, Motion},
        job::{JobHandle, JobStatus},
        looper::FitMode,
        request::{ImageSource, RenderRequest, TrackData},
        RenderContext,
    },
    thumbnail::{self, ThumbnailSpec},
};

fn workspace(label: &str) -> AppPaths {
    let root = std::env::temp_dir().join(format!("loopkaraoke-e2e-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let effects = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/effects");
    AppPaths::new(&root, effects).expect("workspace")
}

fn request(video: PathBuf, tracks: Vec<TrackData>, effect: &str) -> RenderRequest {
    RenderRequest {
        custom_filename: String::new(),
        video_path: video,
        image_source: None,
        tracks,
        aspect_ratio: "16:9".to_owned(),
        resolution: "720p".to_owned(),
        fit: FitMode::BlurPad,
        fps: 30,
        style: StyleConfig::default(),
        encoder: Encoder::CpuX264,
        preset: "ultrafast".to_owned(),
        quality: 28,
        beat_pulse: true,
        visual_effect: effect.to_owned(),
    }
}

#[tokio::test]
async fn demo_playlist_renders_to_a_video_with_subtitles_and_an_overlay() {
    let paths = workspace("render");
    let tools = Tools::locate();
    let demo = demo::generate(&tools, &paths.temp.join("demo"))
        .await
        .unwrap();
    let tracks: Vec<TrackData> = demo
        .tracks
        .iter()
        .map(|track| TrackData {
            path: PathBuf::from(&track.info.path),
            title: track.info.title.clone(),
            artist: track.info.artist.clone(),
            duration: track.info.duration,
            lyrics: track.lyrics.clone(),
        })
        .collect();

    let context = RenderContext {
        hardware: HardwareInfo::detect(&tools).await,
        paths: paths.clone(),
        tools: tools.clone(),
    };
    let job = JobHandle::new();
    render::execute(
        &context,
        &job,
        request(PathBuf::from(&demo.video.path), tracks, "rain"),
    )
    .await;

    let snapshot = job.snapshot();
    assert_eq!(
        snapshot.status,
        JobStatus::Completed,
        "{}",
        snapshot.error_message
    );
    let output = snapshot.output_file.expect("output path");
    let info = probe_video(&tools, &output).await.unwrap();
    assert_eq!((info.width, info.height), (1280, 720));
    assert!(
        (info.duration - 18.0).abs() < 1.5,
        "duration {}",
        info.duration
    );
    assert!(info.has_audio);
    let leftovers: Vec<_> = std::fs::read_dir(&paths.temp)
        .unwrap()
        .flatten()
        .filter(|entry| entry.path().is_file())
        .collect();
    assert!(leftovers.is_empty(), "temp files left: {leftovers:?}");
    let _ = std::fs::remove_dir_all(&paths.data_dir);
}

#[tokio::test]
async fn image_background_is_rebuilt_and_thumbnail_is_drawn() {
    let paths = workspace("image");
    let tools = Tools::locate();
    let hardware = HardwareInfo::detect(&tools).await;
    let demo = demo::generate(&tools, &paths.temp.join("demo"))
        .await
        .unwrap();

    let image = paths.temp.join("still.png");
    let mut cmd = tools.ffmpeg();
    cmd.args([
        "-v",
        "error",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=800x600",
        "-frames:v",
        "1",
    ])
    .arg(&image);
    loopkaraoke::ffmpeg::run(&mut cmd, "ffmpeg").await.unwrap();

    let clip = paths.temp.join("clip.mp4");
    let effect = AppliedEffect::resolve("vhs", &paths.effects);
    let info = ImageVideo {
        image: &image,
        output: &clip,
        duration: 2.0,
        fps: 30,
        aspect_ratio: "9:16",
        resolution: "720p",
        motion: Motion::PanLeft,
        beat_pulse: true,
        bpm: 120.0,
        effect: &effect,
    }
    .render(&tools, &hardware)
    .await
    .unwrap();
    assert_eq!((info.width, info.height), (720, 1280));

    let context = RenderContext {
        hardware,
        paths: paths.clone(),
        tools: tools.clone(),
    };
    let job = JobHandle::new();
    let mut render_request = request(
        clip.clone(),
        demo.tracks
            .iter()
            .take(1)
            .map(|track| TrackData {
                path: PathBuf::from(&track.info.path),
                title: track.info.title.clone(),
                artist: String::new(),
                duration: track.info.duration,
                lyrics: track.lyrics.clone(),
            })
            .collect(),
        "bokeh",
    );
    render_request.image_source = Some(ImageSource {
        path: image.clone(),
        motion: Motion::ZoomIn,
    });
    render_request.fit = FitMode::Crop;
    render::execute(&context, &job, render_request).await;
    let snapshot = job.snapshot();
    assert_eq!(
        snapshot.status,
        JobStatus::Completed,
        "{}",
        snapshot.error_message
    );

    let thumbnail_path = paths.outputs.join("thumb.jpg");
    let spec = ThumbnailSpec {
        title: "Powerful Worship".into(),
        subtitle: "Hillsong Worship".into(),
        track_titles: vec!["first song".into(), "second song".into()],
        column_mode: "auto".into(),
        number_style: "mirrored".into(),
        title_font_size: 120,
        track_font_size: 36,
        subtitle_font_size: 80,
        vignette_opacity: 0.55,
        title_color: "#FFFFFF".into(),
        track_color: "#FFFFFF".into(),
        subtitle_color: "#FFD700".into(),
        title_y_pct: 0.055,
        subtitle_y_pct: 0.82,
        title_font_path: None,
        subtitle_font_path: None,
    };
    thumbnail::render(
        &tools,
        &paths.temp,
        &PathBuf::from(&demo.video.path),
        1.0,
        spec,
        thumbnail_path.clone(),
    )
    .await
    .unwrap();
    let size = std::fs::metadata(&thumbnail_path).unwrap().len();
    assert!(size > 20_000, "thumbnail is only {size} bytes");
    let _ = std::fs::remove_dir_all(&paths.data_dir);
}

#[tokio::test]
async fn every_listed_effect_renders() {
    let paths = workspace("effects");
    let tools = Tools::locate();
    let hardware = HardwareInfo::default();

    let image = paths.temp.join("still.png");
    let mut cmd = tools.ffmpeg();
    cmd.args([
        "-v",
        "error",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=800x600",
        "-frames:v",
        "1",
    ])
    .arg(&image);
    loopkaraoke::ffmpeg::run(&mut cmd, "ffmpeg").await.unwrap();

    for definition in loopkaraoke::render::effects::EFFECTS.iter() {
        let effect = AppliedEffect::resolve(definition.id, &paths.effects);
        if definition.id != "none" {
            assert!(
                !matches!(effect, AppliedEffect::None),
                "{} has no asset",
                definition.id
            );
        }
        let output = paths.temp.join(format!("{}.mp4", definition.id));
        ImageVideo {
            image: &image,
            output: &output,
            duration: 1.0,
            fps: 30,
            aspect_ratio: "9:16",
            resolution: "720p",
            motion: Motion::Static,
            beat_pulse: false,
            bpm: 100.0,
            effect: &effect,
        }
        .render(&tools, &hardware)
        .await
        .unwrap_or_else(|error| panic!("{}: {error}", definition.id));
    }
    let _ = std::fs::remove_dir_all(&paths.data_dir);
}
