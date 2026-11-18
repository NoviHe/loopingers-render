//! Independent text, background and logo layers for the new Visualizer preview.
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime},
};

use eframe::egui::{self, Color32, FontFamily, FontId, Pos2, Rect, TextureHandle, Vec2};
use image::{imageops, ImageReader, Rgba, RgbaImage};
use loopkaraoke::config::{VisualizerPreviewSettings, VisualizerTextFont};

const PROBE_INTERVAL: Duration = Duration::from_secs(1);
const PREVIEW_WIDTH: u32 = 960;
const PREVIEW_HEIGHT: u32 = 540;

#[derive(Clone, Debug, PartialEq, Eq)]
struct FileStamp {
    path: PathBuf,
    modified: Option<SystemTime>,
    length: u64,
}

impl FileStamp {
    fn read(path: &Path) -> Result<Self, String> {
        let metadata = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if !metadata.is_file() || metadata.len() > 64 * 1024 * 1024 {
            return Err("Pilih gambar berukuran maksimal 64 MB.".to_owned());
        }
        Ok(Self {
            path: path.to_owned(),
            modified: metadata.modified().ok(),
            length: metadata.len(),
        })
    }
}

struct CachedImage {
    requested: Option<PathBuf>,
    stamp: Option<FileStamp>,
    next_probe: Instant,
    pixels: Option<RgbaImage>,
    texture: Option<TextureHandle>,
}

impl Default for CachedImage {
    fn default() -> Self {
        Self {
            requested: None,
            stamp: None,
            next_probe: Instant::now(),
            pixels: None,
            texture: None,
        }
    }
}

impl CachedImage {
    /// Changes are checked at most once a second; unchanged frames perform no file I/O.
    fn refresh(
        &mut self,
        path: Option<&Path>,
        max_width: u32,
        max_height: u32,
    ) -> Option<Result<(), String>> {
        let changed = self.requested.as_deref() != path;
        if !changed && Instant::now() < self.next_probe {
            return None;
        }
        self.next_probe = Instant::now() + PROBE_INTERVAL;
        self.requested = path.map(Path::to_owned);
        let Some(path) = path else {
            if changed {
                self.stamp = None;
                self.pixels = None;
                self.texture = None;
                return Some(Ok(()));
            }
            return None;
        };
        let stamp = match FileStamp::read(path) {
            Ok(stamp) => stamp,
            Err(error) => {
                self.stamp = None;
                self.pixels = None;
                self.texture = None;
                return Some(Err(error));
            }
        };
        if !changed && self.stamp.as_ref() == Some(&stamp) {
            return None;
        }
        self.stamp = Some(stamp);
        self.texture = None;
        match decode_preview(path, max_width, max_height) {
            Ok(image) => {
                self.pixels = Some(image);
                Some(Ok(()))
            }
            Err(error) => {
                self.pixels = None;
                Some(Err(error))
            }
        }
    }

    fn texture(&mut self, ctx: &egui::Context, name: &str) -> Option<&TextureHandle> {
        if self.texture.is_none() {
            self.texture = self.pixels.as_ref().map(|pixels| upload(ctx, name, pixels));
        }
        self.texture.as_ref()
    }
}

pub(super) struct VisualizerPreviewState {
    pub settings: VisualizerPreviewSettings,
    pub active_tab: usize,
    pub error: Option<String>,
    background: CachedImage,
    logo: CachedImage,
    blurred: Option<(u32, TextureHandle)>,
    blur_base: Option<RgbaImage>,
    blur_updated: Instant,
    placeholder: Option<RgbaImage>,
    background_error: Option<String>,
    logo_error: Option<String>,
}

impl VisualizerPreviewState {
    pub fn new(settings: VisualizerPreviewSettings) -> Self {
        Self {
            settings,
            active_tab: 0,
            error: None,
            background: CachedImage::default(),
            logo: CachedImage::default(),
            blurred: None,
            blur_base: None,
            blur_updated: Instant::now(),
            placeholder: None,
            background_error: None,
            logo_error: None,
        }
    }

