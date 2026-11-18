//! Controls for the independent layers in Preview Visualizer.
use eframe::egui;
use loopkaraoke::config::VisualizerTextFont;

use super::{
    theme::{self, UiText},
    StudioApp,
};

fn position_controls(ui: &mut egui::Ui, x: &mut u32, y: &mut u32) {
    ui.columns(2, |columns| {
        theme::control_slider_fill(&mut columns[0], "Posisi X", x, 0..=100, "%");
        theme::control_slider_fill(&mut columns[1], "Posisi Y", y, 0..=100, "%");
    });
}

impl StudioApp {
    pub(super) fn visualizer_text_controls(&mut self, ui: &mut egui::Ui) {
        let settings = &mut self.visualizer_preview.settings.text;
        ui.add_enabled_ui(settings.enabled, |ui| {
            ui.checkbox(
                &mut settings.use_song_title,
                "Gunakan judul lagu yang dipilih",
            );
            ui.add_enabled_ui(!settings.use_song_title, |ui| {
                ui.muted_label("Teks");
                theme::text_field(
                    ui,
                    &mut settings.content,
                    "Masukkan teks Visualizer",
                    theme::InputWidth::Fill,
                );
            });
            ui.muted_label("Font");
            egui::ComboBox::from_id_salt("visualizer_preview_text_font")
                .width(ui.available_width())
                .selected_text(settings.font.label())
                .show_ui(ui, |ui| {
                    for font in VisualizerTextFont::ALL {
                        ui.selectable_value(&mut settings.font, font, font.label());
                    }
                });
            ui.horizontal(|ui| {
                ui.label("Warna teks");
                ui.color_edit_button_srgb(&mut settings.color);
            });
            ui.columns(2, |columns| {
                theme::control_slider_fill(
                    &mut columns[0],
                    "Ukuran Font",
                    &mut settings.size_px,
                    16..=160,
                    " px",
                );
                theme::control_slider_fill(
                    &mut columns[1],
                    "Opasitas",
                    &mut settings.opacity_pct,
                    0..=100,
                    "%",
                );
            });
            position_controls(ui, &mut settings.x_pct, &mut settings.y_pct);
        });
        ui.muted_label(
            "Teks ditampilkan pada Preview Visualizer. Ukuran font mengikuti skala video 1080p.",
        );
    }

    pub(super) fn visualizer_background_controls(&mut self, ui: &mut egui::Ui) {
        let settings = &mut self.visualizer_preview.settings.background;
        ui.columns(2, |columns| {
            theme::control_slider_fill(
                &mut columns[0],
                "Blur",
                &mut settings.blur_px,
                0..=30,
                " px",
            );
            theme::control_slider_fill(
                &mut columns[1],
                "Vignette",
                &mut settings.vignette_pct,
                0..=100,
                "%",
            );
        });
        ui.columns(2, |columns| {
            theme::control_slider_fill(
                &mut columns[0],
                "Gelap (Dim)",
                &mut settings.dim_pct,
                0..=100,
                "%",
            );
            theme::control_slider_fill(
                &mut columns[1],
                "Tint",
                &mut settings.tint_pct,
                0..=100,
                "%",
            );
        });
        ui.horizontal(|ui| {
            ui.label("Warna tint");
            ui.color_edit_button_srgb(&mut settings.tint_color);
        });
        ui.checkbox(
            &mut settings.beat_pulse,
            "Getaran Ketukan Musik (Beat Pulse)",
        );
        ui.add_enabled_ui(settings.beat_pulse, |ui| {
            theme::control_slider_fill(
                ui,
                "Kekuatan Ketukan",
                &mut settings.beat_strength_pct,
                0..=20,
                "%",
            );
        });
        ui.muted_label("Efek hanya mengubah latar preview. Beat Pulse mengikuti bass lagu yang diputar melalui Playlist Album.");
    }

    pub(super) fn visualizer_logo_controls(&mut self, ui: &mut egui::Ui) {
        let mut choose = false;
        let mut remove = false;
        ui.horizontal(|ui| {
            choose = ui
                .add(theme::StyledButton::secondary("Pilih Logo..."))
                .clicked();
            remove = ui
                .add_enabled(
                    self.visualizer_preview.settings.logo.path.is_some(),
                    theme::StyledButton::secondary("Hapus Logo"),
                )
                .clicked();
        });
        if choose {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Gambar PNG / JPEG", &["png", "jpg", "jpeg"])
                .pick_file()
            {
                if let Err(error) = self.visualizer_preview.set_logo(path) {
                    self.visualizer_preview.error = Some(error);
                }
            }
        }
        if remove {
            self.visualizer_preview.clear_logo();
        }
        if let Some(path) = &self.visualizer_preview.settings.logo.path {
            ui.muted_label(
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
            )
            .on_hover_text(path.display().to_string());
        }
        let settings = &mut self.visualizer_preview.settings.logo;
        ui.add_enabled_ui(settings.enabled, |ui| {
            ui.columns(2, |columns| {
                theme::control_slider_fill(
                    &mut columns[0],
                    "Ukuran Logo",
                    &mut settings.size_pct,
                    5..=70,
                    "%",
                );
                theme::control_slider_fill(
                    &mut columns[1],
                    "Opasitas",
                    &mut settings.opacity_pct,
                    0..=100,
                    "%",
                );
            });
            position_controls(ui, &mut settings.x_pct, &mut settings.y_pct);
            ui.checkbox(&mut settings.beat_pulse, "Logo mengikuti ketukan musik");
        });
        if let Some(error) = &self.visualizer_preview.error {
            ui.colored_label(theme::Palette::of(ui.ctx()).danger, error);
        }
        ui.muted_label("Gunakan PNG transparan untuk logo tanpa latar. Logo ditampilkan pada Preview Visualizer.");
    }
}
