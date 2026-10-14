use eframe::egui;
use loopkaraoke::media::{build_chapters, format_clock, probe_audio, ChapterSource};

use super::{help::playlist as help, theme, Event, StudioApp};

pub(super) const AUDIO_EXTENSIONS: [&str; 7] = ["mp3", "wav", "m4a", "aac", "flac", "ogg", "opus"];

enum RowAction {
    Remove(usize),
    MoveUp(usize),
    MoveDown(usize),
}

impl StudioApp {
    pub(super) fn playlist_tab(&mut self, ui: &mut egui::Ui) {
        theme::heading_with_info(ui, "Langkah 2 — Daftar Lagu", help::HEADING);
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
                    if track.lyrics.is_empty() {
                        ui.weak("tanpa lirik");
                    } else {
                        ui.label(format!("{} baris", track.lyrics.len()));
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
                self.tracks.remove(index);
                self.selected_track = self.selected_track.min(self.tracks.len().saturating_sub(1));
            }
            Some(RowAction::MoveUp(index)) => self.tracks.swap(index, index - 1),
            Some(RowAction::MoveDown(index)) => self.tracks.swap(index, index + 1),
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

    fn build_timestamps(&self) -> String {
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