    fn sync_error(&mut self) {
        let errors: Vec<_> = [&self.background_error, &self.logo_error]
            .into_iter()
            .flatten()
            .cloned()
            .collect();
        self.error = if errors.is_empty() {
            None
        } else {
            Some(errors.join("\n"))
        };
    }

    pub fn set_logo(&mut self, path: PathBuf) -> Result<(), String> {
        let result = FileStamp::read(&path)
            .and_then(|stamp| decode_preview(&path, 1024, 1024).map(|pixels| (stamp, pixels)));
        match result {
            Ok((stamp, pixels)) => {
                self.logo = CachedImage {
                    requested: Some(path.clone()),
                    stamp: Some(stamp),
                    next_probe: Instant::now() + PROBE_INTERVAL,
                    pixels: Some(pixels),
                    texture: None,
                };
                self.settings.logo.path = Some(path);
                self.settings.logo.enabled = true;
                self.logo_error = None;
                self.sync_error();
                Ok(())
            }
            Err(error) => {
                self.logo_error = Some(format!("Logo tidak dapat dibuka: {error}"));
                self.sync_error();
                Err(error)
            }
        }
    }

    pub fn clear_logo(&mut self) {
        self.settings.logo.path = None;
        self.settings.logo.enabled = false;
        self.logo = CachedImage::default();
        self.logo_error = None;
        self.sync_error();
    }

    pub fn draw_background(
        &mut self,
        ui: &egui::Ui,
        canvas: Rect,
        source: Option<&Path>,
        beat: f32,
    ) {
        if !canvas.is_positive() {
            return;
        }
        if !self.settings.background.enabled {
            ui.painter_at(canvas).rect_filled(canvas, 0, Color32::BLACK);
            return;
        }
        if let Some(result) = self
            .background
            .refresh(source, PREVIEW_WIDTH, PREVIEW_HEIGHT)
        {
            self.blurred = None;
            self.blur_base = None;
            self.background_error = result
                .err()
                .map(|error| format!("Latar tidak dapat dibuka: {error}"));
            self.sync_error();
        }
        let settings = &self.settings.background;
        let blur = settings.blur_px.min(30);
        let zoom = if settings.beat_pulse {
            1.0 + bounded_beat(beat) * settings.beat_strength_pct.min(20) as f32 / 100.0
        } else {
            1.0
        };
        let painter = ui.painter_at(canvas);
        let destination = Rect::from_center_size(canvas.center(), canvas.size() * zoom);
        if blur > 0 {
            let needs_update = self
                .blurred
                .as_ref()
                .is_none_or(|(previous, _)| *previous != blur);
            let update_interval = Duration::from_millis(60);
            let elapsed = self.blur_updated.elapsed();
            if needs_update && self.blurred.is_some() && elapsed < update_interval {
                ui.ctx().request_repaint_after(update_interval - elapsed);
            } else if needs_update {
                let base = self.blur_base.get_or_insert_with(|| {
                    let original =
                        self.background.pixels.as_ref().unwrap_or_else(|| {
                            self.placeholder.get_or_insert_with(placeholder_pixels)
                        });
                    // Cache a smaller image: blur slider drags do not resize or
                    // decode the original again, and stay bounded in cost.
                    image::DynamicImage::ImageRgba8(original.clone())
                        .thumbnail(480, 270)
                        .to_rgba8()
                });
                let sigma = (blur as f32 * base.height() as f32 / 1080.0).max(0.1);
                let blurred = blur_pixels(base, sigma);
                self.blurred = Some((
                    blur,
                    upload(ui.ctx(), "visualizer-blurred-background", &blurred),
                ));
                self.blur_updated = Instant::now();
            }
            if let Some((_, texture)) = &self.blurred {
                paint_image_cover(&painter, texture, destination, Color32::WHITE);
            }
        } else if let Some(texture) = self.background.texture(ui.ctx(), "visualizer-background") {
            paint_image_cover(&painter, texture, destination, Color32::WHITE);
        } else {
            super::thumbnail_tab::draw_scenic_placeholder(&painter, destination);
        }
        let tint = settings.tint_color;
        painter.rect_filled(
            canvas,
            0,
            Color32::from_rgba_unmultiplied(tint[0], tint[1], tint[2], alpha(settings.tint_pct)),
        );
        painter.rect_filled(
            canvas,
            0,
            Color32::from_black_alpha(alpha(settings.dim_pct)),
        );
        paint_vignette(&painter, canvas, settings.vignette_pct);
    }

