//! Downloads FFmpeg (about 80 MB), so it is opt-in: `cargo test --test install -- --ignored`

use std::sync::Arc;

use loopkaraoke::ffmpeg::{ensure_installed, DownloadProgress};

#[tokio::test]
#[ignore = "downloads FFmpeg from the internet"]
async fn missing_ffmpeg_is_downloaded_and_works() {
    // Hide any FFmpeg on PATH so the downloaded copy is the one that gets used.
    std::env::set_var("PATH", "");
    let dir = std::env::temp_dir().join(format!("loopkaraoke-install-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    let progress = Arc::new(DownloadProgress::default());
    let tools = ensure_installed(dir.clone(), progress.clone())
        .await
        .expect("download and install");

    assert!(dir.join("ffmpeg.exe").is_file());
    assert!(dir.join("ffprobe.exe").is_file());
    assert!(!dir.join("ffmpeg-download.zip").exists());
    assert!(
        progress
            .downloaded
            .load(std::sync::atomic::Ordering::Relaxed)
            > 10_000_000
    );
    tools.check().await.expect("downloaded tools run");
    let _ = std::fs::remove_dir_all(&dir);
}
