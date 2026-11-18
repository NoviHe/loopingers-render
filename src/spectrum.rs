//! Bounded, in-memory spectral envelopes decoded from the original audio file.
use std::{f32::consts::TAU, path::Path, process::Stdio};

use tokio::io::{AsyncRead, AsyncReadExt};

use crate::{
    error::{Error, Result},
    ffmpeg::Tools,
};

pub const BAND_COUNT: usize = 48;
const SAMPLE_RATE: usize = 16_000;
const FFT_SIZE: usize = 2048;
const HOP_SIZE: usize = 1024;
const MIN_FREQUENCY: f32 = 40.0;
const MAX_FREQUENCY: f32 = SAMPLE_RATE as f32 / 2.0;
const DB_FLOOR: f32 = -60.0;
const MAX_FRAMES: usize = 250_000;
const STDERR_LIMIT: usize = 8192;

#[derive(Debug)]
pub struct SpectrumData {
    frames: Vec<[f32; BAND_COUNT]>,
    duration_seconds: f64,
}

impl SpectrumData {
    /// Frames are centered on their Hann windows; interpolate between their centers.
    /// The first frame covers the opening half-window, and audio past its end is silent.
    pub fn levels_at(&self, time: f64) -> [f32; BAND_COUNT] {
        if self.frames.is_empty()
            || !time.is_finite()
            || time < 0.0
            || time >= self.duration_seconds
        {
            return [0.0; BAND_COUNT];
        }
        let position =
            ((time * SAMPLE_RATE as f64 - FFT_SIZE as f64 / 2.0) / HOP_SIZE as f64).max(0.0);
        let first = (position.floor() as usize).min(self.frames.len() - 1);
        let second = (first + 1).min(self.frames.len() - 1);
        let blend = (position - position.floor()) as f32;
        std::array::from_fn(|band| {
            self.frames[first][band] * (1.0 - blend) + self.frames[second][band] * blend
        })
    }
}

/// FFmpeg streams mono 16 kHz float PCM into the analyzer; no PCM temp file is created.
/// Dropping this future also drops/kills the child via the shared FFmpeg command helper.
pub async fn analyze(tools: &Tools, path: &Path) -> Result<SpectrumData> {
    let mut command = tools.ffmpeg();
    command
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(path)
        .args([
            "-map", "0:a:0", "-vn", "-ac", "1", "-ar", "16000", "-f", "f32le", "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| Error::Tool(format!("Analisis spektrum tidak dapat dimulai: {error}")))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::internal("Output spektrum tidak tersedia."))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| Error::internal("Log spektrum tidak tersedia."))?;
    let (data, errors, status) =
        tokio::try_join!(read_pcm(stdout), read_error_tail(stderr), async {
            child.wait().await.map_err(Error::from)
        },)?;
    if !status.success() {
        return Err(Error::Tool(format!(
            "Analisis spektrum gagal: {}",
            String::from_utf8_lossy(&errors).trim()
        )));
    }
    Ok(data)
}

async fn read_error_tail(mut reader: impl AsyncRead + Unpin) -> Result<Vec<u8>> {
    let mut tail = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        let count = reader.read(&mut chunk).await?;
        if count == 0 {
            break;
        }
        tail.extend_from_slice(&chunk[..count]);
        if tail.len() > STDERR_LIMIT {
            tail.drain(..tail.len() - STDERR_LIMIT);
        }
    }
    Ok(tail)
}

async fn read_pcm(mut reader: impl AsyncRead + Unpin) -> Result<SpectrumData> {
    let mut analyzer = Analyzer::new();
    let mut bytes = Vec::with_capacity(16_388);
    let mut chunk = [0_u8; 16_384];
    loop {
        let count = reader.read(&mut chunk).await?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..count]);
        let complete = bytes.len() / 4 * 4;
        for sample in bytes[..complete].chunks_exact(4) {
            analyzer.push(f32::from_le_bytes(sample.try_into().unwrap()))?;
        }
        bytes.drain(..complete);
    }
    if !bytes.is_empty() {
        return Err(Error::Tool("Data audio spektrum tidak lengkap.".to_owned()));
    }
    analyzer.finish()
}