    pub fn draw_foreground(&mut self, ui: &egui::Ui, canvas: Rect, title: Option<&str>, beat: f32) {
        if !canvas.is_positive() {
            return;
        }
        self.draw_logo(ui, canvas, beat);
        let settings = &self.settings.text;
        if !settings.enabled || settings.opacity_pct == 0 {
            return;
        }
        let text = if settings.use_song_title {
            title.unwrap_or("")
        } else {
            &settings.content
        };
        if text.trim().is_empty() {
            return;
        }
        let family = match settings.font {
            VisualizerTextFont::Sans => FontFamily::Proportional,
            VisualizerTextFont::Semibold => FontFamily::Name(super::theme::SEMIBOLD.into()),
            VisualizerTextFont::Monospace => FontFamily::Monospace,
        };
        let painter = ui.painter_at(canvas);
        let font = FontId::new(
            (settings.size_px.clamp(12, 160) as f32 * canvas.height() / 1080.0).max(5.0),
            family,
        );
        // Bound user-provided content and wrap it before positioning the full block.
        let text: String = text.chars().take(4096).collect();
        let white = painter.layout(text, font, Color32::WHITE, canvas.width() * 0.92);
        let rect = positioned_rect(canvas, white.size(), settings.x_pct, settings.y_pct);
        let color = settings.color;
        painter.galley_with_override_text_color(
            rect.min + Vec2::splat((canvas.height() / 540.0).max(0.6)),
            white.clone(),
            Color32::from_black_alpha((alpha(settings.opacity_pct) as f32 * 0.7) as u8),
        );
        painter.galley_with_override_text_color(
            rect.min,
            white,
            Color32::from_rgba_unmultiplied(
                color[0],
                color[1],
                color[2],
                alpha(settings.opacity_pct),
            ),
        );
    }

    fn draw_logo(&mut self, ui: &egui::Ui, canvas: Rect, beat: f32) {
        if !self.settings.logo.enabled {
            return;
        }
        let path = self.settings.logo.path.as_deref();
        if let Some(result) = self.logo.refresh(path, 1024, 1024) {
            self.logo_error = result
                .err()
                .map(|error| format!("Logo tidak dapat dibuka: {error}"));
            self.sync_error();
        }
        let settings = &self.settings.logo;
        let Some(texture) = self.logo.texture(ui.ctx(), "visualizer-custom-logo") else {
            return;
        };
        let pulse = if settings.beat_pulse {
            1.0 + bounded_beat(beat) * 0.08
        } else {
            1.0
        };
        let size = logo_size(canvas, texture.size_vec2(), settings.size_pct, pulse);
        let rect = positioned_rect(canvas, size, settings.x_pct, settings.y_pct);
        ui.painter_at(canvas).image(
            texture.id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
            Color32::from_white_alpha(alpha(settings.opacity_pct)),
        );
    }
}

fn decode_preview(path: &Path, max_width: u32, max_height: u32) -> Result<RgbaImage, String> {
    let mut reader = ImageReader::open(path)
        .map_err(|error| error.to_string())?
        .with_guessed_format()
        .map_err(|error| error.to_string())?;
    if !matches!(
        reader.format(),
        Some(image::ImageFormat::Png | image::ImageFormat::Jpeg)
    ) {
        return Err("Format gambar harus PNG atau JPEG.".to_owned());
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16_384);
    limits.max_image_height = Some(16_384);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().map_err(|error| error.to_string())?;
    Ok(image.thumbnail(max_width, max_height).to_rgba8())
}

