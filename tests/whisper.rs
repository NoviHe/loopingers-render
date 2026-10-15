//! Downloads the tiny Whisper model on first run, so it is opt-in:
//! `WHISPER_TEST_AUDIO=path/to/speech.wav cargo test --test whisper -- --ignored --nocapture`

use std::path::PathBuf;

use loopkaraoke::{
    ffmpeg::Tools,
    karaoke::transcribe::{ModelSize, TranscribeOptions, Transcriber},
};

#[tokio::test]
#[ignore = "downloads a model and needs WHISPER_TEST_AUDIO"]
async fn speech_is_transcribed_with_word_timestamps() {
    let audio = PathBuf::from(std::env::var("WHISPER_TEST_AUDIO").expect("WHISPER_TEST_AUDIO"));
    let models = std::env::temp_dir().join("loopkaraoke-test-models");
    std::fs::create_dir_all(&models).unwrap();

    let lines = Transcriber::default()
        .transcribe(
            &Tools::locate(),
            &models,
            &audio,
            TranscribeOptions {
                model: ModelSize::Tiny,
                language: Some("en".to_owned()),
                use_gpu: false,
            },
        )
        .await
        .unwrap();

    for line in &lines {
        println!("{:.2}-{:.2} {}", line.start, line.end, line.text);
        for word in &line.words {
            println!("    {:.2}-{:.2} {}", word.start, word.end, word.word);
        }
    }
    assert!(!lines.is_empty());
    assert!(lines.iter().all(|line| !line.words.is_empty()));
    assert!(lines
        .iter()
        .flat_map(|line| &line.words)
        .all(|word| word.end >= word.start));
}