struct Analyzer {
    samples: Vec<f32>,
    sample_count: u64,
    window: [f32; FFT_SIZE],
    window_sum: f32,
    frames: Vec<[f32; BAND_COUNT]>,
}

impl Analyzer {
    fn new() -> Self {
        let window = std::array::from_fn(|index| {
            0.5 - 0.5 * (TAU * index as f32 / (FFT_SIZE - 1) as f32).cos()
        });
        let window_sum = window.iter().sum();
        Self {
            samples: Vec::with_capacity(FFT_SIZE),
            sample_count: 0,
            window,
            window_sum,
            frames: Vec::new(),
        }
    }

    fn push(&mut self, sample: f32) -> Result<()> {
        self.samples
            .push(if sample.is_finite() { sample } else { 0.0 });
        self.sample_count += 1;
        if self.samples.len() == FFT_SIZE {
            self.push_frame()?;
            self.samples.drain(..HOP_SIZE);
        }
        Ok(())
    }

    fn push_frame(&mut self) -> Result<()> {
        if self.frames.len() >= MAX_FRAMES {
            return Err(Error::invalid(
                "Audio terlalu panjang untuk analisis spektrum (batas sekitar 4 jam 26 menit).",
            ));
        }
        let mut real = std::array::from_fn(|index| {
            self.samples.get(index).copied().unwrap_or(0.0) * self.window[index]
        });
        let mut imaginary = [0.0; FFT_SIZE];
        fft(&mut real, &mut imaginary);
        let mut peaks = [0.0_f32; BAND_COUNT];
        let log_range = (MAX_FREQUENCY / MIN_FREQUENCY).ln();
        for bin in 1..=FFT_SIZE / 2 {
            let frequency = bin as f32 * SAMPLE_RATE as f32 / FFT_SIZE as f32;
            if frequency < MIN_FREQUENCY {
                continue;
            }
            let band = (((frequency / MIN_FREQUENCY).ln() / log_range * BAND_COUNT as f32).floor()
                as usize)
                .min(BAND_COUNT - 1);
            let magnitude = (real[bin] * real[bin] + imaginary[bin] * imaginary[bin]).sqrt();
            let scale = if bin == FFT_SIZE / 2 { 1.0 } else { 2.0 };
            peaks[band] = peaks[band].max(scale * magnitude / self.window_sum);
        }
        self.frames.push(peaks.map(|amplitude| {
            let db = 20.0 * amplitude.max(1e-12).log10();
            ((db - DB_FLOOR) / -DB_FLOOR).clamp(0.0, 1.0)
        }));
        Ok(())
    }

    fn finish(mut self) -> Result<SpectrumData> {
        if self.sample_count == 0 {
            return Err(Error::invalid(
                "Audio tidak memiliki sampel untuk spektrum.",
            ));
        }
        if !self.samples.is_empty() {
            self.push_frame()?;
        }
        Ok(SpectrumData {
            frames: self.frames,
            duration_seconds: self.sample_count as f64 / SAMPLE_RATE as f64,
        })
    }
}

