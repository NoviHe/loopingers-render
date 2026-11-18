use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use eframe::egui::{
    self, Align, Color32, CornerRadius, FontFamily, FontId, Layout, Rect, Sense, Stroke, UiBuilder,
};
use loopkaraoke::media::{IMAGE_EXTENSIONS, VIDEO_EXTENSIONS};

use super::{
    generate_tab::{
        accent_badge_colors, card_header, draw_stepper_button, format_track_duration,
        generate_card, outline_icon_button, paint_vertical_divider, row_icon_button,
        show_volume_slider_popup, square_icon_button, GenerateIcon,
    },
    lyrics,
    theme::{self, UiButtons, UiText},
    StudioApp, Tab,
};

#[path = "batch_execution.rs"]
mod execution;
#[path = "batch_plan.rs"]
mod planning;
use planning::{BatchPlan, BatchSettings, PlanTrack};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BatchMediaPair {
    pub stem: String,
    pub visual_path: PathBuf,
    pub thumbnail_path: PathBuf,
}

#[derive(Default)]
pub(crate) struct BatchRenderState {
    pub visual_dir: Option<PathBuf>,
    pub thumbnail_dir: Option<PathBuf>,
    pub visual_files: Vec<PathBuf>,
    pub thumbnail_files: Vec<PathBuf>,
    pub pairs: Vec<BatchMediaPair>,
    settings: BatchSettings,
    plan: Option<BatchPlan>,
    plan_error: Option<String>,
    execution: Option<loopkaraoke::render::batch::BatchHandle>,
    execution_codes: Vec<String>,
    auto_thumbnail: bool,
}

impl BatchRenderState {
    pub fn is_running(&self) -> bool {
        self.execution
            .as_ref()
            .is_some_and(|handle| handle.is_running())
    }
    pub fn set_visual_dir(&mut self, dir: PathBuf) {
        self.plan = None;
        self.plan_error = None;
        self.visual_files = scan_dir_by_extensions(&dir, &VIDEO_EXTENSIONS);
        self.visual_dir = Some(dir);
        self.refresh_pairs();
    }

    pub fn clear_visual_dir(&mut self) {
        self.plan = None;
        self.plan_error = None;
        self.visual_dir = None;
        self.visual_files.clear();
        self.pairs.clear();
    }

    pub fn set_thumbnail_dir(&mut self, dir: PathBuf) {
        self.plan = None;
        self.plan_error = None;
        self.thumbnail_files = scan_dir_by_extensions(&dir, &IMAGE_EXTENSIONS);
        self.thumbnail_dir = Some(dir);
        self.refresh_pairs();
    }

    pub fn clear_thumbnail_dir(&mut self) {
        self.plan = None;
        self.plan_error = None;
        self.thumbnail_dir = None;
        self.thumbnail_files.clear();
        self.pairs.clear();
    }

    fn refresh_pairs(&mut self) {
        self.pairs = match_visual_thumbnail_pairs(&self.visual_files, &self.thumbnail_files);
    }
}

impl StudioApp {
    pub(super) fn batch_render_tab(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 8.0;

        theme::page_header(ui, "Batch Render", "Dari satu project (daftar lagu) → banyak video, tiap video urutan lagu, durasi, visual & thumbnail berbeda.");

        ui.add_space(4.0);
        self.batch_playlist_card(ui);
        self.batch_settings_card(ui);
    }

