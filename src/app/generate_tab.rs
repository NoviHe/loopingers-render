use std::{f32::consts::PI, path::PathBuf, time::Duration};

use eframe::egui::{
    self, Align, Color32, CornerRadius, FontFamily, FontId, Layout, Pos2, Rect, Sense, Shape,
    Stroke, Ui, UiBuilder,
};
use loopkaraoke::{
    config::GenerateAdvancedSettings,
    media::{probe_audio, probe_video, AudioInfo, VideoInfo},
    render::{
        self, build_anti_duplicate_stamp,
        encoder::VideoTarget,
        job::{JobHandle, JobStatus},
        looper::VisualVariation,
        RenderContext, SimpleRenderRequest, VisualVariationMode,
    },
    util::open_path,
};

use super::{
    background, lyrics, playlist,
    theme::{self, UiText},
    Event, StudioApp,
};

const RESOLUTION_OPTIONS: [&str; 6] = [
    "Ikut sumber",
    "640x360",
    "1280x720",
    "1920x1080",
    "2560x1440",
    "3840x2160",
];
const AUDIO_BITRATE_OPTIONS: [&str; 4] = ["128k", "192k", "256k", "320k"];
const VISUAL_VARIATION_OPTIONS: [&str; 4] =
    ["Tanpa", "Zoom pelan", "Geser pelan", "Acak tiap render"];
const VOLUME_NORM_OPTIONS: [&str; 2] = ["Aktif (rata-ratakan volume)", "Off (render lebih cepat)"];
const ANTI_DUPLICATE_OPTIONS: [&str; 2] = ["Aktif (unik tiap render)", "Off (standar)"];
const MAX_FADE_OUT_SECS: f64 = 60.0;
const MAX_CROSSFADE_SECS: f64 = 30.0;

#[derive(Clone)]
pub(crate) struct GenerateTrack {
    pub id: u32,
    pub path: PathBuf,
    pub title: String,
    pub duration: f64,
}

pub(crate) struct GenerateState {
    pub video: Option<VideoInfo>,
    pub thumbnail_path: Option<PathBuf>,
    pub tracks: Vec<GenerateTrack>,
    pub next_track_id: u32,
    pub output_dir: Option<PathBuf>,
    pub job: Option<JobHandle>,
    pub job_reported: bool,
    pub dragging_track_idx: Option<usize>,
    pub loading_preview_id: Option<u32>,
    pub playing_track_id: Option<u32>,
    pub preview_volume: f32,
    pub volume_popup_open: bool,
    pub advanced_open: bool,
    pub fade_out_secs: f64,
    pub crossfade_secs: f64,
    pub manual_loop_duration: String,
    pub resolution_idx: usize,
    pub audio_bitrate_idx: usize,
    pub visual_variation_idx: usize,
    pub volume_norm_idx: usize,
    pub anti_duplicate_idx: usize,
}

impl Default for GenerateState {
    fn default() -> Self {
        Self {
            video: None,
            thumbnail_path: None,
            tracks: Vec::new(),
            next_track_id: 1,
            output_dir: None,
            job: None,
            job_reported: false,
            dragging_track_idx: None,
            loading_preview_id: None,
            playing_track_id: None,
            preview_volume: 100.0,
            volume_popup_open: false,
            advanced_open: false,
            fade_out_secs: 0.0,
            crossfade_secs: 0.0,
            manual_loop_duration: String::new(),
            resolution_idx: 0,
            audio_bitrate_idx: 1,
            visual_variation_idx: 0,
            volume_norm_idx: 1,
            anti_duplicate_idx: 0,
        }
    }
}

impl GenerateState {
    pub fn push_audio(&mut self, info: AudioInfo) {
        let id = self.next_track_id;
        self.next_track_id += 1;
        let path = PathBuf::from(&info.path);
        let title = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .filter(|n| !n.is_empty())
            .unwrap_or(info.title);
        self.tracks.push(GenerateTrack {
            id,
            path,
            title,
            duration: info.duration,
        });
    }

    fn total_duration(&self) -> f64 {
        self.tracks.iter().map(|track| track.duration).sum()
    }

    pub fn from_settings(saved: &GenerateAdvancedSettings) -> Self {
        let fade_out_secs = parse_non_negative_secs(&saved.fade_out_secs)
            .unwrap_or(0.0)
            .clamp(0.0, MAX_FADE_OUT_SECS);
        let crossfade_secs = parse_non_negative_secs(&saved.crossfade_secs)
            .unwrap_or(0.0)
            .clamp(0.0, MAX_CROSSFADE_SECS);
        Self {
            fade_out_secs,
            crossfade_secs,
            manual_loop_duration: saved.manual_loop_duration.clone(),
            resolution_idx: saved.resolution_idx.min(RESOLUTION_OPTIONS.len() - 1),
            audio_bitrate_idx: saved.audio_bitrate_idx.min(AUDIO_BITRATE_OPTIONS.len() - 1),
            visual_variation_idx: saved
                .visual_variation_idx
                .min(VISUAL_VARIATION_OPTIONS.len() - 1),
            volume_norm_idx: saved.volume_norm_idx.min(VOLUME_NORM_OPTIONS.len() - 1),
            anti_duplicate_idx: if saved.anti_duplicate { 0 } else { 1 },
            ..Self::default()
        }
    }

    pub fn to_settings(&self) -> GenerateAdvancedSettings {
        GenerateAdvancedSettings {
            fade_out_secs: format_secs_setting(self.fade_out_secs),
            crossfade_secs: format_secs_setting(self.crossfade_secs),
            manual_loop_duration: self.manual_loop_duration.clone(),
            resolution_idx: self.resolution_idx,
            audio_bitrate_idx: self.audio_bitrate_idx,
            visual_variation_idx: self.visual_variation_idx,
            volume_norm_idx: self.volume_norm_idx,
            anti_duplicate: self.anti_duplicate_idx == 0,
        }
    }

    fn reset_advanced(&mut self) {
        self.fade_out_secs = 0.0;
        self.crossfade_secs = 0.0;
        self.manual_loop_duration.clear();
        self.resolution_idx = 0;
        self.audio_bitrate_idx = 1;
        self.visual_variation_idx = 0;
        self.volume_norm_idx = 1;
        self.anti_duplicate_idx = 0;
    }

    fn summary_text(&self, yt_standard: bool) -> String {
        let mut parts = Vec::new();
        let res = if self.resolution_idx == 0 {
            if yt_standard {
                "720p/1080p (Standar YT)"
            } else {
                "ikut sumber"
            }
        } else if yt_standard && !matches!(self.resolution_idx, 2 | 3) {
            "1920x1080 (Standar YT)"
        } else {
            RESOLUTION_OPTIONS[self.resolution_idx]
        };
        parts.push(format!("Res: {res}"));
        let bitrate = if yt_standard {
            "192k (Standar YT)"
        } else {
            AUDIO_BITRATE_OPTIONS[self.audio_bitrate_idx]
        };
        parts.push(format!("Audio {bitrate}"));
        if self.visual_variation_idx != 0 {
            parts.push(format!(
                "Visual: {}",
                VISUAL_VARIATION_OPTIONS[self.visual_variation_idx]
            ));
        }
        if self.anti_duplicate_idx == 0 {
            parts.push("Anti-duplikat".to_owned());
        }
        let norm = if self.volume_norm_idx == 0 {
            "Normalisasi aktif"
        } else {
            "Tanpa normalisasi"
        };
        parts.push(norm.to_owned());
        if self.crossfade_secs > 0.005 {
            parts.push(format!(
                "Cross-fade {}s",
                format_secs_setting(self.crossfade_secs)
            ));
        }
        if self.fade_out_secs > 0.005 {
            parts.push(format!("Fade {}s", format_secs_setting(self.fade_out_secs)));
        }
        if !self.manual_loop_duration.trim().is_empty() {
            parts.push(format!("Loop {}", self.manual_loop_duration.trim()));
        }
        parts.join(" \u{00B7} ")
    }
}

#[derive(Clone, Copy)]
enum GenerateIcon {
    Upload,
    Folder,
    Music,
    Shuffle,
    Copy,
    Speaker,
    SpeakerMuted,
    Trash,
    Play,
    Stop,
    ArrowUp,
    ArrowDown,
    Sliders,
    ChevronUp,
    ChevronDown,
    Reset,
}

enum PlaylistRowAction {
    TogglePlay(usize),
    MoveUp(usize),
    MoveDown(usize),
    Remove(usize),
}

