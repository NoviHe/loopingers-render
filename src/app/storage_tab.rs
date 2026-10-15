use std::path::PathBuf;

use eframe::egui;
use loopkaraoke::{
    storage::{self, CleanupOptions},
    util::open_path,
};

use super::{help::storage as help, theme, Event, StudioApp, Tab};

impl StudioApp {
    pub(super) fn storage_tab(&mut self, ui: &mut egui::Ui) {
        if self.storage.is_none() && self.pending == 0 {
            self.refresh_storage();
        }

        theme::heading_with_info(ui, "Settings", help::HEADING);
        ui.weak(format!(
            "Loopingers Render v{} — Pengaturan aplikasi, visibilitas menu, dan manajemen penyimpanan.",
            env!("CARGO_PKG_VERSION")
        ));
        ui.add_space(8.0);

        ui.columns(2, |columns| {
            // Card 1: Visibilitas Menu (Semua menu kecuali Main Menu dan Settings)
            let card_vis = egui::Frame::new()
                .fill(theme::PANEL_BG)
                .stroke(egui::Stroke::new(1.0_f32, theme::FIELD_BORDER))
                .corner_radius(6)
                .inner_margin(egui::Margin::same(12));

            card_vis.show(&mut columns[0], |ui| {
                ui.set_width(ui.available_width());
                ui.strong("Visibilitas Menu");
                ui.weak("Pilih menu lanjutan yang ingin ditampilkan pada navigasi atas.");
                ui.add_space(8.0);

                ui.checkbox(&mut self.show_tab_background, "Background");
                ui.checkbox(&mut self.show_tab_playlist, "Playlist");
                ui.checkbox(&mut self.show_tab_lyrics, "Lirik & Gaya");
                ui.checkbox(&mut self.show_tab_render, "Render");
                ui.checkbox(&mut self.show_tab_thumbnail, "Thumbnail");

                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Tampilkan Semua").clicked() {
                        self.show_tab_background = true;
                        self.show_tab_playlist = true;
                        self.show_tab_lyrics = true;
                        self.show_tab_render = true;
                        self.show_tab_thumbnail = true;
                    }
                    if ui.button("Sembunyikan Semua").clicked() {
                        self.show_tab_background = false;
                        self.show_tab_playlist = false;
                        self.show_tab_lyrics = false;
                        self.show_tab_render = false;
                        self.show_tab_thumbnail = false;
                    }
                });

                ui.add_space(10.0);
                ui.small("💡 Catatan: Main Menu dan Settings selalu aktif secara bawaan.");
            });

            // Card 2: Manajemen Penyimpanan & Berkas (Pengelompokan fitur storage yang sudah ada)
            let card_storage = egui::Frame::new()
                .fill(theme::PANEL_BG)
                .stroke(egui::Stroke::new(1.0_f32, theme::FIELD_BORDER))
                .corner_radius(6)
                .inner_margin(egui::Margin::same(12));

            card_storage.show(&mut columns[1], |ui| {
                ui.set_width(ui.available_width());
                ui.strong("Manajemen Penyimpanan & Berkas");
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.button("🔄 Segarkan").clicked() {
                        self.refresh_storage();
                    }
                    if ui.button("📂 Buka Folder Hasil").clicked() {
                        let _ = open_path(&self.paths.outputs);
                    }
                });
                ui.add_space(8.0);

                let Some(stats) = self.storage.clone() else {
                    ui.label("Memuat statistik penyimpanan...");
                    return;
                };

                ui.horizontal(|ui| {
                    ui.label(format!(
                        "Berkas sementara: {:.2} MB ({} berkas)",
                        stats.temp.size_mb, stats.temp.count
                    ));
                    theme::info(ui, help::TEMP);
                });
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "Video hasil render: {:.2} MB ({} berkas)",
                        stats.outputs.size_mb, stats.outputs.count
                    ));
                    theme::info(ui, help::OUTPUTS);
                });

                let model_names: Vec<&str> = stats
                    .models
                    .files
                    .iter()
                    .map(|file| file.name.as_str())
                    .collect();
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "Model AI: {:.2} MB — {}",
                        stats.models.size_mb,
                        if model_names.is_empty() {
                            "belum ada (diunduh saat pertama dipakai)".to_owned()
                        } else {
                            model_names.join(", ")
                        }
                    ));
                    theme::info(ui, help::MODELS);
                });
                ui.strong(format!("Total pemakaian: {:.2} MB", stats.total_app_mb));

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Hapus Berkas Sementara").clicked() {
                        self.clean(CleanupOptions {
                            clean_temp: true,
                            ..CleanupOptions::default()
                        });
                    }
                    theme::info(ui, help::CLEAN_TEMP);
                    if ui.button("Hapus Semua Video Hasil").clicked() {
                        self.clean(CleanupOptions {
                            clean_outputs: true,
                            ..CleanupOptions::default()
                        });
                    }
                    theme::info(ui, help::CLEAN_OUTPUTS);
                });

                ui.add_space(8.0);
                theme::strong_with_info(ui, "Daftar Video & Gambar Hasil", help::FILES);
                let mut delete: Option<PathBuf> = None;
                egui::ScrollArea::vertical()
                    .id_salt("settings_outputs_scroll")
                    .max_height(200.0)
                    .show(ui, |ui| {
                        if stats.outputs.files.is_empty() {
                            ui.weak("Belum ada berkas hasil render.");
                        } else {
                            for file in &stats.outputs.files {
                                ui.horizontal(|ui| {
                                    ui.label(format!(
                                        "{} • {:.2} MB • {}",
                                        file.name, file.size_mb, file.modified_str
                                    ));
                                    if ui.small_button("Buka").clicked() {
                                        let _ = open_path(&file.path);
                                    }
                                    if ui.small_button("Hapus").clicked() {
                                        delete = Some(file.path.clone());
                                    }
                                });
                            }
                        }
                    });

                if let Some(path) = delete {
                    let paths = self.paths.clone();
                    self.spawn(async move {
                        let removed = tokio::task::spawn_blocking(move || {
                            storage::delete_file(&paths, &path)
                        })
                        .await
                        .unwrap_or(false);
                        Event::Cleaned(
                            if removed {
                                "File dihapus."
                            } else {
                                "File tidak dapat dihapus."
                            }
                            .to_owned(),
                        )
                    });
                    self.storage = None;
                }
            });
        });

        if !self.tab_visible(self.tab) {
            self.tab = Tab::Quick;
        }
    }

    fn refresh_storage(&mut self) {
        let paths = self.paths.clone();
        self.spawn(async move {
            let stats = tokio::task::spawn_blocking(move || storage::stats(&paths))
                .await
                .unwrap_or_default();
            Event::Storage(stats)
        });
    }

    fn clean(&mut self, options: CleanupOptions) {
        let paths = self.paths.clone();
        self.spawn(async move {
            let counts = tokio::task::spawn_blocking(move || storage::cleanup(&paths, &options))
                .await
                .unwrap_or_default();
            Event::Cleaned(format!(
                "Dihapus: {} berkas sementara, {} video hasil",
                counts.temp, counts.outputs
            ))
        });
        self.storage = None;
    }
}
