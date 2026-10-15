use std::{
    env,
    fs::File,
    io::BufWriter,
    path::{Path, PathBuf},
};

use ab_glyph::{Font, FontVec, PxScale};
use image::{
    codecs::jpeg::JpegEncoder, imageops::FilterType, DynamicImage, ExtendedColorType, Rgb, RgbImage,
};
use imageproc::drawing::{draw_text_mut, text_size};

use crate::{
    error::{Error, Result},
    ffmpeg::{self, Tools},
    media::clean_proper_title,
};

pub const WIDTH: u32 = 1920;
pub const HEIGHT: u32 = 1080;

const JPEG_QUALITY: u8 = 95;
const BACKGROUND_FALLBACK: Rgb<u8> = Rgb([20, 24, 33]);
const VIDEO_EXTENSIONS: [&str; 5] = ["mp4", "mov", "mkv", "webm", "avi"];
const MAX_SINGLE_COLUMN_TRACKS: usize = 15;
const MAX_ROWS_PER_COLUMN: usize = 11;
const SINGLE_COLUMN_TITLE_CHARS: usize = 30;
const TWO_COLUMN_TITLE_CHARS: usize = 24;

#[derive(Debug, Clone)]
pub struct ThumbnailSpec {
    pub title: String,
    pub subtitle: String,
    pub track_titles: Vec<String>,
    pub column_mode: String,
    pub number_style: String,
    pub title_font_size: u32,
    pub track_font_size: u32,
    pub subtitle_font_size: u32,
    pub vignette_opacity: f32,
    pub title_color: String,
    pub track_color: String,
    pub subtitle_color: String,
    pub title_y_pct: f32,
    pub subtitle_y_pct: f32,
    pub title_font_path: Option<PathBuf>,
    pub subtitle_font_path: Option<PathBuf>,
}

/// Grabs one frame at `timestamp`, scaled and cropped to 1920x1080 when FFmpeg allows it.
pub async fn extract_frame(
    tools: &Tools,
    video: &Path,
    timestamp: f64,
    output: &Path,
) -> Result<()> {
    let seek = timestamp.max(0.0).to_string();
    let mut scaled = tools.ffmpeg();
    scaled
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-ss",
            &seek,
            "-i",
        ])
        .arg(video)
        .args(["-frames:v", "1", "-vf"])
        .arg(format!(
            "scale={WIDTH}:{HEIGHT}:force_original_aspect_ratio=increase,crop={WIDTH}:{HEIGHT}"
        ))
        .args(["-q:v", "2"])
        .arg(output);
    if ffmpeg::run(&mut scaled, "FFmpeg (ambil frame)")
        .await
        .is_ok()
        && output.is_file()
    {
        return Ok(());
    }

    let mut plain = tools.ffmpeg();
    plain
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-ss",
            &seek,
            "-i",
        ])
        .arg(video)
        .args(["-frames:v", "1", "-q:v", "2"])
        .arg(output);
    ffmpeg::run(&mut plain, "FFmpeg (ambil frame)")
        .await
        .map_err(|error| {
            Error::Tool(format!(
                "Gagal mengambil frame video di detik {timestamp}: {error}"
            ))
        })?;
    Ok(())
}

/// Renders the playlist thumbnail for `source` (a video or an image) into a JPEG at `output`.
pub async fn render(
    tools: &Tools,
    temp_dir: &Path,
    source: &Path,
    timestamp: f64,
    spec: ThumbnailSpec,
    output: PathBuf,
) -> Result<()> {
    let is_video = source
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| VIDEO_EXTENSIONS.contains(&ext.to_lowercase().as_str()));

    let (background, frame) = if is_video {
        let frame = temp_dir.join(format!(
            "temp_thumb_frame_{}.jpg",
            crate::util::random_id(12)
        ));
        extract_frame(tools, source, timestamp, &frame).await?;
        (frame.clone(), Some(frame))
    } else {
        (source.to_path_buf(), None)
    };

    let drawn = tokio::task::spawn_blocking(move || draw(&background, &spec, &output))
        .await
        .map_err(|error| Error::internal(format!("Proses thumbnail terhenti: {error}")))?;
    if let Some(frame) = frame {
        let _ = std::fs::remove_file(frame);
    }
    drawn
}

