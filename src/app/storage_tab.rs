use std::{f32::consts::PI, path::PathBuf, sync::Arc};

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, Sense, Shape, Stroke,
};
use loopkaraoke::{
    dependencies,
    ffmpeg::{self, DownloadProgress, Tools},
    karaoke::transcribe::ModelSize,
    render::encoder::{RenderSpeed, VideoCodec, VideoCompression, VideoTarget},
    storage::{self, CleanupOptions},
    util::open_path,
};

#[derive(Default)]
pub(super) struct DependencyUiState {
    pub report: Option<dependencies::DependencyReport>,
    pub refreshing: bool,
    pub operation: Option<String>,
    pub error: Option<String>,
    pub downloading_model: Option<ModelSize>,
    progress: Arc<DownloadProgress>,
    confirm_remove: Option<ModelSize>,
}

fn dependency_tool_card(ui: &mut egui::Ui, tool: &dependencies::ToolInfo) -> Option<PathBuf> {
    let p = theme::Palette::of(ui.ctx());
    let mut open_location = None;
    dependency_row(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.strong_label(&tool.name);
            ui.colored_label(
                if tool.error.is_none() {
                    p.success
                } else {
                    p.danger
                },
                if tool.error.is_none() {
                    "Siap"
                } else {
                    "Bermasalah"
                },
            );
            ui.muted_label(format!("{:.2} MB", tool.size_bytes as f64 / 1_048_576.0));
        });
        if let Some(version) = &tool.version {
            let short = version.split(" Copyright").next().unwrap_or(version);
            ui.add(egui::Label::new(egui::RichText::new(short).color(p.text_muted)).truncate())
                .on_hover_text(version);
        }
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if tool.path.is_file() && ui.secondary_button("Buka Lokasi").clicked() {
                    open_location = tool.path.parent().map(PathBuf::from);
                }
                ui.add(egui::Label::new(tool.path.display().to_string()).truncate())
                    .on_hover_text(tool.path.display().to_string());
            });
        });
        if let Some(error) = &tool.error {
            ui.add(egui::Label::new(egui::RichText::new(error).color(p.danger)).wrap());
        }
    });
    open_location
}

fn dependency_row(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui)) {
    let p = theme::Palette::of(ui.ctx());
    egui::Frame::new()
        .fill(p.bg)
        .stroke(Stroke::new(1.0_f32, p.border))
        .corner_radius(theme::metrics::CONTROL_RADIUS)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            contents(ui);
        });
    ui.add_space(6.0);
}

use super::{
    help::storage as help,
    theme::{self, UiButtons, UiMetrics, UiText},
    Event, StudioApp,
};

impl StudioApp {
    pub(super) fn storage_tab(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 8.0;
        if self.storage.is_none() && self.pending == 0 {
            self.refresh_storage();
        }

        theme::page_header(
            ui,
            "Settings",
            "Atur tampilan, video, penyimpanan, dan dependensi aplikasi.",
        )
        .on_hover_text(help::HEADING);
        ui.add_space(4.0);

        self.appearance_card(ui);
        ui.space2();
        self.video_settings_card(ui);
        ui.space3();

        self.storage_management_card(ui);
        ui.space2();
        self.dependency_management_card(ui);
        ui.space2();
        self.about_card(ui);
    }

    pub(super) fn refresh_dependencies(&mut self) {
        if self.dependencies.refreshing {
            return;
        }
        self.dependencies.refreshing = true;
        let tools = self.tools.clone();
        let models = self.paths.models.clone();
        let directory = self.dependency_settings.tools_dir.clone();
        self.spawn(async move {
            Event::Dependencies(if let Some(directory) = directory {
                dependencies::inspect_directory(directory, models).await
            } else {
                dependencies::inspect(tools, models).await
            })
        });
    }

    fn dependencies_editable(&self) -> bool {
        self.pending == 0
            && self.dependencies.operation.is_none()
            && !self.job_running()
            && !self
                .generate
                .job
                .as_ref()
                .is_some_and(|job| job.snapshot().is_running())
            && !self
                .lrc_studio
                .tracks
                .iter()
                .any(|track| track.transcribing)
    }