fn upload(ctx: &egui::Context, name: &str, pixels: &RgbaImage) -> TextureHandle {
    ctx.load_texture(
        name,
        egui::ColorImage::from_rgba_unmultiplied(
            [pixels.width() as usize, pixels.height() as usize],
            pixels.as_raw(),
        ),
        egui::TextureOptions::LINEAR,
    )
}

fn blur_pixels(base: &RgbaImage, sigma: f32) -> RgbaImage {
    // The three-box Gaussian approximation has bounded cost regardless of radius.
    // Premultiplication prevents dark fringes around transparent PNG pixels.
    let mut premultiplied = base.clone();
    for pixel in premultiplied.pixels_mut() {
        let a = pixel[3] as u16;
        for channel in pixel.0.iter_mut().take(3) {
            *channel = (*channel as u16 * a / 255) as u8;
        }
    }
    let mut blurred = imageops::fast_blur(&premultiplied, sigma);
    for pixel in blurred.pixels_mut() {
        let a = pixel[3] as u16;
        for channel in pixel.0.iter_mut().take(3) {
            *channel = (*channel as u16 * 255).checked_div(a).unwrap_or(0).min(255) as u8;
        }
    }
    blurred
}

fn placeholder_pixels() -> RgbaImage {
    RgbaImage::from_fn(PREVIEW_WIDTH, PREVIEW_HEIGHT, |_, y| {
        let fraction = y as f32 / PREVIEW_HEIGHT as f32;
        let rgb = if fraction >= 0.72 {
            [20, 18, 22]
        } else if fraction >= 0.52 {
            [42, 34, 40]
        } else if fraction >= 0.36 {
            [120, 75, 45]
        } else {
            [16, 24, 38]
        };
        Rgba([rgb[0], rgb[1], rgb[2], 255])
    })
}

fn paint_image_cover(painter: &egui::Painter, texture: &TextureHandle, rect: Rect, tint: Color32) {
    let texture_size = texture.size_vec2();
    let scale = (rect.width() / texture_size.x).max(rect.height() / texture_size.y);
    let visible = rect.size() / (texture_size * scale);
    let uv = Rect::from_center_size(egui::pos2(0.5, 0.5), visible);
    painter.image(texture.id(), rect, uv, tint);
}

fn positioned_rect(canvas: Rect, requested: Vec2, x_pct: u32, y_pct: u32) -> Rect {
    let size = requested.min(canvas.size());
    let desired = canvas.min
        + canvas.size() * egui::vec2(x_pct.min(100) as f32 / 100.0, y_pct.min(100) as f32 / 100.0);
    let min = canvas.min + size * 0.5;
    let max = canvas.max - size * 0.5;
    Rect::from_center_size(
        egui::pos2(desired.x.clamp(min.x, max.x), desired.y.clamp(min.y, max.y)),
        size,
    )
}

fn logo_size(canvas: Rect, original: Vec2, size_pct: u32, pulse: f32) -> Vec2 {
    let desired = canvas.height() * size_pct.clamp(5, 70) as f32 / 100.0 * pulse;
    let scale = (desired / original.y.max(1.0)).min(canvas.width() * 0.95 / original.x.max(1.0));
    original * scale
}

fn bounded_beat(beat: f32) -> f32 {
    if beat.is_finite() {
        beat.clamp(0.0, 1.0)
    } else {
        0.0
    }
}
fn alpha(percent: u32) -> u8 {
    (percent.min(100) as f32 * 2.55).round() as u8
}

fn vignette_alpha(x: f32, y: f32, strength: u32) -> u8 {
    let radius = (x * x + y * y).sqrt() * std::f32::consts::FRAC_1_SQRT_2;
    let t = ((radius - 0.22) / 0.78).clamp(0.0, 1.0);
    (t * t * (3.0 - 2.0 * t) * alpha(strength) as f32 * 0.94).round() as u8
}

