use eframe::egui;
use loopkaraoke::{
    media::format_clock,
    render::{
        effects::EFFECTS,
        encoder::Encoder,
        image_video::Motion,
        looper::FitMode,
        presets::{self, ASPECT_RATIOS, RESOLUTION_LABELS},
    },
    util::open_path,
};

use super::{theme, StudioApp};

impl StudioApp {
    pub(super) fn quick_tab(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new("Loopingers Render")
                        .size(20.0)
                        .strong()
                        .color(egui::Color32::WHITE),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("— Audio/Video Merger, Karaoke FX, Playlist Generator")
                        .size(13.0)
                        .color(theme::FOCUS),
                );
            });
            ui.weak(
                "Main Menu: masukan video/gambar dan lagu di sisi kiri, pantau pratinjau di sisi kanan, lalu tekan tombol render.",
            );
        });
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);

        ui.columns(2, |columns| {
            self.quick_form(&mut columns[0]);
            self.quick_preview(&mut columns[1]);
        });
    }

    fn quick_form(&mut self, ui: &mut egui::Ui) {
        let frame = egui::Frame::new()
            .fill(theme::PANEL_BG)
            .stroke(egui::Stroke::new(1.0_f32, theme::FIELD_BORDER))
            .corner_radius(6)
            .inner_margin(egui::Margin::same(12));

        frame.show(ui, |ui| {
            // Form: Nama File
            ui.strong("Nama File Output");
            ui.add_space(2.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.custom_filename)
                    .hint_text("Nama khusus (opsional, misal: Lagu Santai)")
                    .desired_width(f32::INFINITY),
            );
            if self.custom_filename.trim().is_empty() {
                ui.small("Kosong: menggunakan nama default `loopingers_render_{id}.mp4`");
            } else {
                let clean = self
                    .custom_filename
                    .trim()
                    .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
                ui.small(format!("Hasil: `{clean}_loopingers_render_{{id}}.mp4`"));
            }
            ui.add_space(10.0);

            // Form: Pilih Background Dinamis (Video / Gambar)
            ui.strong("Background Video / Gambar");
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add(theme::primary_button("📁 Pilih Video / Gambar..."))
                    .on_hover_text(
                        "Pilih berkas video (MP4/MKV/WebM) atau gambar (JPG/PNG/WebP/JFIF)",
                    )
                    .clicked()
                {
                    self.pick_background_dynamic();
                }
                if ui.button("Coba Demo").clicked() {
                    self.load_demo();
                }
            });
            if let Some(bg) = &self.background {
                let is_image = bg.image.is_some();
                ui.horizontal(|ui| {
                    if is_image {
                        ui.colored_label(theme::SUCCESS, "🖼️ Gambar:");
                    } else {
                        ui.colored_label(theme::SUCCESS, "🎬 Video:");
                    }
                    ui.label(&bg.video.filename);
                });
            } else {
                ui.weak("Belum ada background dipilih.");
            }
            ui.add_space(10.0);

            // Form: Daftar Lagu (Folder / Satuan Dinamis & Shuffle)
            ui.strong("Daftar Lagu (Playlist)");
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                if ui.button("➕ Tambah Lagu...").clicked() {
                    self.pick_audio_files();
                }
                if ui.button("📂 Tambah dari Folder...").clicked() {
                    self.pick_audio_folder();
                }
                if ui
                    .add_enabled(
                        !self.tracks.is_empty(),
                        egui::Button::new("🔀 Acak Urutan (Shuffle)"),
                    )
                    .on_hover_text("Acak susunan urutan lagu di playlist secara instan")
                    .clicked()
                {
                    self.shuffle_tracks();
                }
                if ui
                    .add_enabled(
                        !self.tracks.is_empty(),
                        egui::Button::new("Hapus Semua").small(),
                    )
                    .clicked()
                {
                    self.tracks.clear();
                    self.timestamps_text.clear();
                    self.quick_selected_track = None;
                    self.info("Seluruh lagu di playlist dihapus.");
                }
            });
            if self.tracks.is_empty() {
                ui.weak("Belum ada lagu. Tambah lagu satuan atau dari folder.");
            } else {
                ui.label(format!(
                    "{} lagu dipilih • Total durasi {}",
                    self.tracks.len(),
                    format_clock(self.total_duration().round() as u64)
                ));
                ui.add_space(4.0);
                let any_transcribing = self.tracks.iter().any(|t| t.transcribing);
                ui.horizontal(|ui| {
                    let btn = if any_transcribing {
                        egui::Button::new("⏳ Sedang Transkripsi Whisper...")
                    } else {
                        theme::primary_button("✨ Auto Lirik Semua Lagu")
                    };
                    if ui
                        .add_enabled(!any_transcribing, btn)
                        .on_hover_text("Transkripsikan semua lagu di playlist secara otomatis dengan Whisper AI")
                        .clicked()
                    {
                        for index in 0..self.tracks.len() {
                            self.transcribe_track(index);
                        }
                    }
                    if any_transcribing {
                        ui.spinner();
                    }
                });
                ui.add_space(4.0);
                let whisper_frame = egui::Frame::new()
                    .fill(theme::ROW_ALT)
                    .stroke(egui::Stroke::new(1.0_f32, theme::FIELD_BORDER))
                    .corner_radius(4)
                    .inner_margin(egui::Margin::same(8));
                whisper_frame.show(ui, |ui| {
                    ui.strong("🎙️ Pengaturan Whisper AI");
                    ui.add_space(2.0);
                    self.whisper_settings(ui);
                });
            }
            ui.add_space(10.0);

            // Form: Efek Visual, Resolusi, Frame rate, & Encoder
            ui.strong("Pengaturan Render");
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label("Efek Visual:");
                egui::ComboBox::from_id_salt("quick_visual_effect")
                    .selected_text(EFFECTS[self.visual_effect].name)
                    .show_ui(ui, |ui| {
                        for (index, effect) in EFFECTS.iter().enumerate() {
                            ui.selectable_value(&mut self.visual_effect, index, effect.name)
                                .on_hover_text(effect.desc);
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Resolusi:");
                for label in RESOLUTION_LABELS {
                    ui.radio_value(&mut self.resolution, label, label);
                }
            });
            ui.horizontal(|ui| {
                ui.label("Frame rate:");
                ui.radio_value(&mut self.fps, 30, "30 FPS");
                ui.radio_value(&mut self.fps, 60, "60 FPS");
            });

            let hardware = self.hardware.clone().unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label("Encoder:");
                let previous_encoder = self.encoder;
                egui::ComboBox::from_id_salt("quick_encoder_select")
                    .selected_text(self.encoder.name())
                    .show_ui(ui, |ui| {
                        for encoder in Encoder::ALL {
                            let available = encoder.is_available(&hardware);
                            let label = if available {
                                encoder.name().to_string()
                            } else {
                                format!("{} (tidak didukung)", encoder.name())
                            };
                            ui.add_enabled_ui(available, |ui| {
                                ui.selectable_value(&mut self.encoder, encoder, label);
                            });
                        }
                    });
                if self.encoder != previous_encoder {
                    self.preset = self.encoder.default_preset().to_owned();
                }
            });
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut self.auto_generate_thumbnail,
                    "Buat Thumbnail YouTube Otomatis (1920x1080)",
                )
                .on_hover_text("Otomatis membuat gambar thumbnail YouTube 1920x1080 di folder hasil saat render video");
            });
            ui.add_space(12.0);

            ui.separator();
            ui.add_space(8.0);

            // Tombol Render
            let blocker = if self.background.is_none() {
                Some("Pilih video atau gambar background terlebih dahulu.")
            } else if self.tracks.is_empty() {
                Some("Tambahkan minimal 1 lagu ke playlist.")
            } else if self.hardware.is_none() {
                Some("Sedang mendeteksi perangkat hardware...")
            } else {
                None
            };

            let running = self.job_running();
            if let Some(reason) = blocker {
                ui.colored_label(theme::WARNING, format!("⚠️ {reason}"));
            }

            let render_button = egui::Button::new(
                egui::RichText::new(if running {
                    "⏳ Sedang Merender..."
                } else {
                    "🚀 Mulai Render Video"
                })
                .size(15.0)
                .strong(),
            )
            .min_size(egui::vec2(220.0, 38.0));

            if ui
                .add_enabled(blocker.is_none() && !running, render_button)
                .clicked()
            {
                self.start_render();
            }

            ui.add_space(10.0);
            let summary_frame = egui::Frame::new()
                .fill(theme::ROW_ALT)
                .stroke(egui::Stroke::new(1.0_f32, theme::FIELD_BORDER))
                .corner_radius(4)
                .inner_margin(egui::Margin::same(10));

            summary_frame.show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.strong("Ringkasan Render");
                ui.add_space(4.0);

                let (width, height) = presets::dimensions(self.aspect_ratio, self.resolution);
                let without_lyrics = self
                    .tracks
                    .iter()
                    .filter(|track| track.lyrics.is_empty())
                    .count();

                ui.label(format!(
                    "Keluaran: {width}x{height} @ {} FPS • {}",
                    self.fps,
                    self.encoder.name()
                ));
                ui.label(format!(
                    "Playlist: {} lagu • total durasi {}",
                    self.tracks.len(),
                    format_clock(self.total_duration().round() as u64)
                ));
                ui.label(format!("Efek Visual: {}", EFFECTS[self.visual_effect].name));
                ui.label(format!(
                    "Thumbnail: {}",
                    if self.auto_generate_thumbnail {
                        "Otomatis dibuat (1920x1080)"
                    } else {
                        "Nonaktif"
                    }
                ));

                if without_lyrics > 0 {
                    ui.colored_label(
                        theme::WARNING,
                        format!("⚠️ {without_lyrics} lagu belum punya lirik; hanya banner judul yang tampil."),
                    );
                }

                if let Some(reason) = blocker {
                    ui.colored_label(theme::WARNING, format!("⚠️ {reason}"));
                } else if self.job.is_none() {
                    ui.colored_label(theme::SUCCESS, "✓ Semua siap untuk dirender.");
                }

                if let Some(job) = self.job.clone() {
                    ui.add_space(8.0);
                    self.job_section(ui, &job);
                }
            });
        });
    }

    fn quick_preview(&mut self, ui: &mut egui::Ui) {
        let frame = egui::Frame::new()
            .fill(theme::FIELD_BG)
            .stroke(egui::Stroke::new(1.0_f32, theme::FIELD_BORDER))
            .corner_radius(6)
            .inner_margin(egui::Margin::same(12));

        frame.show(ui, |ui| {
            // Preview background
            ui.strong("Pratinjau Background");
            ui.add_space(4.0);
            if let Some(bg) = &self.background {
                let video = bg.video.clone();
                let image = bg.image.clone();
                let is_image = image.is_some();

                ui.columns(2, |cols| {
                    // Sisi Kiri: Card Preview Background & Tombol Buka di Pemutar
                    let info_frame = egui::Frame::new()
                        .fill(theme::ROW_ALT)
                        .stroke(egui::Stroke::new(1.0_f32, theme::FIELD_BORDER))
                        .corner_radius(4)
                        .inner_margin(egui::Margin::same(10));

                    info_frame.show(&mut cols[0], |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let icon = if is_image { "🖼️" } else { "🎬" };
                            ui.label(egui::RichText::new(icon).size(26.0));
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(&video.filename).strong());
                                ui.small(format!(
                                    "{}x{} • {:.1}s durasi",
                                    video.width, video.height, video.duration
                                ));
                                ui.small(format!("Rasio: {}", video.aspect_ratio));
                                if !is_image {
                                    let total = self.total_duration();
                                    if video.duration > 0.0 && total > 0.0 {
                                        let loops = (total / video.duration).ceil() as u64;
                                        ui.small(format!("Video meloop {loops} kali"));
                                    }
                                }
                            });
                        });
                        ui.add_space(8.0);
                        if ui
                            .button("▶ Buka di Pemutar")
                            .on_hover_text("Buka file video/gambar di pemutar media bawaan sistem")
                            .clicked()
                        {
                            let _ = open_path(std::path::Path::new(&video.path));
                        }
                    });

                    // Sisi Kanan: Opsi dari Menu Background (Rasio, Tampilan, Gerakan)
                    let opt_frame = egui::Frame::new()
                        .fill(theme::ROW_ALT)
                        .stroke(egui::Stroke::new(1.0_f32, theme::FIELD_BORDER))
                        .corner_radius(4)
                        .inner_margin(egui::Margin::same(10));

                    opt_frame.show(&mut cols[1], |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal_wrapped(|ui| {
                            ui.label(egui::RichText::new("Rasio Layar:").strong());
                            for aspect in ASPECT_RATIOS {
                                ui.radio_value(&mut self.aspect_ratio, aspect, aspect);
                            }
                        });
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.label(egui::RichText::new("Mode Tampilan:").strong());
                            for fit in FitMode::ALL {
                                ui.radio_value(&mut self.fit, fit, fit.label());
                            }
                        });

                        if let Some(img_path) = image {
                            ui.add_space(4.0);
                            ui.separator();
                            ui.add_space(4.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.label(egui::RichText::new("Gerakan Gambar:").strong());
                                for motion in Motion::ALL {
                                    ui.radio_value(&mut self.motion, motion, motion.label());
                                }
                            });
                            ui.checkbox(&mut self.beat_pulse, "Denyut mengikuti ketukan musik");
                            ui.add_space(4.0);
                            if ui
                                .button("🔄 Terapkan ke Background Gambar")
                                .on_hover_text("Proses ulang gambar latar dengan efek gerakan baru")
                                .clicked()
                            {
                                self.load_image(img_path);
                            }
                        }
                    });
                });
            } else {
                let placeholder_frame = egui::Frame::new()
                    .fill(theme::ROW_ALT)
                    .stroke(egui::Stroke::new(1.0_f32, theme::FIELD_BORDER))
                    .corner_radius(4)
                    .inner_margin(egui::Margin::symmetric(14, 18));

                placeholder_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new("🎬 / 🖼️").size(24.0));
                        ui.label("Belum ada background");
                        ui.weak("Pilih video atau gambar di formulir sebelah kiri");
                    });
                });
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);

            // Preview Playlist, Timestamps, & Editor Lirik
            let track_count = self.tracks.len();
            ui.horizontal(|ui| {
                ui.strong(format!("Pratinjau Playlist ({track_count} lagu)"));
                if track_count > 0 {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.weak(format_clock(self.total_duration().round() as u64));
                    });
                }
            });
            ui.add_space(4.0);

            let mut remove_index = None;
            let mut clicked_track = None;
            let mut close_lyrics = false;

            if self.tracks.is_empty() {
                let placeholder_frame = egui::Frame::new()
                    .fill(theme::ROW_ALT)
                    .stroke(egui::Stroke::new(1.0_f32, theme::FIELD_BORDER))
                    .corner_radius(4)
                    .inner_margin(egui::Margin::symmetric(14, 18));

                placeholder_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.vertical_centered(|ui| {
                        ui.add_space(10.0);
                        ui.label(egui::RichText::new("🎵").size(24.0));
                        ui.label("Daftar lagu masih kosong");
                        ui.weak("Tambahkan lagu satuan atau dari folder di form sebelah kiri");
                        ui.add_space(10.0);
                    });
                });
            } else {
                let active_track_opt = self
                    .quick_selected_track
                    .filter(|&idx| idx < self.tracks.len());

                if let Some(active_idx) = active_track_opt {
                    // 3 Kolom: [Timestamp YT] | [Menu Lirik Lagu] | [Daftar Lagu Playlist]
                    ui.columns(3, |cols| {
                        // Kolom 0: Timestamp YouTube
                        let ts_frame = egui::Frame::new()
                            .fill(theme::ROW_ALT)
                            .stroke(egui::Stroke::new(1.0_f32, theme::FIELD_BORDER))
                            .corner_radius(4)
                            .inner_margin(egui::Margin::same(8));

                        ts_frame.show(&mut cols[0], |ui| {
                            ui.set_width(ui.available_width());
                            ui.strong("⏱️ Timestamp YT");
                            ui.add_space(2.0);
                            ui.horizontal_wrapped(|ui| {
                                if ui
                                    .button("Buat")
                                    .on_hover_text("Buat timestamp YouTube dari urutan playlist")
                                    .clicked()
                                {
                                    self.timestamps_text = self.build_timestamps();
                                    self.log(
                                        super::LogLevel::Success,
                                        "Timestamp YouTube berhasil dibuat.",
                                    );
                                }
                                if !self.timestamps_text.is_empty()
                                    && ui
                                        .button("📋 Salin")
                                        .on_hover_text("Salin ke clipboard")
                                        .clicked()
                                {
                                    ui.ctx().copy_text(self.timestamps_text.clone());
                                    self.info("Timestamp disalin ke clipboard.");
                                }
                            });
                            ui.add_space(4.0);
                            if self.timestamps_text.is_empty() {
                                ui.weak("Klik 'Buat' untuk menghasilkan timestamp.");
                            } else {
                                egui::ScrollArea::vertical()
                                    .id_salt("quick_ts_scroll_3")
                                    .max_height(210.0)
                                    .show(ui, |ui| {
                                        ui.add(
                                            egui::TextEdit::multiline(&mut self.timestamps_text)
                                                .desired_width(f32::INFINITY)
                                                .desired_rows(8),
                                        );
                                    });
                            }
                        });

                        // Kolom 1: Menu Lirik Lagu Terpilih (di antara timestamp dan playlist)
                        let lyric_frame = egui::Frame::new()
                            .fill(theme::ROW_ALT)
                            .stroke(egui::Stroke::new(1.0_f32, theme::FOCUS))
                            .corner_radius(4)
                            .inner_margin(egui::Margin::same(8));

                        lyric_frame.show(&mut cols[1], |ui| {
                            ui.set_width(ui.available_width());
                            let track_title = self.tracks[active_idx].title.clone();
                            let display_title = if track_title.chars().count() > 14 {
                                format!("{}...", track_title.chars().take(11).collect::<String>())
                            } else {
                                track_title
                            };

                            ui.horizontal(|ui| {
                                ui.strong(format!("📝 {display_title}"));
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .small_button("✖")
                                            .on_hover_text("Tutup panel lirik")
                                            .clicked()
                                        {
                                            close_lyrics = true;
                                        }
                                    },
                                );
                            });
                            ui.add_space(2.0);

                            // Tombol Auto Lirik
                            let transcribing = self.tracks[active_idx].transcribing;
                            ui.horizontal(|ui| {
                                let btn = if transcribing {
                                    egui::Button::new("⏳ Proses...")
                                } else {
                                    theme::primary_button("✨ Auto Lirik")
                                };
                                if ui
                                    .add_enabled(!transcribing, btn)
                                    .on_hover_text("Buat auto lirik lagu ini dengan Whisper AI")
                                    .clicked()
                                {
                                    self.transcribe_track(active_idx);
                                }
                                if transcribing {
                                    ui.spinner();
                                }
                            });
                            ui.add_space(2.0);

                            // Impor LRC
                            egui::CollapsingHeader::new("Impor LRC")
                                .default_open(false)
                                .show(ui, |ui| {
                                    if ui.button("Pilih berkas .lrc...").clicked() {
                                        let picked = rfd::FileDialog::new()
                                            .add_filter("Lirik", &["lrc", "txt"])
                                            .pick_file();
                                        if let Some(path) = picked {
                                            match std::fs::read_to_string(&path) {
                                                Ok(text) => {
                                                    let lyrics =
                                                        loopkaraoke::karaoke::lrc::parse_lrc(&text);
                                                    if lyrics.is_empty() {
                                                        self.error(
                                                            "Format [mm:ss.xx] tidak ditemukan.",
                                                        );
                                                    } else {
                                                        let c = lyrics.len();
                                                        self.tracks[active_idx].lyrics = lyrics;
                                                        self.info(format!(
                                                            "{c} baris lirik diterapkan."
                                                        ));
                                                    }
                                                }
                                                Err(e) => {
                                                    self.error(format!("Gagal membaca berkas: {e}"))
                                                }
                                            }
                                        }
                                    }
                                });

                            let line_count = self.tracks[active_idx].lyrics.len();
                            ui.horizontal(|ui| {
                                ui.small(format!("Lirik: {line_count} baris"));
                                if line_count > 0 && ui.small_button("Hapus").clicked() {
                                    self.tracks[active_idx].lyrics.clear();
                                }
                            });

                            let mut remove_line = None;
                            egui::ScrollArea::vertical()
                                .id_salt(format!("quick_lyric_lines_{active_idx}"))
                                .max_height(140.0)
                                .show(ui, |ui| {
                                    if line_count == 0 {
                                        ui.weak("Belum ada lirik.");
                                    } else {
                                        for (l_idx, line) in
                                            self.tracks[active_idx].lyrics.iter().enumerate()
                                        {
                                            ui.horizontal(|ui| {
                                                ui.monospace(super::lyrics::clock(line.start));
                                                ui.label(&line.text);
                                                if ui
                                                    .small_button("x")
                                                    .on_hover_text("Hapus baris")
                                                    .clicked()
                                                {
                                                    remove_line = Some(l_idx);
                                                }
                                            });
                                        }
                                    }
                                });
                            if let Some(l_i) = remove_line {
                                self.tracks[active_idx].lyrics.remove(l_i);
                            }
                        });

                        // Kolom 2: Pratinjau Playlist (Daftar Lagu)
                        let pl_frame = egui::Frame::new()
                            .fill(theme::ROW_ALT)
                            .stroke(egui::Stroke::new(1.0_f32, theme::FIELD_BORDER))
                            .corner_radius(4)
                            .inner_margin(egui::Margin::same(8));

                        pl_frame.show(&mut cols[2], |ui| {
                            ui.set_width(ui.available_width());
                            ui.strong("Daftar Lagu");
                            ui.small("Klik lagu untuk ganti lirik.");
                            ui.add_space(2.0);

                            egui::ScrollArea::vertical()
                                .id_salt("quick_playlist_scroll_3")
                                .max_height(210.0)
                                .show(ui, |ui| {
                                    egui::Grid::new("quick_playlist_grid_3")
                                        .striped(true)
                                        .spacing([4.0, 4.0])
                                        .show(ui, |ui| {
                                            for (index, track) in self.tracks.iter_mut().enumerate()
                                            {
                                                let is_active = Some(index) == active_track_opt;
                                                let badge = if is_active { "▶" } else { "•" };
                                                if ui
                                                    .selectable_label(is_active, badge)
                                                    .on_hover_text("Pilih lagu ini")
                                                    .clicked()
                                                {
                                                    clicked_track = Some(index);
                                                }

                                                let text_resp = ui.add(
                                                    egui::TextEdit::singleline(&mut track.title)
                                                        .desired_width(90.0),
                                                );
                                                if text_resp.clicked() {
                                                    clicked_track = Some(index);
                                                }

                                                ui.label(format_clock(
                                                    track.duration.round() as u64
                                                ));

                                                let status_text = if track.transcribing {
                                                    "⏳".to_string()
                                                } else if track.lyrics.is_empty() {
                                                    "+".to_string()
                                                } else {
                                                    format!("✓{}", track.lyrics.len())
                                                };
                                                if ui
                                                    .small_button(status_text)
                                                    .on_hover_text("Buka editor lirik lagu ini")
                                                    .clicked()
                                                {
                                                    clicked_track = Some(index);
                                                }

                                                if ui
                                                    .small_button("x")
                                                    .on_hover_text("Hapus lagu ini")
                                                    .clicked()
                                                {
                                                    remove_index = Some(index);
                                                }
                                                ui.end_row();
                                            }
                                        });
                                });
                        });
                    });
                } else {
                    // 2 Kolom: [Timestamp YT] | [Daftar Lagu Playlist]
                    ui.columns(2, |cols| {
                        // Kolom 0: Timestamp YouTube
                        let ts_frame = egui::Frame::new()
                            .fill(theme::ROW_ALT)
                            .stroke(egui::Stroke::new(1.0_f32, theme::FIELD_BORDER))
                            .corner_radius(4)
                            .inner_margin(egui::Margin::same(8));

                        ts_frame.show(&mut cols[0], |ui| {
                            ui.set_width(ui.available_width());
                            ui.strong("⏱️ Timestamp YouTube");
                            ui.add_space(2.0);
                            ui.horizontal_wrapped(|ui| {
                                if ui
                                    .button("Buat Timestamp")
                                    .on_hover_text("Buat timestamp YouTube dari urutan playlist")
                                    .clicked()
                                {
                                    self.timestamps_text = self.build_timestamps();
                                    self.log(
                                        super::LogLevel::Success,
                                        "Timestamp YouTube berhasil dibuat.",
                                    );
                                }
                                if !self.timestamps_text.is_empty()
                                    && ui
                                        .button("📋 Salin")
                                        .on_hover_text("Salin ke clipboard")
                                        .clicked()
                                {
                                    ui.ctx().copy_text(self.timestamps_text.clone());
                                    self.info("Timestamp disalin ke clipboard.");
                                }
                            });
                            ui.add_space(4.0);
                            if self.timestamps_text.is_empty() {
                                ui.weak(
                                    "Klik 'Buat Timestamp' untuk menghasilkan daftar bab lagu.",
                                );
                            } else {
                                egui::ScrollArea::vertical()
                                    .id_salt("quick_ts_scroll_2")
                                    .max_height(210.0)
                                    .show(ui, |ui| {
                                        ui.add(
                                            egui::TextEdit::multiline(&mut self.timestamps_text)
                                                .desired_width(f32::INFINITY)
                                                .desired_rows(8),
                                        );
                                    });
                            }
                        });

                        // Kolom 1: Pratinjau Playlist (Daftar Lagu)
                        let pl_frame = egui::Frame::new()
                            .fill(theme::ROW_ALT)
                            .stroke(egui::Stroke::new(1.0_f32, theme::FIELD_BORDER))
                            .corner_radius(4)
                            .inner_margin(egui::Margin::same(8));

                        pl_frame.show(&mut cols[1], |ui| {
                            ui.set_width(ui.available_width());
                            ui.strong(format!("Daftar Lagu ({track_count})"));
                            ui.small("Klik lagu untuk melihat/mengedit lirik.");
                            ui.add_space(2.0);

                            egui::ScrollArea::vertical()
                                .id_salt("quick_playlist_scroll_2")
                                .max_height(210.0)
                                .show(ui, |ui| {
                                    egui::Grid::new("quick_playlist_grid_2")
                                        .striped(true)
                                        .spacing([6.0, 4.0])
                                        .show(ui, |ui| {
                                            for (index, track) in self.tracks.iter_mut().enumerate()
                                            {
                                                if ui
                                                    .selectable_label(false, "•")
                                                    .on_hover_text(
                                                        "Pilih lagu ini untuk membuka lirik",
                                                    )
                                                    .clicked()
                                                {
                                                    clicked_track = Some(index);
                                                }

                                                let text_resp = ui.add(
                                                    egui::TextEdit::singleline(&mut track.title)
                                                        .desired_width(120.0),
                                                );
                                                if text_resp.clicked() {
                                                    clicked_track = Some(index);
                                                }

                                                ui.add(
                                                    egui::TextEdit::singleline(&mut track.artist)
                                                        .hint_text("Artis")
                                                        .desired_width(70.0),
                                                );
                                                ui.label(format_clock(
                                                    track.duration.round() as u64
                                                ));

                                                let status_text = if track.transcribing {
                                                    "⏳...".to_string()
                                                } else if track.lyrics.is_empty() {
                                                    "+ lirik".to_string()
                                                } else {
                                                    format!("✓ {}b", track.lyrics.len())
                                                };
                                                if ui
                                                    .small_button(status_text)
                                                    .on_hover_text("Buka editor lirik lagu ini")
                                                    .clicked()
                                                {
                                                    clicked_track = Some(index);
                                                }

                                                if ui
                                                    .small_button("x")
                                                    .on_hover_text("Hapus lagu ini")
                                                    .clicked()
                                                {
                                                    remove_index = Some(index);
                                                }
                                                ui.end_row();
                                            }
                                        });
                                });
                        });
                    });
                }
            }

            if let Some(idx) = clicked_track {
                self.quick_selected_track = Some(idx);
                self.selected_track = idx;
            }
            if close_lyrics {
                self.quick_selected_track = None;
            }

            if let Some(index) = remove_index {
                let title = self.tracks[index].title.clone();
                self.tracks.remove(index);
                if self.quick_selected_track == Some(index) {
                    self.quick_selected_track = None;
                } else if let Some(sel) = self.quick_selected_track {
                    if sel > index {
                        self.quick_selected_track = Some(sel - 1);
                    }
                }
                self.timestamps_text.clear();
                self.selected_track = self.selected_track.min(self.tracks.len().saturating_sub(1));
                self.log(
                    super::LogLevel::Info,
                    format!("Lagu \"{title}\" dihapus dari playlist."),
                );
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            // Card Gaya Teks Karaoke (dari menu Lirik & Gaya sebelah kanan)
            let style_card = egui::Frame::new()
                .fill(theme::ROW_ALT)
                .stroke(egui::Stroke::new(1.0_f32, theme::FIELD_BORDER))
                .corner_radius(4)
                .inner_margin(egui::Margin::same(10));

            style_card.show(ui, |ui| {
                self.style_section(ui);
            });
        });
    }
}
