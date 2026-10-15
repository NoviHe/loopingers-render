use std::path::PathBuf;

use eframe::egui;
use loopkaraoke::{
    thumbnail::{self, ThumbnailSpec},
    util::{open_path, random_id},
};

use super::{help::thumbnail as help, hex, theme, Event, StudioApp};

const COLUMN_MODES: [(&str, &str); 3] = [("auto", "Otomatis"), ("1", "1 kolom"), ("2", "2 kolom")];

pub struct ThumbnailState {
    title: String,
    subtitle: String,
    timestamp: f64,
    column_mode: usize,
    mirrored_numbers: bool,
    title_size: u32,
    track_size: u32,
    vignette: f32,
    title_color: [u8; 3],
    track_color: [u8; 3],
    subtitle_color: [u8; 3],
    pub busy: bool,
    pub result: Option<PathBuf>,
}

impl Default for ThumbnailState {
    fn default() -> Self {
        Self {
            title: "Powerful Worship".to_owned(),
            subtitle: "Hillsong Worship".to_owned(),
            timestamp: 0.0,
            column_mode: 0,
            mirrored_numbers: true,
            title_size: 120,
            track_size: 36,
            vignette: 0.55,
            title_color: [255, 255, 255],
            track_color: [255, 255, 255],
            subtitle_color: [255, 215, 0],
            busy: false,
            result: None,
        }
    }
}

impl StudioApp {
    pub(super) fn thumbnail_tab(&mut self, ui: &mut egui::Ui) {
        theme::heading_with_info(ui, "Thumbnail YouTube", help::HEADING);
        ui.label("Dibuat dari frame video (atau gambar) background dan judul lagu di playlist.");
        ui.add_space(4.0);
        ui.columns(2, |columns| {
            self.thumbnail_fields(&mut columns[0]);
            self.thumbnail_output(&mut columns[1]);
        });
    }

    fn thumbnail_fields(&mut self, ui: &mut egui::Ui) {
        let state = &mut self.thumbnail;
        egui::Grid::new("thumbnail_fields")
            .num_columns(2)
            .show(ui, |ui| {
                theme::label_with_info(ui, "Judul utama", help::TITLE);
                ui.text_edit_singleline(&mut state.title);
                ui.end_row();
                theme::label_with_info(ui, "Sub judul / tanda tangan", help::SUBTITLE);
                ui.text_edit_singleline(&mut state.subtitle);
                ui.end_row();
                theme::label_with_info(ui, "Ambil frame pada detik", help::FRAME);
                ui.add(
                    egui::DragValue::new(&mut state.timestamp)
                        .range(0.0..=36000.0)
                        .speed(0.5),
                );
                ui.end_row();
                theme::label_with_info(ui, "Kolom daftar lagu", help::COLUMNS);
                egui::ComboBox::from_id_salt("thumbnail_columns")
                    .selected_text(COLUMN_MODES[state.column_mode].1)
                    .show_ui(ui, |ui| {
                        for (index, (_, label)) in COLUMN_MODES.iter().enumerate() {
                            ui.selectable_value(&mut state.column_mode, index, *label);
                        }
                    });
                ui.end_row();
                theme::label_with_info(ui, "Penomoran kolom kanan", help::NUMBERING);
                ui.horizontal(|ui| {
                    ui.radio_value(&mut state.mirrored_numbers, true, "Cermin (Judul .01)");
                    ui.radio_value(&mut state.mirrored_numbers, false, "Berurutan (11. Judul)");
                });
                ui.end_row();
                theme::label_with_info(ui, "Ukuran judul", help::TITLE_SIZE);
                ui.add(egui::Slider::new(&mut state.title_size, 40..=240));
                ui.end_row();
                theme::label_with_info(ui, "Ukuran daftar lagu", help::TRACK_SIZE);
                ui.add(egui::Slider::new(&mut state.track_size, 16..=80));
                ui.end_row();
                theme::label_with_info(ui, "Gelap di tepi", help::VIGNETTE);
                ui.add(egui::Slider::new(&mut state.vignette, 0.0..=1.0));
                ui.end_row();
                theme::label_with_info(ui, "Warna", help::COLORS);
                ui.horizontal(|ui| {
                    ui.color_edit_button_srgb(&mut state.title_color);
                    ui.color_edit_button_srgb(&mut state.track_color);
                    ui.color_edit_button_srgb(&mut state.subtitle_color);
                    ui.small("judul • lagu • sub judul");
                });
                ui.end_row();
            });
    }

    fn thumbnail_output(&mut self, ui: &mut egui::Ui) {
        let can_generate = self.background.is_some() && !self.thumbnail.busy;
        if ui
            .add_enabled(can_generate, egui::Button::new("Buat Thumbnail"))
            .clicked()
        {
            self.generate_thumbnail();
        }
        theme::info(ui, help::GENERATE);
        if self.background.is_none() {
            ui.small("Pilih background di langkah 1 dulu.");
        }

        if let Some(path) = self.thumbnail.result.clone() {
            ui.add_space(8.0);
            ui.label(path.display().to_string());
            ui.horizontal(|ui| {
                if ui.button("Buka Gambar").clicked() {
                    let _ = open_path(&path);
                }
                if ui.button("Buka Folder").clicked() {
                    let _ = open_path(&self.paths.outputs);
                }
            });
            ui.add(
                egui::Image::new(format!("file://{}", path.display()))
                    .max_width(ui.available_width())
                    .corner_radius(4.0),
            );
        }
    }

    fn generate_thumbnail(&mut self) {
        let Some(background) = &self.background else {
            return;
        };
        let source = background
            .image
            .clone()
            .unwrap_or_else(|| background.video.path.clone().into());
        let state = &self.thumbnail;
        let spec = ThumbnailSpec {
            title: state.title.clone(),
            subtitle: state.subtitle.clone(),
            track_titles: self
                .tracks
                .iter()
                .map(|track| track.title.clone())
                .collect(),
            column_mode: COLUMN_MODES[state.column_mode].0.to_owned(),
            number_style: if state.mirrored_numbers {
                "mirrored"
            } else {
                "sequential"
            }
            .to_owned(),
            title_font_size: state.title_size,
            track_font_size: state.track_size,
            vignette_opacity: state.vignette,
            title_color: hex(state.title_color),
            track_color: hex(state.track_color),
            subtitle_color: hex(state.subtitle_color),
        };
        let timestamp = state.timestamp;
        let output = self
            .paths
            .outputs
            .join(format!("thumbnail_{}.jpg", random_id(8)));
        let (tools, temp) = (self.tools.clone(), self.paths.temp.clone());

        self.thumbnail.busy = true;
        self.spawn(async move {
            let rendered =
                thumbnail::render(&tools, &temp, &source, timestamp, spec, output.clone()).await;
            Event::Thumbnail(rendered.map(|()| output))
        });
    }
}
