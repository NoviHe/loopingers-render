use std::path::Path;

use super::{effects::AppliedEffect, encoder::Encoder, graph::FilterGraph, presets};
use crate::{
    error::Result,
    ffmpeg::{self, Tools},
    hardware::HardwareInfo,
    media::{probe_video, VideoInfo},
};

pub const DEFAULT_BPM: f64 = 105.0;
const MIN_DURATION_SECONDS: f64 = 1.0;
const TEMPO_SAMPLE_RATE: f64 = 22050.0;
const TEMPO_HOP: usize = 220;
const MIN_TEMPO: f64 = 65.0;
const MAX_TEMPO: f64 = 155.0;
const MAX_SEARCH_TEMPO: f64 = 160.0;
const MIN_TEMPO_SECONDS: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    ZoomIn,
    ZoomOut,
    PanLeft,
    PanRight,
    Static,
}

impl Motion {
    pub const ALL: [Self; 5] = [
        Self::ZoomIn,
        Self::ZoomOut,
        Self::PanLeft,
        Self::PanRight,
        Self::Static,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::ZoomIn => "Zoom In",
            Self::ZoomOut => "Zoom Out",
            Self::PanLeft => "Geser Kiri",
            Self::PanRight => "Geser Kanan",
            Self::Static => "Diam",
        }
    }
}

pub struct ImageVideo<'a> {
    pub image: &'a Path,
    pub output: &'a Path,
    pub duration: f64,
    pub fps: u32,
    pub aspect_ratio: &'a str,
    pub resolution: &'a str,
    pub motion: Motion,
    pub beat_pulse: bool,
    pub bpm: f64,
    pub effect: &'a AppliedEffect,
}

impl ImageVideo<'_> {
    /// Turns a still image into a looping clip with slow camera motion, an optional beat pulse
    /// and an optional atmosphere effect.
    pub async fn render(&self, tools: &Tools, hardware: &HardwareInfo) -> Result<VideoInfo> {
        let (width, height) = presets::dimensions(self.aspect_ratio, self.resolution);

        let mut graph = FilterGraph::new("0:v");
        graph.chain(&self.motion_filter(width, height));
        self.effect.apply(&mut graph, 1, width, height);
        let filter = graph.finish("null");

        let encoder = Encoder::best_available(hardware);
        let mut cmd = tools.ffmpeg();
        cmd.args([
            "-hide_banner",
            "-nostats",
            "-loglevel",
            "error",
            "-y",
            "-loop",
            "1",
            "-i",
        ])
        .arg(self.image)
        .args(self.effect.input_args())
        .arg("-t")
        .arg(self.duration.to_string())
        .arg("-filter_complex")
        .arg(&filter)
        .args(["-map", "[outv]"])
        .args(encoder.args("p4", 20))
        .arg("-r")
        .arg(self.fps.to_string())
        .arg(self.output);
        ffmpeg::run(&mut cmd, "FFmpeg (konversi gambar)").await?;

        probe_video(tools, self.output).await
    }

    fn motion_filter(&self, width: u32, height: u32) -> String {
        let frames = (self.duration.max(MIN_DURATION_SECONDS) * f64::from(self.fps)) as u64;
        let fps = self.fps;
        let pulse = if self.beat_pulse {
            let beats_per_second = self.bpm.max(60.0) / 60.0;
            format!("+0.038*pow(abs(cos(3.14159*time*{beats_per_second:.2})),4)")
        } else {
            String::new()
        };

        // Working at 2x resolution keeps zoompan from visibly stepping between pixels.
        let (big_w, big_h) = (width * 2, height * 2);
        let zoompan = |zoom: String, x: &str, y: &str| {
            format!(
                "scale={big_w}:{big_h}:force_original_aspect_ratio=increase,crop={big_w}:{big_h},\
                 zoompan=z='{zoom}':d={frames}:x='{x}':y='{y}':s={width}x{height}:fps={fps},\
                 format=yuv420p"
            )
        };
        let center_x = "iw/2-(iw/zoom/2)";
        let center_y = "ih/2-(ih/zoom/2)";

        match self.motion {
            Motion::ZoomIn => zoompan(
                format!("min(1.25, 1.0+on*(0.15/{frames}){pulse})"),
                center_x,
                center_y,
            ),
            Motion::ZoomOut => zoompan(
                format!("max(1.0, 1.20-on*(0.15/{frames}){pulse})"),
                center_x,
                center_y,
            ),
            Motion::PanLeft => zoompan(
                format!("1.15{pulse}"),
                &format!("(iw-iw/zoom)*(1-on/{frames})"),
                "(ih-ih/zoom)/2",
            ),
            Motion::PanRight => zoompan(
                format!("1.15{pulse}"),
                &format!("(iw-iw/zoom)*(on/{frames})"),
                "(ih-ih/zoom)/2",
            ),
            Motion::Static if self.beat_pulse => {
                zoompan(format!("1.01{pulse}"), center_x, center_y)
            }
            Motion::Static => format!(
                "scale={width}:{height}:force_original_aspect_ratio=increase,crop={width}:{height},format=yuv420p"
            ),
        }
    }
}