impl StudioApp {
    pub(super) fn generate_tab(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 8.0;

        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.page_title("Generate");
            ui.muted_label(
                "Gabungkan video pendekmu dengan daftar lagu mu menjadi satu file video.",
            );
        });

        self.generate_source_card(ui);
        self.generate_playlist_card(ui);
        self.generate_advanced_card(ui);
        self.generate_render_card(ui);
    }

    fn generate_source_card(&mut self, ui: &mut egui::Ui) {
        let mut pick_video = false;
        let mut pick_thumbnail = false;
        let mut clear_thumbnail = false;
        let mut pick_folders = false;
        let mut pick_files = false;
        let mut pick_output = false;

        let video_status = self.generate.video.as_ref().map(|video| {
            let dur = format_track_duration(video.duration.max(0.0).round() as u64);
            (
                format!(
                    "{} ({}x{}, {dur})",
                    video.filename, video.width, video.height
                ),
                video.path.clone(),
            )
        });
        let thumbnail_info = self.generate.thumbnail_path.as_ref().map(|path| {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            (name, path.display().to_string())
        });
        let effective_output_dir = self
            .generate
            .output_dir
            .as_ref()
            .unwrap_or(&self.paths.outputs);
        let output_dir_text = effective_output_dir.display().to_string();

        generate_card(ui, |ui| {
            card_header(
                ui,
                GenerateIcon::Music,
                "Sumber & Output",
                "Pilih materi yang akan digabung menjadi album.",
            );

            ui.add_space(8.0);

            // Section 1: Video visual (seamless loop)
            ui.muted_label("Video visual (seamless loop)");
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                if outline_icon_button(ui, GenerateIcon::Upload, "Pilih Video").clicked() {
                    pick_video = true;
                }
                ui.add_space(6.0);
                if let Some((summary, full_path)) = &video_status {
                    let p = theme::Palette::of(ui.ctx());
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(summary)
                                .size(theme::metrics::FONT_SM)
                                .color(p.text_strong),
                        )
                        .truncate(),
                    )
                    .on_hover_text(full_path);
                } else {
                    let p = theme::Palette::of(ui.ctx());
                    ui.label(
                        egui::RichText::new("Belum dipilih")
                            .size(theme::metrics::FONT_SM)
                            .color(p.text),
                    );
                }
            });

            ui.add_space(6.0);

            // Section 2: Thumbnail (opsional, disalin ke folder output tiap render)
            ui.muted_label("Thumbnail (opsional, disalin ke folder output tiap render)");
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                if outline_icon_button(ui, GenerateIcon::Upload, "Pilih Thumbnail").clicked() {
                    pick_thumbnail = true;
                }
                ui.add_space(6.0);
                if let Some((name, full_path)) = &thumbnail_info {
                    let p = theme::Palette::of(ui.ctx());
                    ui.scope(|ui| {
                        ui.set_max_width((ui.available_width() - 36.0).max(60.0));
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(name)
                                    .size(theme::metrics::FONT_SM)
                                    .color(p.text_strong),
                            )
                            .truncate(),
                        )
                        .on_hover_text(full_path);
                    });
                    ui.add_space(4.0);
                    if ui
                        .small_button("✕")
                        .on_hover_text("Hapus thumbnail")
                        .clicked()
                    {
                        clear_thumbnail = true;
                    }
                } else {
                    ui.muted_label("Belum dipilih");
                }
            });

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(4.0);

            // Section 3: Lagu album (folder, beberapa folder, atau file sekaligus)
            ui.muted_label("Lagu album (folder, beberapa folder, atau file sekaligus)");
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                if outline_icon_button(ui, GenerateIcon::Folder, "+ Folder Lagu").clicked() {
                    pick_folders = true;
                }
                ui.add_space(2.0);
                if outline_icon_button(ui, GenerateIcon::Music, "+ File Lagu").clicked() {
                    pick_files = true;
                }
                ui.add_space(6.0);
                ui.muted_label("mp3 · wav · m4a · flac · ogg · opus, dll.");
            });

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(4.0);

            // Section 4: Folder output
            ui.muted_label("Folder output");
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                if outline_icon_button(ui, GenerateIcon::Folder, "Pilih Folder").clicked() {
                    pick_output = true;
                }
                ui.add_space(6.0);
                ui.muted_label(&output_dir_text)
                    .on_hover_text(&output_dir_text);
            });
        });

        if pick_video {
            if !self.ffmpeg_ready {
                self.warn("Menunggu FFmpeg siap sebelum mengimpor video.");
            } else if let Some(path) = rfd::FileDialog::new()
                .add_filter("Video", &background::VIDEO_EXTENSIONS)
                .pick_file()
            {
                self.load_generate_video(path);
            }
        }
        if pick_thumbnail {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Gambar", &background::IMAGE_EXTENSIONS)
                .pick_file()
            {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string());
                self.generate.thumbnail_path = Some(path);
                self.info(format!("Thumbnail dipilih: {name}"));
            }
        }
        if clear_thumbnail {
            self.generate.thumbnail_path = None;
            self.info("Thumbnail dihapus.");
        }
        if pick_folders {
            if !self.ffmpeg_ready {
                self.warn("Menunggu FFmpeg siap sebelum mengimpor folder lagu.");
            } else {
                self.pick_generate_audio_folders();
            }
        }
        if pick_files {
            if !self.ffmpeg_ready {
                self.warn("Menunggu FFmpeg siap sebelum mengimpor file lagu.");
            } else {
                self.pick_generate_audio_files();
            }
        }
        if pick_output {
            let current = self
                .generate
                .output_dir
                .as_ref()
                .unwrap_or(&self.paths.outputs);
            let mut dialog = rfd::FileDialog::new();
            if current.exists() {
                dialog = dialog.set_directory(current);
            }
            if let Some(folder) = dialog.pick_folder() {
                self.info(format!("Folder output Generate: {}", folder.display()));
                self.generate.output_dir = Some(folder);
            }
        }
    }

    fn generate_playlist_card(&mut self, ui: &mut egui::Ui) {
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
        let track_count = rows.len();
        let total_secs = self.generate.total_duration().max(0.0).round() as u64;
        let subtitle = format!(
            "({track_count} lagu, {})",
            format_track_duration(total_secs)
        );

        let mut shuffle_clicked = false;
        let mut speaker_clicked = false;
        let mut copy_clicked = false;
        let mut clear_clicked = false;
        let mut speaker_rect = None;
        if !has_tracks {
            self.generate.volume_popup_open = false;
        }
        let was_volume_popup_open = self.generate.volume_popup_open;
        let mut row_action = None;
        let mut drag_started = None;
        let mut drop_target = None;
        let dragging_idx = self.generate.dragging_track_idx;
        let playing_id = self.generate.playing_track_id;
        let loading_id = self.generate.loading_preview_id;

        if dragging_idx.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        }

        generate_card(ui, |ui| {
            // Card Header: Badge + Title/Subtitle + Right Icon Toolbar
            ui.horizontal(|ui| {
                card_header(ui, GenerateIcon::Music, "Playlist Album", &subtitle);

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    if square_icon_button(
                        ui,
                        GenerateIcon::Trash,
                        has_tracks,
                        false,
                        "Hapus semua lagu",
                    )
                    .clicked()
                    {
                        clear_clicked = true;
                    }
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
                        has_tracks,
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
                        has_tracks,
                        false,
                        "Salin daftar lagu",
                    )
                    .clicked()
                    {
                        copy_clicked = true;
                    }
                    if square_icon_button(
                        ui,
                        GenerateIcon::Shuffle,
                        track_count > 1,
                        false,
                        "Acak urutan lagu",
                    )
                    .clicked()
                    {
                        shuffle_clicked = true;
                    }
                });
            });

            ui.add_space(8.0);

            // Scrollable track table container
            let p = theme::Palette::of(ui.ctx());
            egui::Frame::new()
                .fill(p.bg)
                .stroke(Stroke::new(1.0_f32, p.border))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(egui::Margin::ZERO)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    egui::ScrollArea::vertical()
                        .id_salt("generate_playlist_scroll")
                        .min_scrolled_height(170.0)
                        .max_height(170.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if rows.is_empty() {
                                ui.allocate_ui_with_layout(
                                    egui::vec2(ui.available_width(), 170.0),
                                    Layout::centered_and_justified(egui::Direction::TopDown),
                                    |ui| {
                                        ui.muted_label("Belum ada lagu.");
                                    },
                                );
                            } else {
                                ui.spacing_mut().item_spacing.y = 0.0;
                                let last_idx = rows.len().saturating_sub(1);
                                for (idx, (track_id, name, secs)) in rows.iter().enumerate() {
                                    let is_playing = playing_id == Some(*track_id)
                                        || loading_id == Some(*track_id);
                                    let (action, started_this, hovered_this) = playlist_row(
                                        ui,
                                        idx,
                                        last_idx,
                                        name,
                                        *secs,
                                        is_playing,
                                        dragging_idx,
                                    );
                                    if action.is_some() {
                                        row_action = action;
                                    }
                                    if started_this {
                                        drag_started = Some(idx);
                                    }
                                    if hovered_this {
                                        drop_target = Some(idx);
                                    }
                                }
                            }
                        });
                });

            ui.add_space(6.0);
            ui.muted_label(
                "Urutan daftar = urutan lagu di album. Seret baris untuk menyusun ulang, atau pakai tombol naik/turun. Klik tombol play di tiap lagu untuk memutar review per lagu.",
            );
        });

        if let Some(src) = drag_started {
            self.generate.dragging_track_idx = Some(src);
        }
        if self.generate.dragging_track_idx.is_some() && ui.input(|i| i.pointer.any_released()) {
            if let (Some(src), Some(dst)) = (self.generate.dragging_track_idx.take(), drop_target) {
                if src != dst
                    && src < self.generate.tracks.len()
                    && dst < self.generate.tracks.len()
                {
                    let moved = self.generate.tracks.remove(src);
                    self.generate.tracks.insert(dst, moved);
                }
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

        if shuffle_clicked {
            self.shuffle_generate_tracks();
        }
        if copy_clicked {
            let list = rows
                .iter()
                .enumerate()
                .map(|(i, (_, name, secs))| {
                    format!("{}. {} ({})", i + 1, name, format_track_duration(*secs))
                })
                .collect::<Vec<_>>()
                .join("\n");
            ui.ctx().copy_text(list);
            self.info("Daftar playlist disalin ke clipboard.");
        }
        if clear_clicked {
            self.stop_generate_preview();
            self.generate.tracks.clear();
            self.generate.dragging_track_idx = None;
            self.generate.volume_popup_open = false;
            self.info("Seluruh lagu di playlist dihapus.");
        }

        match row_action {
            Some(PlaylistRowAction::TogglePlay(idx)) => self.toggle_generate_preview(idx),
            Some(PlaylistRowAction::MoveUp(idx)) if idx > 0 && idx < self.generate.tracks.len() => {
                self.generate.tracks.swap(idx, idx - 1);
            }
            Some(PlaylistRowAction::MoveDown(idx)) if idx + 1 < self.generate.tracks.len() => {
                self.generate.tracks.swap(idx, idx + 1);
            }
            Some(PlaylistRowAction::Remove(idx)) if idx < self.generate.tracks.len() => {
                let removed_id = self.generate.tracks[idx].id;
                if self.generate.playing_track_id == Some(removed_id)
                    || self.generate.loading_preview_id == Some(removed_id)
                {
                    self.stop_generate_preview();
                }
                let title = self.generate.tracks[idx].title.clone();
                self.generate.tracks.remove(idx);
                if self.generate.tracks.is_empty() {
                    self.generate.volume_popup_open = false;
                }
                self.info(format!("Lagu \"{title}\" dihapus dari playlist."));
            }
            _ => {}
        }
    }

    fn stop_generate_preview(&mut self) {
        self.generate.loading_preview_id = None;
        self.generate.playing_track_id = None;
        lyrics::win_audio::stop();
    }

    fn toggle_generate_preview(&mut self, index: usize) {
        let Some(track) = self.generate.tracks.get(index) else {
            return;
        };
        let track_id = track.id;
        if self.generate.playing_track_id == Some(track_id)
            || self.generate.loading_preview_id == Some(track_id)
        {
            self.stop_generate_preview();
            return;
        }
        if !self.ffmpeg_ready {
            self.warn("Menunggu FFmpeg siap untuk memutar pratinjau lagu.");
            return;
        }

        let source = track.path.clone();
        let tools = self.tools.clone();
        let wav_out = self.paths.temp.join(format!("gen_preview_{track_id}.wav"));

        self.stop_generate_preview();
        self.generate.loading_preview_id = Some(track_id);

        self.spawn(async move {
            let result = if wav_out.is_file() {
                Ok(wav_out)
            } else {
                if let Some(parent) = wav_out.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let mut cmd = tools.ffmpeg();
                cmd.args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
                    .arg(&source)
                    .args(["-vn", "-ac", "2", "-ar", "44100", "-acodec", "pcm_s16le"])
                    .arg(&wav_out);
                loopkaraoke::ffmpeg::run(&mut cmd, "FFmpeg (pratinjau audio)")
                    .await
                    .map(|_| wav_out)
            };
            Event::GeneratePreview { track_id, result }
        });
    }

    fn generate_advanced_card(&mut self, ui: &mut egui::Ui) {
        let open = self.generate.advanced_open;
        let yt_standard = self.video_settings.target == VideoTarget::YouTubeStandard;
        let summary = self.generate.summary_text(yt_standard);
        let p = theme::Palette::of(ui.ctx());

        egui::Frame::new()
            .fill(p.surface)
            .stroke(Stroke::new(1.0_f32, p.border))
            .corner_radius(CornerRadius::same(12))
            .inner_margin(egui::Margin::ZERO)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());

                // Full-width clickable collapsible header strip
                let header_w = ui.available_width();
                let (header_rect, header_resp) =
                    ui.allocate_exact_size(egui::vec2(header_w, 38.0), Sense::click());
                let header_resp = header_resp.on_hover_cursor(egui::CursorIcon::PointingHand);

                let header_fill = if header_resp.hovered() {
                    p.surface_alt
                } else if open {
                    p.elevated
                } else {
                    p.surface
                };
                let header_radius = CornerRadius {
                    nw: 12,
                    ne: 12,
                    sw: if open { 0 } else { 12 },
                    se: if open { 0 } else { 12 },
                };
                let cy = header_rect.center().y;
                let title_galley = ui.fonts(|f| {
                    f.layout_no_wrap(
                        "Opsi Lanjutan".to_owned(),
                        FontId::new(14.5, FontFamily::Name(theme::SEMIBOLD.into())),
                        p.text_strong,
                    )
                });
                let title_x = header_rect.min.x + 42.0;
                let summary_x = title_x + title_galley.size().x + 12.0;
                {
                    let painter = ui.painter();
                    painter.rect_filled(header_rect, header_radius, header_fill);

                    if open {
                        let sep_y = header_rect.bottom() - 0.5;
                        painter.line_segment(
                            [
                                egui::pos2(header_rect.min.x, sep_y),
                                egui::pos2(header_rect.max.x, sep_y),
                            ],
                            Stroke::new(1.0_f32, p.border),
                        );
                    }

                    // Header left: sliders icon + title
                    draw_generate_icon(
                        painter,
                        egui::pos2(header_rect.min.x + 24.0, cy),
                        GenerateIcon::Sliders,
                        p.accent_hover,
                    );
                    painter.galley(
                        egui::pos2(title_x, cy - title_galley.size().y / 2.0),
                        title_galley,
                        p.text_strong,
                    );
                }

                let chevron_x = header_rect.max.x - 24.0;
                let summary_max_w = (chevron_x - 16.0 - summary_x).max(0.0);
                if summary_max_w > 24.0 {
                    let summary_rect = Rect::from_min_max(
                        egui::pos2(summary_x, header_rect.min.y),
                        egui::pos2(chevron_x - 16.0, header_rect.max.y),
                    );
                    let mut summary_ui = ui.new_child(
                        UiBuilder::new()
                            .id_salt("gen_adv_summary")
                            .max_rect(summary_rect)
                            .layout(Layout::left_to_right(Align::Center)),
                    );
                    summary_ui.add(
                        egui::Label::new(
                            egui::RichText::new(summary)
                                .size(theme::metrics::FONT_SM)
                                .color(p.text_muted),
                        )
                        .truncate(),
                    );
                }

                // Header right: collapse/expand chevron
                let chevron_icon = if open {
                    GenerateIcon::ChevronUp
                } else {
                    GenerateIcon::ChevronDown
                };
                let chevron_color = if header_resp.hovered() {
                    p.text_strong
                } else {
                    p.text_muted
                };
                draw_generate_icon(
                    ui.painter(),
                    egui::pos2(chevron_x, cy),
                    chevron_icon,
                    chevron_color,
                );

                if header_resp.clicked() {
                    self.generate.advanced_open = !self.generate.advanced_open;
                }

                if !self.generate.advanced_open {
                    return;
                }

                egui::Frame::NONE
                    .inner_margin(egui::Margin {
                        left: 16,
                        right: 16,
                        top: 10,
                        bottom: 12,
                    })
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.spacing_mut().item_spacing.y = 4.0;
                        let mut changed_adv_text = false;
                        let mut combo_notice: Option<String> = None;
                        let mut reset_clicked = false;
                        ui.scope(|ui| {
                            let s = ui.style_mut();
                            s.spacing.interact_size.y = 30.0;
                            let r = CornerRadius::same(8);
                            for state in [
                                &mut s.visuals.widgets.inactive,
                                &mut s.visuals.widgets.hovered,
                                &mut s.visuals.widgets.active,
                                &mut s.visuals.widgets.open,
                            ] {
                                state.corner_radius = r;
                            }
                            s.visuals.widgets.inactive.bg_fill = p.bg;
                            s.visuals.widgets.inactive.weak_bg_fill = p.bg;
                            s.visuals.extreme_bg_color = p.bg;

                            // Row 1: Fade out, Cross-fade, Durasi loop manual
                            let mut fade_event = StepperChange::none();
                            let mut cf_event = StepperChange::none();
                            ui.columns(3, |cols| {
                                fade_event = labeled_seconds_stepper(
                                    &mut cols[0],
                                    "gen_adv_fade",
                                    "Fade out",
                                    "Mengecilkan suara perlahan hingga habis di akhir video (0 detik = mati).",
                                    &mut self.generate.fade_out_secs,
                                    MAX_FADE_OUT_SECS,
                                );
                                cf_event = labeled_seconds_stepper(
                                    &mut cols[1],
                                    "gen_adv_crossfade",
                                    "Cross-fade antar lagu",
                                    "Mencampur akhir lagu sebelumnya ke awal lagu berikutnya agar nyambung halus (0 detik = mati).",
                                    &mut self.generate.crossfade_secs,
                                    MAX_CROSSFADE_SECS,
                                );
                                changed_adv_text |= labeled_duration_field(
                                    &mut cols[2],
                                    "Durasi loop manual",
                                    "Ulang video dan audio sampai durasi yang diisi (HH:MM:SS). Kosongkan untuk mengikuti total lagu.",
                                    &mut self.generate.manual_loop_duration,
                                    "cth: 01:02:00",
                                );
                            });
                            if fade_event.save || cf_event.save {
                                changed_adv_text = true;
                            }
                            if fade_event.commit {
                                let label = if self.generate.fade_out_secs <= 0.005 {
                                    "mati".to_owned()
                                } else {
                                    format!("{} detik", format_secs_setting(self.generate.fade_out_secs))
                                };
                                combo_notice = Some(format!("Fade out Generate diubah ke {label}."));
                            } else if cf_event.commit {
                                let label = if self.generate.crossfade_secs <= 0.005 {
                                    "mati".to_owned()
                                } else {
                                    format!("{} detik", format_secs_setting(self.generate.crossfade_secs))
                                };
                                combo_notice =
                                    Some(format!("Cross-fade antar lagu diubah ke {label}."));
                            }

                            ui.add_space(6.0);

                            let prev_res = self.generate.resolution_idx;
                            let prev_bitrate = self.generate.audio_bitrate_idx;
                            let prev_visual = self.generate.visual_variation_idx;
                            let prev_norm = self.generate.volume_norm_idx;
                            let prev_antidup = self.generate.anti_duplicate_idx;

                            // Row 2: Resolusi, Bitrate audio, Variasi visual
                            ui.columns(3, |cols| {
                                labeled_combo(
                                    &mut cols[0],
                                    "gen_adv_res",
                                    "Resolusi",
                                    &mut self.generate.resolution_idx,
                                    &RESOLUTION_OPTIONS,
                                );
                                if yt_standard {
                                    cols[1].muted_label("Bitrate audio (Standar YT)");
                                    cols[1].add_space(2.0);
                                    let combo_w =
                                        (cols[1].available_width() - 12.0).clamp(140.0, 260.0);
                                    cols[1].add_enabled_ui(false, |ui| {
                                        egui::ComboBox::from_id_salt("gen_adv_bitrate_locked")
                                            .width(combo_w)
                                            .selected_text("192k (Standar YT)")
                                            .show_ui(ui, |_| {});
                                    });
                                } else {
                                    labeled_combo(
                                        &mut cols[1],
                                        "gen_adv_bitrate",
                                        "Bitrate audio",
                                        &mut self.generate.audio_bitrate_idx,
                                        &AUDIO_BITRATE_OPTIONS,
                                    );
                                }
                                labeled_combo(
                                    &mut cols[2],
                                    "gen_adv_visual",
                                    "Variasi visual",
                                    &mut self.generate.visual_variation_idx,
                                    &VISUAL_VARIATION_OPTIONS,
                                );
                            });

                            ui.add_space(6.0);

                            // Row 3: Normalisasi volume & Anti-duplikat
                            ui.columns(3, |cols| {
                                labeled_combo(
                                    &mut cols[0],
                                    "gen_adv_norm",
                                    "Normalisasi volume",
                                    &mut self.generate.volume_norm_idx,
                                    &VOLUME_NORM_OPTIONS,
                                );
                                labeled_combo(
                                    &mut cols[1],
                                    "gen_adv_antidup",
                                    "Anti-duplikat",
                                    &mut self.generate.anti_duplicate_idx,
                                    &ANTI_DUPLICATE_OPTIONS,
                                );
                            });

                            if self.generate.resolution_idx != prev_res {
                                combo_notice = Some(format!(
                                    "Resolusi Generate diubah ke {}.",
                                    RESOLUTION_OPTIONS[self.generate.resolution_idx]
                                ));
                            } else if self.generate.audio_bitrate_idx != prev_bitrate {
                                combo_notice = Some(format!(
                                    "Bitrate audio Generate diubah ke {}.",
                                    AUDIO_BITRATE_OPTIONS[self.generate.audio_bitrate_idx]
                                ));
                            } else if self.generate.visual_variation_idx != prev_visual {
                                combo_notice = Some(format!(
                                    "Variasi visual Generate diubah ke {}.",
                                    VISUAL_VARIATION_OPTIONS[self.generate.visual_variation_idx]
                                ));
                            } else if self.generate.volume_norm_idx != prev_norm {
                                combo_notice = Some(format!(
                                    "Normalisasi volume Generate diubah ke {}.",
                                    VOLUME_NORM_OPTIONS[self.generate.volume_norm_idx]
                                ));
                            } else if self.generate.anti_duplicate_idx != prev_antidup {
                                combo_notice = Some(format!(
                                    "Anti-duplikat Generate diubah ke {}.",
                                    ANTI_DUPLICATE_OPTIONS[self.generate.anti_duplicate_idx]
                                ));
                            }
                        });

                        if yt_standard {
                            let info_color = if ui.ctx().theme() == egui::Theme::Dark {
                                p.accent_hover
                            } else {
                                p.accent
                            };
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(
                                    "Standar YouTube aktif di Settings: Audio dikunci ke 192k, resolusi selain 1280x720 disesuaikan ke 1920x1080 16:9.",
                                )
                                .size(theme::metrics::FONT_SM)
                                .color(info_color),
                            );
                        }

                        ui.add_space(6.0);
                        ui.muted_label(
                            "Resolusi dan variasi visual hanya dipakai saat video di-re-encode. Variasi visual (zoom/geser pelan, fase acak) membuat tiap render unik secara visual — anti template.",
                        );
                        ui.add_space(2.0);

                        let mut note_job = egui::text::LayoutJob::default();
                        note_job.append(
                            "Durasi loop manual",
                            0.0,
                            egui::TextFormat {
                                font_id: FontId::new(
                                    theme::metrics::FONT_SM,
                                    FontFamily::Name(theme::SEMIBOLD.into()),
                                ),
                                color: p.text_strong,
                                ..Default::default()
                            },
                        );
                        note_job.append(
                            " mengulang video + audio sampai durasi yang diisi (mis. video 2 menit + lagu 4 menit, isi 01:02:00 \u{2192} hasil 1 jam 2 menit); kosong = ikut total durasi lagu.",
                            0.0,
                            egui::TextFormat {
                                font_id: FontId::proportional(theme::metrics::FONT_SM),
                                color: p.text_muted,
                                ..Default::default()
                            },
                        );
                        ui.label(note_job);

                        ui.add_space(6.0);
                        if danger_outline_icon_button(ui, GenerateIcon::Reset, "Reset Pengaturan")
                            .clicked()
                        {
                            self.generate.reset_advanced();
                            reset_clicked = true;
                        }

                        if let Some(msg) = combo_notice {
                            self.save_settings();
                            self.info(msg);
                        } else if reset_clicked {
                            self.save_settings();
                            self.info("Pengaturan Opsi Lanjutan dikembalikan ke bawaan.");
                        } else if changed_adv_text {
                            self.save_settings();
                        }
                    });
            });
    }

    fn generate_render_card(&mut self, ui: &mut egui::Ui) {
        let snap = self.generate.job.as_ref().map(|job| job.snapshot());
        let generate_running = snap.as_ref().is_some_and(|s| s.is_running());
        if generate_running {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }

        if let Some(snap) = &snap {
            if !self.generate.job_reported {
                match snap.status {
                    JobStatus::Completed => {
                        self.generate.job_reported = true;
                        let toast_msg = snap.output_file.as_ref().map_or_else(
                            || "Render Generate selesai.".to_owned(),
                            |p| format!("Render Generate selesai: {}", p.display()),
                        );
                        let mut detail_parts = Vec::new();
                        if let Some(p) = &snap.output_file {
                            detail_parts.push(format!("Path: {}", p.display()));
                        }
                        if let Some(comment) = &snap.metadata_comment {
                            detail_parts.push(format!("comment: {comment}"));
                        }
                        if let Some(visual) = &snap.visual_label {
                            detail_parts.push(format!("Variasi: {visual}"));
                        }
                        let log_msg = if detail_parts.is_empty() {
                            toast_msg.clone()
                        } else {
                            format!("Render Generate selesai ({})", detail_parts.join(" • "))
                        };
                        self.notify_success(log_msg, toast_msg);
                    }
                    JobStatus::Failed => {
                        self.generate.job_reported = true;
                        let err = if snap.error_message.is_empty() {
                            "Render Generate gagal."
                        } else {
                            &snap.error_message
                        };
                        self.error(format!("Render Generate gagal: {err}"));
                    }
                    _ => {}
                }
            }
        }

        let running = generate_running || self.job_running();
        let progress = snap
            .as_ref()
            .map(|s| match s.status {
                JobStatus::Completed => 1.0,
                JobStatus::Failed => 0.0,
                _ => (s.progress_percent / 100.0).clamp(0.0, 1.0) as f32,
            })
            .unwrap_or(0.0);

        let mut render_clicked = false;
        let mut open_result_folder: Option<PathBuf> = None;
        generate_card(ui, |ui| {
            plain_progress_bar(ui, progress);
            ui.add_space(4.0);
            if primary_render_button(ui, "Mulai Render", !running).clicked() {
                render_clicked = true;
            }
            if let Some(done) = snap.as_ref().filter(|s| s.status == JobStatus::Completed) {
                if let Some(output_path) = &done.output_file {
                    let p = theme::Palette::of(ui.ctx());
                    let file_name = output_path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| output_path.display().to_string());
                    let link_color = if ui.ctx().theme() == egui::Theme::Dark {
                        p.accent_hover
                    } else {
                        p.accent
                    };
                    ui.add_space(6.0);
                    egui::Frame::new()
                        .fill(p.bg)
                        .stroke(Stroke::new(1.0_f32, p.border))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(egui::Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 2.0;
                                    ui.horizontal(|ui| {
                                        ui.muted_label("File:");
                                        ui.label(
                                            egui::RichText::new(&file_name)
                                                .size(theme::metrics::FONT_SM)
                                                .color(link_color),
                                        )
                                        .on_hover_text(output_path.display().to_string());
                                    });
                                    if let Some(comment) = &done.metadata_comment {
                                        ui.horizontal(|ui| {
                                            ui.muted_label("comment:");
                                            ui.label(
                                                egui::RichText::new(comment)
                                                    .size(theme::metrics::FONT_SM)
                                                    .family(FontFamily::Monospace)
                                                    .color(p.text),
                                            );
                                        });
                                    }
                                    if let Some(visual) = &done.visual_label {
                                        ui.horizontal(|ui| {
                                            ui.muted_label("Variasi:");
                                            ui.label(
                                                egui::RichText::new(visual)
                                                    .size(theme::metrics::FONT_SM)
                                                    .color(p.text_strong),
                                            );
                                        });
                                    }
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if outline_icon_button(ui, GenerateIcon::Folder, "Buka Folder")
                                        .clicked()
                                    {
                                        open_result_folder = output_path
                                            .parent()
                                            .map(PathBuf::from)
                                            .or_else(|| Some(output_path.clone()));
                                    }
                                });
                            });
                        });
                }
            }
        });

        if let Some(folder) = open_result_folder {
            let _ = open_path(&folder);
        }

        if render_clicked && !running {
            if let Some(reason) = self.generate_render_blocker() {
                self.warn(reason);
            } else {
                self.start_generate_render();
            }
        }
    }

    pub(super) fn load_generate_video(&mut self, path: PathBuf) {
        let tools = self.tools.clone();
        self.spawn(async move { Event::GenerateVideo(probe_video(&tools, &path).await) });
    }

    pub(super) fn load_generate_audios(&mut self, paths: Vec<PathBuf>) {
        let tools = self.tools.clone();
        self.spawn(async move {
            let mut results = Vec::with_capacity(paths.len());
            for path in paths {
                results.push(probe_audio(&tools, &path).await);
            }
            Event::GenerateAudios(results)
        });
    }

    pub(super) fn load_generate_audio_folder(&mut self, folder: PathBuf) {
        let mut audios = Vec::new();
        super::collect_audios_in_folder(&folder, &mut audios);
        if audios.is_empty() {
            self.warn(format!(
                "Tidak ada file audio ditemukan di folder: {}",
                folder.display()
            ));
        } else {
            let count = audios.len();
            self.log(
                super::LogLevel::Info,
                format!(
                    "Memindai folder Generate: {count} audio ditemukan di {}",
                    folder.display()
                ),
            );
            self.load_generate_audios(audios);
        }
    }

    fn pick_generate_audio_folders(&mut self) {
        let Some(folders) = rfd::FileDialog::new().pick_folders() else {
            return;
        };
        let mut audios = Vec::new();
        for folder in &folders {
            let before = audios.len();
            super::collect_audios_in_folder(folder, &mut audios);
            if audios.len() == before {
                self.warn(format!(
                    "Tidak ada file audio ditemukan di folder: {}",
                    folder.display()
                ));
            }
        }
        if !audios.is_empty() {
            let count = audios.len();
            let folder_count = folders.len();
            self.log(
                super::LogLevel::Info,
                format!("Memindai {folder_count} folder Generate: {count} audio ditemukan."),
            );
            self.load_generate_audios(audios);
        }
    }

    fn pick_generate_audio_files(&mut self) {
        let picked = rfd::FileDialog::new()
            .add_filter("Audio", &playlist::AUDIO_EXTENSIONS)
            .pick_files();
        if let Some(paths) = picked {
            self.load_generate_audios(paths);
        }
    }

    fn shuffle_generate_tracks(&mut self) {
        if self.generate.tracks.len() <= 1 {
            return;
        }
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        self.generate.tracks.shuffle(&mut rng);
        self.info(format!(
            "Urutan {} lagu di Playlist Album diacak (shuffle).",
            self.generate.tracks.len()
        ));
    }

    fn generate_render_blocker(&self) -> Option<&'static str> {
        if self.generate.video.is_none() {
            Some("Pilih video visual terlebih dahulu.")
        } else if self.generate.tracks.is_empty() {
            Some("Tambahkan lagu ke Playlist Album terlebih dahulu.")
        } else if let Err(msg) = parse_manual_loop_duration(&self.generate.manual_loop_duration) {
            Some(msg)
        } else if self.hardware.is_none() {
            Some("Mendeteksi hardware...")
        } else if self.job_running() {
            Some("Tunggu proses render lain selesai.")
        } else {
            None
        }
    }

    fn start_generate_render(&mut self) {
        let (Some(video), Some(hardware)) = (self.generate.video.clone(), self.hardware.clone())
        else {
            return;
        };

        let output_dir = self
            .generate
            .output_dir
            .clone()
            .unwrap_or_else(|| self.paths.outputs.clone());
        let _ = std::fs::create_dir_all(&output_dir);

        let fps = if video.fps.is_finite() && video.fps > 0.0 {
            video.fps.round().clamp(1.0, 120.0) as u32
        } else {
            30
        };
        let audio_files: Vec<PathBuf> = self
            .generate
            .tracks
            .iter()
            .map(|track| track.path.clone())
            .collect();
        let track_titles: Vec<String> = self
            .generate
            .tracks
            .iter()
            .map(|track| track.title.clone())
            .collect();
        let track_count = audio_files.len();

        let fade_out_secs = self.generate.fade_out_secs.clamp(0.0, MAX_FADE_OUT_SECS);
        let crossfade_secs = self.generate.crossfade_secs.clamp(0.0, MAX_CROSSFADE_SECS);
        let target_duration_secs = parse_manual_loop_duration(&self.generate.manual_loop_duration)
            .ok()
            .flatten();
        let normalize_volume = self.generate.volume_norm_idx == 0;

        let requested_size = match RESOLUTION_OPTIONS
            .get(self.generate.resolution_idx)
            .copied()
        {
            Some("640x360") => oriented_size(video.width, video.height, 640, 360),
            Some("1280x720") => oriented_size(video.width, video.height, 1280, 720),
            Some("1920x1080") => oriented_size(video.width, video.height, 1920, 1080),
            Some("2560x1440") => oriented_size(video.width, video.height, 2560, 1440),
            Some("3840x2160") => oriented_size(video.width, video.height, 3840, 2160),
            _ => (video.width, video.height),
        };
        let size = self.video_settings.resolve_target_size(requested_size);
        if self.video_settings.target == VideoTarget::YouTubeStandard && size != requested_size {
            self.log(
                super::LogLevel::Info,
                format!(
                    "Standar YouTube menyesuaikan resolusi {}x{} -> {}x{}.",
                    requested_size.0, requested_size.1, size.0, size.1
                ),
            );
        }
        let encoder = self.video_settings.resolve_encoder(&hardware);
        let preset = self.video_settings.resolve_preset(encoder).to_owned();
        let quality = self.video_settings.resolve_quality(encoder);
        let preferred_bitrate = AUDIO_BITRATE_OPTIONS
            .get(self.generate.audio_bitrate_idx)
            .copied()
            .unwrap_or("192k");
        let audio_bitrate = self.video_settings.resolve_audio_bitrate(preferred_bitrate);
        let visual_mode = VisualVariationMode::from_index(self.generate.visual_variation_idx);
        let anti_duplicate = self.generate.anti_duplicate_idx == 0;
        let stamp = build_anti_duplicate_stamp(visual_mode, anti_duplicate);
        let stream_copy_video = stamp.visual_variation == VisualVariation::None
            && self.video_settings.can_stream_copy(
                &video.codec,
                &video.pix_fmt,
                (video.width, video.height),
                size,
            );

        let request = SimpleRenderRequest {
            video_path: PathBuf::from(&video.path),
            audio_files,
            track_titles,
            thumbnail_path: self.generate.thumbnail_path.clone(),
            output_dir: output_dir.clone(),
            size,
            fps,
            encoder,
            preset,
            quality,
            audio_bitrate: audio_bitrate.clone(),
            stream_copy_video,
            youtube_standard: self.video_settings.target == VideoTarget::YouTubeStandard,
            visual_mode,
            anti_duplicate,
            pre_resolved_stamp: Some(stamp.clone()),
            fade_out_secs,
            crossfade_secs,
            normalize_volume,
            target_duration_secs,
        };
        let mut render_paths = self.paths.clone();
        render_paths.outputs = output_dir.clone();
        let context = RenderContext {
            paths: render_paths,
            tools: self.tools.clone(),
            hardware,
        };

        let job = JobHandle::new();
        self.generate.job = Some(job.clone());
        self.generate.job_reported = false;

        let mode_label = if stream_copy_video {
            "Copy (Kualitas Asli)".to_owned()
        } else {
            format!("{} CRF/CQ {quality}", encoder.name())
        };
        let mut adv_notes = Vec::new();
        if stamp.visual_label != "Tanpa" {
            adv_notes.push(format!("Visual {}", stamp.visual_label));
        }
        if let Some(comment) = &stamp.comment {
            adv_notes.push(format!("ID {comment}"));
        }
        if normalize_volume {
            adv_notes.push("Normalisasi aktif".to_owned());
        }
        if crossfade_secs > 0.005 {
            adv_notes.push(format!(
                "Cross-fade {}s",
                format_secs_setting(crossfade_secs)
            ));
        }
        if fade_out_secs > 0.005 {
            adv_notes.push(format!("Fade out {}s", format_secs_setting(fade_out_secs)));
        }
        if let Some(target_secs) = target_duration_secs {
            adv_notes.push(format!("Loop manual {target_secs:.0}s"));
        }
        let adv_suffix = if adv_notes.is_empty() {
            String::new()
        } else {
            format!(" • {}", adv_notes.join(", "))
        };
        self.log(
            super::LogLevel::Info,
            format!(
                "Mulai render Generate: {} ({}x{} @ {fps} FPS • {mode_label} • AAC {audio_bitrate}{adv_suffix}) + {track_count} lagu -> {}",
                video.filename,
                size.0,
                size.1,
                output_dir.display()
            ),
        );

        let ctx = self.ctx.clone();
        self.runtime.spawn(async move {
            render::execute_simple(&context, &job, request).await;
            ctx.request_repaint();
        });
    }
}

