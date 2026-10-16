use eframe::egui;
use loopkaraoke::karaoke::{
    lrc::parse_lrc,
    styles::THEMES,
    transcribe::{ModelSize, TranscribeOptions},
};

use super::{help::lyrics as help, theme, Event, StudioApp};

const LANGUAGES: [(&str, Option<&str>); 5] = [
    ("Deteksi otomatis", None),
    ("Indonesia", Some("id")),
    ("Inggris", Some("en")),
    ("Jepang", Some("ja")),
    ("Korea", Some("ko")),
];

const PREVIEW_HEIGHT: f32 = 84.0;

pub(super) fn clock(seconds: f64) -> String {
    format!("{:02}:{:05.2}", (seconds / 60.0) as u32, seconds % 60.0)
}

impl StudioApp {
    pub(super) fn lyrics_tab(&mut self, ui: &mut egui::Ui) {
        theme::heading_with_info(ui, "Lirik Karaoke & Gaya Teks", help::HEADING);
        ui.add_space(4.0);
        ui.columns(2, |columns| {
            if self.tracks.is_empty() {
                columns[0].label("Tambahkan lagu di langkah 2 terlebih dahulu.");
            } else {
                self.lyrics_source(&mut columns[0]);
            }
            self.style_section(&mut columns[1]);
        });
    }

