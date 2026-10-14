use std::{collections::HashMap, path::Path};

use serde::Deserialize;

use super::title::{clean_artist, clean_proper_title, format_clock};
use crate::{
    error::{Error, Result},
    ffmpeg::{self, Tools},
    util::round2,
};

const DEFAULT_FPS: f64 = 30.0;
const BYTES_PER_MB: f64 = 1024.0 * 1024.0;

#[derive(Debug, Clone)]
pub struct VideoInfo {
    pub path: String,
    pub filename: String,
    pub duration: f64,
    pub duration_str: String,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub aspect_ratio: String,
    pub has_audio: bool,
    pub size_bytes: u64,
    pub size_mb: f64,
}

#[derive(Debug, Clone)]
pub struct AudioInfo {
    pub path: String,
    pub filename: String,
    pub title: String,
    pub artist: String,
    pub duration: f64,
    pub duration_str: String,
    pub size_bytes: u64,
    pub size_mb: f64,
}

#[derive(Debug, Default, Deserialize)]
struct Probe {
    #[serde(default)]
    streams: Vec<Stream>,
    #[serde(default)]
    format: Format,
}

#[derive(Debug, Default, Deserialize)]
struct Format {
    duration: Option<String>,
    #[serde(default)]
    tags: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct Stream {
    codec_type: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    avg_frame_rate: Option<String>,
    r_frame_rate: Option<String>,
}

impl Probe {
    fn duration(&self) -> f64 {
        self.format
            .duration
            .as_deref()
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| value.is_finite() && *value > 0.0)
            .unwrap_or(0.0)
    }

    fn tag(&self, name: &str) -> Option<&str> {
        self.format
            .tags
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
            .filter(|value| !value.trim().is_empty())
    }

    fn stream(&self, codec_type: &str) -> Option<&Stream> {
        self.streams
            .iter()
            .find(|stream| stream.codec_type.as_deref() == Some(codec_type))
    }
}

async fn probe(tools: &Tools, path: &Path) -> Result<Probe> {
    let mut cmd = tools.ffprobe();
    cmd.args([
        "-v",
        "error",
        "-print_format",
        "json",
        "-show_format",
        "-show_streams",
    ])
    .arg(path);
    let output = ffmpeg::run(&mut cmd, "ffprobe").await?;
    serde_json::from_slice(&output.stdout)
        .map_err(|error| Error::Tool(format!("Output ffprobe tidak valid: {error}")))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

async fn file_size(path: &Path) -> Result<(u64, f64)> {
    let bytes = tokio::fs::metadata(path).await?.len();
    Ok((bytes, round2(bytes as f64 / BYTES_PER_MB)))
}

fn format_duration(seconds: f64) -> String {
    format_clock(seconds.round() as u64)
}

fn parse_frame_rate(raw: &str) -> Option<f64> {
    let (numerator, denominator) = raw.split_once('/')?;
    let (numerator, denominator): (f64, f64) = (numerator.parse().ok()?, denominator.parse().ok()?);
    (denominator > 0.0 && numerator > 0.0).then(|| numerator / denominator)
}

fn aspect_label(width: u32, height: u32) -> String {
    if width == 0 || height == 0 {
        return "16:9".to_owned();
    }
    let ratio = f64::from(width) / f64::from(height);
    let known = [
        (16.0 / 9.0, "16:9"),
        (9.0 / 16.0, "9:16"),
        (1.0, "1:1"),
        (4.0 / 3.0, "4:3"),
    ];
    known
        .iter()
        .find(|(target, _)| (ratio - target).abs() < 0.1)
        .map(|(_, label)| (*label).to_owned())
        .unwrap_or_else(|| format!("{width}:{height}"))
}

pub async fn probe_video(tools: &Tools, path: &Path) -> Result<VideoInfo> {
    let probe = probe(tools, path).await?;
    let (size_bytes, size_mb) = file_size(path).await?;
    let duration = probe.duration();
    let video = probe.stream("video");
    let width = video.and_then(|stream| stream.width).unwrap_or(0);
    let height = video.and_then(|stream| stream.height).unwrap_or(0);
    let fps = video
        .and_then(|stream| {
            [&stream.avg_frame_rate, &stream.r_frame_rate]
                .into_iter()
                .flatten()
                .find_map(|raw| parse_frame_rate(raw))
        })
        .unwrap_or(DEFAULT_FPS);

    Ok(VideoInfo {
        path: path.to_string_lossy().into_owned(),
        filename: file_name(path),
        duration,
        duration_str: format_duration(duration),
        width,
        height,
        fps,
        aspect_ratio: aspect_label(width, height),
        has_audio: probe.stream("audio").is_some(),
        size_bytes,
        size_mb,
    })
}

pub async fn probe_audio(tools: &Tools, path: &Path) -> Result<AudioInfo> {
    let probe = probe(tools, path).await?;
    let (size_bytes, size_mb) = file_size(path).await?;
    let filename = file_name(path);
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let title = probe
        .tag("title")
        .map(clean_proper_title)
        .unwrap_or_else(|| clean_proper_title(&stem));
    let artist = probe.tag("artist").map(clean_artist).unwrap_or_default();
    let duration = probe.duration();

    Ok(AudioInfo {
        path: path.to_string_lossy().into_owned(),
        filename,
        title,
        artist,
        duration,
        duration_str: format_duration(duration),
        size_bytes,
        size_mb,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_rate_handles_fractions_and_zero() {
        assert_eq!(parse_frame_rate("30/1"), Some(30.0));
        assert!((parse_frame_rate("30000/1001").unwrap() - 29.97).abs() < 0.01);
        assert_eq!(parse_frame_rate("0/0"), None);
    }

    #[test]
    fn aspect_label_matches_common_ratios() {
        assert_eq!(aspect_label(1920, 1080), "16:9");
        assert_eq!(aspect_label(1080, 1920), "9:16");
        assert_eq!(aspect_label(1000, 1000), "1:1");
        assert_eq!(aspect_label(2560, 1080), "2560:1080");
    }

    #[test]
    fn tags_are_matched_case_insensitively() {
        let probe: Probe = serde_json::from_str(
            r#"{"format":{"duration":"12.5","tags":{"TITLE":"Hello","ARTIST":"  "}}}"#,
        )
        .unwrap();
        assert_eq!(probe.tag("title"), Some("Hello"));
        assert_eq!(probe.tag("artist"), None);
        assert_eq!(probe.duration(), 12.5);
    }
}