fn oriented_size(src_w: u32, src_h: u32, long: u32, short: u32) -> (u32, u32) {
    if src_h > src_w {
        (short, long)
    } else {
        (long, short)
    }
}

fn parse_non_negative_secs(raw: &str) -> Result<f64, &'static str> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(0.0);
    }
    let secs: f64 = trimmed
        .replace(',', ".")
        .parse()
        .map_err(|_| "Nilai detik harus berupa angka valid.")?;
    if !secs.is_finite() || secs < 0.0 {
        return Err("Nilai detik tidak boleh negatif.");
    }
    Ok(secs)
}

fn parse_manual_loop_duration(raw: &str) -> Result<Option<f64>, &'static str> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let parts: Vec<&str> = trimmed.split(':').collect();
    let total_secs = match parts.as_slice() {
        [secs] => secs
            .trim()
            .replace(',', ".")
            .parse::<f64>()
            .map_err(|_| "Format durasi loop manual harus HH:MM:SS (mis. 01:02:00).")?,
        [mins, secs] => {
            let m = mins
                .trim()
                .parse::<u64>()
                .map_err(|_| "Format menit pada durasi loop manual tidak valid.")?;
            let s = secs
                .trim()
                .replace(',', ".")
                .parse::<f64>()
                .map_err(|_| "Format detik pada durasi loop manual tidak valid.")?;
            if s < 0.0 || s >= 60.0 {
                return Err("Detik pada format MM:SS harus antara 0 dan 59.");
            }
            (m as f64) * 60.0 + s
        }
        [hours, mins, secs] => {
            let h = hours
                .trim()
                .parse::<u64>()
                .map_err(|_| "Format jam pada durasi loop manual tidak valid.")?;
            let m = mins
                .trim()
                .parse::<u64>()
                .map_err(|_| "Format menit pada durasi loop manual tidak valid.")?;
            let s = secs
                .trim()
                .replace(',', ".")
                .parse::<f64>()
                .map_err(|_| "Format detik pada durasi loop manual tidak valid.")?;
            if m >= 60 || s < 0.0 || s >= 60.0 {
                return Err("Menit/detik pada format HH:MM:SS harus antara 0 dan 59.");
            }
            (h as f64) * 3600.0 + (m as f64) * 60.0 + s
        }
        _ => return Err("Format durasi loop manual harus HH:MM:SS (mis. 01:02:00)."),
    };
    if !total_secs.is_finite() || total_secs <= 0.0 {
        return Err("Durasi loop manual harus lebih dari 0 detik.");
    }
    Ok(Some(total_secs))
}

