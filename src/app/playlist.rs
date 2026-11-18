use eframe::egui;
use loopkaraoke::{karaoke::ass::StyleConfig, media::probe_audio};

use super::{theme, Event, StudioApp};

pub(super) const AUDIO_EXTENSIONS: [&str; 7] = ["mp3", "wav", "m4a", "aac", "flac", "ogg", "opus"];

impl StudioApp {
    pub(super) fn build_style_config(&self) -> StyleConfig {
        let mut cfg = self.style.to_config();
        let enable_karaoke = self.show_karaoke_lyrics && self.has_any_track_lyrics();
        cfg.show_lyrics = Some(enable_karaoke);
        cfg.show_intro = Some(self.style.show_intro);
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
                    "Aktifkan lirik karaoke saat render",
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