    fn batch_playlist_card(&mut self, ui: &mut egui::Ui) {
        let rows: Vec<(u32, String, u64)> = self
            .generate
            .tracks
            .iter()
            .map(|track| {
                (
                    track.id,
                    track.title.clone(),
                    track.duration.round().max(0.0) as u64,
                )
            })
            .collect();
        let has_tracks = !rows.is_empty();
        let total_secs = self.generate.total_duration().max(0.0).round() as u64;
        if !has_tracks {
            self.generate.volume_popup_open = false;
        }
        let was_volume_popup_open = self.generate.volume_popup_open;
        let playing_id = self.generate.playing_track_id;
        let loading_id = self.generate.loading_preview_id;

        let mut open_generate = false;
        let mut add_files = false;
        let mut add_folder = false;
        let mut copy_clicked = false;
        let mut speaker_clicked = false;
        let mut speaker_rect = None;
        let mut toggle_play_idx = None;

        generate_card(ui, |ui| {
            ui.horizontal(|ui| {
                if outline_icon_button(ui, GenerateIcon::PlusCircle, "Tambah Lagu").clicked() {
                    add_files = true;
                }
                if outline_icon_button(ui, GenerateIcon::Folder, "Tambah Folder").clicked() {
                    add_folder = true;
                }
                if outline_icon_button(ui, GenerateIcon::Play, "Buka Generate").clicked() {
                    open_generate = true;
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    if has_tracks {
                        let vol_pct = self.generate.preview_volume.round() as u32;
                        let speaker_icon = if vol_pct == 0 {
                            GenerateIcon::SpeakerMuted
                        } else {
                            GenerateIcon::Speaker
                        };
                        let speaker_tooltip = if was_volume_popup_open {
                            String::new()
                        } else {
                            format!("Atur volume pratinjau ({vol_pct}%)")
                        };
                        let speaker_resp = square_icon_button(
                            ui,
                            speaker_icon,
                            true,
                            was_volume_popup_open,
                            &speaker_tooltip,
                        );
                        speaker_rect = Some(speaker_resp.rect);
                        if speaker_resp.clicked() {
                            speaker_clicked = true;
                        }
                        if square_icon_button(
                            ui,
                            GenerateIcon::Copy,
                            true,
                            false,
                            "Salin daftar lagu",
                        )
                        .clicked()
                        {
                            copy_clicked = true;
                        }
                        album_summary_pill(ui, rows.len(), total_secs);
                    }
                });
            });

            ui.muted_label("Seret file audio atau folder ke halaman ini. Perubahan daftar dan urutan langsung berlaku di Generate dan Batch Render.");
            ui.add_space(6.0);
            draw_card_divider(ui);

            if has_tracks {
                ui.add_space(8.0);
                let p = theme::Palette::of(ui.ctx());
                egui::Frame::new()
                    .fill(p.bg)
                    .stroke(Stroke::new(1.0_f32, p.border))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(egui::Margin::ZERO)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        egui::ScrollArea::vertical()
                            .id_salt("batch_playlist_scroll")
                            .max_height(204.0)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing.y = 0.0;
                                let last_idx = rows.len().saturating_sub(1);
                                for (idx, (track_id, title, secs)) in rows.iter().enumerate() {
                                    let is_playing = playing_id == Some(*track_id)
                                        || loading_id == Some(*track_id);
                                    if synced_playlist_row(
                                        ui,
                                        idx,
                                        idx < last_idx,
                                        title,
                                        *secs,
                                        is_playing,
                                    ) {
                                        toggle_play_idx = Some(idx);
                                    }
                                }
                            });
                    });

                ui.add_space(6.0);
                ui.muted_label(
                    "Daftar lagu otomatis sinkron dengan Generate. Klik tombol play untuk pratinjau audio, atau klik Kelola di Generate untuk mengubah urutan dan daftar lagu.",
                );
            }
        });

        if open_generate {
            self.tab = Tab::Generate;
        }
        // Keep import actions available when the shared Generate playlist is
        // empty; the preview controls below are the only part that needs tracks.
        if add_files || add_folder {
            if !self.ffmpeg_ready {
                self.warn("Menunggu FFmpeg siap sebelum mengimpor lagu.");
            } else if add_files {
                self.pick_generate_audio_files();
            } else {
                self.pick_generate_audio_folders();
            }
        }
        if !has_tracks {
            return;
        }

        if speaker_clicked {
            self.generate.volume_popup_open = !self.generate.volume_popup_open;
        }
        if self.generate.volume_popup_open {
            if let Some(anchor_rect) = speaker_rect {
                let changed = show_volume_slider_popup(
                    ui,
                    anchor_rect,
                    was_volume_popup_open && !speaker_clicked,
                    &mut self.generate.volume_popup_open,
                    &mut self.generate.preview_volume,
                );
                if changed {
                    lyrics::win_audio::set_volume(self.generate.preview_volume);
                }
            }
        }
        if copy_clicked {
            let list = rows
                .iter()
                .enumerate()
                .map(|(i, (_, title, secs))| {
                    format!("{}. {} ({})", i + 1, title, format_track_duration(*secs))
                })
                .collect::<Vec<_>>()
                .join("\n");
            ui.ctx().copy_text(list);
            self.info("Daftar playlist disalin ke clipboard.");
        }
        if let Some(idx) = toggle_play_idx {
            self.toggle_generate_preview(idx);
        }
    }

    fn batch_settings_card(&mut self, ui: &mut egui::Ui) {
        let tracks: Vec<_> = self
            .generate
            .tracks
            .iter()
            .map(|track| PlanTrack {
                path: track.path.clone(),
                title: track.title.clone(),
                duration: track.duration,
            })
            .collect();
        if self.batch_render.plan.as_ref().is_some_and(|plan| {
            plan.tracks != tracks
                || plan.crossfade_secs != self.generate.crossfade_secs.clamp(0.0, 30.0)
        }) {
            self.batch_render.plan = None;
            self.batch_render.plan_error = None;
        }
        let previous_settings = self.batch_render.settings.clone();
        let mut build_plan = false;
        let mut export_plan = false;
        let mut build_and_export = false;
        let mut pick_visual_dir = false;
        let mut clear_visual_dir = false;
        let mut pick_thumbnail_dir = false;
        let mut clear_thumbnail_dir = false;
        let mut start_batch = false;
        let mut open_error = None;
        let mut pick_output = false;
        let running = self.batch_render.is_running();
        let can_start = !self.job_running() && self.ffmpeg_ready && self.hardware.is_some();

        let visual_status = self.batch_render.visual_dir.as_ref().map(|dir| {
            (
                format!(
                    "{} ({} video)",
                    dir.display(),
                    self.batch_render.visual_files.len()
                ),
                dir.display().to_string(),
            )
        });
        let thumb_status = self.batch_render.thumbnail_dir.as_ref().map(|dir| {
            let pair_note = if self.batch_render.visual_dir.is_some() {
                format!(", {} pasangan cocok", self.batch_render.pairs.len())
            } else {
                String::new()
            };
            (
                format!(
                    "{} ({} gambar{pair_note})",
                    dir.display(),
                    self.batch_render.thumbnail_files.len()
                ),
                dir.display().to_string(),
            )
        });

        generate_card(ui, |ui| {
            card_header(
                ui,
                GenerateIcon::Shuffle,
                "Pengaturan Batch",
                "Hasilkan banyak video dari satu daftar lagu. Setiap video memiliki urutan lagu, durasi, video visual, dan thumbnail yang berbeda.",
            );

            ui.add_enabled_ui(!running, |ui| {
            ui.add_space(8.0);
            draw_usage_callout(ui);

            ui.add_space(8.0);
            draw_card_divider(ui);
            ui.add_space(8.0);

            ui.muted_label("Folder video visual");
            ui.add_space(2.0);
            if folder_picker_row(
                ui,
                "Pilih Folder Visual",
                visual_status.as_ref(),
                "Hapus folder visual",
                &mut clear_visual_dir,
            ) {
                pick_visual_dir = true;
            }
            ui.add_space(2.0);
            ui.muted_label(
                "Pilih satu folder yang berisi beberapa video (mis. p-1.mp4, p-2.mp4, p-3.mp4). Setiap video dipakai untuk satu hasil.",
            );

            ui.add_space(10.0);

            ui.muted_label("Folder thumbnail");
            ui.add_space(2.0);
            if folder_picker_row(
                ui,
                "Pilih Folder Thumbnail",
                thumb_status.as_ref(),
                "Hapus folder thumbnail",
                &mut clear_thumbnail_dir,
            ) {
                pick_thumbnail_dir = true;
            }
            ui.add_space(2.0);
            thumbnail_rule_hint(ui);
            ui.horizontal_wrapped(|ui| {
                ui.toggle_value(&mut self.batch_render.auto_thumbnail, "Auto Thumbnail");
                if self.batch_render.auto_thumbnail
                    && outline_icon_button(ui, GenerateIcon::Sliders, "Thumbnail").clicked()
                {
                    self.open_generate_thumbnail_settings();
                }
            });
            ui.muted_label("Thumbnail dari folder diprioritaskan. Aktifkan Auto Thumbnail untuk membuat thumbnail bagi visual yang belum punya pasangan.");
            ui.add_space(8.0);
            ui.muted_label("Folder output");
            if outline_icon_button(ui, GenerateIcon::Folder, "Pilih Folder Output").clicked() {
                pick_output = true;
            }
            ui.muted_label(
                self.generate
                    .output_dir
                    .as_ref()
                    .unwrap_or(&self.paths.outputs)
                    .display()
                    .to_string(),
            );
            ui.muted_label(
                "Kualitas, resolusi, audio, dan variasi visual mengikuti pengaturan Generate.",
            );

            });
            ui.add_space(12.0);
            draw_card_divider(ui);
            ui.add_space(10.0);
            let settings = &mut self.batch_render.settings;
            ui.muted_label("Jumlah video");
            ui.horizontal_wrapped(|ui| {
                batch_count_stepper(ui, &mut settings.count);
                ui.muted_label("Biarkan kosong untuk mengikuti jumlah file otomatis.");
            });
            ui.add_space(8.0);
            ui.muted_label("Nama blok daftar lagu");
            batch_text_field(ui, &mut settings.block_name, "Contoh: Acakaja", 260.0);
            ui.muted_label(format!(
                "Menjadi “{} 1”, “{} 2”, dan seterusnya pada file daftar lagu.",
                settings.block_name, settings.block_name
            ));
            ui.add_space(4.0);
            if ui
                .secondary_button("⊕  Buat File Daftar Lagu (.txt)")
                .clicked()
            {
                build_and_export = true;
            }
            ui.muted_label("Buat file acakan_1-N_xx_lagu.txt berisi urutan lagu untuk setiap video berdasarkan playlist di halaman Generate. File ini bisa Anda pakai sebagai acuan saat membuat thumbnail.");
            ui.add_space(10.0);
            draw_card_divider(ui);
            ui.add_space(10.0);
            ui.muted_label("Durasi tiap video (opsional)");
            let duration_width = ((ui.available_width() - 16.0) / 2.0).clamp(80.0, 180.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                ui.vertical(|ui| {
                    ui.set_width(duration_width);
                    ui.muted_label("Minimum");
                    batch_text_field(ui, &mut settings.range_start, "40:00", duration_width);
                });
                ui.vertical(|ui| {
                    ui.set_width(duration_width);
                    ui.muted_label("Maksimum");
                    batch_text_field(ui, &mut settings.range_end, "60:00", duration_width);
                });
            });
            ui.muted_label(
                "Kosongkan semua durasi untuk mengacak jumlah lagu. Isi minimum dan maksimum untuk menyusun lagu utuh dalam rentang, dengan pengulangan jika playlist terlalu pendek.",
            );
            ui.add_space(8.0);
            ui.muted_label("Durasi manual (opsional)");
            ui.muted_label("Isi satu perkiraan durasi per video. Rencana memakai lagu utuh dalam toleransi ±30 detik dan mengutamakan isian ini di atas rentang minimum–maksimum.");
            let p = theme::Palette::of(ui.ctx());
            egui::Frame::new()
                .fill(p.bg)
                .stroke(Stroke::new(1.0_f32, p.border))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut settings.manual)
                            .hint_text(
                                egui::RichText::new("40:00\n45:30\n52:00").color(p.text_muted),
                            )
                            .frame(false)
                            .margin(egui::Margin::ZERO)
                            .desired_width(f32::INFINITY)
                            .desired_rows(4),
                    );
                });
            settings
                .range_start
                .retain(|c| c.is_ascii_digit() || c == ':');
            settings
                .range_end
                .retain(|c| c.is_ascii_digit() || c == ':');
            settings
                .manual
                .retain(|c| c.is_ascii_digit() || c == ':' || c == '\n');
            let changed = *settings != previous_settings;
            if changed {
                self.batch_render.plan = None;
                self.batch_render.plan_error = None;
            }
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                build_plan = ui
                    .add_enabled(!running, theme::StyledButton::secondary("⟳  Buat Rencana"))
                    .clicked();
                export_plan = ui
                    .add_enabled(
                        self.batch_render.plan.is_some(),
                        theme::StyledButton::secondary("▣  Export Daftar Lagu"),
                    )
                    .clicked();
                start_batch = ui
                    .add_enabled(
                        can_start && self.batch_render.plan.is_some(),
                        theme::StyledButton::primary("▷  Mulai Batch"),
                    )
                    .on_disabled_hover_text(
                        "Buat rencana dan tunggu FFmpeg, hardware, serta render lain selesai.",
                    )
                    .clicked();
                self.batch_execution_actions(ui);
            });
            self.batch_execution_progress(ui);
            if build_plan || build_and_export {
                match planning::build_plan(
                    &self.batch_render.settings,
                    &self.batch_render.visual_files,
                    &self.batch_render.thumbnail_files,
                    &tracks,
                    &mut rand::thread_rng(),
                    self.generate.crossfade_secs.clamp(0.0, 30.0),
                ) {
                    Ok(plan) => {
                        self.batch_render.plan = Some(plan);
                        self.batch_render.plan_error = None;
                    }
                    Err(error) => {
                        self.batch_render.plan = None;
                        self.batch_render.plan_error = Some(error);
                    }
                }
            }
            if let Some(error) = &self.batch_render.plan_error {
                batch_plan_panel(ui, true, |ui| {
                    ui.colored_label(theme::Palette::of(ui.ctx()).danger, error);
                });
            }
            if let Some(plan) = &self.batch_render.plan {
                ui.add_space(4.0);
                let visual_count = self.batch_render.visual_files.len();
                if plan.videos.len() > visual_count {
                    let warning = theme::Palette::of(ui.ctx()).warning;
                    batch_plan_panel(ui, false, |ui| {
                        ui.colored_label(
                            warning,
                            format!("Folder visual hanya berisi {visual_count} video. File visual dipakai ulang secara acak."),
                        );
                    });
                }
                batch_plan_panel(ui, false, |ui| {
                    ui.muted_label(format!(
                        "Rencana siap: {} video. Mulai Batch menjalankan rencana ini tanpa mengacak ulang urutan lagu.",
                        plan.videos.len()
                    ));
                });
                ui.add_space(6.0);
                let font = FontId::new(
                    theme::metrics::FONT_SM,
                    FontFamily::Name(theme::SEMIBOLD.into()),
                );
                ui.scope(|ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    let execution = self.batch_render.execution.as_ref().map(|handle| handle.snapshot());
                    for (index, video) in plan.videos.iter().enumerate() {
                        let result = execution.as_ref().and_then(|snapshot| {
                            if self.batch_render.execution_codes.get(index) == Some(&video.output_code) {
                                snapshot.finished.get(index)
                            } else {
                                None
                            }
                        });
                        batch_plan_frame(ui, false)
                            .inner_margin(egui::Margin::symmetric(12, 3))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                let visual = video
                                    .visual
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy();
                                let thumbnail = video
                                    .thumbnail
                                    .as_ref()
                                    .and_then(|path| path.file_name())
                                    .map(|name| name.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| {
                                        if self.batch_render.auto_thumbnail {
                                            "Otomatis".into()
                                        } else {
                                            "Belum cocok".into()
                                        }
                                    });
                                let seconds = video.duration;
                                let duration = if seconds < 3600 {
                                format!("{:02}:{:02}", seconds / 60, seconds % 60)
                            } else {
                                format!("{:02}:{:02}:{:02}", seconds / 3600, (seconds % 3600) / 60, seconds % 60)
                            };
                            let label = format!(
                                    "Video {} · {} · {} · Thumbnail: {}",
                                    index + 1,
                                    visual,
                                    duration,
                                    thumbnail
                                );
                                let palette = theme::Palette::of(ui.ctx());
                                ui.visuals_mut().widgets.hovered.fg_stroke.color =
                                    palette.accent_hover;
                                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                                    if let Some(path) = result.and_then(|result| result.output.as_ref()) {
                                        if let Some(folder) = path.parent() {
                                            if outline_icon_button(ui, GenerateIcon::Folder, "Folder").clicked() {
                                                if let Err(error) = loopkaraoke::util::open_path(folder) {
                                                    open_error = Some(error);
                                                }
                                            }
                                        }
                                        if outline_icon_button(ui, GenerateIcon::Play, "Putar").clicked() {
                                            if let Err(error) = loopkaraoke::util::open_path(path) {
                                                open_error = Some(error);
                                            }
                                        }
                                    }
                                    ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 0.0), Layout::top_down(Align::Min), |ui| {
                                let header = egui::CollapsingHeader::new(
                                    egui::RichText::new(label).font(font.clone()),
                                )
                                .id_salt(("batch_plan_video", index))
                                .show(ui, |ui| {
                                    let code = &video.output_code;
                                    ui.muted_label(format!(
                                        "Visual: video_{code} · Thumbnail: thumbnail_{code} · Durasi: {duration}"
                                    ));
                                    if let Some(requested) = video.requested_duration {
                                        ui.muted_label(format!(
                                            "Durasi diminta: {} · Durasi rencana: {duration}",
                                            format_track_duration(requested)
                                        ));
                                    }
                                    ui.separator();
                                    ui.label(egui::RichText::new("Urutan Lagu").strong());
                                    for (position, track_index) in video.order.iter().enumerate() {
                                        if let Some(track) = plan.tracks.get(*track_index) {
                                            let duration = format_track_duration(
                                                track.duration.round().max(0.0) as u64,
                                            );
                                            ui.label(format!(
                                                "{}. {} · {duration}",
                                                position + 1,
                                                track.title
                                            ));
                                        }
                                    }
                                });
                                header
                                    .header_response
                                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                                        if let Some(result) = result {
                                            if let Some(error) = &result.error {
                                                ui.colored_label(palette.danger, error);
                                            }
                                            if let Some(warning) = &result.warning {
                                                ui.colored_label(palette.warning, warning);
                                            }
                                        }
                                    });
                                });
                            });
                    }
                });
            }
        });

        if let Some(error) = open_error {
            self.error(format!("Gagal membuka hasil render: {error}"));
        }
        if pick_output {
            if let Some(folder) = pick_folder_dialog(self.generate.output_dir.as_deref()) {
                self.generate.output_dir = Some(folder);
            }
        }
        if start_batch {
            self.start_batch_execution();
        }

        if export_plan || build_and_export {
            if let Some(plan) = &self.batch_render.plan {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Daftar Lagu", &["txt"])
                    .set_file_name(plan.filename())
                    .save_file()
                {
                    match planning::export_plan(plan, &path) {
                        Ok(()) => self.info(format!("Daftar lagu diekspor: {}", path.display())),
                        Err(error) => self.error(format!("Gagal mengekspor daftar lagu: {error}")),
                    }
                }
            }
        }

        if pick_visual_dir {
            if let Some(folder) = pick_folder_dialog(self.batch_render.visual_dir.as_deref()) {
                self.batch_render.set_visual_dir(folder.clone());
                self.info(format!(
                    "Folder visual batch dipilih: {} ({} video).",
                    folder.display(),
                    self.batch_render.visual_files.len()
                ));
            }
        }
        if clear_visual_dir {
            self.batch_render.clear_visual_dir();
            self.info("Folder video visual batch dihapus.");
        }
        if pick_thumbnail_dir {
            if let Some(folder) = pick_folder_dialog(self.batch_render.thumbnail_dir.as_deref()) {
                self.batch_render.set_thumbnail_dir(folder.clone());
                self.info(format!(
                    "Folder thumbnail batch dipilih: {} ({} gambar, {} pasangan cocok).",
                    folder.display(),
                    self.batch_render.thumbnail_files.len(),
                    self.batch_render.pairs.len()
                ));
            }
        }
        if clear_thumbnail_dir {
            self.batch_render.clear_thumbnail_dir();
            self.info("Folder thumbnail batch dihapus.");
        }
    }
}