fn format_secs_setting(secs: f64) -> String {
    let rounded = (secs.clamp(0.0, 999.0) * 100.0).round() / 100.0;
    if rounded.abs() < 0.005 {
        "0".to_owned()
    } else {
        let s = format!("{rounded:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

fn muted_placeholder_color(p: &theme::Palette) -> Color32 {
    Color32::from_rgba_unmultiplied(p.text_muted.r(), p.text_muted.g(), p.text_muted.b(), 128)
}

#[derive(Default)]
struct StepperChange {
    save: bool,
    commit: bool,
}

impl StepperChange {
    const fn none() -> Self {
        Self {
            save: false,
            commit: false,
        }
    }
}

fn draw_stepper_button(
    ui: &mut Ui,
    symbol_plus: bool,
    enabled: bool,
    p: &theme::Palette,
    is_left: bool,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(28.0, 28.0),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let resp = if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    };
    let rounding = if is_left {
        CornerRadius {
            nw: 7,
            sw: 7,
            ne: 0,
            se: 0,
        }
    } else {
        CornerRadius {
            nw: 0,
            sw: 0,
            ne: 7,
            se: 7,
        }
    };
    if enabled && resp.hovered() {
        ui.painter().rect_filled(rect, rounding, p.surface_alt);
    }
    let stroke_color = if !enabled {
        Color32::from_rgba_unmultiplied(p.text_muted.r(), p.text_muted.g(), p.text_muted.b(), 100)
    } else if resp.hovered() {
        p.text_strong
    } else {
        p.text
    };
    let c = rect.center();
    let stroke = Stroke::new(1.5_f32, stroke_color);
    ui.painter().line_segment(
        [egui::pos2(c.x - 4.0, c.y), egui::pos2(c.x + 4.0, c.y)],
        stroke,
    );
    if symbol_plus {
        ui.painter().line_segment(
            [egui::pos2(c.x, c.y - 4.0), egui::pos2(c.x, c.y + 4.0)],
            stroke,
        );
    }
    resp
}

fn paint_vertical_divider(ui: &mut Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(1.0, 28.0), Sense::hover());
    ui.painter().line_segment(
        [
            egui::pos2(rect.center().x, rect.min.y + 4.0),
            egui::pos2(rect.center().x, rect.max.y - 4.0),
        ],
        Stroke::new(1.0_f32, color),
    );
}

fn labeled_seconds_stepper(
    ui: &mut Ui,
    id_salt: &str,
    label: &str,
    help: &str,
    value: &mut f64,
    max_secs: f64,
) -> StepperChange {
    let p = theme::Palette::of(ui.ctx());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        ui.muted_label(label);
        theme::info(ui, help);
    });
    ui.add_space(2.0);

    let ctrl_w = (ui.available_width() - 12.0).clamp(140.0, 260.0);
    let mid_w = (ctrl_w - 58.0).max(64.0);
    let mut change = StepperChange::none();

    let editing_id = egui::Id::new(id_salt).with("editing");
    let buf_id = egui::Id::new(id_salt).with("buf");
    let start_val_id = egui::Id::new(id_salt).with("start_val");

    let was_editing = ui.data(|d| d.get_temp::<bool>(editing_id)).unwrap_or(false);
    let mut buf = if was_editing {
        ui.data(|d| d.get_temp::<String>(buf_id))
            .unwrap_or_else(|| {
                if *value <= 0.005 {
                    String::new()
                } else {
                    format_secs_setting(*value)
                }
            })
    } else if *value <= 0.005 {
        String::new()
    } else {
        format_secs_setting(*value)
    };

    let buf_valid = if buf.trim().is_empty() {
        Ok(0.0)
    } else {
        parse_non_negative_secs(&buf).and_then(|v| {
            if v > max_secs {
                Err("Melebihi batas maksimum.")
            } else {
                Ok(v)
            }
        })
    };
    let border_color = if was_editing && buf_valid.is_err() {
        p.danger
    } else if was_editing {
        p.accent
    } else {
        p.border
    };

    let mut btn_clicked = false;
    let hint_color = muted_placeholder_color(&p);
    let suffix_text = if *value <= 0.005 && !was_editing {
        if mid_w >= 142.0 {
            "detik (mati)"
        } else {
            "dtk"
        }
    } else if mid_w >= 120.0 {
        "detik"
    } else {
        "dtk"
    };
    let suffix_w = ui.fonts(|f| {
        f.layout_no_wrap(
            suffix_text.to_owned(),
            FontId::proportional(theme::metrics::FONT_SM),
            p.text_muted,
        )
        .size()
        .x
    });

    egui::Frame::new()
        .fill(p.bg)
        .stroke(Stroke::new(1.0_f32, border_color))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::ZERO)
        .show(ui, |ui| {
            ui.set_width(ctrl_w);
            ui.set_height(28.0);
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                if draw_stepper_button(ui, false, *value > 0.005, &p, true).clicked() {
                    btn_clicked = true;
                    *value =
                        (((*value - 0.5).max(0.0) * 100.0).round() / 100.0).clamp(0.0, max_secs);
                    ui.data_mut(|d| {
                        d.remove::<bool>(editing_id);
                        d.remove::<String>(buf_id);
                    });
                    change.save = true;
                    change.commit = true;
                }
                paint_vertical_divider(ui, p.border);

                let edit_w = (mid_w - suffix_w - 22.0).max(34.0);
                ui.add_space(8.0);
                let resp = ui
                    .add_sized(
                        [edit_w, 26.0],
                        egui::TextEdit::singleline(&mut buf)
                            .hint_text(egui::RichText::new("0").color(hint_color))
                            .vertical_align(Align::Center)
                            .frame(false)
                            .margin(egui::Margin::ZERO),
                    )
                    .on_hover_text("Ketik angka desimal (mis. 0,5 atau 2.5), atau klik − / +");
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(suffix_text)
                        .size(theme::metrics::FONT_SM)
                        .color(p.text_muted),
                );
                let tail_pad = (mid_w - 12.0 - edit_w - suffix_w).max(0.0);
                if tail_pad > 0.0 {
                    ui.add_space(tail_pad);
                }

                if resp.gained_focus() {
                    ui.data_mut(|d| d.insert_temp(start_val_id, *value));
                }
                if !btn_clicked && resp.changed() {
                    if buf.trim().is_empty() {
                        *value = 0.0;
                        change.save = true;
                    } else if let Ok(parsed) = parse_non_negative_secs(&buf) {
                        *value = (parsed.clamp(0.0, max_secs) * 100.0).round() / 100.0;
                        change.save = true;
                    }
                }
                if !btn_clicked && resp.has_focus() {
                    ui.data_mut(|d| {
                        d.insert_temp(editing_id, true);
                        d.insert_temp(buf_id, buf.clone());
                    });
                } else {
                    ui.data_mut(|d| {
                        d.remove::<bool>(editing_id);
                        d.remove::<String>(buf_id);
                    });
                    if !btn_clicked && resp.lost_focus() {
                        let start_val = ui
                            .data(|d| d.get_temp::<f64>(start_val_id))
                            .unwrap_or(*value);
                        if (*value - start_val).abs() > 0.005 {
                            change.save = true;
                            change.commit = true;
                        }
                    }
                }

                paint_vertical_divider(ui, p.border);
                if draw_stepper_button(ui, true, *value < max_secs - 0.005, &p, false).clicked() {
                    *value = (((*value + 0.5) * 100.0).round() / 100.0).clamp(0.0, max_secs);
                    ui.data_mut(|d| {
                        d.remove::<bool>(editing_id);
                        d.remove::<String>(buf_id);
                    });
                    change.save = true;
                    change.commit = true;
                }
            });
        });

    if was_editing {
        if let Err(err) = buf_valid {
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new(err)
                    .size(theme::metrics::FONT_XS)
                    .color(p.danger),
            );
        }
    }

    change
}

