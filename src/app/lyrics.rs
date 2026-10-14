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

fn clock(seconds: f64) -> String {
    format!("{:02}:{:05.2}", (seconds / 60.0) as u32, seconds % 60.0)
}

impl StudioApp {
    pub(super) fn lyrics_tab(&mut self, ui: &mut egui::Ui) {
        theme::heading_with_info(ui, "Langkah 3 — Lirik Karaoke & Gaya Teks", help::HEADING);
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
        let current = &self.tracks[self.selected_track];
        let current_title = current.title.clone();

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

        ui.add_space(6.0);
        theme::strong_with_info(ui, "Auto lirik dengan Whisper AI", help::WHISPER);
        ui.horizontal(|ui| {
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
            let gpu_available = self
                .hardware
                .as_ref()
                .is_some_and(|hardware| hardware.gpu_whisper);
            ui.add_enabled(
                gpu_available,
                egui::Checkbox::new(&mut self.use_gpu, "GPU (CUDA/Vulkan)"),
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
        ui.small("Model diunduh otomatis saat pertama dipakai dan disimpan di folder models.");

        let busy = self.tracks[self.selected_track].transcribing;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!busy, egui::Button::new("Buat Auto Lirik lagu ini"))
                .clicked()
            {
                self.transcribe_track(self.selected_track);
            }
            theme::info(ui, help::GENERATE_ONE);
            if ui.button("Auto Lirik semua lagu").clicked() {
                for index in 0..self.tracks.len() {
                    self.transcribe_track(index);
                }
            }
            theme::info(ui, help::GENERATE_ALL);
        });

        ui.add_space(6.0);
        theme::strong_with_info(ui, "Impor lirik LRC", help::LRC);
        ui.horizontal(|ui| {
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
        });
        ui.add(
            egui::TextEdit::multiline(&mut self.lrc_text)
                .hint_text("Tempel lirik format [mm:ss.xx] teks di sini")
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

        ui.add_space(8.0);
        let line_count = self.tracks[self.selected_track].lyrics.len();
        ui.horizontal(|ui| {
            ui.strong(format!("Lirik ({line_count} baris)"));
            theme::info(ui, help::LINES);
            if line_count > 0 && ui.small_button("Hapus semua").clicked() {
                self.tracks[self.selected_track].lyrics.clear();
            }
        });

        let mut remove = None;
        egui::ScrollArea::vertical()
            .id_salt("lyrics_lines")
            .max_height(180.0)
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

    fn apply_lrc(&mut self, text: &str) {
        let lyrics = parse_lrc(text);
        if lyrics.is_empty() {
            self.error("Tidak ada baris lirik berformat [mm:ss.xx] yang ditemukan.");
            return;
        }
        let count = lyrics.len();
        self.tracks[self.selected_track].lyrics = lyrics;
        self.info(format!("{count} baris lirik diterapkan."));
    }

    fn transcribe_track(&mut self, index: usize) {
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
        let (id, path) = (track.id, track.path.clone());

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

    fn style_section(&mut self, ui: &mut egui::Ui) {
        theme::strong_with_info(ui, "Gaya teks karaoke", help::STYLE);
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
            theme::info(ui, help::COLORS);
            ui.label("Normal");
            ui.color_edit_button_srgb(&mut self.style.normal);
            ui.label("Saat bernyanyi");
            ui.color_edit_button_srgb(&mut self.style.singing);
            ui.label("Garis tepi");
            ui.color_edit_button_srgb(&mut self.style.outline);
            ui.label("Bayangan");
            ui.color_edit_button_srgb(&mut self.style.shadow);
        });
        ui.horizontal(|ui| {
            ui.add(egui::Slider::new(&mut self.style.font_size, 20..=120).text("Ukuran font"));
            theme::info(ui, help::FONT_SIZE);
        });
        ui.horizontal(|ui| {
            ui.add(egui::Slider::new(&mut self.style.margin_v, 0..=300).text("Jarak dari tepi"));
            theme::info(ui, help::MARGIN);
        });
        ui.horizontal(|ui| {
            theme::label_with_info(ui, "Posisi:", help::POSITION);
            ui.radio_value(&mut self.style.alignment, 2, "Bawah");
            ui.radio_value(&mut self.style.alignment, 5, "Tengah");
            ui.radio_value(&mut self.style.alignment, 8, "Atas");
        });
        ui.horizontal(|ui| {
            theme::label_with_info(ui, "Efek wipe:", help::WIPE);
            ui.radio_value(&mut self.style.smooth_wipe, true, "Halus (menyapu)");
            ui.radio_value(&mut self.style.smooth_wipe, false, "Per kata (lompat)");
        });
        ui.horizontal(|ui| {
            ui.checkbox(
                &mut self.style.show_intro,
                "Banner judul & penyanyi di awal setiap lagu",
            );
            theme::info(ui, help::INTRO);
        });
    }
}
