use std::{f32::consts::PI, path::PathBuf};

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, Sense, Shape, Stroke,
};
use loopkaraoke::{
    render::encoder::{VideoCodec, VideoCompression, VideoTarget},
    storage::{self, CleanupOptions},
    util::open_path,
};

use super::{
    help::storage as help,
    theme::{self, UiButtons, UiMetrics, UiPanels, UiText},
    Event, StudioApp, Tab,
};

impl StudioApp {
    pub(super) fn storage_tab(&mut self, ui: &mut egui::Ui) {
        if self.storage.is_none() && self.pending == 0 {
            self.refresh_storage();
        }

        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            theme::heading_with_info(ui, "Settings", help::HEADING);
            ui.muted_label("Atur tampilan dan periksa update aplikasi.");
        });
        ui.space2();

        self.appearance_card(ui);
        ui.space2();
        self.video_settings_card(ui);
        ui.space3();

        ui.muted_label(format!(
            "Loopingers Render v{} — Pengaturan aplikasi, visibilitas menu, dan manajemen penyimpanan.",
            env!("CARGO_PKG_VERSION")
        ));
        ui.space2();

        let mut menu_notice: Option<String> = None;
        ui.columns(2, |columns| {
            // Card 1: Visibilitas Menu (Semua menu kecuali Dashboard, Generate, dan Settings)
            columns[0].card(|ui| {
                ui.set_width(ui.available_width());
                ui.card_title("Visibilitas Menu");
                ui.muted_label("Pilih menu lanjutan yang ingin ditampilkan pada sidebar.");
                ui.space2();

                let mut menu_flags = [
                    (&mut self.show_tab_background, "Background"),
                    (&mut self.show_tab_playlist, "Playlist"),
                    (&mut self.show_tab_lyrics, "Lirik"),
                    (&mut self.show_tab_visualizer, "Visualizer"),
                    (&mut self.show_tab_render, "Render"),
                    (&mut self.show_tab_thumbnail, "Thumbnail"),
                ];
                for (flag, name) in &mut menu_flags {
                    if ui.checkbox(flag, *name).changed() {
                        let state = if **flag {
                            "ditampilkan"
                        } else {
                            "disembunyikan"
                        };
                        menu_notice = Some(format!("Menu sidebar \"{name}\" {state}."));
                    }
                }

                ui.space_group();
                ui.horizontal_wrapped(|ui| {
                    if ui.secondary_button("Tampilkan Semua").clicked() {
                        if menu_flags.iter().any(|(flag, _)| !**flag) {
                            menu_notice =
                                Some("Semua menu lanjutan di sidebar ditampilkan.".to_owned());
                        }
                        for (flag, _) in &mut menu_flags {
                            **flag = true;
                        }
                    }
                    if ui.secondary_button("Sembunyikan Semua").clicked() {
                        if menu_flags.iter().any(|(flag, _)| **flag) {
                            menu_notice =
                                Some("Semua menu lanjutan di sidebar disembunyikan.".to_owned());
                        }
                        for (flag, _) in &mut menu_flags {
                            **flag = false;
                        }
                    }
                });

                ui.space_group();
                ui.metadata_label(
                    "💡 Catatan: Dashboard, Generate, dan Settings selalu aktif secara bawaan.",
                );
            });

            // Card 2: Manajemen Penyimpanan & Berkas
            columns[1].card(|ui| {
                ui.set_width(ui.available_width());
                ui.card_title("Manajemen Penyimpanan & Berkas");
                ui.space1();
                ui.horizontal(|ui| {
                    if ui.secondary_button("🔄 Segarkan").clicked() {
                        self.refresh_storage();
                    }
                    if ui.secondary_button("📂 Buka Folder Hasil").clicked() {
                        let _ = open_path(&self.paths.outputs);
                    }
                });
                ui.space2();

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
                ui.strong_label(format!("Total pemakaian: {:.2} MB", stats.total_app_mb));

                ui.space2();
                ui.horizontal(|ui| {
                    if ui.danger_button("Hapus Berkas Sementara").clicked() {
                        self.clean(CleanupOptions {
                            clean_temp: true,
                            ..CleanupOptions::default()
                        });
                    }
                    theme::info(ui, help::CLEAN_TEMP);
                    if ui.danger_button("Hapus Semua Video Hasil").clicked() {
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

        if let Some(msg) = menu_notice {
            self.save_settings();
            self.info(msg);
        }

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