fn labeled_duration_field(
    ui: &mut Ui,
    label: &str,
    help: &str,
    value: &mut String,
    hint: &str,
) -> bool {
    let p = theme::Palette::of(ui.ctx());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        ui.muted_label(label);
        theme::info(ui, help);
    });
    ui.add_space(2.0);

    let ctrl_w = (ui.available_width() - 12.0).clamp(140.0, 260.0);
    let badge_w = 68.0;
    let edit_w = (ctrl_w - badge_w - 19.0).max(52.0);
    let validation = parse_manual_loop_duration(value);
    let border_color = if validation.is_err() {
        p.danger
    } else {
        p.border
    };
    let hint_color = muted_placeholder_color(&p);

    let mut changed = false;
    egui::Frame::new()
        .fill(p.bg)
        .stroke(Stroke::new(1.0_f32, border_color))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::ZERO)
        .show(ui, |ui| {
            ui.set_width(ctrl_w);
            ui.set_height(28.0);
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add_space(10.0);
                let edit = egui::TextEdit::singleline(value)
                    .hint_text(egui::RichText::new(hint).color(hint_color))
                    .vertical_align(Align::Center)
                    .frame(false)
                    .margin(egui::Margin::ZERO);
                changed = ui.add_sized([edit_w, 26.0], edit).changed();
                ui.add_space(8.0);
                paint_vertical_divider(ui, p.border);
                ui.allocate_ui_with_layout(
                    egui::vec2(badge_w, 28.0),
                    Layout::centered_and_justified(egui::Direction::LeftToRight),
                    |ui| {
                        ui.label(
                            egui::RichText::new("HH:MM:SS")
                                .size(theme::metrics::FONT_XS)
                                .color(p.text_muted),
                        );
                    },
                );
            });
        });

    if let Err(err) = validation {
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(err)
                .size(theme::metrics::FONT_XS)
                .color(p.danger),
        );
    }

    changed
}

fn labeled_combo(
    ui: &mut Ui,
    id_salt: &str,
    label: &str,
    selected_idx: &mut usize,
    options: &[&str],
) {
    ui.muted_label(label);
    ui.add_space(2.0);
    let combo_w = (ui.available_width() - 12.0).clamp(140.0, 260.0);
    let current = options.get(*selected_idx).copied().unwrap_or(options[0]);
    egui::ComboBox::from_id_salt(id_salt)
        .width(combo_w)
        .selected_text(current)
        .show_ui(ui, |ui| {
            for (idx, option) in options.iter().enumerate() {
                ui.selectable_value(selected_idx, idx, *option);
            }
        });
}

