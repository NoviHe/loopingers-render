use std::{collections::VecDeque, sync::Arc};

use eframe::egui::{self, Rect};
use loopkaraoke::{
    config::{SpectrumKind, SpectrumOverlaySettings},
    spectrum::{SpectrumData, BAND_COUNT},
};

use super::{
    generate_tab::{card_header, generate_card, GenerateIcon},
    theme::{self, UiText},
    Event, StudioApp,
};

pub(super) struct SpectrumOverlayState {
    pub settings: SpectrumOverlaySettings,
    cache: VecDeque<(u32, Arc<SpectrumData>)>,
    loading: Option<u32>,
    failed: Option<u32>,
    request: u64,
    cancel: tokio::sync::watch::Sender<u64>,
    error: Option<String>,
    displayed: [f32; BAND_COUNT],
    displayed_track: Option<u32>,
    displayed_time: Option<f64>,
    liquid: super::liquid_ring_motion::LiquidRingMotion,
    liquid_active: bool,
    gpu: Option<super::liquid_ring_gpu::LiquidRingRenderer>,
    gpu_attempted: bool,
    gpu_error: Option<String>,
    beat: f32,
    beat_track: Option<u32>,
}

impl SpectrumOverlayState {
    pub fn new(settings: SpectrumOverlaySettings) -> Self {
        Self {
            settings,
            cache: VecDeque::new(),
            loading: None,
            failed: None,
            request: 0,
            cancel: tokio::sync::watch::channel(0).0,
            error: None,
            displayed: [0.0; BAND_COUNT],
            displayed_track: None,
            displayed_time: None,
            liquid: Default::default(),
            liquid_active: false,
            gpu: None,
            gpu_attempted: false,
            gpu_error: None,
            beat: 0.0,
            beat_track: None,
        }
    }

    pub fn cancel_analysis(&mut self) {
        if self.loading.take().is_some() {
            self.request = self.request.wrapping_add(1);
            self.cancel.send_replace(self.request);
        }
    }

    pub fn ensure_gpu(&mut self, gl: Option<&Arc<eframe::glow::Context>>) {
        if self.gpu_attempted
            || self.settings.kind != SpectrumKind::RingGlow
            || !self.settings.enabled
        {
            return;
        }
        self.gpu_attempted = true;
        match gl {
            Some(gl) => match super::liquid_ring_gpu::LiquidRingRenderer::new(gl.clone()) {
                Ok(renderer) => self.gpu = Some(renderer),
                Err(error) => self.gpu_error = Some(error),
            },
            None => self.gpu_error = Some("Konteks OpenGL tidak tersedia.".to_owned()),
        }
    }

    pub fn destroy_gpu(&mut self, gl: &eframe::glow::Context) {
        if let Some(renderer) = self.gpu.take() {
            renderer.destroy(gl);
        }
    }

    pub fn preview_beat(
        &mut self,
        ui: &egui::Ui,
        track_id: Option<u32>,
        time: f64,
        playing: bool,
    ) -> f32 {
        if self.beat_track != track_id {
            self.beat = 0.0;
            self.beat_track = track_id;
        }
        let target = if playing {
            self.cache
                .iter()
                .find(|(id, _)| Some(*id) == track_id)
                .map_or(0.0, |(_, data)| {
                    let levels = data.levels_at(time);
                    (levels[..12].iter().sum::<f32>() / 12.0)
                        .clamp(0.0, 1.0)
                        .powf(2.0)
                })
        } else {
            0.0
        };
        let dt = ui.input(|input| input.stable_dt).clamp(0.001, 0.1);
        let tau = if target > self.beat { 0.025 } else { 0.16 };
        self.beat += (target - self.beat) * (1.0 - (-dt / tau).exp());
        if (target - self.beat).abs() < 0.0001 {
            self.beat = target;
        }
        if (target - self.beat).abs() > 0.0001 {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(16));
        }
        self.beat
    }

    pub fn draw(&mut self, ui: &egui::Ui, canvas: Rect, track_id: Option<u32>, time: f64) {
        if !self.settings.enabled {
            return;
        }
        let data = self.cache.iter().find(|(id, _)| Some(*id) == track_id);
        let target = data.map_or([0.0; BAND_COUNT], |(_, data)| data.levels_at(time));
        let reset = self.displayed_track != track_id
            || self
                .displayed_time
                .is_none_or(|previous| (time - previous).abs() > 0.25);
        let dt = ui.input(|input| input.stable_dt).clamp(0.001, 0.1);
        let liquid_active = self.settings.kind == SpectrumKind::RingGlow;
        let liquid_moving = liquid_active
            && self.liquid.advance(
                &target,
                self.settings.sensitivity_pct,
                self.settings.smoothing_pct,
                dt,
                reset || !self.liquid_active,
            );
        self.liquid_active = liquid_active;
        if smooth_levels(
            &mut self.displayed,
            &target,
            self.settings.smoothing_pct,
            dt,
            reset,
        ) || liquid_moving
        {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(16));
        }
        self.displayed_track = track_id;
        self.displayed_time = Some(time);
        let area = spectrum_area(canvas, &self.settings);
        if liquid_active {
            if let Some(renderer) = &self.gpu {
                renderer.paint(
                    &ui.painter_at(canvas),
                    canvas,
                    area,
                    &self.liquid.profile,
                    &self.settings,
                    time,
                );
            } else {
                super::spectrum_styles::paint_liquid_fallback(
                    &ui.painter_at(canvas),
                    area,
                    &self.liquid.profile,
                    &self.settings,
                    time,
                );
            }
            return;
        }
        super::spectrum_styles::paint_spectrum(
            &ui.painter_at(canvas),
            area,
            &self.displayed,
            &self.settings,
            time,
        );
    }
}