fn batch_plan_frame(ui: &egui::Ui, error: bool) -> egui::Frame {
    let p = theme::Palette::of(ui.ctx());
    let tint = if error { p.danger } else { p.success };
    egui::Frame::new()
        .fill(tint.gamma_multiply(0.06))
        .stroke(Stroke::new(1.0_f32, tint.gamma_multiply(0.22)))
        .corner_radius(CornerRadius::same(8))
}

fn batch_plan_panel(ui: &mut egui::Ui, error: bool, contents: impl FnOnce(&mut egui::Ui)) {
    batch_plan_frame(ui, error)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            contents(ui);
        });
}

fn pick_folder_dialog(current: Option<&Path>) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new();
    if let Some(dir) = current.filter(|p| p.exists()) {
        dialog = dialog.set_directory(dir);
    }
    dialog.pick_folder()
}

fn folder_picker_row(
    ui: &mut egui::Ui,
    button_label: &str,
    status: Option<&(String, String)>,
    clear_tooltip: &str,
    clear_clicked: &mut bool,
) -> bool {
    let mut pick_clicked = false;
    let p = theme::Palette::of(ui.ctx());
    ui.horizontal(|ui| {
        if outline_icon_button(ui, GenerateIcon::Folder, button_label).clicked() {
            pick_clicked = true;
        }
        ui.add_space(6.0);
        if let Some((summary, full_path)) = status {
            ui.scope(|ui| {
                ui.set_max_width((ui.available_width() - 36.0).max(60.0));
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(summary)
                            .size(theme::metrics::FONT_SM)
                            .color(p.text_strong),
                    )
                    .truncate(),
                )
                .on_hover_text(full_path);
            });
            ui.add_space(4.0);
            if ui.small_button("✕").on_hover_text(clear_tooltip).clicked() {
                *clear_clicked = true;
            }
        } else {
            ui.muted_label("Belum dipilih");
        }
    });
    pick_clicked
}

