mod probe;
mod title;

/// Supported visual video formats for file pickers and directory scanning.
pub const VIDEO_EXTENSIONS: [&str; 5] = ["mp4", "mov", "mkv", "webm", "avi"];
/// Supported still-image formats for thumbnails and image backgrounds.
pub const IMAGE_EXTENSIONS: [&str; 5] = ["jpg", "jpeg", "png", "webp", "jfif"];

pub use probe::{probe_audio, probe_video, AudioInfo, VideoInfo};
pub use title::{
    build_chapters, build_timed_chapters, clean_artist, clean_proper_title, format_clock,
    format_ffmetadata, format_playlist_txt, format_timestamp_txt, format_youtube_timestamp,
    Chapter, ChapterSource, TimedChapter,
};