fn smooth_levels(
    current: &mut [f32; BAND_COUNT],
    target: &[f32; BAND_COUNT],
    smoothing: u32,
    dt: f32,
    reset: bool,
) -> bool {
    let amount = smoothing.min(100) as f32 / 100.0;
    let mut moving = false;
    for (current, target) in current.iter_mut().zip(target) {
        let target = if target.is_finite() {
            target.clamp(0.0, 1.0)
        } else {
            0.0
        };
        if reset || smoothing == 0 || !current.is_finite() {
            *current = target;
            continue;
        }
        let tau = if target > *current {
            0.01 + amount * 0.07
        } else {
            0.04 + amount * 0.4
        };
        let weight = 1.0 - (-dt.clamp(0.0, 0.1) / tau).exp();
        *current += (target - *current) * weight;
        if (*current - target).abs() < 0.0005 {
            *current = target;
        }
        moving |= (*current - target).abs() >= 0.0005;
    }
    moving
}

fn spectrum_area(canvas: Rect, settings: &SpectrumOverlaySettings) -> Rect {
    let size = egui::vec2(
        canvas.width() * settings.width_pct.clamp(20, 100) as f32 / 100.0,
        canvas.height() * settings.height_pct.clamp(5, 60) as f32 / 100.0,
    );
    let center = egui::pos2(
        (canvas.left() + canvas.width() * settings.x_pct.min(100) as f32 / 100.0)
            .clamp(canvas.left() + size.x / 2.0, canvas.right() - size.x / 2.0),
        (canvas.top() + canvas.height() * settings.y_pct.min(100) as f32 / 100.0)
            .clamp(canvas.top() + size.y / 2.0, canvas.bottom() - size.y / 2.0),
    );
    Rect::from_center_size(center, size)
}

impl StudioApp {
    pub(super) fn sync_spectrum_analysis(&mut self) {
        self.spectrum_overlay
            .cache
            .retain(|(id, _)| self.generate.tracks.iter().any(|track| track.id == *id));
        let needs_audio = self.spectrum_overlay.settings.enabled
            || (self.visualizer_preview.settings.background.enabled
                && self.visualizer_preview.settings.background.beat_pulse)
            || (self.visualizer_preview.settings.logo.enabled
                && self.visualizer_preview.settings.logo.beat_pulse);
        if !needs_audio {
            self.spectrum_overlay.cancel_analysis();
            return;
        }
        let Some(track) = self.generate.tracks.get(self.visualizer_selected_track) else {
            self.spectrum_overlay.cancel_analysis();
            self.spectrum_overlay.error = None;
            return;
        };
        let track_id = track.id;
        if self.spectrum_overlay.failed != Some(track_id) {
            self.spectrum_overlay.error = None;
            self.spectrum_overlay.failed = None;
        }
        if self.spectrum_overlay.loading == Some(track_id) {
            return;
        }
        self.spectrum_overlay.cancel_analysis();
        if self
            .spectrum_overlay
            .cache
            .iter()
            .any(|(id, _)| *id == track_id)
            || self.spectrum_overlay.failed == Some(track_id)
        {
            return;
        }
        let source = track.path.clone();
        self.spectrum_overlay.request = self.spectrum_overlay.request.wrapping_add(1);
        let request_id = self.spectrum_overlay.request;
        self.spectrum_overlay.cancel.send_replace(request_id);
        let mut cancellation = self.spectrum_overlay.cancel.subscribe();
        self.spectrum_overlay.loading = Some(track_id);
        self.spectrum_overlay.error = None;
        let tools = self.tools.clone();
        self.spawn(async move {
            let result = tokio::select! {
                result = loopkaraoke::spectrum::analyze(&tools, &source) => result,
                _ = cancellation.changed() => Err(loopkaraoke::error::Error::internal("Analisis spektrum dibatalkan.")),
            };
            Event::SpectrumAnalyzed { track_id, request_id, result }
        });
    }

