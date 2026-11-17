use eframe::egui::{self, Color32, CornerRadius, Rect, Sense, Shape, Stroke};
use loopkaraoke::util::open_path;

use super::{
    theme::{self, UiText},
    StudioApp, Tab,
};

impl StudioApp {
    pub(super) fn dashboard_tab(&mut self, ui: &mut egui::Ui) {
        let p = theme::Palette::of(ui.ctx());
        ui.spacing_mut().item_spacing.y = 8.0;
        ui.page_title("Dashboard");
        ui.muted_label(
            "Selamat datang! Siapkan render video full album Anda dalam beberapa langkah.",
        );
        ui.add_space(12.0);

        let output_dir = self
            .generate
            .output_dir
            .as_ref()
            .unwrap_or(&self.paths.outputs)
            .clone();
        let mut open_generate = false;
        let mut open_output = false;
        egui::Frame::new()
            .fill(p.surface)
            .stroke(Stroke::new(1.0_f32, p.border))
            .corner_radius(CornerRadius::same(20))
            .inner_margin(egui::Margin::same(28))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let show_art = ui.available_width() >= 540.0;
                let text_width = if show_art { ui.available_width() - 132.0 } else { ui.available_width() };
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(text_width);
                        ui.add_space(2.0);
                        ui.label(egui::RichText::new("Buat video full album").size(21.0).strong().color(p.text_strong));
                        ui.add_space(3.0);
                        ui.muted_label("1 video loop + daftar lagu → 1 file MP4 lengkap dengan playlist.txt dan Timestamp.txt. Hasilnya siap diputar di TV, ponsel, atau diunggah.");
                        ui.add_space(12.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;
                            open_generate = ui.add(
                                theme::StyledButton::primary(egui::RichText::new("⊕  Mulai Render").size(14.0).strong())
                                    .min_size(egui::vec2(164.0, 44.0))
                                    .corner_radius(CornerRadius::same(11)),
                            ).clicked();
                            open_output = ui.add(
                                theme::StyledButton::secondary(egui::RichText::new("↑  Buka Folder Output").size(14.0).strong())
                                    .min_size(egui::vec2(196.0, 44.0))
                                    .corner_radius(CornerRadius::same(11)),
                            ).clicked();
                        });
                    });
                    if show_art {
                        ui.add_space(28.0);
                        ui.add(
                            egui::Image::from_bytes(
                                "bytes://dashboard-app-icon.png",
                                include_bytes!("../../assets/logo/app-icon-color.png"),
                            )
                            .fit_to_exact_size(egui::vec2(96.0, 96.0))
                            .corner_radius(CornerRadius::same(22)),
                        );
                    }
                });
            });
        ui.add_space(8.0);

        let visual = self
            .generate
            .video
            .as_ref()
            .map(|video| video.filename.clone())
            .unwrap_or_else(|| "Belum dipilih".into());
        let values = [
            visual,
            format!("{} lagu", self.generate.tracks.len()),
            output_dir.display().to_string(),
        ];
        let labels = ["VIDEO VISUAL", "PLAYLIST LAGU", "FOLDER OUTPUT"];
        let wide = ui.available_width() >= 600.0;
        if wide {
            ui.columns(3, |columns| {
                for (index, column) in columns.iter_mut().enumerate() {
                    status_card(column, index, labels[index], &values[index]);
                }
            });
        } else {
            for index in 0..3 {
                status_card(ui, index, labels[index], &values[index]);
            }
        }
        ui.add_space(8.0);
        let steps = [
            (
                "Pilih video loop",
                "Video MP4 yang diputar berulang sebagai latar album.",
            ),
            (
                "Tambahkan lagu",
                "Dari folder atau file sekaligus, lalu susun urutannya.",
            ),
            (
                "Render",
                "FFmpeg menggabungkan semuanya menjadi 1 MP4 utuh.",
            ),
        ];
        if wide {
            ui.columns(3, |columns| {
                for (index, column) in columns.iter_mut().enumerate() {
                    step_card(column, index + 1, steps[index].0, steps[index].1);
                }
            });
        } else {
            for (index, (title, detail)) in steps.iter().enumerate() {
                step_card(ui, index + 1, title, detail);
            }
        }
        if open_generate {
            self.tab = Tab::Generate;
        }
        if open_output {
            if let Err(error) = std::fs::create_dir_all(&output_dir) {
                self.error(format!("Gagal membuat folder output: {error}"));
            } else if let Err(error) = open_path(&output_dir) {
                self.error(format!("Gagal membuka folder output: {error}"));
            }
        }
    }
}