    fn change_dependency_tools(&mut self, directory: Option<PathBuf>) {
        if !self.dependencies_editable() {
            return;
        }
        self.dependencies.error = None;
        self.dependencies.operation = Some("Memeriksa FFmpeg dan FFprobe...".to_owned());
        let default_dir = self.paths.data_dir.join("tools");
        self.ffmpeg_download = Arc::new(DownloadProgress::default());
        let progress = self.ffmpeg_download.clone();
        self.spawn(async move {
            let result = if let Some(path) = directory.as_ref() {
                match Tools::from_directory(path) {
                    Ok(tools) => tools.check().await.map(|()| tools),
                    Err(error) => Err(error),
                }
            } else {
                ffmpeg::ensure_installed(default_dir, progress).await
            };
            Event::DependencyTools { directory, result }
        });
    }

    fn dependency_management_card(&mut self, ui: &mut egui::Ui) {
        if self.dependencies.report.is_none()
            && !self.dependencies.refreshing
            && self.dependencies.operation.is_none()
        {
            self.refresh_dependencies();
        }
        let p = theme::Palette::of(ui.ctx());
        settings_card(ui, |ui| {
            settings_card_header(
                ui,
                self.dark_mode,
                SettingsBadgeIcon::Folder,
                "Dependensi & Model AI",
                "Lihat tools yang digunakan aplikasi dan kelola model Whisper.",
            );
            ui.add_space(10.0);
            let editable = self.dependencies_editable();
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(!self.dependencies.refreshing && self.dependencies.operation.is_none(), theme::StyledButton::secondary("Periksa Ulang")).clicked() {
                    self.refresh_dependencies();
                }
                if ui.add_enabled(editable, theme::StyledButton::secondary("Pilih Folder Tools")).on_hover_text("Pilih folder yang berisi FFmpeg dan FFprobe. Keduanya diperiksa sebelum digunakan.").clicked() {
                    if let Some(path) = rfd::FileDialog::new().set_title("Folder FFmpeg dan FFprobe").pick_folder() {
                        self.change_dependency_tools(Some(path));
                    }
                }
                if ui.add_enabled(editable, theme::StyledButton::secondary("Gunakan Tools Otomatis")).on_hover_text("Cari tools bawaan aplikasi atau PATH. Unduh jika belum tersedia.").clicked() {
                    self.change_dependency_tools(None);
                }
                if ui.secondary_button("Buka Folder Model").clicked() {
                    if let Err(error) = open_path(&self.paths.models) {
                        self.error(format!("Gagal membuka folder model: {error}"));
                    }
                }
            });
            if let Some(directory) = &self.dependency_settings.tools_dir {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(format!(
                            "Folder tools pilihan: {}",
                            directory.display()
                        ))
                        .color(p.text_muted),
                    )
                    .wrap(),
                );
            } else {
                ui.muted_label(
                    "Lokasi tools: otomatis (bawaan aplikasi, folder tools, atau PATH).",
                );
            }
            if let Some(message) = &self.dependencies.operation {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(message);
                });
                let progress = if self.dependencies.downloading_model.is_some() {
                    &self.dependencies.progress
                } else {
                    &self.ffmpeg_download
                };
                let downloaded = progress
                    .downloaded
                    .load(std::sync::atomic::Ordering::Relaxed);
                let total = progress.total.load(std::sync::atomic::Ordering::Relaxed);
                if total > 0 {
                    ui.add(
                        egui::ProgressBar::new(
                            (downloaded as f64 / total as f64).clamp(0.0, 1.0) as f32
                        )
                        .text(format!(
                            "{:.1} / {:.1} MB",
                            downloaded as f64 / 1_048_576.0,
                            total as f64 / 1_048_576.0
                        )),
                    );
                } else if downloaded > 0 {
                    ui.muted_label(format!(
                        "Terunduh {:.1} MB",
                        downloaded as f64 / 1_048_576.0
                    ));
                }
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(200));
            } else if !editable {
                ui.muted_label("Tunggu proses aplikasi selesai untuk mengubah tools atau model.");
            }
            if let Some(error) = &self.dependencies.error {
                ui.colored_label(p.danger, error);
            }
            let Some(report) = self.dependencies.report.clone() else {
                if self.dependencies.refreshing {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.muted_label("Memeriksa dependensi...");
                    });
                }
                return;
            };
            ui.add_space(8.0);
            let mut open_location = None;
            if ui.available_width() >= 680.0 {
                ui.columns(2, |columns| {
                    for (column, tool) in columns.iter_mut().zip(&report.tools) {
                        if let Some(path) = dependency_tool_card(column, tool) {
                            open_location = Some(path);
                        }
                    }
                });
            } else {
                for tool in &report.tools {
                    if let Some(path) = dependency_tool_card(ui, tool) {
                        open_location = Some(path);
                    }
                }
            }
            if let Some(path) = open_location {
                if let Err(error) = open_path(&path) {
                    self.error(format!("Gagal membuka folder tools: {error}"));
                }
            }
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.strong_label("Whisper")
                    .on_hover_text("Model diunduh sekali dan digunakan kembali untuk transkripsi.");
                ui.muted_label(format!("Backend: {}", report.backend));
                ui.muted_label(self.hardware_status_text());
            });
            ui.horizontal(|ui| {
                ui.muted_label("Folder model");
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(self.paths.models.display().to_string())
                            .color(p.text_muted),
                    )
                    .truncate(),
                )
                .on_hover_text(self.paths.models.display().to_string());
            });
            ui.add_space(6.0);
            for model in &report.models {
                self.dependency_model_card(ui, model, editable);
            }
            if let Some(size) = self.dependencies.confirm_remove {
                ui.separator();
                ui.label(format!("Hapus model Whisper {} dari penyimpanan? Model perlu diunduh kembali untuk digunakan.", size.name()));
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(editable, theme::StyledButton::danger("Ya, Hapus Model"))
                        .clicked()
                    {
                        self.dependencies.confirm_remove = None;
                        self.dependencies.error = None;
                        self.dependencies.operation =
                            Some(format!("Menghapus model Whisper {}...", size.name()));
                        let models_dir = self.paths.models.clone();
                        let transcriber = self.transcriber.clone();
                        let slot = self.transcribe_slot.clone();
                        self.spawn(async move {
                            let _permit = slot.acquire_owned().await;
                            let result = tokio::task::spawn_blocking(move || {
                                transcriber.clear_cache()?;
                                dependencies::remove_model(&models_dir, size)
                            })
                            .await
                            .map_err(|error| loopkaraoke::error::Error::internal(error.to_string()))
                            .and_then(|result| result);
                            Event::DependencyModel {
                                result,
                                message: format!("Model Whisper {} dihapus.", size.name()),
                            }
                        });
                    }
                    if ui.secondary_button("Batal").clicked() {
                        self.dependencies.confirm_remove = None;
                    }
                });
            }
        });
    }

    fn dependency_model_card(
        &mut self,
        ui: &mut egui::Ui,
        model: &dependencies::ModelInfo,
        editable: bool,
    ) {
        let p = theme::Palette::of(ui.ctx());
        dependency_row(ui, |ui| {
            let supported = ModelSize::ALL.contains(&model.size);
            let info_width = if ui.available_width() >= 680.0 {
                ui.available_width() - 340.0
            } else {
                ui.available_width()
            };
            ui.horizontal_wrapped(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(info_width, 32.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.strong_label(format!("Whisper {}", model.size.name()))
                            .on_hover_text(model.path.display().to_string());
                        ui.muted_label(if model.installed {
                            format!(
                                "Terpasang · {:.1} MB",
                                model.size_bytes as f64 / 1_048_576.0
                            )
                        } else {
                            "Belum terpasang".to_owned()
                        });
                        if self.lrc_studio.whisper_model == model.size {
                            ui.colored_label(p.accent, "Dipilih");
                        }
                    },
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(
                            editable && model.installed,
                            theme::StyledButton::danger("Hapus Model"),
                        )
                        .clicked()
                    {
                        self.dependencies.confirm_remove = Some(model.size);
                    }
                    if ui
                        .add_enabled(
                            editable && model.installed && supported,
                            theme::StyledButton::secondary("Gunakan"),
                        )
                        .clicked()
                    {
                        self.lrc_studio.whisper_model = model.size;
                        self.save_settings();
                        self.info(format!(
                            "Whisper {} dipilih untuk Studio Lirik.",
                            model.size.name()
                        ));
                    }
                    if ui
                        .add_enabled(
                            editable && !model.installed && supported,
                            theme::StyledButton::secondary("Unduh Model"),
                        )
                        .clicked()
                    {
                        self.dependencies.error = None;
                        self.dependencies.progress = Arc::new(DownloadProgress::default());
                        self.dependencies.downloading_model = Some(model.size);
                        self.dependencies.operation =
                            Some(format!("Mengunduh Whisper {}...", model.size.name()));
                        let models_dir = self.paths.models.clone();
                        let size = model.size;
                        let progress = self.dependencies.progress.clone();
                        let slot = self.transcribe_slot.clone();
                        self.spawn(async move {
                            let _permit = slot.acquire_owned().await;
                            Event::DependencyModel {
                                result: dependencies::download_model(models_dir, size, progress)
                                    .await,
                                message: format!("Model Whisper {} berhasil diunduh.", size.name()),
                            }
                        });
                    }
                });
            });
            if !supported {
                ui.muted_label("Model ini tidak tersedia untuk transkripsi pada build CPU ini.");
            }
        });
    }

    fn about_card(&self, ui: &mut egui::Ui) {
        let dark = self.dark_mode;
        let p = theme::Palette::of(ui.ctx());
        let accent = if dark { p.accent_hover } else { p.accent };
        settings_card(ui, |ui| {
            settings_card_header(
                ui,
                dark,
                SettingsBadgeIcon::About,
                "About",
                "LoopKaraoke Studio — satu video loop jadi satu album.",
            );
            ui.add_space(10.0);
            about_row(ui, "Versi", |ui| {
                ui.label(
                    egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                        .strong()
                        .color(p.text_strong),
                );
            });
            about_row(ui, "GitHub", |ui| {
                ui.add(egui::Hyperlink::from_label_and_url(
                    egui::RichText::new("NoviHe/loopingers-render").color(accent),
                    "https://github.com/NoviHe/loopingers-render",
                ));
            });
            about_row(ui, "Author", |ui| {
                ui.add(egui::Hyperlink::from_label_and_url(
                    egui::RichText::new("NoviHe").color(accent),
                    "https://github.com/NoviHe",
                ));
            });
            about_row(ui, "Telegram", |ui| {
                ui.add(egui::Hyperlink::from_label_and_url(
                    egui::RichText::new("NoviHe").color(accent),
                    "https://t.me/+6282217352703",
                ));
            });
        });
    }

    fn storage_management_card(&mut self, ui: &mut egui::Ui) {
        let p = theme::Palette::of(ui.ctx());
        let output_dir = self
            .generate
            .output_dir
            .clone()
            .unwrap_or_else(|| self.paths.outputs.clone());
        settings_card(ui, |ui| {
            settings_card_header(
                ui,
                self.dark_mode,
                SettingsBadgeIcon::Folder,
                "Manajemen Penyimpanan & Berkas",
                "Pantau pemakaian ruang dan kelola berkas aplikasi.",
            );
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                if ui.secondary_button("Segarkan").clicked() {
                    self.refresh_storage();
                }
                if ui.secondary_button("Buka Folder Hasil").clicked() {
                    if let Err(error) = std::fs::create_dir_all(&output_dir)
                        .map_err(|e| e.to_string())
                        .and_then(|()| open_path(&output_dir).map_err(|e| e.to_string()))
                    {
                        self.error(format!("Gagal membuka folder hasil: {error}"));
                    }
                }
                if ui.secondary_button("Select Output Folder").clicked() {
                    if let Some(folder) = rfd::FileDialog::new()
                        .set_directory(&output_dir)
                        .pick_folder()
                    {
                        self.generate.output_dir = Some(folder);
                        self.storage = None;
                        self.save_settings();
                        self.refresh_storage();
                    }
                }
            });
            ui.muted_label(output_dir.display().to_string());
            ui.add_space(10.0);
            let Some(stats) = self.storage.clone() else {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.muted_label("Memuat statistik penyimpanan...");
                });
                return;
            };
            let summaries = [
                (
                    "Berkas sementara",
                    stats.temp.size_mb,
                    stats.temp.count,
                    help::TEMP,
                ),
                (
                    "Hasil render",
                    stats.outputs.size_mb,
                    stats.outputs.count,
                    help::OUTPUTS,
                ),
                (
                    "Model AI",
                    stats.models.size_mb,
                    stats.models.count,
                    help::MODELS,
                ),
            ];
            let tile = |ui: &mut egui::Ui, index: usize| {
                let (label, mb, count, tooltip) = summaries[index];
                egui::Frame::new()
                    .fill(p.bg)
                    .stroke(Stroke::new(1.0_f32, p.border))
                    .corner_radius(10)
                    .inner_margin(egui::Margin::same(14))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.muted_label(label).on_hover_text(tooltip);
                        ui.label(
                            egui::RichText::new(format!("{mb:.2} MB"))
                                .size(21.0)
                                .strong()
                                .color(p.text_strong),
                        );
                        ui.muted_label(format!("{count} berkas"));
                    });
            };
            if ui.available_width() >= 600.0 {
                ui.columns(3, |columns| {
                    for (index, column) in columns.iter_mut().enumerate() {
                        tile(column, index);
                    }
                });
            } else {
                for index in 0..3 {
                    tile(ui, index);
                    ui.add_space(6.0);
                }
            }
            ui.add_space(8.0);
            ui.strong_label(format!("Total pemakaian: {:.2} MB", stats.total_app_mb));
            if !stats.models.files.is_empty() {
                ui.muted_label(format!(
                    "Model terpasang: {}",
                    stats
                        .models
                        .files
                        .iter()
                        .map(|file| file.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        stats.temp.count > 0,
                        theme::StyledButton::danger("Hapus Berkas Sementara"),
                    )
                    .on_hover_text(help::CLEAN_TEMP)
                    .clicked()
                {
                    self.clean(CleanupOptions {
                        clean_temp: true,
                        ..CleanupOptions::default()
                    });
                }
            });
        });
    }

    fn refresh_storage(&mut self) {
        let mut paths = self.paths.clone();
        paths.outputs = self.generate.output_dir.clone().unwrap_or(paths.outputs);
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

    fn appearance_card(&mut self, ui: &mut egui::Ui) {
        let dark = self.dark_mode;
        let mut notice_msg: Option<&'static str> = None;

        settings_card(ui, |ui| {
            settings_card_header(
                ui,
                dark,
                SettingsBadgeIcon::Sun,
                "Tampilan",
                "Pilih tema aplikasi. Dark mode adalah default.",
            );
            ui.add_space(10.0);

            segment_group(ui, |ui| {
                if segment_pill_button(ui, dark, true, SegmentIcon::Moon, "Dark", dark).clicked()
                    && !self.dark_mode
                {
                    self.dark_mode = true;
                    ui.ctx().set_theme(egui::ThemePreference::Dark);
                    notice_msg = Some("Tema diubah ke Dark mode.");
                }
                if segment_pill_button(ui, !dark, true, SegmentIcon::Sun, "Light", dark).clicked()
                    && self.dark_mode
                {
                    self.dark_mode = false;
                    ui.ctx().set_theme(egui::ThemePreference::Light);
                    notice_msg = Some("Tema diubah ke Light mode.");
                }
            });
        });

        if let Some(msg) = notice_msg {
            self.save_settings();
            self.info(msg);
        }
    }

    fn video_settings_card(&mut self, ui: &mut egui::Ui) {
        let p = theme::Palette::of(ui.ctx());
        let dark = self.dark_mode;
        let settings = self.video_settings;
        let eff_codec = settings.effective_codec();
        let codec_editable = settings.target == VideoTarget::Custom;
        let mut change_notice: Option<String> = None;

        settings_card(ui, |ui| {
            settings_card_header(
                ui,
                dark,
                SettingsBadgeIcon::Folder,
                "Video",
                "Pilih target. Standar YouTube = tanpa re-encode upload.",
            );
            ui.add_space(10.0);

            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);

                // Target preset: Custom | Standar YouTube
                segment_group(ui, |ui| {
                    if segment_pill_button(
                        ui,
                        settings.target == VideoTarget::Custom,
                        true,
                        SegmentIcon::None,
                        "Custom",
                        dark,
                    )
                    .clicked()
                        && settings.target != VideoTarget::Custom
                    {
                        self.video_settings.target = VideoTarget::Custom;
                        let codec_label = if self.video_settings.codec == VideoCodec::Hevc {
                            "HEVC (kecil)"
                        } else {
                            "H.264 (kompatibel)"
                        };
                        let comp_label =
                            if self.video_settings.compression == VideoCompression::CompactSize {
                                "Hemat Ukuran"
                            } else {
                                "Kualitas Asli"
                            };
                        change_notice =
                            Some(format!("Target Custom aktif: {codec_label}, {comp_label}."));
                    }
                    if segment_pill_button(
                        ui,
                        settings.target == VideoTarget::YouTubeStandard,
                        true,
                        SegmentIcon::None,
                        "Standar YouTube",
                        dark,
                    )
                    .clicked()
                        && settings.target != VideoTarget::YouTubeStandard
                    {
                        self.video_settings.target = VideoTarget::YouTubeStandard;
                        change_notice = Some(
                            "Standar YouTube aktif: H.264 + AAC 192 k. YouTube tidak akan re-encode."
                                .to_owned(),
                        );
                    }
                });

                // Codec: HEVC (kecil) | H.264 (kompatibel)
                segment_group(ui, |ui| {
                    if segment_pill_button(
                        ui,
                        eff_codec == VideoCodec::Hevc,
                        codec_editable,
                        SegmentIcon::None,
                        "HEVC (kecil)",
                        dark,
                    )
                    .clicked()
                        && settings.codec != VideoCodec::Hevc
                    {
                        self.video_settings.codec = VideoCodec::Hevc;
                        change_notice = Some("Codec video diubah ke HEVC (kecil).".to_owned());
                    }
                    if segment_pill_button(
                        ui,
                        eff_codec == VideoCodec::H264,
                        codec_editable,
                        SegmentIcon::None,
                        "H.264 (kompatibel)",
                        dark,
                    )
                    .clicked()
                        && settings.codec != VideoCodec::H264
                    {
                        self.video_settings.codec = VideoCodec::H264;
                        change_notice =
                            Some("Codec video diubah ke H.264 (kompatibel).".to_owned());
                    }
                });

                // Compression: Kualitas Asli | Hemat Ukuran
                segment_group(ui, |ui| {
                    if segment_pill_button(
                        ui,
                        settings.compression == VideoCompression::OriginalQuality,
                        true,
                        SegmentIcon::None,
                        "Kualitas Asli",
                        dark,
                    )
                    .clicked()
                        && settings.compression != VideoCompression::OriginalQuality
                    {
                        self.video_settings.compression = VideoCompression::OriginalQuality;
                        change_notice = Some(
                            "Mode Kualitas Asli aktif: video sumber tidak dikompres ulang."
                                .to_owned(),
                        );
                    }
                    if segment_pill_button(
                        ui,
                        settings.compression == VideoCompression::CompactSize,
                        true,
                        SegmentIcon::None,
                        "Hemat Ukuran",
                        dark,
                    )
                    .clicked()
                        && settings.compression != VideoCompression::CompactSize
                    {
                        self.video_settings.compression = VideoCompression::CompactSize;
                        change_notice = Some(
                            "Mode Hemat Ukuran aktif: video dikompres agar ukuran file lebih kecil."
                                .to_owned(),
                        );
                    }
                });
            });

            ui.add_space(10.0);
            ui.muted_label(
                "Kualitas Asli: video sumber tidak dikompres ulang, warna & detail sama, file lebih besar. Hemat Ukuran: file lebih kecil, kualitas sedikit turun.",
            );

            ui.add_space(6.0);
            let link_color = if dark { p.accent_hover } else { p.accent };
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
                ui.label(
                    egui::RichText::new(
                        "Catatan: Windows Movies & TV butuh codec HEVC tambahan. Pakai H.264 jika playback gagal, atau install ",
                    )
                    .size(theme::metrics::FONT_SM)
                    .color(p.text_muted),
                );
                ui.add(egui::Hyperlink::from_label_and_url(
                    egui::RichText::new("VLC gratis")
                        .size(theme::metrics::FONT_SM)
                        .color(link_color),
                    "https://www.videolan.org/vlc/",
                ));
                ui.label(
                    egui::RichText::new(".")
                        .size(theme::metrics::FONT_SM)
                        .color(p.text_muted),
                );
            });

            if self.video_settings.target == VideoTarget::YouTubeStandard {
                ui.add_space(6.0);
                ui.muted_label(
                    "YouTube menerima: H.264, AAC/AC-3/MP3/FLAC \u{2265} 128 kbps, 720p/1080p 16:9. Output di sini dipaksa H.264 + AAC 192 k agar tidak di-re-encode.",
                );
            }
            ui.add_space(10.0);
            ui.strong_label("Kecepatan render");
            ui.horizontal_wrapped(|ui| {
                for (speed, label) in [
                    (RenderSpeed::Balanced, "Seimbang"),
                    (RenderSpeed::Fast, "Lebih Cepat"),
                ] {
                    if ui
                        .add(
                            theme::StyledButton::secondary(label)
                                .selected(self.video_settings.speed == speed),
                        )
                        .clicked()
                        && self.video_settings.speed != speed
                    {
                        self.video_settings.speed = speed;
                        change_notice = Some(format!("Kecepatan render diubah ke {label}."));
                    }
                }
            });
            ui.muted_label("Lebih Cepat memakai preset encoder yang lebih ringan. Resolusi, FPS, dan efek dipertahankan; kualitas gambar dan ukuran file dapat sedikit berbeda. Filter yang berat tetap memerlukan waktu.");
        });

        if let Some(msg) = change_notice {
            self.sync_encoder_with_video_settings();
            self.save_settings();
            self.info(msg);
        }
    }
}

