use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
};

use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

use super::{spread_words, LyricLine, LyricWord};
use crate::{
    error::{Error, Result},
    ffmpeg::{self, Tools},
    util::round2,
};

const MODEL_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";
const SAMPLE_RATE: &str = "16000";
const BEAM_SIZE: i32 = 5;
const MAX_THREADS: usize = 8;
const MIN_SEGMENT_SECONDS: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelSize {
    Tiny,
    Base,
    Small,
}

impl ModelSize {
    pub const ALL: [Self; 3] = [Self::Tiny, Self::Base, Self::Small];

    pub fn name(self) -> &'static str {
        match self {
            Self::Tiny => "tiny",
            Self::Base => "base",
            Self::Small => "small",
        }
    }

    pub fn file_name(self) -> String {
        format!("ggml-{}.bin", self.name())
    }
}

#[derive(Debug, Clone)]
pub struct TranscribeOptions {
    pub model: ModelSize,
    /// ISO language code; `None` lets Whisper detect it.
    pub language: Option<String>,
    pub use_gpu: bool,
}

struct LoadedModel {
    size: ModelSize,
    gpu: bool,
    context: Arc<WhisperContext>,
}

/// Keeps the most recently used Whisper model in memory between requests.
#[derive(Default)]
pub struct Transcriber {
    loaded: Arc<Mutex<Option<LoadedModel>>>,
}

impl Transcriber {
    pub async fn transcribe(
        &self,
        tools: &Tools,
        models_dir: &Path,
        audio: &Path,
        options: TranscribeOptions,
    ) -> Result<Vec<LyricLine>> {
        let samples = decode_audio(tools, audio).await?;
        let loaded = Arc::clone(&self.loaded);
        let models_dir = models_dir.to_path_buf();

        tokio::task::spawn_blocking(move || {
            let context = cached_context(&loaded, &models_dir, options.model, options.use_gpu)?;
            transcribe_samples(&context, &samples, options.language.as_deref())
        })
        .await
        .map_err(|error| Error::internal(format!("Proses transkripsi terhenti: {error}")))?
    }
}