fn draw(background: &Path, spec: &ThumbnailSpec, output: &Path) -> Result<()> {
    let fonts = Fonts::load_for_spec(spec)?;
    let mut canvas = load_background(background);
    apply_vignette(&mut canvas, spec.vignette_opacity);

    let title_scale = fonts.title.scale(spec.title_font_size);
    let track_scale = fonts.track.scale(spec.track_font_size);
    let subtitle_scale = fonts.subtitle.scale(spec.subtitle_font_size);

    if !spec.title.trim().is_empty() {
        let title_y = (f64::from(HEIGHT) * f64::from(spec.title_y_pct)) as i32;
        draw_outlined(
            &mut canvas,
            &fonts.title.font,
            title_scale,
            spec.title.trim(),
            (WIDTH as i32 / 2, title_y),
            Align::Center,
            parse_color(&spec.title_color),
            5,
        );
    }

    let track_color = parse_color(&spec.track_color);
    for row in layout_tracks(spec) {
        draw_outlined(
            &mut canvas,
            &fonts.track.font,
            track_scale,
            &row.text,
            (row.x, row.y),
            row.align,
            track_color,
            3,
        );
    }

    if !spec.subtitle.trim().is_empty() {
        let subtitle_y = (f64::from(HEIGHT) * f64::from(spec.subtitle_y_pct)) as i32;
        draw_outlined(
            &mut canvas,
            &fonts.subtitle.font,
            subtitle_scale,
            spec.subtitle.trim(),
            (WIDTH as i32 / 2, subtitle_y),
            Align::Center,
            parse_color(&spec.subtitle_color),
            3,
        );
    }

    let file = File::create(output)?;
    JpegEncoder::new_with_quality(BufWriter::new(file), JPEG_QUALITY)
        .encode(canvas.as_raw(), WIDTH, HEIGHT, ExtendedColorType::Rgb8)
        .map_err(|error| Error::internal(format!("Gagal menyimpan thumbnail: {error}")))
}

fn load_background(path: &Path) -> RgbImage {
    match image::open(path) {
        Ok(image) => {
            if image.width() == WIDTH && image.height() == HEIGHT {
                image.to_rgb8()
            } else {
                DynamicImage::resize_to_fill(&image, WIDTH, HEIGHT, FilterType::Lanczos3).to_rgb8()
            }
        }
        Err(_) => RgbImage::from_pixel(WIDTH, HEIGHT, BACKGROUND_FALLBACK),
    }
}

/// Darkens the edges so the white lists and title stay readable on bright footage.
/// Later bands overwrite earlier ones, in the order left, right, top, bottom.
fn vignette_alpha(width: u32, height: u32, opacity: f32) -> Vec<u8> {
    let (w, h) = (width as usize, height as usize);
    let max_alpha = 255.0 * f64::from(opacity.clamp(0.0, 1.0));
    let mut alpha = vec![0u8; w * h];
    let falloff = |progress: f64| progress.powf(1.8);

    let edge = (w as f64 * 0.42) as usize;
    for x in 0..edge {
        let value = (max_alpha * falloff(1.0 - x as f64 / edge as f64) * 0.95) as u8;
        (0..h).for_each(|y| alpha[y * w + x] = value);
    }
    for x in (w - edge)..w {
        let value = (max_alpha * falloff((x - (w - edge)) as f64 / edge as f64) * 0.95) as u8;
        (0..h).for_each(|y| alpha[y * w + x] = value);
    }
    let top = (h as f64 * 0.28) as usize;
    for y in 0..top {
        let value = (max_alpha * falloff(1.0 - y as f64 / top as f64) * 0.9) as u8;
        alpha[y * w..(y + 1) * w].fill(value);
    }
    let bottom = (h as f64 * 0.25) as usize;
    for y in (h - bottom)..h {
        let value = (max_alpha * falloff((y - (h - bottom)) as f64 / bottom as f64) * 0.85) as u8;
        alpha[y * w..(y + 1) * w].fill(value);
    }
    alpha
}

