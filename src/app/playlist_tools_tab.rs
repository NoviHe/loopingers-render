use eframe::egui;
use loopkaraoke::media::{build_chapters, ChapterSource};

use super::{
    generate_tab::{card_header, generate_card, outline_icon_button, GenerateIcon, GenerateTrack},
    theme::{self, UiText},
    StudioApp, Tab,
};

impl StudioApp {
    pub(super) fn playlist_tools_tab(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 8.0;
        ui.page_title("Playlist");
        ui.muted_label("Kelola daftar lagu Generate, susun urutan album, dan siapkan teks daftar lagu serta timestamp.");
        ui.add_space(4.0);

        generate_card(ui, |ui| {
            card_header(
                ui,
                GenerateIcon::Music,
                "Sumber Lagu",
                "Sinkron dengan Generate dan Batch Render.",
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if outline_icon_button(ui, GenerateIcon::PlusCircle, "Tambah Lagu").clicked() {
                    self.pick_generate_audio_files();
                }
                if outline_icon_button(ui, GenerateIcon::Folder, "Tambah Folder").clicked() {
                    self.pick_generate_audio_folders();
                }
                if outline_icon_button(ui, GenerateIcon::Play, "Buka Generate").clicked() {
                    self.tab = Tab::Generate;
                }
            });
            ui.muted_label("Seret file audio atau folder ke halaman ini. Perubahan daftar dan urutan langsung berlaku di Generate dan Batch Render.");
        });

        if ui.available_width() >= 1080.0 {
            ui.columns(2, |columns| {
                self.generate_playlist_card(&mut columns[0]);
                self.playlist_text_tools(&mut columns[1], true);
            });
        } else {
            self.generate_playlist_card(ui);
            self.playlist_text_tools(ui, false);
        }
    }

    fn playlist_text_tools(&mut self, ui: &mut egui::Ui, stacked: bool) {
        // Derive both previews after playlist actions so order and timings never go stale.
        let (list, timestamps) = playlist_texts(&self.generate.tracks);
        let enabled = !self.generate.tracks.is_empty();
        let draw_list = |ui: &mut egui::Ui| {
            text_tool_card(
                ui,
                "Daftar Lagu",
                "Daftar bernomor sesuai urutan album.",
                &list,
                enabled,
            )
        };
        let draw_timestamps = |ui: &mut egui::Ui| {
            text_tool_card(
                ui,
                "Timestamp YouTube",
                "Waktu mulai dihitung dari durasi asli setiap lagu.",
                &timestamps,
                enabled,
            )
        };
        let ((copy_list, export_list), (copy_timestamps, export_timestamps)) =
            if !stacked && ui.available_width() >= 720.0 {
                ui.columns(2, |columns| {
                    (draw_list(&mut columns[0]), draw_timestamps(&mut columns[1]))
                })
            } else {
                (draw_list(ui), draw_timestamps(ui))
            };
        if copy_list || copy_timestamps {
            ui.ctx().copy_text(if copy_timestamps {
                timestamps.clone()
            } else {
                list.clone()
            });
            self.info(if copy_timestamps {
                "Timestamp disalin ke clipboard."
            } else {
                "Daftar lagu disalin ke clipboard."
            });
        }
        if export_list || export_timestamps {
            let (filename, text) = if export_timestamps {
                ("Timestamp.txt", timestamps)
            } else {
                ("playlist.txt", list)
            };
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Teks", &["txt"])
                .set_file_name(filename)
                .save_file()
            {
                match std::fs::write(&path, text) {
                    Ok(()) => self.info(format!("Teks disimpan: {}", path.display())),
                    Err(error) => self.error(format!("Gagal menyimpan teks: {error}")),
                }
            }
        }
    }
}

fn playlist_texts(tracks: &[GenerateTrack]) -> (String, String) {
    let list = tracks
        .iter()
        .enumerate()
        .map(|(index, track)| format!("{}. {}", index + 1, track.title))
        .collect::<Vec<_>>()
        .join("\n");
    let sources = tracks
        .iter()
        .map(|track| ChapterSource {
            title: Some(track.title.clone()),
            filename: track
                .path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned()),
            duration: track.duration,
        })
        .collect::<Vec<_>>();
    let timestamps = build_chapters(&sources)
        .iter()
        .map(|chapter| format!("{} {}", chapter.time_str, chapter.title))
        .collect::<Vec<_>>()
        .join("\n");
    (list, timestamps)
}

fn text_tool_card(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: &str,
    text: &str,
    enabled: bool,
) -> (bool, bool) {
    let mut copy = false;
    let mut export = false;
    generate_card(ui, |ui| {
        card_header(ui, GenerateIcon::Copy, title, "");
        ui.muted_label(subtitle);
        ui.add_space(6.0);
        let p = theme::Palette::of(ui.ctx());
        egui::Frame::new()
            .fill(p.bg)
            .stroke(egui::Stroke::new(1.0_f32, p.border))
            .corner_radius(8)
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::ScrollArea::vertical()
                    .id_salt(title)
                    .max_height(170.0)
                    .min_scrolled_height(100.0)
                    .show(ui, |ui| {
                        if enabled {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(text).monospace().color(p.text),
                                )
                                .wrap(),
                            );
                        } else {
                            ui.muted_label("Tambahkan lagu untuk menampilkan daftar.");
                        }
                    });
            });
        ui.add_space(6.0);
        ui.add_enabled_ui(enabled, |ui| {
            ui.horizontal_wrapped(|ui| {
                copy = outline_icon_button(ui, GenerateIcon::Copy, "Salin").clicked();
                export = outline_icon_button(ui, GenerateIcon::Folder, "Simpan .txt").clicked();
            });
        });
    });
    (copy, export)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: u32, title: &str, duration: f64) -> GenerateTrack {
        GenerateTrack {
            id,
            path: format!("{title}.mp3").into(),
            title: title.into(),
            duration,
            lrc: None,
        }
    }

    #[test]
    fn list_and_timestamps_follow_current_generate_order() {
        let mut tracks = vec![track(1, "Pertama", 65.0), track(2, "Kedua", 120.0)];
        let (list, timestamps) = playlist_texts(&tracks);
        assert_eq!(list, "1. Pertama\n2. Kedua");
        assert_eq!(timestamps, "00:00 Pertama\n01:05 Kedua");
        tracks.swap(0, 1);
        let (list, timestamps) = playlist_texts(&tracks);
        assert_eq!(list, "1. Kedua\n2. Pertama");
        assert_eq!(timestamps, "00:00 Kedua\n02:00 Pertama");
    }

    #[test]
    fn empty_generate_playlist_has_no_export_text() {
        assert_eq!(playlist_texts(&[]), (String::new(), String::new()));
    }
}