    pub(super) fn accept_spectrum_analysis(
        &mut self,
        track_id: u32,
        request_id: u64,
        result: loopkaraoke::error::Result<SpectrumData>,
    ) {
        if self.spectrum_overlay.request != request_id
            || self.spectrum_overlay.loading != Some(track_id)
        {
            return;
        }
        self.spectrum_overlay.loading = None;
        match result {
            Ok(data) => {
                self.spectrum_overlay.failed = None;
                self.spectrum_overlay
                    .cache
                    .push_back((track_id, Arc::new(data)));
                while self.spectrum_overlay.cache.len() > 3 {
                    self.spectrum_overlay.cache.pop_front();
                }
            }
            Err(error) => {
                self.spectrum_overlay.failed = Some(track_id);
                self.spectrum_overlay.error = Some(error.to_string());
            }
        }
    }

    pub(super) fn spectrum_settings_card(&mut self, ui: &mut egui::Ui) {
        let previous = self.spectrum_overlay.settings.clone();
        let previous_preview = self.visualizer_preview.settings.clone();
        generate_card(ui, |ui| {
            ui.horizontal(|ui| {
                card_header(
                    ui,
                    GenerateIcon::Sliders,
                    "Setting Visualizer",
                    "Spektrum, teks, background, dan logo untuk preview.",
                );
                let visibility = match self.visualizer_preview.active_tab {
                    0 => Some((
                        &mut self.spectrum_overlay.settings.enabled,
                        "Tampilkan visualizer",
                    )),
                    1 => Some((
                        &mut self.visualizer_preview.settings.text.enabled,
                        "Tampilkan teks",
                    )),
                    2 => Some((
                        &mut self.visualizer_preview.settings.background.enabled,
                        "Tampilkan background",
                    )),
                    3 => Some((
                        &mut self.visualizer_preview.settings.logo.enabled,
                        "Tampilkan logo",
                    )),
                    _ => None,
                };
                if let Some((enabled, tooltip)) = visibility {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.checkbox(enabled, "Tampilkan").on_hover_text(tooltip);
                    });
                }
            });
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                for (index, label) in ["Visualizer", "Teks", "Background", "Logo", "VFX"]
                    .iter()
                    .enumerate()
                {
                    ui.selectable_value(&mut self.visualizer_preview.active_tab, index, *label);
                }
            });
            ui.separator();
            ui.add_space(4.0);
            match self.visualizer_preview.active_tab {
                0 => self.spectrum_controls(ui),
                1 => self.visualizer_text_controls(ui),
                2 => self.visualizer_background_controls(ui),
                3 => self.visualizer_logo_controls(ui),
                _ => {
                    ui.add_space(12.0);
                }
            }
        });
        if previous != self.spectrum_overlay.settings
            || previous_preview != self.visualizer_preview.settings
        {
            self.save_settings();
        }
        self.sync_spectrum_analysis();
    }

    fn spectrum_controls(&mut self, ui: &mut egui::Ui) {
        let mut retry = false;
        ui.muted_label("Jenis Spektrum");
        let mut kind = self.spectrum_overlay.settings.kind;
        egui::ComboBox::from_id_salt("visualizer_spectrum_type")
            .width(ui.available_width())
            .selected_text(kind.label())
            .show_ui(ui, |ui| {
                for option in SpectrumKind::ALL {
                    ui.selectable_value(&mut kind, option, option.label());
                }
            });
        if kind != self.spectrum_overlay.settings.kind {
            self.spectrum_overlay.settings.select_kind(kind);
        }
        ui.add_space(6.0);
        let settings = &mut self.spectrum_overlay.settings;
        ui.horizontal(|ui| {
            ui.label("Warna spektrum");
            ui.color_edit_button_srgb(&mut settings.color);
        });
        if settings.kind == SpectrumKind::HaloGeometry {
            ui.horizontal(|ui| {
                ui.label("Warna gradasi");
                ui.color_edit_button_srgb(&mut settings.secondary_color);
            });
        }
        ui.add_enabled_ui(settings.enabled, |ui| {
            ui.columns(2, |columns| {
                if settings.kind == SpectrumKind::Bars {
                    theme::control_slider_fill(
                        &mut columns[0],
                        "Lebar",
                        &mut settings.width_pct,
                        20..=100,
                        "%",
                    );
                    theme::control_slider_fill(
                        &mut columns[1],
                        "Tinggi",
                        &mut settings.height_pct,
                        5..=60,
                        "%",
                    );
                } else {
                    theme::control_slider_fill(
                        &mut columns[0],
                        "Ukuran Cincin",
                        &mut settings.height_pct,
                        5..=60,
                        "%",
                    );
                }
            });
            ui.columns(2, |columns| {
                theme::control_slider_fill(
                    &mut columns[0],
                    "Posisi X",
                    &mut settings.x_pct,
                    0..=100,
                    "%",
                );
                theme::control_slider_fill(
                    &mut columns[1],
                    "Posisi Y",
                    &mut settings.y_pct,
                    0..=100,
                    "%",
                );
            });
            ui.columns(2, |columns| {
                theme::control_slider_fill(
                    &mut columns[0],
                    "Sensitivitas",
                    &mut settings.sensitivity_pct,
                    25..=300,
                    "%",
                );
                theme::control_slider_fill(
                    &mut columns[1],
                    "Kehalusan",
                    &mut settings.smoothing_pct,
                    0..=100,
                    "%",
                );
            });
        });
        ui.add_enabled_ui(settings.enabled, |ui| {
            ui.columns(2, |columns| {
                theme::control_slider_fill(
                    &mut columns[0],
                    "Opasitas",
                    &mut settings.opacity_pct,
                    0..=100,
                    "%",
                );
                if settings.kind != SpectrumKind::Bars {
                    theme::control_slider_fill(
                        &mut columns[1],
                        "Cahaya (Glow)",
                        &mut settings.glow_pct,
                        0..=100,
                        "%",
                    );
                }
            });
            if settings.kind != SpectrumKind::Bars {
                ui.columns(2, |columns| {
                    theme::control_slider_fill(
                        &mut columns[0],
                        "Ketebalan",
                        &mut settings.thickness_px,
                        1..=8,
                        " px",
                    );
                    theme::control_slider_fill(
                        &mut columns[1],
                        "Kecepatan Rotasi",
                        &mut settings.rotation_speed,
                        0..=100,
                        "%",
                    );
                });
            }
        });
        if self.spectrum_overlay.loading.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.muted_label("Menganalisis audio untuk spektrum...");
            });
        }
        if let Some(error) = &self.spectrum_overlay.error {
            ui.colored_label(theme::Palette::of(ui.ctx()).danger, error);
            retry = ui
                .add(theme::StyledButton::secondary("Coba Analisis Lagi"))
                .clicked();
        }
        if self.spectrum_overlay.settings.kind == SpectrumKind::RingGlow {
            if let Some(error) = &self.spectrum_overlay.gpu_error {
                ui.muted_label(format!("Ring Glow memakai renderer cadangan: {error}"));
            }
        }
        ui.muted_label("Putar lagu melalui Playlist Album. Spektrum mengikuti frekuensi audio dan posisi pemutaran, termasuk saat seek. Overlay baru ini berlaku pada preview.");
        if retry {
            self.spectrum_overlay.failed = None;
            self.spectrum_overlay.error = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoothing_is_time_based_and_reacts_faster_than_it_decays() {
        let target = [1.0; BAND_COUNT];
        let mut at_30fps = [0.0; BAND_COUNT];
        let mut at_60fps = [0.0; BAND_COUNT];
        for _ in 0..30 {
            smooth_levels(&mut at_30fps, &target, 65, 1.0 / 30.0, false);
        }
        for _ in 0..60 {
            smooth_levels(&mut at_60fps, &target, 65, 1.0 / 60.0, false);
        }
        assert!((at_30fps[0] - at_60fps[0]).abs() < 0.001);
        let mut attack = [0.0; BAND_COUNT];
        let mut release = [1.0; BAND_COUNT];
        smooth_levels(&mut attack, &target, 65, 1.0 / 60.0, false);
        smooth_levels(&mut release, &[0.0; BAND_COUNT], 65, 1.0 / 60.0, false);
        assert!(attack[0] > 1.0 - release[0]);
        smooth_levels(&mut release, &[0.25; BAND_COUNT], 100, 0.016, true);
        assert_eq!(release, [0.25; BAND_COUNT], "Seek resets the old envelope");
    }

    #[test]
    fn spectrum_remains_inside_canvas_for_edge_positions_and_invalid_saved_sizes() {
        let canvas = Rect::from_min_size(egui::pos2(20.0, 30.0), egui::vec2(640.0, 360.0));
        for (x, y) in [(0, 0), (100, 100), (50, 82)] {
            let settings = SpectrumOverlaySettings {
                x_pct: x,
                y_pct: y,
                width_pct: u32::MAX,
                height_pct: u32::MAX,
                ..SpectrumOverlaySettings::default()
            };
            let area = spectrum_area(canvas, &settings);
            assert!(canvas.contains_rect(area));
        }
    }
}