fn batch_text_field(ui: &mut egui::Ui, text: &mut String, hint: &str, width: f32) {
    theme::text_field(ui, text, hint, theme::InputWidth::Fixed(width));
}

fn batch_count_stepper(ui: &mut egui::Ui, count: &mut String) {
    let p = theme::Palette::of(ui.ctx());
    let width = ui.available_width().min(260.0);
    let value = count.trim().parse::<usize>().unwrap_or(0);
    let automatic = count.trim().is_empty();
    egui::Frame::new()
        .fill(p.bg)
        .stroke(Stroke::new(1.0_f32, p.border))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::ZERO)
        .show(ui, |ui| {
            ui.set_width(width);
            ui.set_height(theme::metrics::CONTROL_HEIGHT - 2.0);
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                if draw_stepper_button(ui, false, value > 0, &p, true).clicked() {
                    *count = if value <= 1 {
                        String::new()
                    } else {
                        (value - 1).to_string()
                    };
                }
                paint_vertical_divider(ui, p.border);
                ui.add_space(8.0);
                let suffix = if automatic {
                    "video (otomatis)"
                } else {
                    "video"
                };
                let suffix_width = ui.fonts(|f| {
                    f.layout_no_wrap(
                        suffix.to_owned(),
                        FontId::proportional(theme::metrics::FONT_SM),
                        p.text_muted,
                    )
                    .size()
                    .x
                });
                let edit_width = (width - 58.0 - suffix_width - 20.0).max(30.0);
                ui.add_sized(
                    [edit_width, theme::metrics::INPUT_HEIGHT - 6.0],
                    egui::TextEdit::singleline(count)
                        .hint_text("0")
                        .vertical_align(Align::Center)
                        .frame(false)
                        .margin(egui::Margin::ZERO),
                )
                .on_hover_text("Ketik jumlah video, atau klik − / +. Kosongkan untuk otomatis.");
                ui.add_space(4.0);
                ui.add_sized(
                    [suffix_width + 8.0, theme::metrics::INPUT_HEIGHT - 6.0],
                    egui::Label::new(
                        egui::RichText::new(suffix)
                            .size(theme::metrics::FONT_SM)
                            .color(p.text_muted),
                    ),
                );
                paint_vertical_divider(ui, p.border);
                count.retain(|c| c.is_ascii_digit());
                let value = count.trim().parse::<usize>().unwrap_or(0);
                if draw_stepper_button(ui, true, value < 10_000, &p, false).clicked() {
                    *count = (value + 1).to_string();
                }
            });
        });
}