fn fft(real: &mut [f32; FFT_SIZE], imaginary: &mut [f32; FFT_SIZE]) {
    for index in 0..FFT_SIZE {
        let reverse = index.reverse_bits() >> (usize::BITS - FFT_SIZE.trailing_zeros());
        if index < reverse {
            real.swap(index, reverse);
            imaginary.swap(index, reverse);
        }
    }
    let mut length = 2;
    while length <= FFT_SIZE {
        let (step_imaginary, step_real) = (-TAU / length as f32).sin_cos();
        for start in (0..FFT_SIZE).step_by(length) {
            let (mut twiddle_real, mut twiddle_imaginary) = (1.0, 0.0);
            for offset in 0..length / 2 {
                let a = start + offset;
                let b = a + length / 2;
                let product_real = twiddle_real * real[b] - twiddle_imaginary * imaginary[b];
                let product_imaginary = twiddle_real * imaginary[b] + twiddle_imaginary * real[b];
                real[b] = real[a] - product_real;
                imaginary[b] = imaginary[a] - product_imaginary;
                real[a] += product_real;
                imaginary[a] += product_imaginary;
                (twiddle_real, twiddle_imaginary) = (
                    twiddle_real * step_real - twiddle_imaginary * step_imaginary,
                    twiddle_real * step_imaginary + twiddle_imaginary * step_real,
                );
            }
        }
        length *= 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(frequency: f32, amplitude: f32) -> SpectrumData {
        let mut analyzer = Analyzer::new();
        for index in 0..SAMPLE_RATE {
            analyzer
                .push(amplitude * (TAU * frequency * index as f32 / SAMPLE_RATE as f32).sin())
                .unwrap();
        }
        analyzer.finish().unwrap()
    }

    #[test]
    fn real_tones_peak_in_their_logarithmic_frequency_bands() {
        for frequency in [125.0_f32, 500.0, 2000.0, 6000.0] {
            let data = tone(frequency, 0.5);
            let levels = data.levels_at(0.5);
            let strongest = levels
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .unwrap()
                .0;
            let expected = ((frequency / MIN_FREQUENCY).ln() / (MAX_FREQUENCY / MIN_FREQUENCY).ln()
                * BAND_COUNT as f32)
                .floor() as usize;
            assert!(
                strongest.abs_diff(expected) <= 1,
                "{frequency}: peak {strongest}, expected {expected}"
            );
            assert!(levels[strongest] > 0.8);
        }
    }

    #[test]
    fn silence_stays_zero_and_quiet_audio_is_not_normalized_to_full_height() {
        assert!(tone(500.0, 0.0)
            .levels_at(0.5)
            .iter()
            .all(|level| *level == 0.0));
        let loud = tone(500.0, 0.5).levels_at(0.5);
        let quiet = tone(500.0, 0.005).levels_at(0.5);
        let peak = |levels: [f32; BAND_COUNT]| levels.into_iter().fold(0.0_f32, f32::max);
        assert!(peak(loud) - peak(quiet) > 0.5);
        assert!(quiet
            .iter()
            .all(|level| level.is_finite() && (0.0..=1.0).contains(level)));
    }

    #[test]
    fn levels_interpolate_at_frame_centers_and_are_silent_past_the_audio_end() {
        let data = SpectrumData {
            frames: vec![[0.2; BAND_COUNT], [0.8; BAND_COUNT]],
            duration_seconds: 0.2,
        };
        assert_eq!(data.levels_at(0.0), [0.2; BAND_COUNT]);
        assert!((data.levels_at(0.096)[0] - 0.5).abs() < 1e-5);
        assert_eq!(data.levels_at(0.19), [0.8; BAND_COUNT]);
        assert_eq!(data.levels_at(0.2), [0.0; BAND_COUNT]);
        assert_eq!(data.levels_at(f64::NAN), [0.0; BAND_COUNT]);
    }

    #[tokio::test]
    async fn streaming_decode_handles_fragmented_samples_and_rejects_truncation() {
        use tokio::io::AsyncWriteExt;
        let bytes: Vec<u8> = (0..FFT_SIZE).flat_map(|_| 0.0_f32.to_le_bytes()).collect();
        // A three-byte pipe deliberately splits each float across separate reads.
        let (mut writer, reader) = tokio::io::duplex(3);
        let writing = tokio::spawn(async move {
            writer.write_all(&bytes).await.unwrap();
        });
        let data = read_pcm(reader).await.unwrap();
        writing.await.unwrap();
        assert!(data.levels_at(0.01).iter().all(|level| *level == 0.0));
        assert!(read_pcm(std::io::Cursor::new(vec![0_u8; 3])).await.is_err());
        assert!(read_pcm(std::io::Cursor::new(Vec::<u8>::new()))
            .await
            .is_err());
    }
}