#[derive(Clone, Copy)]
enum SettingsBadgeIcon {
    Sun,
    Folder,
    About,
}

#[derive(Clone, Copy)]
enum SegmentIcon {
    None,
    Moon,
    Sun,
}

fn settings_card(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    let p = theme::Palette::of(ui.ctx());
    egui::Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0_f32, p.border))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(egui::Margin::symmetric(16, 14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add_contents(ui);
        });
}

fn settings_card_header(
    ui: &mut egui::Ui,
    dark: bool,
    icon: SettingsBadgeIcon,
    title: &str,
    subtitle: &str,
) {
    let p = theme::Palette::of(ui.ctx());
    ui.horizontal(|ui| {
        let (badge_rect, _) = ui.allocate_exact_size(egui::vec2(34.0, 34.0), Sense::hover());
        let (badge_fill, badge_border, badge_fg) = if dark {
            (
                Color32::from_rgb(24, 38, 66),
                Color32::from_rgb(42, 70, 122),
                p.accent_hover,
            )
        } else {
            (
                Color32::from_rgb(239, 246, 255),
                Color32::from_rgb(191, 219, 254),
                p.accent,
            )
        };
        let painter = ui.painter();
        painter.rect(
            badge_rect,
            CornerRadius::same(9),
            badge_fill,
            Stroke::new(1.0_f32, badge_border),
            egui::StrokeKind::Inside,
        );
        match icon {
            SettingsBadgeIcon::Sun => draw_sun_icon(painter, badge_rect.center(), badge_fg, 3.0),
            SettingsBadgeIcon::Folder => draw_folder_icon(painter, badge_rect.center(), badge_fg),
            SettingsBadgeIcon::About => {
                painter.circle_filled(badge_rect.center(), 7.0, badge_fg);
                let center = badge_rect.center();
                painter.add(Shape::convex_polygon(
                    vec![
                        Pos2::new(center.x - 2.0, center.y - 3.0),
                        Pos2::new(center.x + 3.0, center.y),
                        Pos2::new(center.x - 2.0, center.y + 3.0),
                    ],
                    p.surface,
                    Stroke::NONE,
                ));
            }
        }

        ui.add_space(4.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.label(
                egui::RichText::new(title)
                    .font(FontId::new(15.0, FontFamily::Name(theme::SEMIBOLD.into())))
                    .color(p.text_strong),
            );
            ui.muted_label(subtitle);
        });
    });
}