fn card(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui)) {
    let p = theme::Palette::of(ui.ctx());
    egui::Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0_f32, p.border))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            contents(ui);
        });
}

fn status_card(ui: &mut egui::Ui, index: usize, label: &str, value: &str) {
    let p = theme::Palette::of(ui.ctx());
    card(ui, |ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(38.0, 38.0), Sense::hover());
        ui.painter()
            .rect_filled(rect, CornerRadius::same(11), p.accent.gamma_multiply(0.16));
        status_icon(ui, rect.center(), index, p.accent);
        ui.add_space(4.0);
        ui.label(egui::RichText::new(label).size(11.0).color(p.text_muted));
        ui.add(
            egui::Label::new(egui::RichText::new(value).strong().color(p.text_strong)).truncate(),
        )
        .on_hover_text(value);
    });
}

fn step_card(ui: &mut egui::Ui, number: usize, title: &str, detail: &str) {
    let p = theme::Palette::of(ui.ctx());
    card(ui, |ui| {
        ui.set_min_height(118.0);
        ui.spacing_mut().item_spacing.y = 8.0;
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(34.0, 34.0), Sense::hover());
            ui.painter()
                .rect_filled(rect, CornerRadius::same(11), p.accent.gamma_multiply(0.14));
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{number:02}"),
                egui::FontId::new(14.0, egui::FontFamily::Name(theme::SEMIBOLD.into())),
                p.accent,
            );
            ui.add_space(3.0);
            ui.label(
                egui::RichText::new(format!("LANGKAH {number}"))
                    .size(10.0)
                    .strong()
                    .color(p.text_muted),
            );
        });
        ui.label(
            egui::RichText::new(title)
                .size(15.0)
                .strong()
                .color(p.text_strong),
        );
        ui.add(egui::Label::new(egui::RichText::new(detail).size(12.0).color(p.text_muted)).wrap());
    });
}

fn status_icon(ui: &egui::Ui, center: egui::Pos2, index: usize, color: Color32) {
    let painter = ui.painter();
    let stroke = Stroke::new(1.3_f32, color);
    let point = |x, y| center + egui::vec2(x, y);
    match index {
        0 => {
            painter.rect_stroke(
                Rect::from_center_size(center, egui::vec2(12.0, 10.0)),
                CornerRadius::same(2),
                stroke,
                egui::StrokeKind::Inside,
            );
            painter.add(Shape::line(
                vec![
                    point(6.0, -2.0),
                    point(10.0, -4.0),
                    point(10.0, 4.0),
                    point(6.0, 2.0),
                ],
                stroke,
            ));
        }
        1 => {
            painter.add(Shape::line(
                vec![
                    point(-3.0, 5.0),
                    point(-3.0, -5.0),
                    point(6.0, -7.0),
                    point(6.0, 3.0),
                ],
                stroke,
            ));
            painter.circle_stroke(point(-5.0, 6.0), 2.5, stroke);
            painter.circle_stroke(point(4.0, 4.0), 2.5, stroke);
        }
        _ => {
            painter.add(Shape::closed_line(
                vec![
                    point(-7.0, -5.0),
                    point(-1.0, -5.0),
                    point(1.0, -3.0),
                    point(7.0, -3.0),
                    point(7.0, 6.0),
                    point(-7.0, 6.0),
                ],
                stroke,
            ));
        }
    }
}