/// Estimates the tempo from a 25 second excerpt by autocorrelating its loudness onsets.
pub async fn estimate_tempo_bpm(tools: &Tools, audio: &Path) -> f64 {
    let mut cmd = tools.ffmpeg();
    cmd.args(["-v", "error", "-ss", "5", "-t", "25", "-i"])
        .arg(audio)
        .args(["-f", "f32le", "-ac", "1", "-ar", "22050", "-"]);
    let Ok(output) = ffmpeg::run(&mut cmd, "FFmpeg (estimasi tempo)").await else {
        return DEFAULT_BPM;
    };
    let samples: Vec<f32> = output
        .stdout
        .chunks_exact(4)
        .map(|bytes| f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        .collect();
    tempo_from_samples(&samples)
}

fn tempo_from_samples(samples: &[f32]) -> f64 {
    if samples.len() < TEMPO_SAMPLE_RATE as usize * MIN_TEMPO_SECONDS {
        return DEFAULT_BPM;
    }
    let envelope: Vec<f64> = samples
        .chunks_exact(TEMPO_HOP)
        .map(|frame| {
            let energy: f64 = frame.iter().map(|s| f64::from(*s).powi(2)).sum();
            (energy / TEMPO_HOP as f64).sqrt()
        })
        .collect();
    if envelope.len() < 50 {
        return DEFAULT_BPM;
    }

    let mut previous = 0.0;
    let mut onset: Vec<f64> = envelope
        .iter()
        .map(|&level| {
            let rise = (level - previous).max(0.0);
            previous = level;
            rise
        })
        .collect();
    let mean = onset.iter().sum::<f64>() / onset.len() as f64;
    onset.iter_mut().for_each(|value| *value -= mean);

    let hops_per_minute = TEMPO_SAMPLE_RATE / TEMPO_HOP as f64 * 60.0;
    let min_lag = (hops_per_minute / MAX_SEARCH_TEMPO) as usize;
    let max_lag = (hops_per_minute / MIN_TEMPO) as usize;
    if max_lag >= onset.len() {
        return DEFAULT_BPM;
    }

    let correlation =
        |lag: usize| -> f64 { onset.iter().zip(&onset[lag..]).map(|(a, b)| a * b).sum() };
    let best_lag = (min_lag..max_lag).fold((min_lag, f64::MIN), |best, lag| {
        let score = correlation(lag);
        if score > best.1 {
            (lag, score)
        } else {
            best
        }
    });
    (hops_per_minute / best_lag.0 as f64).clamp(MIN_TEMPO, MAX_TEMPO)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clicks(bpm: f64, seconds: usize) -> Vec<f32> {
        let rate = TEMPO_SAMPLE_RATE as usize;
        let beat_samples = (rate as f64 * 60.0 / bpm) as usize;
        let mut samples = vec![0.0f32; rate * seconds];
        let mut position = 0;
        while position < samples.len() {
            for offset in 0..400.min(samples.len() - position) {
                samples[position + offset] = if offset % 2 == 0 { 0.9 } else { -0.9 };
            }
            position += beat_samples;
        }
        samples
    }

    #[test]
    fn detects_a_steady_click_track() {
        let bpm = tempo_from_samples(&clicks(120.0, 25));
        assert!((bpm - 120.0).abs() < 3.0, "estimated {bpm}");
    }

    #[test]
    fn short_or_silent_audio_uses_the_default_tempo() {
        assert_eq!(tempo_from_samples(&[0.0; 1000]), DEFAULT_BPM);
    }

    #[test]
    fn static_motion_without_pulse_is_a_plain_scale() {
        let effect = AppliedEffect::None;
        let job = ImageVideo {
            image: Path::new("a.png"),
            output: Path::new("a.mp4"),
            duration: 10.0,
            fps: 30,
            aspect_ratio: "16:9",
            resolution: "720p",
            motion: Motion::Static,
            beat_pulse: false,
            bpm: 100.0,
            effect: &effect,
        };
        assert_eq!(
            job.motion_filter(1280, 720),
            "scale=1280:720:force_original_aspect_ratio=increase,crop=1280:720,format=yuv420p"
        );
    }
}