fn about_row(ui: &mut egui::Ui, label: &str, value: impl FnOnce(&mut egui::Ui)) {
    let p = theme::Palette::of(ui.ctx());
    egui::Frame::new()
        .fill(p.bg)
        .stroke(Stroke::new(1.0_f32, p.border))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(12, 9))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.muted_label(label);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), value);
            });
        });
    ui.add_space(6.0);
}

fn segment_group(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    let p = theme::Palette::of(ui.ctx());
    egui::Frame::new()
        .fill(p.bg)
        .stroke(Stroke::new(1.0_f32, p.border))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                add_contents(ui);
            });
        });
}

fn segment_pill_button(
    ui: &mut egui::Ui,
    active: bool,
    enabled: bool,
    icon: SegmentIcon,
    label: &str,
    dark_palette: bool,
) -> egui::Response {
    let p = theme::Palette::of(ui.ctx());
    let font_id = FontId::new(
        theme::metrics::FONT_MD,
        FontFamily::Name(theme::SEMIBOLD.into()),
    );
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font_id, p.text));
    let has_icon = !matches!(icon, SegmentIcon::None);
    let icon_space = if has_icon { 14.0 + 6.0 } else { 0.0 };
    let mut width = (12.0 + icon_space + galley.size().x + 12.0).ceil();
    if has_icon {
        width = width.max(82.0);
    }
    let height = 28.0;

    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, mut response) = ui.allocate_exact_size(egui::vec2(width, height), sense);
    if enabled {
        response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    }

    let (fill, border, mut fg) = if active {
        if dark_palette {
            (
                Color32::from_rgb(24, 38, 66),
                Stroke::new(1.0_f32, p.accent),
                p.accent_hover,
            )
        } else {
            (
                Color32::from_rgb(219, 234, 254),
                Stroke::new(1.0_f32, p.accent),
                p.accent,
            )
        }
    } else if enabled && response.hovered() {
        (p.surface_alt, Stroke::NONE, p.text)
    } else {
        (Color32::TRANSPARENT, Stroke::NONE, p.text_muted)
    };
    if !enabled && !active {
        fg = fg.gamma_multiply(0.45);
    }

    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(6),
        fill,
        border,
        egui::StrokeKind::Inside,
    );

    let icon_center = egui::pos2(rect.min.x + 19.0, rect.center().y);
    match icon {
        SegmentIcon::None => {}
        SegmentIcon::Moon => draw_moon_icon(painter, icon_center, fg),
        SegmentIcon::Sun => draw_sun_icon(painter, icon_center, fg, 2.5),
    }

    let text_x = if has_icon {
        rect.min.x + 31.0
    } else {
        rect.min.x + 12.0
    };
    let text_pos = egui::pos2(text_x, rect.center().y - galley.size().y / 2.0);
    painter.galley(text_pos, galley, fg);

    response
}