fn generate_card(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
    let p = theme::Palette::of(ui.ctx());
    egui::Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0_f32, p.border))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(egui::Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 4.0;
            add_contents(ui);
        });
}

fn card_header(ui: &mut Ui, icon: GenerateIcon, title: &str, subtitle: &str) {
    let p = theme::Palette::of(ui.ctx());
    let dark = ui.ctx().theme() == egui::Theme::Dark;
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
        draw_generate_icon(painter, badge_rect.center(), icon, badge_fg);

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

fn playlist_row(
    ui: &mut Ui,
    idx: usize,
    last_idx: usize,
    name: &str,
    secs: u64,
    is_playing: bool,
    dragging_idx: Option<usize>,
) -> (Option<PlaylistRowAction>, bool, bool) {
    let p = theme::Palette::of(ui.ctx());
    let row_width = ui.available_width();
    let (rect, row_resp) = ui.allocate_exact_size(egui::vec2(row_width, 34.0), Sense::hover());
    let hovered_pointer = dragging_idx.is_some()
        && ui
            .ctx()
            .pointer_latest_pos()
            .is_some_and(|pt| pt.y >= rect.min.y && pt.y <= rect.max.y);

    if ui.is_rect_visible(rect) {
        if dragging_idx == Some(idx) {
            ui.painter()
                .rect_filled(rect, CornerRadius::ZERO, p.surface_alt.gamma_multiply(0.7));
        } else if is_playing || row_resp.contains_pointer() {
            ui.painter()
                .rect_filled(rect, CornerRadius::ZERO, p.surface_alt.gamma_multiply(0.45));
        }
    }

    // Divider line across bottom of each row
    let sep_y = rect.bottom() - 0.5;
    ui.painter().line_segment(
        [egui::pos2(rect.min.x, sep_y), egui::pos2(rect.max.x, sep_y)],
        Stroke::new(1.0_f32, p.border.gamma_multiply(0.7)),
    );
    if hovered_pointer {
        if let Some(src) = dragging_idx {
            if src != idx {
                let drop_y = if src > idx { rect.min.y } else { rect.max.y };
                ui.painter().line_segment(
                    [
                        egui::pos2(rect.min.x, drop_y),
                        egui::pos2(rect.max.x, drop_y),
                    ],
                    Stroke::new(2.0_f32, p.accent_hover),
                );
            }
        }
    }

    // Track number (right-aligned)
    let index_text = format!("{}", idx + 1);
    let index_galley = ui.fonts(|f| {
        f.layout_no_wrap(
            index_text,
            FontId::proportional(theme::metrics::FONT_SM),
            p.text_muted,
        )
    });
    let index_x = rect.min.x + 28.0 - index_galley.size().x;
    let index_y = rect.center().y - index_galley.size().y / 2.0;
    ui.painter()
        .galley(egui::pos2(index_x, index_y), index_galley, p.text_muted);

    // Right-side action buttons layout (right-to-left geometry inside row rect)
    let btn_size = 22.0;
    let btn_gap = 4.0;
    let right_pad = 12.0;
    let cy = rect.center().y;

    let trash_rect = Rect::from_center_size(
        egui::pos2(rect.max.x - right_pad - btn_size * 0.5, cy),
        egui::vec2(btn_size, btn_size),
    );
    let down_rect = Rect::from_center_size(
        egui::pos2(trash_rect.min.x - btn_gap - btn_size * 0.5, cy),
        egui::vec2(btn_size, btn_size),
    );
    let up_rect = Rect::from_center_size(
        egui::pos2(down_rect.min.x - btn_gap - btn_size * 0.5, cy),
        egui::vec2(btn_size, btn_size),
    );
    let play_rect = Rect::from_center_size(
        egui::pos2(up_rect.min.x - btn_gap - btn_size * 0.5, cy),
        egui::vec2(btn_size, btn_size),
    );

    // Duration text just to the left of the play button
    let dur_text = format_track_duration(secs);
    let dur_galley = ui.fonts(|f| {
        f.layout_no_wrap(
            dur_text,
            FontId::proportional(theme::metrics::FONT_SM),
            p.text_muted,
        )
    });
    let dur_x = play_rect.min.x - 14.0 - dur_galley.size().x;
    let dur_y = cy - dur_galley.size().y / 2.0;
    ui.painter()
        .galley(egui::pos2(dur_x, dur_y), dur_galley, p.text_muted);

    // Drag handle hit zone covering track number + title (away from right action icons)
    let drag_rect = Rect::from_min_max(
        rect.min,
        egui::pos2((dur_x - 8.0).max(rect.min.x), rect.max.y),
    );
    let mut drag_resp = ui.interact(
        drag_rect,
        ui.make_persistent_id(("gen_row_drag", idx)),
        Sense::drag(),
    );
    if dragging_idx.is_none() {
        drag_resp = drag_resp.on_hover_cursor(egui::CursorIcon::Grab);
    }

    // Track filename with clean truncation before duration column
    let title_min_x = rect.min.x + 42.0;
    let title_max_x = (dur_x - 12.0).max(title_min_x + 40.0);
    let title_rect = Rect::from_min_max(
        egui::pos2(title_min_x, rect.min.y),
        egui::pos2(title_max_x, rect.max.y),
    );
    let mut title_ui = ui.new_child(
        UiBuilder::new()
            .id_salt(("gen_row_title", idx))
            .max_rect(title_rect)
            .layout(Layout::left_to_right(Align::Center)),
    );
    title_ui.add(
        egui::Label::new(
            egui::RichText::new(name)
                .size(theme::metrics::FONT_MD)
                .color(if is_playing {
                    p.accent_hover
                } else {
                    p.text_strong
                }),
        )
        .truncate(),
    );

    // Interactive row icons
    let play_clicked = row_icon_button(
        ui,
        ("gen_row_play", idx),
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
    );
    let up_clicked = row_icon_button(
        ui,
        ("gen_row_up", idx),
        up_rect,
        GenerateIcon::ArrowUp,
        idx > 0,
        "Naikkan urutan",
    );
    let down_clicked = row_icon_button(
        ui,
        ("gen_row_down", idx),
        down_rect,
        GenerateIcon::ArrowDown,
        idx < last_idx,
        "Turunkan urutan",
    );
    let trash_clicked = row_icon_button(
        ui,
        ("gen_row_del", idx),
        trash_rect,
        GenerateIcon::Trash,
        true,
        "Hapus lagu",
    );

    let action = if play_clicked {
        Some(PlaylistRowAction::TogglePlay(idx))
    } else if up_clicked {
        Some(PlaylistRowAction::MoveUp(idx))
    } else if down_clicked {
        Some(PlaylistRowAction::MoveDown(idx))
    } else if trash_clicked {
        Some(PlaylistRowAction::Remove(idx))
    } else {
        None
    };
    (action, drag_resp.drag_started(), hovered_pointer)
}

fn button_visuals(response: &egui::Response, p: &theme::Palette) -> (Color32, Color32) {
    let fill = if response.is_pointer_button_down_on() {
        p.surface_alt.lerp_to_gamma(p.text, 0.16)
    } else if response.hovered() {
        p.surface_alt.lerp_to_gamma(p.text, 0.08)
    } else {
        p.bg
    };
    let border_color = if response.hovered() {
        p.border_strong
    } else {
        p.border
    };
    (fill, border_color)
}

fn outline_icon_button(ui: &mut Ui, icon: GenerateIcon, label: &str) -> egui::Response {
    styled_outline_icon_button(ui, icon, label, false)
}

fn plain_progress_bar(ui: &mut Ui, progress: f32) {
    let p = theme::Palette::of(ui.ctx());
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 8.0), Sense::hover());
    let radius = CornerRadius::same(4);
    let painter = ui.painter();
    painter.rect(
        rect,
        radius,
        p.bg,
        Stroke::new(1.0_f32, p.border),
        egui::StrokeKind::Inside,
    );

    let frac = progress.clamp(0.0, 1.0);
    if frac > 0.0 {
        let fill_w = (rect.width() * frac).max(8.0).min(rect.width());
        let fill_rect = Rect::from_min_size(rect.min, egui::vec2(fill_w, rect.height()));
        painter.rect_filled(fill_rect, radius, p.accent);
    }
}

fn primary_render_button(ui: &mut Ui, label: &str, enabled: bool) -> egui::Response {
    let width = ui.available_width();
    let height = 38.0;
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, mut response) = ui.allocate_exact_size(egui::vec2(width, height), sense);
    if enabled {
        response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    }

    let p = theme::Palette::of(ui.ctx());
    let fill = if !enabled {
        p.accent.gamma_multiply(0.55)
    } else if response.is_pointer_button_down_on() {
        p.accent_active
    } else if response.hovered() {
        p.accent_hover
    } else {
        p.accent
    };
    let fg = if enabled {
        p.on_accent
    } else {
        p.on_accent.gamma_multiply(0.75)
    };

    let font_id = FontId::new(14.5, FontFamily::Name(theme::SEMIBOLD.into()));
    let text_galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font_id, fg));

    let icon_w = 10.0;
    let gap = 8.0;
    let content_w = icon_w + gap + text_galley.size().x;
    let content_start_x = rect.center().x - content_w / 2.0;
    let cy = rect.center().y;

    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(8), fill);
    draw_generate_icon(
        painter,
        egui::pos2(content_start_x + 5.0, cy),
        GenerateIcon::Play,
        fg,
    );
    painter.galley(
        egui::pos2(
            content_start_x + icon_w + gap,
            cy - text_galley.size().y / 2.0,
        ),
        text_galley,
        fg,
    );

    response
}

fn danger_outline_icon_button(ui: &mut Ui, icon: GenerateIcon, label: &str) -> egui::Response {
    styled_outline_icon_button(ui, icon, label, true)
}

fn styled_outline_icon_button(
    ui: &mut Ui,
    icon: GenerateIcon,
    label: &str,
    danger: bool,
) -> egui::Response {
    let p = theme::Palette::of(ui.ctx());
    let dark = ui.ctx().theme() == egui::Theme::Dark;
    let font_id = FontId::new(
        theme::metrics::FONT_MD,
        FontFamily::Name(theme::SEMIBOLD.into()),
    );
    let text_galley = ui.fonts(|fonts| fonts.layout_no_wrap(label.to_owned(), font_id, p.text));
    let width = (14.0 + 14.0 + 8.0 + text_galley.size().x + 16.0).ceil();
    let height = 30.0;

    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    let (fill, border_color, fg_color) = if danger {
        let fill = if response.is_pointer_button_down_on() {
            if dark {
                Color32::from_rgb(64, 24, 29)
            } else {
                Color32::from_rgb(254, 202, 202)
            }
        } else if response.hovered() {
            if dark {
                Color32::from_rgb(44, 20, 25)
            } else {
                Color32::from_rgb(254, 226, 226)
            }
        } else {
            p.bg
        };
        let border = if response.hovered() {
            p.danger
        } else {
            p.danger.gamma_multiply(0.55)
        };
        let fg = if dark {
            Color32::from_rgb(240, 108, 114)
        } else {
            p.danger
        };
        (fill, border, fg)
    } else {
        let (fill, border) = button_visuals(&response, &p);
        (fill, border, p.text_strong)
    };

    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(8),
        fill,
        Stroke::new(1.0_f32, border_color),
        egui::StrokeKind::Inside,
    );

    let icon_center = egui::pos2(rect.min.x + 20.0, rect.center().y);
    draw_generate_icon(painter, icon_center, icon, fg_color);

    let text_pos = egui::pos2(
        rect.min.x + 34.0,
        rect.center().y - text_galley.size().y / 2.0,
    );
    painter.galley(text_pos, text_galley, fg_color);

    response
}

