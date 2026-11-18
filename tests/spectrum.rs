//! Real FFmpeg spectrum checks; requires ffmpeg and ffprobe on PATH, like pipeline.rs.
use loopkaraoke::{
    ffmpeg::{self, Tools},
    spectrum,
};

#[tokio::test]
async fn decoded_tone_and_silence_produce_audio_driven_spectrum_without_pcm_cache_files() {
    let root = std::env::temp_dir().join(format!("loopkaraoke-spectrum-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let tools = Tools::locate();
    let tone = root.join("tone.wav");
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
                "sine=frequency=1000:sample_rate=16000:duration=1",
                "-c:a",
                "pcm_s16le",
            ])
            .arg(&tone),
        "spectrum test tone",
    )
    .await
    .unwrap();
    let data = spectrum::analyze(&tools, &tone).await.unwrap();
    let levels = data.levels_at(0.5);
    let (peak, amplitude) = levels
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .unwrap();
    assert!(
        (28..=31).contains(&peak),
        "1 kHz should peak in the correct logarithmic band, got {peak}"
    );
    assert!(*amplitude > 0.1);
    assert!(data.levels_at(2.0).iter().all(|level| *level == 0.0));
    let silence = root.join("silence.wav");
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
                "anullsrc=r=16000:cl=mono",
                "-t",
                "0.25",
                "-c:a",
                "pcm_s16le",
            ])
            .arg(&silence),
        "spectrum test silence",
    )
    .await
    .unwrap();
    let quiet = spectrum::analyze(&tools, &silence).await.unwrap();
    assert!(quiet.levels_at(0.1).iter().all(|level| *level == 0.0));
    let mut files = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    files.sort();
    assert_eq!(
        files,
        ["silence.wav", "tone.wav"].map(std::ffi::OsString::from)
    );
    std::fs::remove_dir_all(root).unwrap();
}