fn draw_usage_callout(ui: &mut egui::Ui) {
    let p = theme::Palette::of(ui.ctx());
    let fill = if ui.ctx().theme() == egui::Theme::Dark {
        p.surface_alt.gamma_multiply(0.7)
    } else {
        p.surface_alt
    };

    let response = egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0_f32, p.border))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin {
            left: 16,
            right: 14,
            top: 11,
            bottom: 11,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.label(
                egui::RichText::new("Cara pakai (3 langkah):")
                    .font(FontId::new(
                        theme::metrics::FONT_MD,
                        FontFamily::Name(theme::SEMIBOLD.into()),
                    ))
                    .color(p.text_strong),
            );
            ui.add_space(1.0);
            callout_step(
                ui,
                "1. ",
                "Buat Rencana",
                " — aplikasi menyusun urutan lagu untuk setiap video.",
                &p,
            );
            callout_step(
                ui,
                "2. ",
                "Export Daftar Lagu",
                " — simpan daftar urutan tiap video, lalu pakai untuk membuat thumbnail (thumbnail ke-1 untuk video ke-1, dan seterusnya).",
                &p,
            );
            callout_step(
                ui,
                "3. ",
                "Mulai Batch",
                " — render seluruh rencana secara berurutan, lengkap dengan video dan thumbnail.",
                &p,
            );
        })
        .response;

    let rect = response.rect;
    let bar_rect = Rect::from_min_max(rect.min, egui::pos2(rect.min.x + 3.5, rect.max.y));
    ui.painter().rect_filled(
        bar_rect,
        CornerRadius {
            nw: 8,
            sw: 8,
            ne: 0,
            se: 0,
        },
        p.accent,
    );
}