fn square_icon_button(
    ui: &mut Ui,
    icon: GenerateIcon,
    enabled: bool,
    active: bool,
    tooltip: &str,
) -> egui::Response {
    let p = theme::Palette::of(ui.ctx());
    let dark = ui.ctx().theme() == egui::Theme::Dark;
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, mut response) = ui.allocate_exact_size(egui::vec2(30.0, 30.0), sense);
    if enabled {
        response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    }
    let (fill, border_color) = if !enabled {
        (p.bg, p.border.gamma_multiply(0.55))
    } else if active {
        (
            if dark {
                Color32::from_rgb(24, 38, 66)
            } else {
                Color32::from_rgb(219, 234, 254)
            },
            p.accent_hover,
        )
    } else {
        button_visuals(&response, &p)
    };
    let fg_color = if !enabled {
        p.text_muted.gamma_multiply(0.45)
    } else if active {
        p.accent_hover
    } else if response.hovered() {
        p.text_strong
    } else {
        p.text
    };

    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(8),
        fill,
        Stroke::new(1.0_f32, border_color),
        egui::StrokeKind::Inside,
    );
    draw_generate_icon(painter, rect.center(), icon, fg_color);

    if tooltip.is_empty() {
        response
    } else {
        response.on_hover_text(tooltip)
    }
}

const VOLUME_POPUP_WIDTH: f32 = 48.0;
const VOLUME_POPUP_HEIGHT: f32 = 152.0;

fn volume_from_drag_y(pointer_y: f32, rail_top: f32, rail_bottom: f32) -> f32 {
    let span = (rail_bottom - rail_top).max(1.0);
    let frac = ((rail_bottom - pointer_y) / span).clamp(0.0, 1.0);
    (frac * 100.0).round().clamp(0.0, 100.0)
}

fn volume_thumb_y(volume: f32, rail_top: f32, rail_bottom: f32) -> f32 {
    let span = (rail_bottom - rail_top).max(1.0);
    let frac = (volume / 100.0).clamp(0.0, 1.0);
    rail_bottom - frac * span
}

fn volume_popup_pos(speaker_rect: Rect, popup_size: egui::Vec2, screen_rect: Rect) -> egui::Pos2 {
    let gap = 8.0_f32;
    let margin = 8.0_f32;
    let preferred_x = speaker_rect.center().x - popup_size.x * 0.5;
    let preferred_y = speaker_rect.top() - gap - popup_size.y;
    let y = if preferred_y < screen_rect.top() + margin
        && speaker_rect.bottom() + gap + popup_size.y <= screen_rect.bottom() - margin
    {
        speaker_rect.bottom() + gap
    } else {
        let max_y = (screen_rect.bottom() - popup_size.y - margin).max(screen_rect.top() + margin);
        preferred_y.clamp(screen_rect.top() + margin, max_y)
    };
    let max_x = (screen_rect.right() - popup_size.x - margin).max(screen_rect.left() + margin);
    let x = preferred_x.clamp(screen_rect.left() + margin, max_x);
    egui::pos2(x, y)
}

fn show_volume_slider_popup(
    ui: &Ui,
    speaker_rect: Rect,
    allow_outside_close: bool,
    open: &mut bool,
    volume: &mut f32,
) -> bool {
    let ctx = ui.ctx();
    let popup_size = egui::vec2(VOLUME_POPUP_WIDTH, VOLUME_POPUP_HEIGHT);
    let popup_pos = volume_popup_pos(speaker_rect, popup_size, ctx.screen_rect());
    let p = theme::Palette::of(ctx);
    let dark = ctx.theme() == egui::Theme::Dark;
    let accent = if dark { p.accent_hover } else { p.accent };
    let track_bg = if dark {
        Color32::from_rgb(52, 56, 70)
    } else {
        p.border_strong
    };

    let mut changed = false;
    let mut slider_dragging = false;

    let area_resp = egui::Area::new(egui::Id::new("generate_volume_popup"))
        .order(egui::Order::Foreground)
        .fixed_pos(popup_pos)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(p.elevated)
                .stroke(Stroke::new(1.0_f32, p.border_strong))
                .corner_radius(CornerRadius::same(11))
                .shadow(ui.style().visuals.popup_shadow)
                .inner_margin(egui::Margin::ZERO)
                .show(ui, |ui| {
                    let (popup_rect, _) = ui.allocate_exact_size(popup_size, Sense::hover());
                    let label_rect = Rect::from_min_max(
                        popup_rect.min,
                        egui::pos2(popup_rect.max.x, popup_rect.min.y + 30.0),
                    );
                    let slider_rect = Rect::from_min_max(
                        egui::pos2(popup_rect.min.x + 6.0, label_rect.max.y),
                        egui::pos2(popup_rect.max.x - 6.0, popup_rect.max.y - 12.0),
                    );
                    let thumb_r = 6.5_f32;
                    let rail_top = slider_rect.top() + thumb_r + 2.0;
                    let rail_bottom = slider_rect.bottom() - thumb_r - 2.0;

                    let slider_resp = ui.interact(
                        slider_rect,
                        ui.id().with("volume_rail"),
                        Sense::click_and_drag(),
                    );
                    slider_dragging = slider_resp.dragged();

                    if slider_resp.dragged() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    } else if slider_resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    if slider_resp.clicked() || slider_resp.dragged() {
                        if let Some(pos) = slider_resp.interact_pointer_pos() {
                            let next = volume_from_drag_y(pos.y, rail_top, rail_bottom);
                            if (next - *volume).abs() > f32::EPSILON {
                                *volume = next;
                                changed = true;
                            }
                        }
                    }

                    if ui
                        .input(|i| i.pointer.hover_pos())
                        .is_some_and(|pos| popup_rect.contains(pos))
                    {
                        let scroll_y = ui.input(|i| i.raw_scroll_delta.y);
                        if scroll_y.abs() > f32::EPSILON {
                            let step = scroll_y.signum() * 5.0;
                            let next = (*volume + step).round().clamp(0.0, 100.0);
                            if (next - *volume).abs() > f32::EPSILON {
                                *volume = next;
                                changed = true;
                            }
                        }
                    }

                    let pct = volume.round() as u32;
                    let painter = ui.painter();
                    painter.text(
                        egui::pos2(label_rect.center().x, label_rect.center().y + 2.0),
                        egui::Align2::CENTER_CENTER,
                        format!("{pct}%"),
                        egui::FontId::proportional(11.5),
                        accent,
                    );

                    let cx = slider_rect.center().x;
                    let half_w = 2.5_f32;
                    let bg_track = Rect::from_min_max(
                        egui::pos2(cx - half_w, rail_top - half_w),
                        egui::pos2(cx + half_w, rail_bottom + half_w),
                    );
                    painter.rect_filled(bg_track, CornerRadius::same(3), track_bg);

                    let thumb_y = volume_thumb_y(*volume, rail_top, rail_bottom);
                    if *volume > 0.0 {
                        let fill_track = Rect::from_min_max(
                            egui::pos2(cx - half_w, thumb_y),
                            egui::pos2(cx + half_w, rail_bottom + half_w),
                        );
                        painter.rect_filled(fill_track, CornerRadius::same(3), accent);
                    }

                    let draw_r = if slider_resp.hovered() || slider_resp.dragged() {
                        7.2
                    } else {
                        thumb_r
                    };
                    let knob_stroke = if dark {
                        Stroke::new(1.0_f32, Color32::from_black_alpha(55))
                    } else {
                        Stroke::new(1.2_f32, accent)
                    };
                    painter.circle(egui::pos2(cx, thumb_y), draw_r, Color32::WHITE, knob_stroke);
                });
        });

    if allow_outside_close {
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            *open = false;
        } else if !slider_dragging && ctx.input(|i| i.pointer.any_pressed()) {
            let press_pos = ctx.input(|i| i.pointer.interact_pos());
            let popup_rect = area_resp.response.rect;
            if let Some(pos) = press_pos {
                if !popup_rect.expand(2.0).contains(pos) && !speaker_rect.contains(pos) {
                    *open = false;
                }
            }
        }
    }

    changed
}

fn row_icon_button(
    ui: &mut Ui,
    id_salt: (&str, usize),
    rect: Rect,
    icon: GenerateIcon,
    enabled: bool,
    tooltip: &str,
) -> bool {
    let p = theme::Palette::of(ui.ctx());
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let mut response = ui.interact(rect, ui.make_persistent_id(id_salt), sense);
    if enabled {
        response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    }

    if enabled && response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(4), p.surface_alt);
    }

    let color = if !enabled {
        p.text_muted.gamma_multiply(0.45)
    } else if response.hovered() {
        p.text_strong
    } else {
        p.text_muted
    };
    draw_generate_icon(ui.painter(), rect.center(), icon, color);

    let clicked = enabled && response.clicked();
    if enabled {
        response.on_hover_text(tooltip);
    }
    clicked
}