fn draw_folder_icon(painter: &egui::Painter, c: Pos2, color: Color32) {
    let stroke = Stroke::new(1.3_f32, color);
    let r = Rect::from_center_size(c, egui::vec2(14.0, 11.0));
    painter.line_segment(
        [
            egui::pos2(r.min.x + 1.0, r.min.y + 3.0),
            egui::pos2(r.max.x - 1.0, r.min.y + 3.0),
        ],
        stroke,
    );
    painter.add(Shape::closed_line(
        vec![
            egui::pos2(r.min.x, r.min.y + 1.5),
            egui::pos2(r.min.x + 4.8, r.min.y + 1.5),
            egui::pos2(r.min.x + 6.4, r.min.y + 3.0),
            egui::pos2(r.max.x, r.min.y + 3.0),
            egui::pos2(r.max.x, r.max.y),
            egui::pos2(r.min.x, r.max.y),
        ],
        stroke,
    ));
}

fn draw_sun_icon(painter: &egui::Painter, c: Pos2, color: Color32, core_r: f32) {
    let stroke = Stroke::new(1.3_f32, color);
    painter.circle_stroke(c, core_r, stroke);
    let ray_in = core_r + 1.8;
    let ray_out = core_r + 3.4;
    for step in 0..8 {
        let angle = (step as f32) * (PI / 4.0);
        let (sin, cos) = angle.sin_cos();
        painter.line_segment(
            [
                egui::pos2(c.x + ray_in * cos, c.y + ray_in * sin),
                egui::pos2(c.x + ray_out * cos, c.y + ray_out * sin),
            ],
            stroke,
        );
    }
}

fn draw_moon_icon(painter: &egui::Painter, c: Pos2, color: Color32) {
    let stroke = Stroke::new(1.3_f32, color);
    let radius = 5.2_f32;
    let start = -0.35 * PI;
    let end = -1.65 * PI;
    let steps = 10;
    let mut pts: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let t = i as f32 / steps as f32;
            let a = start + (end - start) * t;
            egui::pos2(c.x + radius * a.cos(), c.y + radius * a.sin())
        })
        .collect();
    let p0 = *pts.last().unwrap_or(&c);
    let p1 = pts[0];
    let ctrl = egui::pos2(c.x - 4.4, c.y - 0.3);
    for i in 1..steps {
        let t = i as f32 / steps as f32;
        let inv = 1.0 - t;
        let x = inv * inv * p0.x + 2.0 * inv * t * ctrl.x + t * t * p1.x;
        let y = inv * inv * p0.y + 2.0 * inv * t * ctrl.y + t * t * p1.y;
        pts.push(egui::pos2(x, y));
    }
    painter.add(Shape::closed_line(pts, stroke));
}