fn callout_step(
    ui: &mut egui::Ui,
    prefix: &str,
    action: &str,
    description: &str,
    p: &theme::Palette,
) {
    let mut job = egui::text::LayoutJob::default();
    let size = theme::metrics::FONT_SM;
    append_text(&mut job, prefix, size, false, p.text_muted);
    append_text(&mut job, action, size, true, p.text_strong);
    append_text(&mut job, description, size, false, p.text_muted);
    ui.label(job);
}

fn thumbnail_rule_hint(ui: &mut egui::Ui) {
    let p = theme::Palette::of(ui.ctx());
    let mut job = egui::text::LayoutJob::default();
    let size = theme::metrics::FONT_SM;
    append_text(
        &mut job,
        "Pilih folder berisi gambar thumbnail. ",
        size,
        false,
        p.text_muted,
    );
    append_text(&mut job, "Nama file harus sama", size, true, p.text_strong);
    append_text(
        &mut job,
        " dengan video visual pasangannya — contoh: p-1.mp4 berpasangan dengan p-1.png. Rencana tetap dapat dibuat sebelum thumbnail tersedia; pasangan yang belum cocok ditandai pada rencana.",
        size,
        false,
        p.text_muted,
    );
    ui.label(job);
}

fn append_text(
    job: &mut egui::text::LayoutJob,
    text: &str,
    size: f32,
    semibold: bool,
    color: Color32,
) {
    let family = if semibold {
        FontFamily::Name(theme::SEMIBOLD.into())
    } else {
        FontFamily::Proportional
    };
    job.append(
        text,
        0.0,
        egui::TextFormat {
            font_id: FontId::new(size, family),
            color,
            ..Default::default()
        },
    );
}

