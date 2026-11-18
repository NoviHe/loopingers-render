use eframe::egui;
use loopkaraoke::{
    media::format_clock,
    render::{
        effects::EFFECTS,
        encoder::Encoder,
        presets::{self, RESOLUTION_LABELS},
    },
};

use super::{
    theme::{
        self, InputWidth, PanelVariant, StyledButton, TextColor, TextSize, UiButtons, UiInputs,
        UiMetrics, UiPanels, UiText,
    },
    StudioApp,
};

impl StudioApp {
    pub(super) fn quick_tab(&mut self, ui: &mut egui::Ui) {
        theme::page_header(ui, "Render Old", "Masukkan video/gambar dan lagu di sisi kiri, pantau preview di sisi kanan, lalu tekan tombol render.");
        ui.space_group();
        ui.separator();
        ui.space_group();

        if ui.available_width() >= 720.0 {
            ui.columns(2, |columns| {
                self.quick_form(&mut columns[0]);
                self.quick_preview(&mut columns[1]);
            });
        } else {
            self.quick_form(ui);
            ui.space_group();
            self.quick_preview(ui);
        }
    }

    fn quick_form(&mut self, ui: &mut egui::Ui) {
        ui.card(|ui| {
            ui.set_width(ui.available_width());

            // Form: Nama File
            ui.strong_label("Nama File Output");
            ui.space1();
            ui.text_input_hint_with_width(
                &mut self.custom_filename,
                "Nama khusus (opsional, misal: Lagu Santai)",
                InputWidth::Fill,
            );
            if self.custom_filename.trim().is_empty() {
                ui.metadata_label("Kosong: menggunakan nama default `loopingers_render_{id}.mp4`");
            } else {
                let clean = self
                    .custom_filename
                    .trim()
                    .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
                ui.metadata_label(format!("Hasil: `{clean}_loopingers_render_{{id}}.mp4`"));
            }
            ui.space_group();

            // Form: Pilih Background Dinamis (Video / Gambar)
            ui.strong_label("Background Video / Gambar");
            ui.space1();
            ui.horizontal_wrapped(|ui| {
                if ui
                    .primary_button("📁 Pilih Video / Gambar...")
                    .on_hover_text(
                        "Pilih berkas video (MP4/MKV/WebM) atau gambar (JPG/PNG/WebP/JFIF)",
                    )
                    .clicked()
                {
                    self.pick_background_dynamic();
                }
            });
            if self.background.is_some() {
                ui.space1();
                self.background_effects_card(ui, "quick_bg");
            } else {
                ui.muted_label("Belum ada background dipilih.");
            }
            ui.space_group();

            // Form: Daftar Lagu (Folder / Satuan Dinamis & Shuffle)
            ui.strong_label("Daftar Lagu (Playlist)");
            ui.space1();
            wrapped_row(ui, |ui| {
                if ui.secondary_button("➕ Tambah Lagu...").clicked() {
                    self.pick_audio_files();
                }
                if ui.secondary_button("📂 Tambah dari Folder...").clicked() {
                    self.pick_audio_folder();
                }
                if ui
                    .add_enabled(
                        !self.tracks.is_empty(),
                        StyledButton::secondary("🔀 Acak Urutan (Shuffle)"),
                    )
                    .on_hover_text("Acak susunan urutan lagu di playlist secara instan")
                    .clicked()
                {
                    self.shuffle_tracks();
                }
                if ui
                    .add_enabled(
                        !self.tracks.is_empty(),
                        StyledButton::danger("Hapus Semua").small(),
                    )
                    .clicked()
                {
                    self.tracks.clear();
                    self.quick_selected_track = None;
                    self.info("Seluruh lagu di playlist dihapus.");
                }
            });
            if self.tracks.is_empty() {
                ui.muted_label("Belum ada lagu. Tambah lagu satuan atau dari folder.");
            } else {
                ui.label(format!(
                    "{} lagu dipilih • Total durasi {}",
                    self.tracks.len(),
                    format_clock(self.total_duration().round() as u64)
                ));
                ui.space1();
                self.playlist_display_mode_card(ui, "quick_form");
            }
            ui.space_group();

            // Form: Resolusi, Frame rate, & Encoder
            ui.strong_label("Pengaturan Render");
            ui.space1();
            let prev_res = self.resolution;
            let prev_fps = self.fps;
            let prev_thumb = self.auto_generate_thumbnail;

            wrapped_row(ui, |ui| {
                ui.label("Resolusi:");
                for label in RESOLUTION_LABELS {
                    ui.radio_value(&mut self.resolution, label, label);
                }
            });
            wrapped_row(ui, |ui| {
                ui.label("Frame rate:");
                ui.radio_value(&mut self.fps, 30, "30 FPS");
                ui.radio_value(&mut self.fps, 60, "60 FPS");
            });

            let hardware = self.hardware.clone().unwrap_or_default();
            wrapped_row(ui, |ui| {
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
                    self.info(format!("Encoder diubah ke {}.", self.encoder.name()));
                }
            });
            ui.space1();
            ui.checkbox(
                &mut self.auto_generate_thumbnail,
                "Buat Thumbnail YouTube Otomatis (1920x1080)",
            )
            .on_hover_text(
                "Otomatis membuat gambar thumbnail YouTube 1920x1080 di folder hasil saat render video",
            );

            if self.resolution != prev_res {
                self.save_settings();
                self.info(format!("Resolusi render diubah ke {}.", self.resolution));
            } else if self.fps != prev_fps {
                self.save_settings();
                self.info(format!("Frame rate diubah ke {} FPS.", self.fps));
            } else if self.auto_generate_thumbnail != prev_thumb {
                self.save_settings();
                let state = if self.auto_generate_thumbnail {
                    "diaktifkan"
                } else {
                    "dimatikan"
                };
                self.info(format!("Buat Thumbnail YouTube Otomatis {state}."));
            }
            ui.space_section();

            ui.separator();
            ui.space2();

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
                ui.text_colored(format!("⚠ {reason}"), TextSize::Sm, TextColor::Warning);
            }

            let render_btn_width = ui.available_width().min(220.0);
            let render_button = StyledButton::primary(
                egui::RichText::new(if running {
                    "⏳ Sedang Merender..."
                } else {
                    "🚀 Mulai Render Video"
                })
                .size(theme::metrics::FONT_MD)
                .strong(),
            )
            .min_size(egui::vec2(render_btn_width, theme::metrics::CONTROL_HEIGHT_LG));

            if ui
                .add_enabled(blocker.is_none() && !running, render_button)
                .clicked()
            {
                self.start_render();
            }

            ui.space_group();
            ui.card_with_variant(PanelVariant::SurfaceAlt, |ui| {
                ui.set_width(ui.available_width());
                ui.card_title("Ringkasan Render");
                ui.space1();

                let (width, height) = presets::dimensions(self.aspect_ratio, self.resolution);

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
                ui.label(format!(
                    "Daftar Lagu di Video: {}",
                    if self.show_tracklist_visual {
                        "Ditampilkan (gaya Thumbnail)"
                    } else {
                        "Tidak ditampilkan"
                    }
                ));
                if self.has_any_track_lyrics() {
                    ui.label(format!(
                        "Karaoke di Video: {}",
                        if self.show_karaoke_lyrics {
                            "Ditampilkan (Lirik LRC)"
                        } else {
                            "Tidak ditampilkan"
                        }
                    ));
                }
                ui.label(format!("Efek Visual: {}", EFFECTS[self.visual_effect].name));
                ui.label(format!(
                    "Thumbnail: {}",
                    if self.auto_generate_thumbnail {
                        "Otomatis dibuat (1920x1080)"
                    } else {
                        "Nonaktif"
                    }
                ));

                if let Some(reason) = blocker {
                    ui.text_colored(format!("⚠ {reason}"), TextSize::Sm, TextColor::Warning);
                } else if self.job.is_none() {
                    ui.text_colored(
                        "✓ Semua siap untuk dirender.",
                        TextSize::Sm,
                        TextColor::Success,
                    );
                }

                if let Some(job) = self.job.clone() {
                    ui.space2();
                    self.job_section(ui, &job);
                }
            });
        });
    }

    fn quick_preview(&mut self, ui: &mut egui::Ui) {
        ui.card(|ui| {
            ui.set_width(ui.available_width());

            // Preview background live
            self.draw_live_background_preview(ui);

            ui.space2();
            ui.separator();
            ui.space2();

            // Pratinjau Playlist: menampilkan daftar judul lagu, badge hijau LRC (jika ada), dan durasi
            let track_count = self.tracks.len();
            let lrc_count = self
                .tracks
                .iter()
                .filter(|t| t.has_lrc || !t.lyrics.is_empty())
                .count();
            ui.horizontal(|ui| {
                ui.card_title(format!("Pratinjau Playlist ({track_count} lagu)"));
                if track_count > 0 {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.muted_label(format!(
                            "Total: {}",
                            format_clock(self.total_duration().round() as u64)
                        ));
                        if lrc_count > 0 {
                            ui.text_colored(
                                format!("• {lrc_count}/{track_count} LRC"),
                                TextSize::Sm,
                                TextColor::Success,
                            );
                        }
                    });
                }
            });
            ui.space1();

            if self.tracks.is_empty() {
                ui.card_with_variant(PanelVariant::SurfaceAlt, |ui| {
                    ui.set_width(ui.available_width());
                    ui.set_min_height(180.0);
                    ui.vertical_centered(|ui| {
                        ui.add_space(45.0);
                        ui.label(egui::RichText::new("🎵").size(26.0));
                        ui.strong_label("Daftar lagu masih kosong");
                        ui.muted_label(
                            "Tambahkan lagu satuan atau dari folder di form sebelah kiri",
                        );
                        ui.add_space(45.0);
                    });
                });
            } else {
                ui.card_with_variant(PanelVariant::SurfaceAlt, |ui| {
                    ui.set_width(ui.available_width());
                    ui.set_min_height(260.0);
                    egui::ScrollArea::vertical()
                        .id_salt("quick_simple_playlist_scroll")
                        .min_scrolled_height(320.0)
                        .max_height(520.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for (index, track) in self.tracks.iter().enumerate() {
                                let has_lrc = track.has_lrc || !track.lyrics.is_empty();
                                ui.horizontal(|ui| {
                                    ui.monospace(
                                        egui::RichText::new(format!("{:02}.", index + 1))
                                            .color(theme::TEXT_DIM),
                                    );
                                    ui.label(&track.title);
                                    if has_lrc {
                                        super::playlist::draw_lrc_badge(ui, track.lyrics.len());
                                    }
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.monospace(
                                                egui::RichText::new(format_clock(
                                                    track.duration.round() as u64,
                                                ))
                                                .color(theme::TEXT_DIM),
                                            );
                                        },
                                    );
                                });
                            }
                        });
                });
            }
        });
    }
}

/// Wraps intact single-line controls onto new rows when horizontal space runs out.
fn wrapped_row(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal_wrapped(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        add_contents(ui);
    });
}