fn format_track_duration(total_seconds: u64) -> String {
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

fn draw_generate_icon(painter: &egui::Painter, c: Pos2, icon: GenerateIcon, color: Color32) {
    let stroke = Stroke::new(1.35_f32, color);
    match icon {
        GenerateIcon::Upload => {
            painter.line_segment(
                [egui::pos2(c.x, c.y + 2.5), egui::pos2(c.x, c.y - 5.2)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(c.x - 3.0, c.y - 2.2), egui::pos2(c.x, c.y - 5.2)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(c.x + 3.0, c.y - 2.2), egui::pos2(c.x, c.y - 5.2)],
                stroke,
            );
            let tray = [
                egui::pos2(c.x - 5.5, c.y + 1.8),
                egui::pos2(c.x - 5.5, c.y + 5.2),
                egui::pos2(c.x + 5.5, c.y + 5.2),
                egui::pos2(c.x + 5.5, c.y + 1.8),
            ];
            for seg in tray.windows(2) {
                painter.line_segment([seg[0], seg[1]], stroke);
            }
        }
        GenerateIcon::Folder => {
            let pts = [
                egui::pos2(c.x - 6.0, c.y - 4.5),
                egui::pos2(c.x - 1.5, c.y - 4.5),
                egui::pos2(c.x + 0.2, c.y - 2.5),
                egui::pos2(c.x + 6.0, c.y - 2.5),
                egui::pos2(c.x + 6.0, c.y + 4.8),
                egui::pos2(c.x - 6.0, c.y + 4.8),
            ];
            for seg in pts.windows(2) {
                painter.line_segment([seg[0], seg[1]], stroke);
            }
            painter.line_segment([pts[5], pts[0]], stroke);
        }
        GenerateIcon::Music => {
            painter.circle_stroke(egui::pos2(c.x - 3.2, c.y + 3.8), 1.8, stroke);
            painter.circle_stroke(egui::pos2(c.x + 3.0, c.y + 2.8), 1.8, stroke);
            let left_top = egui::pos2(c.x - 1.4, c.y - 4.2);
            let right_top = egui::pos2(c.x + 4.8, c.y - 5.2);
            painter.line_segment([egui::pos2(c.x - 1.4, c.y + 3.8), left_top], stroke);
            painter.line_segment([egui::pos2(c.x + 4.8, c.y + 2.8), right_top], stroke);
            painter.line_segment([left_top, right_top], stroke);
            painter.line_segment(
                [
                    egui::pos2(left_top.x, left_top.y + 2.4),
                    egui::pos2(right_top.x, right_top.y + 2.4),
                ],
                stroke,
            );
        }
        GenerateIcon::Shuffle => {
            let path_a = [
                egui::pos2(c.x - 5.5, c.y + 3.5),
                egui::pos2(c.x - 1.8, c.y + 3.5),
                egui::pos2(c.x + 1.8, c.y - 3.5),
                egui::pos2(c.x + 5.5, c.y - 3.5),
            ];
            for seg in path_a.windows(2) {
                painter.line_segment([seg[0], seg[1]], stroke);
            }
            let path_b = [
                egui::pos2(c.x - 5.5, c.y - 3.5),
                egui::pos2(c.x - 1.8, c.y - 3.5),
                egui::pos2(c.x + 1.8, c.y + 3.5),
                egui::pos2(c.x + 5.5, c.y + 3.5),
            ];
            for seg in path_b.windows(2) {
                painter.line_segment([seg[0], seg[1]], stroke);
            }
            // Arrowheads on right side
            painter.line_segment(
                [
                    egui::pos2(c.x + 3.4, c.y - 5.4),
                    egui::pos2(c.x + 5.5, c.y - 3.5),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(c.x + 3.4, c.y - 1.6),
                    egui::pos2(c.x + 5.5, c.y - 3.5),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(c.x + 3.4, c.y + 1.6),
                    egui::pos2(c.x + 5.5, c.y + 3.5),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(c.x + 3.4, c.y + 5.4),
                    egui::pos2(c.x + 5.5, c.y + 3.5),
                ],
                stroke,
            );
        }
        GenerateIcon::Copy => {
            let front = Rect::from_min_max(
                egui::pos2(c.x - 1.2, c.y - 1.2),
                egui::pos2(c.x + 4.8, c.y + 4.8),
            );
            painter.rect_stroke(
                front,
                CornerRadius::same(2),
                stroke,
                egui::StrokeKind::Middle,
            );
            let back = [
                egui::pos2(c.x - 1.2, c.y + 1.4),
                egui::pos2(c.x - 4.8, c.y + 1.4),
                egui::pos2(c.x - 4.8, c.y - 4.8),
                egui::pos2(c.x + 1.4, c.y - 4.8),
                egui::pos2(c.x + 1.4, c.y - 1.2),
            ];
            for seg in back.windows(2) {
                painter.line_segment([seg[0], seg[1]], stroke);
            }
        }
        GenerateIcon::Speaker | GenerateIcon::SpeakerMuted => {
            let body = [
                egui::pos2(c.x - 5.2, c.y - 1.8),
                egui::pos2(c.x - 2.8, c.y - 1.8),
                egui::pos2(c.x + 0.5, c.y - 4.6),
                egui::pos2(c.x + 0.5, c.y + 4.6),
                egui::pos2(c.x - 2.8, c.y + 1.8),
                egui::pos2(c.x - 5.2, c.y + 1.8),
            ];
            painter.add(Shape::closed_line(body.to_vec(), stroke));
            if matches!(icon, GenerateIcon::Speaker) {
                // Sound waves
                painter.line_segment(
                    [egui::pos2(c.x + 2.7, c.y - 2.0), egui::pos2(c.x + 3.4, c.y)],
                    stroke,
                );
                painter.line_segment(
                    [egui::pos2(c.x + 3.4, c.y), egui::pos2(c.x + 2.7, c.y + 2.0)],
                    stroke,
                );
                painter.line_segment(
                    [egui::pos2(c.x + 4.5, c.y - 3.6), egui::pos2(c.x + 5.6, c.y)],
                    stroke,
                );
                painter.line_segment(
                    [egui::pos2(c.x + 5.6, c.y), egui::pos2(c.x + 4.5, c.y + 3.6)],
                    stroke,
                );
            } else {
                // Mute cross
                painter.line_segment(
                    [
                        egui::pos2(c.x + 2.4, c.y - 2.0),
                        egui::pos2(c.x + 5.6, c.y + 2.0),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(c.x + 5.6, c.y - 2.0),
                        egui::pos2(c.x + 2.4, c.y + 2.0),
                    ],
                    stroke,
                );
            }
        }
        GenerateIcon::Trash => {
            // Lid + top handle
            painter.line_segment(
                [
                    egui::pos2(c.x - 4.8, c.y - 3.2),
                    egui::pos2(c.x + 4.8, c.y - 3.2),
                ],
                stroke,
            );
            let handle = [
                egui::pos2(c.x - 1.8, c.y - 3.2),
                egui::pos2(c.x - 1.8, c.y - 5.0),
                egui::pos2(c.x + 1.8, c.y - 5.0),
                egui::pos2(c.x + 1.8, c.y - 3.2),
            ];
            for seg in handle.windows(2) {
                painter.line_segment([seg[0], seg[1]], stroke);
            }
            // Bin
            let bin = [
                egui::pos2(c.x - 3.6, c.y - 3.2),
                egui::pos2(c.x - 3.0, c.y + 4.8),
                egui::pos2(c.x + 3.0, c.y + 4.8),
                egui::pos2(c.x + 3.6, c.y - 3.2),
            ];
            for seg in bin.windows(2) {
                painter.line_segment([seg[0], seg[1]], stroke);
            }
            // Inner ribs
            painter.line_segment(
                [
                    egui::pos2(c.x - 1.2, c.y - 1.2),
                    egui::pos2(c.x - 1.0, c.y + 2.8),
                ],
                Stroke::new(1.1_f32, color),
            );
            painter.line_segment(
                [
                    egui::pos2(c.x + 1.2, c.y - 1.2),
                    egui::pos2(c.x + 1.0, c.y + 2.8),
                ],
                Stroke::new(1.1_f32, color),
            );
        }
        GenerateIcon::Play => {
            let tri = [
                egui::pos2(c.x - 2.6, c.y - 4.0),
                egui::pos2(c.x + 3.8, c.y),
                egui::pos2(c.x - 2.6, c.y + 4.0),
            ];
            painter.add(Shape::closed_line(tri.to_vec(), stroke));
        }
        GenerateIcon::Stop => {
            let box_rect = Rect::from_center_size(c, egui::vec2(7.0, 7.0));
            painter.rect_filled(box_rect, CornerRadius::same(1), color);
        }
        GenerateIcon::ArrowUp => {
            painter.line_segment(
                [egui::pos2(c.x, c.y + 4.4), egui::pos2(c.x, c.y - 4.2)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(c.x - 3.0, c.y - 1.2), egui::pos2(c.x, c.y - 4.2)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(c.x + 3.0, c.y - 1.2), egui::pos2(c.x, c.y - 4.2)],
                stroke,
            );
        }
        GenerateIcon::ArrowDown => {
            painter.line_segment(
                [egui::pos2(c.x, c.y - 4.2), egui::pos2(c.x, c.y + 4.4)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(c.x - 3.0, c.y + 1.4), egui::pos2(c.x, c.y + 4.4)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(c.x + 3.0, c.y + 1.4), egui::pos2(c.x, c.y + 4.4)],
                stroke,
            );
        }
        GenerateIcon::Sliders => {
            for (dy, knob_dx) in [(-4.0_f32, -2.5_f32), (0.0, 2.5), (4.0, -1.5)] {
                painter.line_segment(
                    [
                        egui::pos2(c.x - 6.0, c.y + dy),
                        egui::pos2(c.x + 6.0, c.y + dy),
                    ],
                    stroke,
                );
                painter.circle_filled(egui::pos2(c.x + knob_dx, c.y + dy), 1.9, color);
            }
        }
        GenerateIcon::ChevronUp => {
            painter.line_segment(
                [egui::pos2(c.x - 4.2, c.y + 2.0), egui::pos2(c.x, c.y - 2.2)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(c.x, c.y - 2.2), egui::pos2(c.x + 4.2, c.y + 2.0)],
                stroke,
            );
        }
        GenerateIcon::ChevronDown => {
            painter.line_segment(
                [egui::pos2(c.x - 4.2, c.y - 2.0), egui::pos2(c.x, c.y + 2.2)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(c.x, c.y + 2.2), egui::pos2(c.x + 4.2, c.y - 2.0)],
                stroke,
            );
        }
        GenerateIcon::Reset => {
            let radius = 4.6_f32;
            let start = -2.2_f32;
            let end = start + 1.55 * PI;
            let steps = 12;
            let pts: Vec<Pos2> = (0..=steps)
                .map(|i| {
                    let t = i as f32 / steps as f32;
                    let a = start + (end - start) * t;
                    egui::pos2(c.x + radius * a.cos(), c.y + radius * a.sin())
                })
                .collect();
            for seg in pts.windows(2) {
                painter.line_segment([seg[0], seg[1]], stroke);
            }
            // Arrowhead at the starting top-left end pointing counter-clockwise
            let head = pts[0];
            painter.line_segment([head, egui::pos2(head.x + 3.2, head.y - 0.6)], stroke);
            painter.line_segment([head, egui::pos2(head.x + 0.5, head.y + 3.2)], stroke);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_non_negative_seconds() {
        assert_eq!(parse_non_negative_secs(""), Ok(0.0));
        assert_eq!(parse_non_negative_secs(" 2,5 "), Ok(2.5));
        assert_eq!(parse_non_negative_secs("0,5"), Ok(0.5));
        assert_eq!(parse_non_negative_secs(",5"), Ok(0.5));
        assert!(parse_non_negative_secs("-1").is_err());
        assert!(parse_non_negative_secs("abc").is_err());
    }

    #[test]
    fn formats_seconds_with_two_decimals_without_trailing_zeros() {
        assert_eq!(format_secs_setting(0.0), "0");
        assert_eq!(format_secs_setting(0.5), "0.5");
        assert_eq!(format_secs_setting(0.25), "0.25");
        assert_eq!(format_secs_setting(5.0), "5");
    }

    #[test]
    fn parses_manual_loop_durations() {
        assert_eq!(parse_manual_loop_duration("   "), Ok(None));
        assert_eq!(parse_manual_loop_duration("01:02:00"), Ok(Some(3720.0)));
        assert_eq!(parse_manual_loop_duration("05:30"), Ok(Some(330.0)));
        assert_eq!(parse_manual_loop_duration("90"), Ok(Some(90.0)));
        assert!(parse_manual_loop_duration("00:00:00").is_err());
        assert!(parse_manual_loop_duration("01:75:00").is_err());
        assert!(parse_manual_loop_duration("bad").is_err());
    }

    #[test]
    fn default_volume_norm_is_off() {
        let state = GenerateState::default();
        assert_eq!(
            VOLUME_NORM_OPTIONS[state.volume_norm_idx],
            "Off (render lebih cepat)"
        );
        assert_eq!(RESOLUTION_OPTIONS[state.resolution_idx], "Ikut sumber");
        assert_eq!(
            VISUAL_VARIATION_OPTIONS[state.visual_variation_idx],
            "Tanpa"
        );
        assert_eq!(state.preview_volume, 100.0);
        assert!(!state.volume_popup_open);
        assert_eq!(
            ANTI_DUPLICATE_OPTIONS[state.anti_duplicate_idx],
            "Aktif (unik tiap render)"
        );
    }

    #[test]
    fn maps_volume_drag_and_thumb_coordinates() {
        assert_eq!(volume_from_drag_y(20.0, 20.0, 120.0), 100.0);
        assert_eq!(volume_from_drag_y(70.0, 20.0, 120.0), 50.0);
        assert_eq!(volume_from_drag_y(120.0, 20.0, 120.0), 0.0);
        assert_eq!(volume_from_drag_y(0.0, 20.0, 120.0), 100.0);
        assert_eq!(volume_from_drag_y(150.0, 20.0, 120.0), 0.0);

        assert_eq!(volume_thumb_y(100.0, 20.0, 120.0), 20.0);
        assert_eq!(volume_thumb_y(50.0, 20.0, 120.0), 70.0);
        assert_eq!(volume_thumb_y(0.0, 20.0, 120.0), 120.0);
    }

    #[test]
    fn positions_volume_popup_above_speaker_and_clamps_near_edges() {
        let screen = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1280.0, 720.0));
        let size = egui::vec2(48.0, 152.0);
        let speaker = Rect::from_min_size(egui::pos2(900.0, 400.0), egui::vec2(30.0, 30.0));
        let above = volume_popup_pos(speaker, size, screen);
        assert_eq!(above, egui::pos2(891.0, 240.0));

        // Near top edge: flips below speaker button instead of clipping.
        let top_speaker = Rect::from_min_size(egui::pos2(900.0, 40.0), egui::vec2(30.0, 30.0));
        let flipped = volume_popup_pos(top_speaker, size, screen);
        assert_eq!(flipped, egui::pos2(891.0, 78.0));
    }

    #[test]
    fn scales_preview_volume_to_mci_range() {
        assert_eq!(super::super::lyrics::win_audio::volume_to_mci(-10.0), 0);
        assert_eq!(super::super::lyrics::win_audio::volume_to_mci(0.0), 0);
        assert_eq!(super::super::lyrics::win_audio::volume_to_mci(50.0), 500);
        assert_eq!(super::super::lyrics::win_audio::volume_to_mci(100.0), 1000);
        assert_eq!(super::super::lyrics::win_audio::volume_to_mci(120.0), 1000);
    }
}
