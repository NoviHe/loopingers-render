use eframe::egui::{self, Stroke, Vec2};

use super::{
    generate_tab::{card_header, generate_card, outline_icon_button, GenerateIcon},
    help::visualizer as help,
    theme::{self, UiText},
    StudioApp, Tab,
};

impl StudioApp {
    pub(super) fn visualizer_tab(&mut self, ui: &mut egui::Ui) {
        let can_pin = ui.input(|input| input.viewport().maximized.unwrap_or(false));
        let pinned = can_pin && self.visualizer_preview_pinned;
        self.update_album_preview(ui);

        ui.spacing_mut().item_spacing.y = 8.0;
        theme::page_header(
            ui,
            "Visualizer",
            "Atur spektrum audio, teks, logo, dan efek latar untuk preview Visualizer.",
        )
        .on_hover_text(help::HEADING);
        ui.add_space(4.0);

        generate_card(ui, |ui| {
            super::generate_tab::pinnable_card_header(
                ui,
                GenerateIcon::Sliders,
                "Tampilan Visualizer",
                "Sinkron dengan Generate dan Batch Render. Preview Visualizer.",
                can_pin.then_some(&mut self.visualizer_preview_pinned),
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if outline_icon_button(ui, GenerateIcon::Music, "Buka Playlist").clicked() {
                    self.tab = Tab::Playlist;
                }
                if outline_icon_button(ui, GenerateIcon::Sliders, "Karaoke GFX").clicked() {
                    self.tab = Tab::Karaoke;
                }
                if outline_icon_button(ui, GenerateIcon::Play, "Buka Generate").clicked() {
                    self.tab = Tab::Generate;
                }
                ui.add_enabled_ui(
                    self.generate.video.is_some() && !self.thumbnail.extracting_frame,
                    |ui| {
                        if outline_icon_button(ui, GenerateIcon::Reset, "Ambil Frame Video")
                            .clicked()
                        {
                            self.extract_thumbnail_frame();
                        }
                    },
                );
            });
            ui.muted_label("Atur lagu melalui Playlist dan gaya lirik melalui Karaoke GFX. Ambil frame video Generate sebagai latar contoh untuk memeriksa tampilan efek Visualizer.");
        });
        ui.add_space(6.0);

        let maximized = ui
            .ctx()
            .input(|input| input.viewport().maximized.unwrap_or(false));
        if maximized || ui.available_width() >= 1280.0 {
            ui.columns(2, |columns| {
                super::generate_tab::pinned_content(
                    &mut columns[0],
                    pinned,
                    "visualizer_scroll",
                    |ui| {
                        self.album_preview_playlist_card(ui);
                        self.spectrum_settings_card(ui);
                    },
                );
                self.render_visualizer_preview(&mut columns[1]);
            });
        } else {
            self.album_preview_playlist_card(ui);
            self.render_visualizer_preview(ui);
            self.spectrum_settings_card(ui);
        }
    }

    fn render_visualizer_preview(&mut self, ui: &mut egui::Ui) {
        generate_card(ui, |ui| {
            card_header(ui, GenerateIcon::Play, "Preview Visualizer", "");
            ui.add_space(6.0);
            self.draw_visualizer_preview_base(ui);
            ui.add_space(8.0);
            ui.muted_label("Preview real-time spektrum, teks, background, dan logo. Atur melalui tab di Setting Visualizer; gunakan Ambil Frame Video untuk mengganti latar.");
        });
    }

    fn draw_visualizer_preview_base(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width().max(1.0);
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(width, width * 9.0 / 16.0), egui::Sense::hover());
        let path = self.thumbnail_preview_path();
        let beat = self.spectrum_overlay.preview_beat(
            ui,
            self.visualizer_active_track_id,
            self.visualizer_time,
            self.visualizer_playing,
        );
        self.visualizer_preview
            .draw_background(ui, rect, path.as_deref(), beat);
        let p = theme::Palette::of(ui.ctx());
        self.spectrum_overlay.draw(
            ui,
            rect,
            self.visualizer_active_track_id,
            self.visualizer_time,
        );
        let title = self
            .generate
            .tracks
            .get(self.visualizer_selected_track)
            .map(|track| track.title.as_str());
        self.visualizer_preview
            .draw_foreground(ui, rect, title, beat);
        ui.painter_at(rect).rect_stroke(
            rect,
            4,
            Stroke::new(1.0_f32, p.border),
            egui::StrokeKind::Inside,
        );
    }
}
