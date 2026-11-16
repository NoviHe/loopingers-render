use eframe::egui;
use loopkaraoke::{
    karaoke::ass::StyleConfig,
    media::{build_chapters, format_clock, probe_audio, ChapterSource},
};

use super::{help::playlist as help, theme, Event, StudioApp};

pub(super) const AUDIO_EXTENSIONS: [&str; 7] = ["mp3", "wav", "m4a", "aac", "flac", "ogg", "opus"];

enum RowAction {
    Remove(usize),
    MoveUp(usize),
    MoveDown(usize),
}

impl StudioApp {
    pub(super) fn build_style_config(&self) -> StyleConfig {
        let mut cfg = self.style.to_config();
        let enable_karaoke = self.show_karaoke_lyrics && self.has_any_track_lyrics();
        cfg.show_lyrics = Some(enable_karaoke);
        cfg.show_intro = Some(enable_karaoke && self.style.show_intro);
        cfg.show_tracklist = Some(false);
        cfg.tracklist_position = Some(self.tracklist_position);
        cfg.tracklist_font_size = Some(self.thumbnail.track_size);
        cfg.tracklist_title = Some(self.thumbnail.title.clone());
        cfg
    }

    /// Checklist sederhana untuk menampilkan daftar lagu dan karaoke pada video.
    pub(super) fn playlist_display_mode_card(&mut self, ui: &mut egui::Ui, _id_prefix: &str) {
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(
                &mut self.show_tracklist_visual,
                "Tampilkan daftar lagu di video (mengikuti pengaturan Thumbnail)",
            )
            .on_hover_text(
                "Tampilkan daftar lagu di layar video dengan tata letak, font, dan warna yang serasi dengan menu Thumbnail",
            );
            if self.show_tracklist_visual
                && ui
                    .small_button("⚙ Atur di Thumbnail")
                    .on_hover_text("Buka menu Thumbnail untuk mengatur judul, kolom, font, dan warna daftar lagu")
                    .clicked()
            {
                self.tab = super::Tab::Thumbnail;
            }
        });

        if self.has_any_track_lyrics() {
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(
                    &mut self.show_karaoke_lyrics,
                    "Tambahkan karaoke di video (tampilkan lirik lagu)",
                )
                .on_hover_text(
                    "Tampilkan teks lirik karaoke pada video untuk lagu yang memiliki berkas LRC / lirik",
                );
                if ui
                    .small_button("⚙ Atur Pengaturan Karaoke")
                    .on_hover_text(
                        "Buka menu Karaoke untuk mengatur gaya teks, warna, font, dan posisi lirik karaoke",
                    )
                    .clicked()
                {
                    self.tab = super::Tab::Karaoke;
                }
            });
        }
    }

    pub(super) fn playlist_tab(&mut self, ui: &mut egui::Ui) {
        theme::heading_with_info(ui, "Daftar Lagu (Playlist)", help::HEADING);
        ui.label(
            "Lagu digabung berurutan menjadi satu audio. Judul dan artis bisa diubah langsung.",
        );
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            if ui.add(theme::primary_button("Tambah Lagu...")).clicked() {
                let picked = rfd::FileDialog::new()
                    .add_filter("Audio", &AUDIO_EXTENSIONS)
                    .pick_files();
                if let Some(paths) = picked {
                    self.load_audios(paths);
                }
            }
            theme::info(ui, help::ADD);
            if ui
                .add_enabled(
                    !self.tracks.is_empty(),
                    egui::Button::new("Buat Timestamp YouTube"),
                )
                .clicked()
            {
                self.timestamps_text = self.build_timestamps();
                self.log(
                    super::LogLevel::Success,
                    "Timestamp YouTube berhasil dibuat dari urutan playlist.",
                );
            }
            theme::info(ui, help::TIMESTAMPS);
        });
        ui.add_space(8.0);

        if self.tracks.is_empty() {
            ui.label("Belum ada lagu. Seret file audio ke jendela ini atau klik Tambah Lagu.");
        } else {
            ui.horizontal(|ui| {
                ui.weak("Urutan • judul • artis • durasi");
                theme::info(ui, help::ROWS);
            });
        }

        let mut action = None;
        let last = self.tracks.len().saturating_sub(1);
        egui::Grid::new("playlist_rows")
            .striped(true)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                for (index, track) in self.tracks.iter_mut().enumerate() {
                    ui.monospace(format!("{:02}", index + 1));
                    ui.add(egui::TextEdit::singleline(&mut track.title).desired_width(260.0));
                    ui.add(
                        egui::TextEdit::singleline(&mut track.artist)
                            .hint_text("Artis")
                            .desired_width(160.0),
                    );
                    ui.label(format_clock(track.duration.round() as u64));
                    if !track.lyrics.is_empty() || track.has_lrc {
                        draw_lrc_badge(ui, track.lyrics.len());
                    } else {
                        ui.weak("tanpa lirik");
                    }
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(index > 0, egui::Button::new("Naik").small())
                            .clicked()
                        {
                            action = Some(RowAction::MoveUp(index));
                        }
                        if ui
                            .add_enabled(index < last, egui::Button::new("Turun").small())
                            .clicked()
                        {
                            action = Some(RowAction::MoveDown(index));
                        }
                        if ui.small_button("Hapus").clicked() {
                            action = Some(RowAction::Remove(index));
                        }
                    });
                    ui.end_row();
                }
            });
        match action {
            Some(RowAction::Remove(index)) => {
                let title = self.tracks[index].title.clone();
                self.tracks.remove(index);
                self.selected_track = self.selected_track.min(self.tracks.len().saturating_sub(1));
                self.log(
                    super::LogLevel::Info,
                    format!("Lagu \"{title}\" dihapus dari playlist."),
                );
            }
            Some(RowAction::MoveUp(index)) => {
                self.tracks.swap(index, index - 1);
                self.log(
                    super::LogLevel::Info,
                    "Urutan lagu diubah (lagu dinaikkan).",
                );
            }
            Some(RowAction::MoveDown(index)) => {
                self.tracks.swap(index, index + 1);
                self.log(
                    super::LogLevel::Info,
                    "Urutan lagu diubah (lagu diturunkan).",
                );
            }
            None => {}
        }

        if !self.tracks.is_empty() {
            ui.add_space(8.0);
            ui.separator();
            let total = self.total_duration();
            let mut summary = format!(
                "{} lagu • total {}",
                self.tracks.len(),
                format_clock(total.round() as u64)
            );
            if let Some(background) = &self.background {
                let clip = background.video.duration;
                if clip > 0.0 {
                    summary.push_str(&format!(
                        " • video meloop {} kali",
                        (total / clip).ceil() as u64
                    ));
                }
            }
            ui.horizontal(|ui| {
                ui.strong(summary);
                theme::info(ui, help::SUMMARY);
            });

            ui.add_space(8.0);
            self.playlist_display_mode_card(ui, "playlist_tab");
        }

        if !self.timestamps_text.is_empty() {
            ui.add_space(8.0);
            theme::strong_with_info(ui, "Timestamp untuk deskripsi YouTube", help::TIMESTAMP_BOX);
            ui.add(
                egui::TextEdit::multiline(&mut self.timestamps_text)
                    .desired_width(f32::INFINITY)
                    .desired_rows(6),
            );
            if ui.button("Salin").clicked() {
                ui.ctx().copy_text(self.timestamps_text.clone());
                self.info("Timestamp disalin ke clipboard.");
            }
        }
    }

    pub(super) fn load_audios(&mut self, paths: Vec<std::path::PathBuf>) {
        let tools = self.tools.clone();
        self.spawn(async move {
            let mut results = Vec::new();
            for path in paths {
                results.push(probe_audio(&tools, &path).await);
            }
            Event::Audios(results)
        });
    }

    pub(super) fn build_timestamps(&self) -> String {
        let sources: Vec<ChapterSource> = self
            .tracks
            .iter()
            .map(|track| ChapterSource {
                title: Some(track.title.clone()),
                filename: track
                    .path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned()),
                duration: track.duration,
            })
            .collect();
        build_chapters(&sources)
            .iter()
            .map(|chapter| format!("{} {}", chapter.time_str, chapter.title))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Mencari berkas `.lrc` di folder yang sama yang memiliki nama sama dengan berkas audio.