fn paint_vignette(painter: &egui::Painter, rect: Rect, strength: u32) {
    if strength == 0 {
        return;
    }
    const X: usize = 32;
    const Y: usize = 18;
    let mut mesh = egui::Mesh::default();
    for y in 0..=Y {
        for x in 0..=X {
            let u = x as f32 / X as f32;
            let v = y as f32 / Y as f32;
            mesh.colored_vertex(
                rect.min + rect.size() * egui::vec2(u, v),
                Color32::from_black_alpha(vignette_alpha(u * 2.0 - 1.0, v * 2.0 - 1.0, strength)),
            );
        }
    }
    for y in 0..Y {
        for x in 0..X {
            let i = (y * (X + 1) + x) as u32;
            let next = i + X as u32 + 1;
            mesh.add_triangle(i, i + 1, next);
            mesh.add_triangle(i + 1, next + 1, next);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_reduces_edges_while_placeholder_and_vignette_have_expected_ranges() {
        let base = placeholder_pixels();
        let blurred = blur_pixels(&base, 8.0);
        let y = (PREVIEW_HEIGHT as f32 * 0.36).ceil() as u32;
        let edge = |image: &RgbaImage| {
            (image.get_pixel(10, y)[0] as i32 - image.get_pixel(10, y - 1)[0] as i32).abs()
        };
        assert!(edge(&blurred) < edge(&base));
        assert_eq!(vignette_alpha(0.0, 0.0, 100), 0);
        assert!(vignette_alpha(1.0, 1.0, 100) > vignette_alpha(0.0, 1.0, 100));
        assert_eq!(vignette_alpha(1.0, 1.0, 0), 0);
    }

    #[test]
    fn logo_import_keeps_previous_valid_image_on_failure_and_bounds_decode() {
        let dir = std::env::temp_dir().join(format!("visualizer-logo-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("logo.png");
        RgbaImage::from_pixel(2000, 100, Rgba([220, 100, 40, 128]))
            .save(&path)
            .unwrap();
        let mut state = VisualizerPreviewState::new(VisualizerPreviewSettings::default());
        state.set_logo(path.clone()).unwrap();
        assert!(state.logo.pixels.as_ref().unwrap().width() <= 1024);
        assert_eq!(state.logo.pixels.as_ref().unwrap().get_pixel(0, 0)[3], 128);
        assert!(state.set_logo(dir.join("missing.png")).is_err());
        assert_eq!(state.settings.logo.path.as_ref(), Some(&path));
        assert!(state.logo.pixels.is_some());
        state.clear_logo();
        assert!(state.settings.logo.path.is_none());
        assert!(!state.settings.logo.enabled);
        assert!(state.error.is_none());
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn cache_skips_unchanged_frames_and_recovers_from_a_missing_source() {
        let mut cache = CachedImage::default();
        let path =
            std::env::temp_dir().join(format!("visualizer-missing-{}.png", uuid::Uuid::new_v4()));
        assert!(cache.refresh(Some(&path), 50, 50).unwrap().is_err());
        assert!(cache.refresh(Some(&path), 50, 50).is_none());
        assert!(cache.refresh(None, 50, 50).unwrap().is_ok());
        assert!(cache.requested.is_none());
    }

    #[test]
    fn placements_fit_canvas_and_logo_keeps_aspect_ratio() {
        let canvas = Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(640.0, 360.0));
        let size = logo_size(canvas, egui::vec2(1000.0, 100.0), 70, 1.08);
        assert!((size.x / size.y - 10.0).abs() < 0.001);
        for position in [0, 50, u32::MAX] {
            let rect = positioned_rect(canvas, size, position, position);
            assert!(canvas.contains_rect(rect));
        }
        assert_eq!(bounded_beat(f32::NAN), 0.0);
        assert_eq!(bounded_beat(3.0), 1.0);
    }
}