fn apply_vignette(canvas: &mut RgbImage, opacity: f32) {
    if opacity <= 0.0 {
        return;
    }
    let alpha = vignette_alpha(canvas.width(), canvas.height(), opacity);
    for (pixel, &a) in canvas.pixels_mut().zip(&alpha) {
        let keep = 1.0 - f32::from(a) / 255.0;
        pixel.0 = pixel
            .0
            .map(|channel| (f32::from(channel) * keep).round() as u8);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TrackRow {
    pub text: String,
    pub x: i32,
    pub y: i32,
    pub align: Align,
}

pub fn shorten(title: &str, limit: usize) -> String {
    if title.chars().count() > limit {
        let kept: String = title.chars().take(limit - 2).collect();
        format!("{kept}...")
    } else {
        title.to_owned()
    }
}

/// Positions the track list: one column on the left, or two columns when there are many tracks.
pub fn layout_tracks(spec: &ThumbnailSpec) -> Vec<TrackRow> {
    let tracks = &spec.track_titles;
    let two_columns = match spec.column_mode.as_str() {
        "2" => true,
        "1" => false,
        _ => tracks.len() > 10,
    };
    let start_y = (f64::from(HEIGHT) * 0.24) as i32;
    let spacing = (f64::from(spec.track_font_size) * 1.55) as i32;
    let left_x = (f64::from(WIDTH) * 0.055) as i32;
    let row_y = |row: usize| start_y + row as i32 * spacing;

    if !two_columns {
        return tracks
            .iter()
            .take(MAX_SINGLE_COLUMN_TRACKS)
            .enumerate()
            .map(|(index, title)| TrackRow {
                text: format!(
                    "{:02}.  {}",
                    index + 1,
                    shorten(&clean_proper_title(title), SINGLE_COLUMN_TITLE_CHARS)
                ),
                x: left_x,
                y: row_y(index),
                align: Align::Left,
            })
            .take_while(|row| row.y <= HEIGHT as i32 - 120)
            .collect();
    }

    let half = tracks.len().div_ceil(2);
    let right_x = (f64::from(WIDTH) * 0.945) as i32;
    let left = tracks
        .iter()
        .take(half)
        .take(MAX_ROWS_PER_COLUMN)
        .enumerate()
        .map(|(index, title)| TrackRow {
            text: format!(
                "{:02}.  {}",
                index + 1,
                shorten(&clean_proper_title(title), TWO_COLUMN_TITLE_CHARS)
            ),
            x: left_x,
            y: row_y(index),
            align: Align::Left,
        });
    let right = tracks
        .iter()
        .skip(half)
        .take(MAX_ROWS_PER_COLUMN)
        .enumerate()
        .map(|(index, title)| {
            let title = shorten(&clean_proper_title(title), TWO_COLUMN_TITLE_CHARS);
            let text = if spec.number_style == "mirrored" {
                format!("{title}  .{:02}", index + 1)
            } else {
                format!("{:02}.  {title}", half + index + 1)
            };
            TrackRow {
                text,
                x: right_x,
                y: row_y(index),
                align: Align::Right,
            }
        });
    left.chain(right).collect()
}

/// `#RRGGBB` or `#RGB`; anything else renders white.
fn parse_color(hex: &str) -> Rgb<u8> {
    let digits = hex.trim().trim_start_matches('#');
    let expanded: String = if digits.len() == 3 {
        digits.chars().flat_map(|c| [c, c]).collect()
    } else {
        digits.to_owned()
    };
    match u32::from_str_radix(&expanded, 16) {
        Ok(value) if expanded.len() == 6 => {
            Rgb([(value >> 16) as u8, (value >> 8) as u8, value as u8])
        }
        _ => Rgb([255, 255, 255]),
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_outlined(
    canvas: &mut RgbImage,
    font: &FontVec,
    scale: PxScale,
    text: &str,
    (x, y): (i32, i32),
    align: Align,
    color: Rgb<u8>,
    outline: i32,
) {
    let (text_width, _) = text_size(scale, font, text);
    let x = match align {
        Align::Left => x,
        Align::Center => x - (text_width / 2) as i32,
        Align::Right => x - text_width as i32,
    };
    let black = Rgb([0, 0, 0]);
    for dx in -outline..=outline {
        for dy in -outline..=outline {
            if dx != 0 || dy != 0 {
                draw_text_mut(canvas, black, x + dx, y + dy, scale, font, text);
            }
        }
    }
    let shadow = outline.max(2);
    draw_text_mut(canvas, black, x + shadow, y + shadow + 1, scale, font, text);
    draw_text_mut(canvas, color, x, y, scale, font, text);
}

struct LoadedFont {
    font: FontVec,
}

impl LoadedFont {
    /// Converts a size in pixels per em, as image editors use, to an `ab_glyph` scale.
    fn scale(&self, size: u32) -> PxScale {
        self.font
            .pt_to_px_scale(size as f32)
            .unwrap_or_else(|| PxScale::from(size as f32))
    }
}

struct Fonts {
    title: LoadedFont,
    track: LoadedFont,
    subtitle: LoadedFont,
}

impl Fonts {
    fn load_for_spec(spec: &ThumbnailSpec) -> Result<Self> {
        let title = if let Some(path) = &spec.title_font_path {
            load_font_file(path).unwrap_or_else(|_| {
                load_font(&["impact.ttf", "arialbd.ttf", "DejaVuSans-Bold.ttf"]).unwrap()
            })
        } else {
            load_font(&["impact.ttf", "arialbd.ttf", "DejaVuSans-Bold.ttf"])?
        };

        let subtitle = if let Some(path) = &spec.subtitle_font_path {
            load_font_file(path).unwrap_or_else(|_| {
                load_font(&["segoesc.ttf", "ariali.ttf", "arial.ttf", "DejaVuSans.ttf"]).unwrap()
            })
        } else {
            load_font(&["segoesc.ttf", "ariali.ttf", "arial.ttf", "DejaVuSans.ttf"])?
        };

        let track = load_font(&["arialbd.ttf", "DejaVuSans-Bold.ttf"])?;

        Ok(Self {
            title,
            track,
            subtitle,
        })
    }
}

fn load_font_file(path: &Path) -> Result<LoadedFont> {
    let bytes = std::fs::read(path).map_err(|e| {
        Error::internal(format!("Gagal membaca berkas font {}: {e}", path.display()))
    })?;
    let font = FontVec::try_from_vec(bytes)
        .map_err(|_| Error::internal(format!("Format font tidak valid: {}", path.display())))?;
    Ok(LoadedFont { font })
}

fn font_dirs() -> Vec<PathBuf> {
    let windows_root =
        env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
    vec![
        windows_root.join("Fonts"),
        PathBuf::from("/usr/share/fonts/truetype/dejavu"),
        PathBuf::from("/Library/Fonts"),
        PathBuf::from("/System/Library/Fonts/Supplemental"),
    ]
}

/// Loads the first font file from `candidates` found in a system font folder.
fn load_font(candidates: &[&str]) -> Result<LoadedFont> {
    let dirs = font_dirs();
    candidates
        .iter()
        .flat_map(|name| dirs.iter().map(move |dir| dir.join(name)))
        .find_map(|path| {
            let bytes = std::fs::read(path).ok()?;
            FontVec::try_from_vec(bytes).ok()
        })
        .map(|font| LoadedFont { font })
        .ok_or_else(|| {
            Error::internal(format!(
                "Font tidak ditemukan di sistem (dicari: {})",
                candidates.join(", ")
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(tracks: usize, mode: &str, style: &str) -> ThumbnailSpec {
        ThumbnailSpec {
            title: "T".into(),
            subtitle: "S".into(),
            track_titles: (1..=tracks).map(|n| format!("song {n}")).collect(),
            column_mode: mode.into(),
            number_style: style.into(),
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
        }
    }

    #[test]
    fn few_tracks_use_a_single_left_column() {
        let rows = layout_tracks(&spec(3, "auto", "mirrored"));
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].text, "01.  Song 1");
        assert!(rows.iter().all(|row| row.align == Align::Left));
    }

    #[test]
    fn many_tracks_split_into_mirrored_columns() {
        let rows = layout_tracks(&spec(12, "auto", "mirrored"));
        assert_eq!(rows.len(), 12);
        assert_eq!(rows[6].text, "Song 7  .01");
        assert_eq!(rows[6].align, Align::Right);
        let sequential = layout_tracks(&spec(12, "2", "sequential"));
        assert_eq!(sequential[6].text, "07.  Song 7");
    }

    #[test]
    fn long_titles_are_shortened() {
        assert_eq!(shorten("abcdefghij", 6), "abcd...");
        assert_eq!(shorten("abc", 6), "abc");
    }

    #[test]
    fn colors_parse_hex_and_fall_back_to_white() {
        assert_eq!(parse_color("#FFD700"), Rgb([255, 215, 0]));
        assert_eq!(parse_color("0f0"), Rgb([0, 255, 0]));
        assert_eq!(parse_color("red"), Rgb([255, 255, 255]));
    }

    #[test]
    fn vignette_is_darkest_at_the_outer_edges() {
        let alpha = vignette_alpha(200, 100, 1.0);
        let middle_row = 50 * 200;
        assert!(alpha[middle_row] > alpha[middle_row + 40]);
        assert_eq!(alpha[middle_row + 100], 0);
    }
}