pub(super) fn find_companion_lrc(
    audio_path: &std::path::Path,
    duration: f64,
) -> Option<(std::path::PathBuf, Vec<loopkaraoke::karaoke::LyricLine>)> {
    let audio_stem = audio_path.file_stem()?.to_str()?;
    let direct = audio_path.with_extension("lrc");
    let lrc_path = if direct.is_file() {
        direct
    } else {
        let parent = audio_path.parent()?;
        let entries = std::fs::read_dir(parent).ok()?;
        entries.flatten().map(|e| e.path()).find(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("lrc"))
                && p.file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.eq_ignore_ascii_case(audio_stem))
        })?
    };

    let content = std::fs::read_to_string(&lrc_path).ok()?;
    let mut lyrics = loopkaraoke::karaoke::lrc::parse_lrc(&content);
    if lyrics.is_empty() {
        let plain_lines: Vec<&str> = content
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !(l.starts_with('[') && l.ends_with(']')))
            .collect();
        if !plain_lines.is_empty() {
            let total = duration.max(10.0);
            let step = (total / (plain_lines.len() as f64 + 1.0)).clamp(1.5, 5.0);
            lyrics = plain_lines
                .into_iter()
                .enumerate()
                .map(|(i, text)| {
                    let start = (i as f64) * step;
                    let end = start + step;
                    let words = loopkaraoke::karaoke::spread_words(text, start, step);
                    loopkaraoke::karaoke::LyricLine {
                        start,
                        end,
                        text: text.to_owned(),
                        words,
                    }
                })
                .collect();
        }
    }
    Some((lrc_path, lyrics))
}

/// Menampilkan lencana (badge) hijau "LRC" sebagai penanda lagu memiliki lirik.
pub(super) fn draw_lrc_badge(ui: &mut egui::Ui, line_count: usize) {
    let frame = egui::Frame::new()
        .fill(egui::Color32::from_rgb(20, 83, 45))
        .stroke(egui::Stroke::new(1.0_f32, theme::SUCCESS))
        .corner_radius(3)
        .inner_margin(egui::Margin::symmetric(5, 1));
    frame.show(ui, |ui| {
        ui.label(
            egui::RichText::new("LRC")
                .size(10.0)
                .strong()
                .color(theme::SUCCESS),
        )
        .on_hover_text(if line_count > 0 {
            format!("Lagu ini memiliki lirik LRC ({line_count} baris)")
        } else {
            "Lagu ini memiliki berkas lirik LRC".to_owned()
        });
    });
}
