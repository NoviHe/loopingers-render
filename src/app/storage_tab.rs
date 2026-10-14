use std::path::PathBuf;

use eframe::egui;
use loopkaraoke::{
    storage::{self, CleanupOptions},
    util::open_path,
};

use super::{help::storage as help, theme, Event, StudioApp};

impl StudioApp {
    pub(super) fn storage_tab(&mut self, ui: &mut egui::Ui) {
        if self.storage.is_none() && self.pending == 0 {
            self.refresh_storage();
        }

        theme::heading_with_info(ui, "Penyimpanan & Pembersihan", help::HEADING);
        ui.horizontal(|ui| {
            if ui.button("Segarkan").clicked() {
                self.refresh_storage();
            }
            if ui.button("Buka folder hasil").clicked() {
                let _ = open_path(&self.paths.outputs);
            }
        });
        ui.add_space(8.0);

        let Some(stats) = self.storage.clone() else {
            ui.label("Memuat statistik...");
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
        ui.strong(format!("Total aplikasi: {:.2} MB", stats.total_app_mb));

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Hapus berkas sementara").clicked() {
                self.clean(CleanupOptions {
                    clean_temp: true,
                    ..CleanupOptions::default()
                });
            }
            theme::info(ui, help::CLEAN_TEMP);
            if ui.button("Hapus semua video hasil").clicked() {
                self.clean(CleanupOptions {
                    clean_outputs: true,
                    ..CleanupOptions::default()
                });
            }
            theme::info(ui, help::CLEAN_OUTPUTS);
        });

        ui.add_space(8.0);
        theme::strong_with_info(ui, "Video & gambar hasil", help::FILES);
        let mut delete: Option<PathBuf> = None;
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
        if let Some(path) = delete {
            let paths = self.paths.clone();
            self.spawn(async move {
                let removed =
                    tokio::task::spawn_blocking(move || storage::delete_file(&paths, &path))
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