/// Decodes any audio file into 16 kHz mono samples, the input format Whisper expects.
async fn decode_audio(tools: &Tools, audio: &Path) -> Result<Vec<f32>> {
    let mut cmd = tools.ffmpeg();
    cmd.args(["-v", "error", "-i"]).arg(audio).args([
        "-f",
        "f32le",
        "-ac",
        "1",
        "-ar",
        SAMPLE_RATE,
        "-",
    ]);
    let output = ffmpeg::run(&mut cmd, "ffmpeg (decode audio)").await?;
    Ok(output
        .stdout
        .chunks_exact(4)
        .map(|bytes| f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        .collect())
}

fn cached_context(
    loaded: &Mutex<Option<LoadedModel>>,
    models_dir: &Path,
    size: ModelSize,
    use_gpu: bool,
) -> Result<Arc<WhisperContext>> {
    let mut guard = loaded
        .lock()
        .map_err(|_| Error::internal("Cache model Whisper rusak."))?;
    if let Some(model) = guard.as_ref() {
        if model.size == size && model.gpu == use_gpu {
            return Ok(Arc::clone(&model.context));
        }
    }

    let path = models_dir.join(size.file_name());
    ensure_model(&path, size)?;
    let (context, gpu) = match load_context(&path, use_gpu) {
        Ok(context) => (context, use_gpu),
        Err(_) if use_gpu => (load_context(&path, false)?, false),
        Err(error) => return Err(error),
    };
    let context = Arc::new(context);
    *guard = Some(LoadedModel {
        size,
        gpu,
        context: Arc::clone(&context),
    });
    Ok(context)
}

fn load_context(path: &Path, use_gpu: bool) -> Result<WhisperContext> {
    let mut params = WhisperContextParameters::default();
    params.use_gpu(use_gpu);
    WhisperContext::new_with_params(path, params)
        .map_err(|error| Error::internal(format!("Model Whisper gagal dimuat: {error}")))
}

fn ensure_model(path: &Path, size: ModelSize) -> Result<()> {
    if path.is_file() {
        return Ok(());
    }
    let url = format!("{MODEL_BASE_URL}/{}", size.file_name());
    let download_error = |error: &dyn std::fmt::Display| {
        Error::internal(format!("Gagal mengunduh model Whisper dari {url}: {error}"))
    };

    let response = ureq::get(&url).call().map_err(|e| download_error(&e))?;
    let partial: PathBuf = path.with_extension("bin.part");
    let mut file = File::create(&partial)?;
    let copied = io::copy(&mut response.into_body().into_reader(), &mut file);
    drop(file);
    if let Err(error) = copied {
        let _ = fs::remove_file(&partial);
        return Err(download_error(&error));
    }
    fs::rename(&partial, path)?;
    Ok(())
}

fn transcribe_samples(
    context: &WhisperContext,
    samples: &[f32],
    language: Option<&str>,
) -> Result<Vec<LyricLine>> {
    let whisper_error = |error: whisper_rs::WhisperError| Error::internal(error.to_string());
    let mut state = context.create_state().map_err(whisper_error)?;

    let mut params = FullParams::new(SamplingStrategy::BeamSearch {
        beam_size: BEAM_SIZE,
        patience: -1.0,
    });
    params.set_language(Some(language.unwrap_or("auto")));
    params.set_token_timestamps(true);
    params.set_suppress_nst(true);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_special(false);
    params.set_print_timestamps(false);
    let threads = thread::available_parallelism().map_or(4, |count| count.get().min(MAX_THREADS));
    params.set_n_threads(threads as i32);

    state.full(params, samples).map_err(whisper_error)?;
    Ok(collect_lines(&state, context.token_eot()))
}

fn collect_lines(state: &WhisperState, end_of_text: i32) -> Vec<LyricLine> {
    state
        .as_iter()
        .filter_map(|segment| {
            let text = segment.to_str_lossy().ok()?.trim().to_owned();
            if text.is_empty() {
                return None;
            }
            let start = segment.start_timestamp() as f64 / 100.0;
            let end = segment.end_timestamp() as f64 / 100.0;

            let pieces = (0..segment.n_tokens()).filter_map(|index| {
                let token = segment.get_token(index)?;
                if token.token_id() >= end_of_text {
                    return None;
                }
                let data = token.token_data();
                Some((
                    token.to_str_lossy().ok()?.into_owned(),
                    data.t0 as f64 / 100.0,
                    data.t1 as f64 / 100.0,
                ))
            });
            let mut words = merge_tokens(pieces);
            if words.is_empty() {
                words = spread_words(&text, start, (end - start).max(MIN_SEGMENT_SECONDS));
            }
            let (first, last) = (words.first()?, words.last()?);
            Some(LyricLine {
                start: first.start,
                end: last.end,
                text,
                words,
            })
        })
        .collect()
}

/// Joins sub-word tokens into words. A token starting with a space opens a new word.
fn merge_tokens(pieces: impl Iterator<Item = (String, f64, f64)>) -> Vec<LyricWord> {
    let mut words: Vec<LyricWord> = Vec::new();
    for (text, start, end) in pieces {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        let end = end.max(start);
        match words.last_mut() {
            Some(word) if !text.starts_with(char::is_whitespace) => {
                word.word.push_str(trimmed);
                word.end = round2(end);
            }
            _ => words.push(LyricWord {
                word: trimmed.to_owned(),
                start: round2(start),
                end: round2(end),
            }),
        }
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(text: &str, start: f64, end: f64) -> (String, f64, f64) {
        (text.to_owned(), start, end)
    }

    #[test]
    fn model_file_names_follow_the_ggml_convention() {
        assert_eq!(ModelSize::Small.file_name(), "ggml-small.bin");
        assert_eq!(ModelSize::ALL.len(), 3);
    }

    #[test]
    fn tokens_merge_into_words_at_leading_spaces() {
        let words = merge_tokens(
            [
                piece(" Kemes", 1.0, 1.4),
                piece("raan", 1.4, 2.0),
                piece(" ini", 2.0, 2.5),
                piece(",", 2.5, 2.6),
                piece("  ", 2.6, 2.7),
            ]
            .into_iter(),
        );
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].word, "Kemesraan");
        assert_eq!((words[0].start, words[0].end), (1.0, 2.0));
        assert_eq!(words[1].word, "ini,");
        assert_eq!(words[1].end, 2.6);
    }

    #[test]
    fn backwards_token_times_never_end_before_they_start() {
        let words = merge_tokens([piece(" a", 2.0, 1.0)].into_iter());
        assert_eq!((words[0].start, words[0].end), (2.0, 2.0));
    }
}