    fn lyrics_source(&mut self, ui: &mut egui::Ui) {
        self.selected_track = self.selected_track.min(self.tracks.len() - 1);
        let current_title = self.tracks[self.selected_track].title.clone();

        ui.horizontal(|ui| {
            theme::label_with_info(ui, "Lagu:", help::TRACK);
            egui::ComboBox::from_id_salt("lyrics_track")
                .selected_text(current_title)
                .show_ui(ui, |ui| {
                    for (index, track) in self.tracks.iter().enumerate() {
                        ui.selectable_value(&mut self.selected_track, index, &track.title);
                    }
                });
        });
        ui.add_space(4.0);

        theme::strong_with_info(ui, "Auto lirik dengan Whisper AI", help::WHISPER);
        let busy = self.tracks[self.selected_track].transcribing;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!busy, theme::primary_button("Buat Auto Lirik lagu ini"))
                .clicked()
            {
                self.transcribe_track(self.selected_track);
            }
            theme::info(ui, help::GENERATE_ONE);
            if ui.button("Semua lagu").clicked() {
                for index in 0..self.tracks.len() {
                    self.transcribe_track(index);
                }
            }
            theme::info(ui, help::GENERATE_ALL);
            if busy {
                ui.spinner();
            }
        });
        ui.small("Model diunduh otomatis saat pertama dipakai.");
        egui::CollapsingHeader::new("Pengaturan Whisper")
            .default_open(false)
            .show(ui, |ui| self.whisper_settings(ui));
        egui::CollapsingHeader::new("Impor lirik LRC")
            .default_open(false)
            .show(ui, |ui| self.lrc_import(ui));

        ui.add_space(4.0);
        let line_count = self.tracks[self.selected_track].lyrics.len();
        ui.horizontal(|ui| {
            ui.strong(format!("Lirik ({line_count} baris)"));
            theme::info(ui, help::LINES);
            if line_count > 0 && ui.small_button("Hapus semua").clicked() {
                self.tracks[self.selected_track].lyrics.clear();
            }
        });
        if line_count == 0 {
            ui.small("Belum ada lirik. Buat otomatis atau impor file .lrc.");
        }

        let mut remove = None;
        egui::ScrollArea::vertical()
            .id_salt("lyrics_lines")
            .max_height(220.0)
            .show(ui, |ui| {
                for (index, line) in self.tracks[self.selected_track].lyrics.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.monospace(clock(line.start));
                        ui.label(&line.text);
                        if ui.small_button("x").clicked() {
                            remove = Some(index);
                        }
                    });
                }
            });
        if let Some(index) = remove {
            self.tracks[self.selected_track].lyrics.remove(index);
        }
    }

    pub(super) fn whisper_settings(&mut self, ui: &mut egui::Ui) {
        if !ModelSize::ALL.contains(&self.whisper_model) {
            self.whisper_model = ModelSize::Base;
        }
        ui.horizontal_wrapped(|ui| {
            theme::label_with_info(ui, "Model:", help::MODEL);
            egui::ComboBox::from_id_salt("whisper_model")
                .selected_text(self.whisper_model.name())
                .show_ui(ui, |ui| {
                    for model in ModelSize::ALL {
                        ui.selectable_value(&mut self.whisper_model, model, model.name());
                    }
                });
            theme::label_with_info(ui, "Bahasa:", help::LANGUAGE);
            egui::ComboBox::from_id_salt("whisper_language")
                .selected_text(LANGUAGES[self.language].0)
                .show_ui(ui, |ui| {
                    for (index, (label, _)) in LANGUAGES.iter().enumerate() {
                        ui.selectable_value(&mut self.language, index, *label);
                    }
                });
        });
        let gpu_available = self
            .hardware
            .as_ref()
            .is_some_and(|hardware| hardware.gpu_whisper);
        ui.horizontal(|ui| {
            ui.add_enabled(
                gpu_available,
                egui::Checkbox::new(&mut self.use_gpu, "Pakai GPU (CUDA/Vulkan)"),
            );
            theme::info(ui, help::GPU);
        });
        if let Some(hardware) = self
            .hardware
            .as_ref()
            .filter(|hw| hw.has_gpu() && !hw.gpu_whisper)
        {
            ui.small(format!(
                "GPU {} terdeteksi, tetapi build ini menjalankan Whisper di CPU (build dengan --features vulkan atau cuda).",
                hardware.vendor()
            ));
        }
    }

    pub(super) fn lrc_import(&mut self, ui: &mut egui::Ui) {
        theme::strong_with_info(ui, "File atau teks LRC", help::LRC);
        if ui.button("Impor file .lrc / .txt...").clicked() {
            let picked = rfd::FileDialog::new()
                .add_filter("Lirik", &["lrc", "txt"])
                .pick_file();
            if let Some(path) = picked {
                match std::fs::read_to_string(&path) {
                    Ok(text) => self.apply_lrc(&text),
                    Err(error) => self.error(format!("File lirik tidak terbaca: {error}")),
                }
            }
        }
        ui.add(
            egui::TextEdit::multiline(&mut self.lrc_text)
                .hint_text("Atau tempel lirik format [mm:ss.xx] teks di sini")
                .desired_width(f32::INFINITY)
                .desired_rows(3),
        );
        if ui
            .add_enabled(
                !self.lrc_text.trim().is_empty(),
                egui::Button::new("Terapkan teks LRC"),
            )
            .clicked()
        {
            let text = std::mem::take(&mut self.lrc_text);
            self.apply_lrc(&text);
        }
    }

    pub(super) fn apply_lrc(&mut self, text: &str) {
        let lyrics = parse_lrc(text);
        if lyrics.is_empty() {
            self.error("Tidak ada baris lirik berformat [mm:ss.xx] yang ditemukan.");
            return;
        }
        let count = lyrics.len();
        self.tracks[self.selected_track].lyrics = lyrics;
        self.info(format!("{count} baris lirik diterapkan."));
    }

    pub(super) fn transcribe_track(&mut self, index: usize) {
        let use_gpu = self.use_gpu
            && self
                .hardware
                .as_ref()
                .is_some_and(|hardware| hardware.gpu_whisper);
        let options = TranscribeOptions {
            model: self.whisper_model,
            language: LANGUAGES[self.language].1.map(str::to_owned),
            use_gpu,
        };
        let track = &mut self.tracks[index];
        if track.transcribing {
            return;
        }
        track.transcribing = true;
        let (id, path, title) = (track.id, track.path.clone(), track.title.clone());
        self.log(
            super::LogLevel::Info,
            format!(
                "Memulai transkripsi Whisper AI untuk \"{title}\" (model: {})...",
                self.whisper_model.name()
            ),
        );

        let (tools, models) = (self.tools.clone(), self.paths.models.clone());
        let (transcriber, slot) = (self.transcriber.clone(), self.transcribe_slot.clone());
        self.spawn(async move {
            // One model run at a time; the rest wait here instead of fighting for CPU.
            let _permit = slot.acquire_owned().await;
            let result = transcriber
                .transcribe(&tools, &models, &path, options)
                .await;
            Event::Lyrics { track: id, result }
        });
    }

    pub(super) fn style_section(&mut self, ui: &mut egui::Ui) {
        theme::strong_with_info(ui, "Gaya teks karaoke", help::STYLE);
        self.style_preview(ui);
        ui.add_space(4.0);

        ui.horizontal_wrapped(|ui| {
            theme::info(ui, help::THEME);
            for (index, (_, theme)) in THEMES.iter().enumerate() {
                if ui
                    .selectable_label(self.theme == index, theme.name)
                    .clicked()
                {
                    self.theme = index;
                    self.style.apply_theme(theme);
                }
            }
        });
        ui.horizontal(|ui| {
            ui.add(egui::Slider::new(&mut self.style.font_size, 20..=120).text("Ukuran font"));
            theme::info(ui, help::FONT_SIZE);
        });
        ui.horizontal(|ui| {
            theme::label_with_info(ui, "Posisi:", help::POSITION);
            ui.radio_value(&mut self.style.alignment, 2, "Bawah");
            ui.radio_value(&mut self.style.alignment, 5, "Tengah");
            ui.radio_value(&mut self.style.alignment, 8, "Atas");
        });
        egui::CollapsingHeader::new("Pengaturan lanjutan")
            .default_open(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    theme::info(ui, help::COLORS);
                    ui.label("Normal");
                    ui.color_edit_button_srgb(&mut self.style.normal);
                    ui.label("Bernyanyi");
                    ui.color_edit_button_srgb(&mut self.style.singing);
                    ui.label("Tepi");
                    ui.color_edit_button_srgb(&mut self.style.outline);
                    ui.label("Bayangan");
                    ui.color_edit_button_srgb(&mut self.style.shadow);
                });
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Slider::new(&mut self.style.margin_v, 0..=300)
                            .text("Jarak dari tepi"),
                    );
                    theme::info(ui, help::MARGIN);
                });
                ui.horizontal(|ui| {
                    theme::label_with_info(ui, "Efek wipe:", help::WIPE);
                    ui.radio_value(&mut self.style.smooth_wipe, true, "Halus");
                    ui.radio_value(&mut self.style.smooth_wipe, false, "Per kata");
                });
                ui.horizontal(|ui| {
                    ui.checkbox(
                        &mut self.style.show_intro,
                        "Banner judul & penyanyi di awal",
                    );
                    theme::info(ui, help::INTRO);
                });
            });
    }

    /// A small mock frame that shows the chosen colors, size and position before rendering.
    pub(super) fn style_preview(&self, ui: &mut egui::Ui) {
        const SAMPLE: &str = "Contoh lirik karaoke";
        let style = &self.style;
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), PREVIEW_HEIGHT),
            egui::Sense::hover(),
        );
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(0x12, 0x18, 0x2a));

        let color = |rgb: [u8; 3]| egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
        let font = egui::FontId::proportional((style.font_size as f32 * 0.45).clamp(10.0, 44.0));
        let galley =
            |rgb: [u8; 3]| painter.layout_no_wrap(SAMPLE.to_owned(), font.clone(), color(rgb));
        let base = galley(style.normal);

        let margin = style.margin_v as f32 * 0.2;
        let y = match style.alignment {
            8 => rect.top() + margin + 4.0,
            5 => rect.center().y - base.size().y / 2.0,
            _ => rect.bottom() - margin - base.size().y - 4.0,
        };
        let origin = egui::pos2(rect.center().x - base.size().x / 2.0, y);

        let outline = color(style.outline);
        for offset in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
            let shadow = painter.layout_no_wrap(SAMPLE.to_owned(), font.clone(), outline);
            painter.galley(origin + egui::vec2(offset.0, offset.1), shadow, outline);
        }
        painter.galley(origin, base.clone(), color(style.normal));

        let sung =
            egui::Rect::from_min_size(origin, egui::vec2(base.size().x * 0.45, base.size().y));
        painter
            .with_clip_rect(sung)
            .galley(origin, galley(style.singing), color(style.singing));
    }
}
