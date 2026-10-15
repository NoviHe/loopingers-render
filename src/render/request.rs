use std::path::PathBuf;

use super::{encoder::Encoder, image_video::Motion, looper::FitMode};
use crate::karaoke::{ass::StyleConfig, LyricLine};

#[derive(Debug, Clone)]
pub struct TrackData {
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub duration: f64,
    pub lyrics: Vec<LyricLine>,
}

/// A still image used as background; the render rebuilds its clip to match the playlist tempo.
#[derive(Debug, Clone)]
pub struct ImageSource {
    pub path: PathBuf,
    pub motion: Motion,
}

#[derive(Debug, Clone)]
pub struct RenderRequest {
    pub custom_filename: String,
    pub video_path: PathBuf,
    pub image_source: Option<ImageSource>,
    pub tracks: Vec<TrackData>,
    pub aspect_ratio: String,
    pub resolution: String,
    pub fit: FitMode,
    pub fps: u32,
    pub style: StyleConfig,
    pub encoder: Encoder,
    pub preset: String,
    pub quality: u32,
    pub beat_pulse: bool,
    pub visual_effect: String,
}