fn scan_dir_by_extensions(dir: &Path, allowed_exts: &[&str]) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| {
                        allowed_exts
                            .iter()
                            .any(|allowed| ext.eq_ignore_ascii_case(allowed))
                    })
        })
        .collect();
    files.sort_by(|a, b| {
        a.file_name()
            .map(|n| n.to_string_lossy().to_ascii_lowercase())
            .cmp(
                &b.file_name()
                    .map(|n| n.to_string_lossy().to_ascii_lowercase()),
            )
    });
    files
}

fn match_visual_thumbnail_pairs(
    visuals: &[PathBuf],
    thumbnails: &[PathBuf],
) -> Vec<BatchMediaPair> {
    let mut thumb_by_stem: BTreeMap<String, PathBuf> = BTreeMap::new();
    for path in thumbnails {
        if let Some(stem) = normalized_stem(path) {
            thumb_by_stem.entry(stem).or_insert_with(|| path.clone());
        }
    }

    let mut pairs = Vec::new();
    for visual_path in visuals {
        let Some(key) = normalized_stem(visual_path) else {
            continue;
        };
        if let Some(thumbnail_path) = thumb_by_stem.get(&key) {
            let stem = visual_path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or(key);
            pairs.push(BatchMediaPair {
                stem,
                visual_path: visual_path.clone(),
                thumbnail_path: thumbnail_path.clone(),
            });
        }
    }
    pairs
}

fn normalized_stem(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
}

fn playlist_album_summary(track_count: usize, total_secs: u64) -> String {
    format!("{track_count} lagu · {}", format_track_duration(total_secs))
}

fn album_summary_pill(ui: &mut egui::Ui, track_count: usize, total_secs: u64) {
    let p = theme::Palette::of(ui.ctx());
    let (fill, border, fg) = accent_badge_colors(ui.ctx(), &p);
    let label = playlist_album_summary(track_count, total_secs);
    let font_id = FontId::new(
        theme::metrics::FONT_SM,
        FontFamily::Name(theme::SEMIBOLD.into()),
    );
    let galley = ui.fonts(|f| f.layout_no_wrap(label, font_id, fg));
    let size = egui::vec2((galley.size().x + 18.0).ceil(), 26.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());

    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(13),
        fill,
        Stroke::new(1.0_f32, border),
        egui::StrokeKind::Inside,
    );
    painter.galley(
        egui::pos2(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        fg,
    );
}

fn draw_card_divider(ui: &mut egui::Ui) {
    let p = theme::Palette::of(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), Sense::hover());
    let y = rect.center().y;
    ui.painter().line_segment(
        [egui::pos2(rect.min.x, y), egui::pos2(rect.max.x, y)],
        Stroke::new(1.0_f32, p.border.gamma_multiply(0.85)),
    );
}

fn synced_playlist_row(
    ui: &mut egui::Ui,
    idx: usize,
    draw_bottom_divider: bool,
    title: &str,
    secs: u64,
    is_playing: bool,
) -> bool {
    let p = theme::Palette::of(ui.ctx());
    let (rect, row_resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 34.0), Sense::hover());

    if ui.is_rect_visible(rect) && (is_playing || row_resp.contains_pointer()) {
        ui.painter()
            .rect_filled(rect, CornerRadius::ZERO, p.surface_alt.gamma_multiply(0.45));
    }

    if draw_bottom_divider {
        let sep_y = rect.bottom() - 0.5;
        ui.painter().line_segment(
            [egui::pos2(rect.min.x, sep_y), egui::pos2(rect.max.x, sep_y)],
            Stroke::new(1.0_f32, p.border.gamma_multiply(0.7)),
        );
    }

    let cy = rect.center().y;
    let index_galley = ui.fonts(|f| {
        f.layout_no_wrap(
            format!("{}", idx + 1),
            FontId::proportional(theme::metrics::FONT_SM),
            p.text_muted,
        )
    });
    let index_x = rect.min.x + 28.0 - index_galley.size().x;
    ui.painter().galley(
        egui::pos2(index_x, cy - index_galley.size().y / 2.0),
        index_galley,
        p.text_muted,
    );

    let play_rect = Rect::from_center_size(
        egui::pos2(rect.max.x - 12.0 - 11.0, cy),
        egui::vec2(22.0, 22.0),
    );

    let dur_galley = ui.fonts(|f| {
        f.layout_no_wrap(
            format_track_duration(secs),
            FontId::proportional(theme::metrics::FONT_SM),
            p.text_muted,
        )
    });
    let dur_x = play_rect.min.x - 14.0 - dur_galley.size().x;
    ui.painter().galley(
        egui::pos2(dur_x, cy - dur_galley.size().y / 2.0),
        dur_galley,
        p.text_muted,
    );

    let title_min_x = rect.min.x + 42.0;
    let title_max_x = (dur_x - 12.0).max(title_min_x + 40.0);
    let title_rect = Rect::from_min_max(
        egui::pos2(title_min_x, rect.min.y),
        egui::pos2(title_max_x, rect.max.y),
    );
    let mut title_ui = ui.new_child(
        UiBuilder::new()
            .id_salt(("batch_playlist_row_title", idx))
            .max_rect(title_rect)
            .layout(Layout::left_to_right(Align::Center)),
    );
    title_ui.add(
        egui::Label::new(
            egui::RichText::new(title)
                .size(theme::metrics::FONT_MD)
                .color(if is_playing {
                    p.accent_hover
                } else {
                    p.text_strong
                }),
        )
        .truncate(),
    );

    row_icon_button(
        ui,
        ("batch_row_play", idx),
        play_rect,
        if is_playing {
            GenerateIcon::Stop
        } else {
            GenerateIcon::Play
        },
        true,
        if is_playing {
            "Hentikan pratinjau"
        } else {
            "Putar pratinjau"
        },
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{match_visual_thumbnail_pairs, playlist_album_summary, BatchMediaPair};

    #[test]
    fn formats_synced_playlist_summary() {
        assert_eq!(playlist_album_summary(3, 542), "3 lagu · 9:02");
        assert_eq!(playlist_album_summary(12, 3725), "12 lagu · 1:02:05");
    }

    #[test]
    fn matches_visual_and_thumbnail_pairs_by_stem() {
        let visuals = vec![
            PathBuf::from("visuals/p-1.mp4"),
            PathBuf::from("visuals/p-2.MP4"),
            PathBuf::from("visuals/p-3.mp4"),
        ];
        let thumbs = vec![
            PathBuf::from("thumbs/P-1.png"),
            PathBuf::from("thumbs/p-3.jpg"),
            PathBuf::from("thumbs/orphan.png"),
        ];
        let pairs = match_visual_thumbnail_pairs(&visuals, &thumbs);
        assert_eq!(
            pairs,
            vec![
                BatchMediaPair {
                    stem: "p-1".to_owned(),
                    visual_path: PathBuf::from("visuals/p-1.mp4"),
                    thumbnail_path: PathBuf::from("thumbs/P-1.png"),
                },
                BatchMediaPair {
                    stem: "p-3".to_owned(),
                    visual_path: PathBuf::from("visuals/p-3.mp4"),
                    thumbnail_path: PathBuf::from("thumbs/p-3.jpg"),
                },
            ]
        );
    }
}
