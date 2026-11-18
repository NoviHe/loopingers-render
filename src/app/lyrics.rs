use std::{path::PathBuf, time::Instant};

use eframe::egui;
use loopkaraoke::{
    karaoke::{
        lrc::parse_lrc,
        preview::PreviewFile,
        spread_words,
        styles::THEMES,
        transcribe::{ModelSize, TranscribeOptions},
        LyricLine,
    },
    media::{clean_proper_title, format_clock, probe_audio, AudioInfo},
    render::presets,
};

use super::{
    generate_tab::{card_header, generate_card, outline_icon_button, GenerateIcon},
    help::lyrics as help,
    playlist::AUDIO_EXTENSIONS,
    theme::{self, UiText},
    Event, StudioApp,
};

const LANGUAGES: [(&str, Option<&str>); 5] = [
    ("Deteksi otomatis", None),
    ("Indonesia", Some("id")),
    ("Inggris", Some("en")),
    ("Jepang", Some("ja")),
    ("Korea", Some("ko")),
];

pub const KARAOKE_FONTS: [super::thumbnail_tab::FontChoice; 10] = [
    super::thumbnail_tab::FontChoice {
        id: "impact",
        label: "Impact (Standar Tebal YouTube / Karaoke)",
        filename: "impact.ttf",
    },
    super::thumbnail_tab::FontChoice {
        id: "ariblk",
        label: "Arial Black (Ultra Bold)",
        filename: "ariblk.ttf",
    },
    super::thumbnail_tab::FontChoice {
        id: "bahnschrift",
        label: "Bahnschrift (Modern Clean)",
        filename: "bahnschrift.ttf",
    },
    super::thumbnail_tab::FontChoice {
        id: "segoeuib",
        label: "Segoe UI Bold (Elegan)",
        filename: "segoeuib.ttf",
    },
    super::thumbnail_tab::FontChoice {
        id: "trebucbd",
        label: "Trebuchet MS Bold",
        filename: "trebucbd.ttf",
    },
    super::thumbnail_tab::FontChoice {
        id: "georgiab",
        label: "Georgia Bold (Serif Klasik)",
        filename: "georgiab.ttf",
    },
    super::thumbnail_tab::FontChoice {
        id: "comicbd",
        label: "Comic Sans Bold (Kasual)",
        filename: "comicbd.ttf",
    },
    super::thumbnail_tab::FontChoice {
        id: "segoesc",
        label: "Segoe Script (Cursive / Kaligrafi)",
        filename: "segoesc.ttf",
    },
    super::thumbnail_tab::FontChoice {
        id: "ariali",
        label: "Arial Italic (Miring)",
        filename: "ariali.ttf",
    },
    super::thumbnail_tab::FontChoice {
        id: "inter-semibold",
        label: "Inter Bold (Bawaan Aplikasi)",
        filename: "",
    },
];

pub fn karaoke_font_ass_name(font_preset: usize, custom_font: &Option<PathBuf>) -> String {
    if let Some(path) = custom_font {
        if let Some(family) = super::thumbnail_tab::extract_font_family_name(path) {
            return family;
        }
        if let Some(stem) = path.file_stem() {
            return stem.to_string_lossy().to_string();
        }
    }
    let choice_id = KARAOKE_FONTS
        .get(font_preset)
        .map(|f| f.id)
        .unwrap_or("impact");
    match choice_id {
        "impact" => "Impact".to_owned(),
        "ariblk" => "Arial Black".to_owned(),
        "bahnschrift" => "Bahnschrift".to_owned(),
        "segoeuib" => "Segoe UI".to_owned(),
        "trebucbd" => "Trebuchet MS".to_owned(),
        "georgiab" => "Georgia".to_owned(),
        "comicbd" => "Comic Sans MS".to_owned(),
        "segoesc" => "Segoe Script".to_owned(),
        "ariali" => "Arial".to_owned(),
        _ => "Arial".to_owned(),
    }
}

/// Menandai teks karaoke yang menumpuk dengan judul / daftar lagu pada pratinjau.
pub(super) fn draw_karaoke_overlap_marker(
    painter: &egui::Painter,
    viewport: egui::Rect,
    karaoke_rect: egui::Rect,
) {
    let painter = painter.with_clip_rect(viewport);
    painter.rect_stroke(
        karaoke_rect,
        3.0,
        egui::Stroke::new(1.6_f32, theme::DANGER),
        egui::StrokeKind::Outside,
    );
    let label_pos = egui::pos2(viewport.right() - 6.0, viewport.top() + 6.0);
    let galley = painter.layout_no_wrap(
        "⚠ Karaoke menumpuk".to_owned(),
        egui::FontId::proportional(11.0),
        egui::Color32::WHITE,
    );
    let bg = egui::Rect::from_min_size(
        egui::pos2(label_pos.x - galley.size().x - 8.0, label_pos.y),
        galley.size() + egui::vec2(8.0, 4.0),
    );
    painter.rect_filled(bg, 3.0, theme::DANGER);
    painter.galley(bg.min + egui::vec2(4.0, 2.0), galley, egui::Color32::WHITE);
}

pub(super) fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0);
    format!("{:02}:{:05.2}", (s / 60.0) as u32, s % 60.0)
}

pub(super) fn format_lrc_lines(lines: &[LyricLine]) -> String {
    let mut out = String::new();
    for line in lines {
        out.push_str(&format!("[{}] {}\n", clock(line.start), line.text.trim()));
    }
    out
}

pub(super) mod win_audio {
    use std::path::Path;

    pub fn volume_to_mci(volume_pct: f32) -> u32 {
        (volume_pct.clamp(0.0, 100.0) * 10.0).round() as u32
    }

    #[cfg(windows)]
    mod imp {
        use std::{os::windows::ffi::OsStrExt, path::Path};

        #[link(name = "winmm")]
        extern "system" {
            fn mciSendStringW(
                lpszCommand: *const u16,
                lpszReturnString: *mut u16,
                cchReturn: u32,
                hwndCallback: usize,
            ) -> u32;
        }

        fn send(cmd: &str) -> bool {
            let wide: Vec<u16> = std::ffi::OsStr::new(cmd)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            unsafe { mciSendStringW(wide.as_ptr(), std::ptr::null_mut(), 0, 0) == 0 }
        }

        pub fn play_from(wav_path: &Path, start_sec: f64, volume_pct: f32) {
            let _ = send("close lrc_studio_audio");
            // Open with `mpegvideo` first so `setaudio ... volume to <0..1000>` scales preview audio;
            // fall back to `waveaudio` if the stream rejects `mpegvideo`.
            let mpeg_cmd = format!(
                "open \"{}\" type mpegvideo alias lrc_studio_audio",
                wav_path.display()
            );
            let wave_cmd = format!(
                "open \"{}\" type waveaudio alias lrc_studio_audio",
                wav_path.display()
            );
            if send(&mpeg_cmd) || send(&wave_cmd) {
                let _ = send("set lrc_studio_audio time format milliseconds");
                set_volume(volume_pct);
                let ms = (start_sec.max(0.0) * 1000.0).round() as u64;
                let _ = send(&format!("play lrc_studio_audio from {ms}"));
            }
        }

        pub fn set_volume(volume_pct: f32) {
            let mci_vol = super::volume_to_mci(volume_pct);
            let _ = send(&format!("setaudio lrc_studio_audio volume to {mci_vol}"));
        }

        pub fn stop() {
            let _ = send("stop lrc_studio_audio");
            let _ = send("close lrc_studio_audio");
        }
    }

    #[cfg(not(windows))]
    mod imp {
        use std::path::Path;
        pub fn play_from(_wav_path: &Path, _start_sec: f64, _volume_pct: f32) {}
        pub fn set_volume(_volume_pct: f32) {}
        pub fn stop() {}
    }

    pub fn play_from(wav_path: &Path, start_sec: f64) {
        imp::play_from(wav_path, start_sec, 100.0);
    }

    pub fn play_from_with_volume(wav_path: &Path, start_sec: f64, volume_pct: f32) {
        imp::play_from(wav_path, start_sec, volume_pct);
    }

    pub fn set_volume(volume_pct: f32) {
        imp::set_volume(volume_pct);
    }

    pub fn stop() {
        imp::stop();
    }
}

pub fn is_break_line_text(text: &str) -> bool {
    let t = text.trim();
    t.is_empty()
        || t == "..."
        || t == "…"
        || t == "♪"
        || t == "♪ ... ♪"
        || t.eq_ignore_ascii_case("[intro]")
        || t.eq_ignore_ascii_case("[instrumental]")
        || t.eq_ignore_ascii_case("[interlude]")
        || t.eq_ignore_ascii_case("[musik]")
        || t.eq_ignore_ascii_case("[music]")
        || t.eq_ignore_ascii_case("[solo]")
        || t.eq_ignore_ascii_case("[break]")
}

fn is_intro_section_header(line: &str) -> bool {
    let t = line.trim();
    t.eq_ignore_ascii_case("[intro]") || t.eq_ignore_ascii_case("(intro)")
}

fn is_interlude_section_header(line: &str) -> bool {
    let t = line.trim().to_ascii_lowercase();
    if !((t.starts_with('[') && t.ends_with(']')) || (t.starts_with('(') && t.ends_with(')'))) {
        return false;
    }
    let inner = &t[1..t.len() - 1];
    inner.contains("instrument")
        || inner.contains("instrumen")
        || inner.contains("interlude")
        || inner.contains("solo")
        || inner.contains("musik")
        || inner.contains("music")
        || inner.contains("break")
}

fn unique_standalone_audio_paths(loaded: &[PathBuf], paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let identity =
        |path: &std::path::Path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let mut seen: std::collections::HashSet<PathBuf> =
        loaded.iter().map(|path| identity(path)).collect();
    paths
        .into_iter()
        .filter(|path| seen.insert(identity(path)))
        .collect()
}

pub struct StandaloneTrack {
    pub id: u32,
    pub info: AudioInfo,
    pub title: String,
    pub wav_preview: Option<PreviewFile>,
    pub lines: Vec<LyricLine>,
    pub raw_lyrics_input: String,
    pub active_row: usize,
    pub transcribing: bool,
    pub saved_lrc_path: Option<PathBuf>,
}

impl StandaloneTrack {
    pub fn has_lyrics(&self) -> bool {
        !self.lines.is_empty()
    }

    pub fn duration(&self) -> f64 {
        self.info.duration.max(1.0)
    }

    pub fn apply_whisper_lines(
        &mut self,
        lyrics: Vec<LyricLine>,
        add_intro: bool,
        min_first_vocal_sec: f64,
        add_interludes: bool,
        interlude_gap_sec: f64,
    ) {
        if self.raw_lyrics_input.trim().is_empty() {
            self.raw_lyrics_input = lyrics
                .iter()
                .filter(|l| !is_break_line_text(&l.text))
                .map(|l| l.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
        }
        self.lines = lyrics;
        self.apply_intro_and_interlude_markers(
            add_intro,
            min_first_vocal_sec,
            add_interludes,
            interlude_gap_sec,
        );
        self.active_row = self
            .lines
            .iter()
            .position(|l| !is_break_line_text(&l.text))
            .unwrap_or(0);
    }

    /// Ensures the song does not immediately show vocal lyrics at `00:00` (inserting `[00:00.00] ...`)
    /// and automatically inserts `[mm:ss.xx] ...` markers during long instrumental gaps in the middle.
    pub fn apply_intro_and_interlude_markers(
        &mut self,
        add_intro: bool,
        min_first_vocal_sec: f64,
        add_interludes: bool,
        interlude_gap_sec: f64,
    ) {
        if self.lines.is_empty() {
            return;
        }
        self.lines.sort_by(|a, b| a.start.total_cmp(&b.start));

        // Normalize existing break lines to "..." and deduplicate consecutive break lines
        let mut deduped: Vec<LyricLine> = Vec::with_capacity(self.lines.len() + 4);
        for mut line in self.lines.drain(..) {
            if is_break_line_text(&line.text) {
                line.text = "...".to_owned();
                if deduped
                    .last()
                    .is_some_and(|prev| is_break_line_text(&prev.text))
                {
                    continue;
                }
            }
            deduped.push(line);
        }
        self.lines = deduped;
        if self.lines.is_empty() {
            return;
        }

        // 1. Intro at 00:00.00
        if add_intro {
            if !is_break_line_text(&self.lines[0].text) {
                let min_vocal = min_first_vocal_sec.max(1.0);
                if self.lines[0].start < min_vocal {
                    let shift = min_vocal - self.lines[0].start;
                    self.lines[0].start = min_vocal;
                    self.lines[0].end = (self.lines[0].end + shift).max(min_vocal + 1.5);
                    for j in 1..self.lines.len() {
                        if self.lines[j].start < self.lines[j - 1].start + 1.0 {
                            let new_s = self.lines[j - 1].start + 1.5;
                            let dur = (self.lines[j].end - self.lines[j].start).max(1.5);
                            self.lines[j].start = new_s;
                            self.lines[j].end = new_s + dur;
                        }
                    }
                }
                let first_vocal_start = self.lines[0].start;
                self.lines.insert(
                    0,
                    LyricLine {
                        start: 0.0,
                        end: first_vocal_start,
                        text: "...".to_owned(),
                        words: Vec::new(),
                    },
                );
            } else {
                self.lines[0].start = 0.0;
                self.lines[0].text = "...".to_owned();
            }
        }

        // 2. Instrumental breaks in the middle of the song
        if add_interludes && self.lines.len() >= 2 {
            let min_gap = interlude_gap_sec.max(2.0);
            let mut with_breaks: Vec<LyricLine> = Vec::with_capacity(self.lines.len() + 6);
            let len = self.lines.len();
            for i in 0..len {
                let cur = self.lines[i].clone();
                let cur_is_break = is_break_line_text(&cur.text);
                with_breaks.push(cur.clone());

                if i + 1 < len && !cur_is_break && !is_break_line_text(&self.lines[i + 1].text) {
                    let next_start = self.lines[i + 1].start;
                    let word_count = cur.text.split_whitespace().count();
                    let est_sing_dur = (word_count as f64 * 0.45).clamp(2.0, 5.5);
                    let raw_end = if cur.end > cur.start + 0.3 && cur.end < next_start {
                        cur.end.min(cur.start + 6.0)
                    } else {
                        cur.start + est_sing_dur
                    };
                    let vocal_end = ((raw_end * 100.0).round() / 100.0)
                        .clamp(cur.start + 1.2, (next_start - 0.5).max(cur.start + 1.2));

                    if next_start - vocal_end >= min_gap {
                        with_breaks.push(LyricLine {
                            start: vocal_end,
                            end: next_start,
                            text: "...".to_owned(),
                            words: Vec::new(),
                        });
                    }
                }
            }
            self.lines = with_breaks;
        }

        self.recalculate_ends_and_words();
        if self.active_row == 0 && self.lines.len() > 1 && is_break_line_text(&self.lines[0].text) {
            self.active_row = 1;
        } else if self.active_row >= self.lines.len() {
            self.active_row = self.lines.len().saturating_sub(1);
        }
    }

    pub fn recalculate_ends_and_words(&mut self) {
        let max_dur = self.duration();
        let len = self.lines.len();
        for i in 0..len {
            let start = self.lines[i].start.max(0.0);
            let next_start = if i + 1 < len {
                self.lines[i + 1].start.max(start + 0.5)
            } else {
                (start + 4.0).min(max_dur).max(start + 0.5)
            };
            let is_break = is_break_line_text(&self.lines[i].text);
            let end = if is_break {
                next_start
            } else {
                (start + 6.5).min(next_start).max(start + 0.5)
            };
            self.lines[i].start = start;
            self.lines[i].end = end;
            self.lines[i].words = spread_words(&self.lines[i].text, start, (end - start).max(0.5));
        }
    }
}

pub struct StandaloneLyricsState {
    pub tracks: Vec<StandaloneTrack>,
    pub selected_track: usize,
    pub next_id: u32,
    pub loading_audio: bool,
    pub loading_preview_id: Option<u32>,
    pub preview_request_id: u64,
    pub playing: bool,
    pub current_time: f64,
    pub last_tick: Option<Instant>,
    pub skip_section_headers: bool,
    pub auto_intro_marker: bool,
    pub default_intro_seconds: f64,
    pub auto_interlude_marker: bool,
    pub interlude_min_gap_seconds: f64,
    pub whisper_model: ModelSize,
    pub whisper_language: usize,
    pub whisper_use_gpu: bool,
}

impl Default for StandaloneLyricsState {
    fn default() -> Self {
        Self {
            tracks: Vec::new(),
            selected_track: 0,
            next_id: 1,
            loading_audio: false,
            loading_preview_id: None,
            preview_request_id: 0,
            playing: false,
            current_time: 0.0,
            last_tick: None,
            skip_section_headers: true,
            auto_intro_marker: true,
            default_intro_seconds: 8.0,
            auto_interlude_marker: true,
            interlude_min_gap_seconds: 5.0,
            whisper_model: ModelSize::Base,
            whisper_language: 0,
            whisper_use_gpu: true,
        }
    }
}

impl Drop for StandaloneLyricsState {
    fn drop(&mut self) {
        win_audio::stop();
    }
}

impl StandaloneLyricsState {
    pub fn current_track(&self) -> Option<&StandaloneTrack> {
        if self.tracks.is_empty() {
            None
        } else {
            self.tracks
                .get(self.selected_track.min(self.tracks.len() - 1))
        }
    }

    pub fn current_track_mut(&mut self) -> Option<&mut StandaloneTrack> {
        if self.tracks.is_empty() {
            None
        } else {
            let idx = self.selected_track.min(self.tracks.len() - 1);
            self.selected_track = idx;
            self.tracks.get_mut(idx)
        }
    }

    pub fn duration(&self) -> f64 {
        self.current_track().map(|t| t.duration()).unwrap_or(180.0)
    }

    pub fn add_audio_track(&mut self, info: AudioInfo, wav_preview: Option<PreviewFile>) {
        let id = self.next_id;
        self.next_id += 1;
        let title = clean_proper_title(&info.title);
        let was_empty = self.tracks.is_empty();
        let companion =
            super::playlist::find_companion_lrc(std::path::Path::new(&info.path), info.duration);
        let (saved_lrc_path, lines) = companion
            .map(|(path, lines)| (Some(path), lines))
            .unwrap_or_default();
        let raw_lyrics_input = lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        self.tracks.push(StandaloneTrack {
            id,
            info,
            title,
            wav_preview,
            lines,
            raw_lyrics_input,
            active_row: 0,
            transcribing: false,
            saved_lrc_path,
        });
        if was_empty {
            self.selected_track = 0;
            self.current_time = 0.0;
        }
    }

    pub fn remove_track(&mut self, index: usize) {
        if index >= self.tracks.len() {
            return;
        }
        if index == self.selected_track {
            self.stop_playback();
            self.current_time = 0.0;
        } else if index < self.selected_track {
            self.selected_track -= 1;
        }
        self.tracks.remove(index);
        self.selected_track = self.selected_track.min(self.tracks.len().saturating_sub(1));
    }

    pub fn clear_tracks(&mut self) {
        self.stop_playback();
        self.tracks.clear();
        self.selected_track = 0;
        self.current_time = 0.0;
    }

    pub fn select_track(&mut self, index: usize) {
        if index < self.tracks.len() && self.selected_track != index {
            self.stop_playback();
            self.selected_track = index;
            self.current_time = 0.0;
        }
    }

    pub fn start_playback(&mut self) {
        if let Some(track) = self.current_track() {
            if let Some(wav) = &track.wav_preview {
                if wav.path().is_file() {
                    win_audio::play_from(wav.path(), self.current_time);
                    self.playing = true;
                    self.last_tick = Some(Instant::now());
                }
            }
        }
    }

    pub fn stop_playback(&mut self) {
        self.preview_request_id = self.preview_request_id.wrapping_add(1);
        self.loading_preview_id = None;
        self.playing = false;
        self.last_tick = None;
        win_audio::stop();
    }

    pub(super) fn preview_request_matches(&self, track_id: u32, request_id: u64) -> bool {
        self.loading_preview_id == Some(track_id)
            && self.preview_request_id == request_id
            && self
                .current_track()
                .is_some_and(|track| track.id == track_id)
    }

    pub fn seek_to(&mut self, time: f64) {
        let max_dur = self.duration();
        self.current_time = time.clamp(0.0, max_dur);
        self.last_tick = Some(Instant::now());
        if self.playing {
            if let Some(track) = self.current_track() {
                if let Some(wav) = &track.wav_preview {
                    if wav.path().is_file() {
                        win_audio::play_from(wav.path(), self.current_time);
                    }
                }
            }
        }
    }

    pub fn tick(&mut self, ctx: &egui::Context) {
        if !self.playing {
            return;
        }
        let now = Instant::now();
        if let Some(prev) = self.last_tick {
            let dt = now.duration_since(prev).as_secs_f64();
            self.current_time += dt;
            let max_dur = self.duration();
            if self.current_time >= max_dur {
                self.current_time = max_dur;
                self.stop_playback();
                return;
            }
        }
        self.last_tick = Some(now);
        ctx.request_repaint_after(std::time::Duration::from_millis(30));
    }
}

impl StudioApp {
    pub(super) fn lyrics_tab(&mut self, ui: &mut egui::Ui) {
        let can_pin = ui.input(|input| input.viewport().maximized.unwrap_or(false));
        let pinned = can_pin && self.lyrics_header_pinned;
        self.lrc_studio.tick(ui.ctx());
        ui.spacing_mut().item_spacing.y = 8.0;
        theme::page_header(
            ui,
            "Lirik",
            "Studio LRC untuk membuat, menyelaraskan waktu, dan memperbaiki lirik lagu.",
        )
        .on_hover_text(help::HEADING);
        ui.add_space(4.0);
        generate_card(ui, |ui| {
            super::generate_tab::pinnable_card_header(
                ui,
                GenerateIcon::Music,
                "Studio Lirik",
                "Mulai dari lagu, buat lirik, lalu simpan LRC.",
                can_pin.then_some(&mut self.lyrics_header_pinned),
            );
            ui.add_space(6.0);
            let has_track = self.lrc_studio.current_track().is_some();
            let ready: Vec<_> = self
                .lrc_studio
                .tracks
                .iter()
                .enumerate()
                .filter_map(|(index, track)| track.has_lyrics().then_some(index))
                .collect();
            let can_import = !self.lrc_studio.loading_audio
                && self.generate.tracks.iter().any(|track| {
                    !self
                        .lrc_studio
                        .tracks
                        .iter()
                        .any(|loaded| std::path::Path::new(&loaded.info.path) == track.path)
                });
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.add_enabled_ui(can_import, |ui| {
                    if outline_icon_button(ui, GenerateIcon::PlusCircle, "Ambil dari Generate")
                        .clicked()
                    {
                        let paths = self
                            .generate
                            .tracks
                            .iter()
                            .filter(|track| {
                                !self.lrc_studio.tracks.iter().any(|loaded| {
                                    std::path::Path::new(&loaded.info.path) == track.path
                                })
                            })
                            .map(|track| track.path.clone())
                            .collect();
                        self.load_standalone_audios(paths);
                    }
                });
                ui.add_enabled_ui(has_track, |ui| {
                    if outline_icon_button(ui, GenerateIcon::Folder, "Impor LRC / Teks").clicked() {
                        self.pick_standalone_lrc_file();
                    }
                });
                ui.add_enabled_ui(!ready.is_empty(), |ui| {
                    let label = if ready.len() > 1 {
                        "Simpan Semua LRC"
                    } else {
                        "Simpan LRC"
                    };
                    if outline_icon_button(ui, GenerateIcon::Copy, label).clicked() {
                        match ready.as_slice() {
                            [index] => self.export_lrc_track(*index),
                            [] => {}
                            _ => self.export_all_lrc_tracks(&ready),
                        }
                    }
                });
            });
            ui.muted_label("Ambil lagu Generate atau tambahkan audio sendiri. File LRC dengan nama yang sama dibaca otomatis saat lagu dimuat.");
        });
        super::generate_tab::pinned_content(ui, pinned, "lyrics_scroll", |ui| {
            ui.add_space(4.0);
            let maximized = ui
                .ctx()
                .input(|input| input.viewport().maximized.unwrap_or(false));
            if maximized {
                let content_width = ui.available_width();
                let columns_width = (content_width - 32.0).max(0.0);
                ui.set_width(columns_width);
                ui.set_max_width(columns_width);
                ui.columns(2, |columns| {
                    self.standalone_lyrics_left(&mut columns[0], false, true);
                    let left_layout_bottom = columns[0].min_rect().bottom();
                    self.standalone_lyrics_right(&mut columns[1], true, Some(left_layout_bottom));
                });
            } else {
                self.standalone_lyrics_left(ui, true, false);
            }
        });
    }

    pub(super) fn karaoke_has_wide_layout(ui: &egui::Ui) -> bool {
        ui.available_width() >= 1080.0
            && ui
                .ctx()
                .input(|input| input.viewport().maximized.unwrap_or(false))
    }

    pub(super) fn karaoke_tab(&mut self, ui: &mut egui::Ui) {
        let wide = Self::karaoke_has_wide_layout(ui);
        let pinned = self.karaoke_preview_pinned;
        self.update_album_preview(ui);
        ui.spacing_mut().item_spacing.y = 8.0;
        theme::page_header(
            ui,
            "Karaoke",
            "Atur gaya teks dan periksa posisi lirik pada video album.",
        );
        ui.add_space(4.0);
        generate_card(ui, |ui| {
            ui.horizontal(|ui| {
                card_header(
                    ui,
                    GenerateIcon::Sliders,
                    "Tampilan Karaoke",
                    "Font, warna, posisi, dan preview lirik.",
                );
                if wide {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let pinned = self.karaoke_preview_pinned;
                        let hint = if pinned {
                            "Lepas pin tampilan Karaoke"
                        } else {
                            "Sematkan tampilan Karaoke"
                        };
                        if super::generate_tab::square_icon_button(
                            ui,
                            GenerateIcon::Pin,
                            true,
                            pinned,
                            hint,
                        )
                        .clicked()
                        {
                            self.karaoke_preview_pinned = !pinned;
                        }
                    });
                }
            });
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if outline_icon_button(ui, GenerateIcon::Music, "Buka Playlist").clicked() {
                    self.tab = super::Tab::Playlist;
                }
                if outline_icon_button(ui, GenerateIcon::Folder, "Kelola LRC").clicked() {
                    self.tab = super::Tab::Lyrics;
                }
                if outline_icon_button(ui, GenerateIcon::Play, "Buka Generate").clicked() {
                    self.tab = super::Tab::Generate;
                }
                ui.add_enabled_ui(
                    self.generate.video.is_some() && !self.thumbnail.extracting_frame,
                    |ui| {
                        if outline_icon_button(ui, GenerateIcon::Reset, "Ambil Frame Video")
                            .clicked()
                        {
                            self.extract_thumbnail_frame();
                        }
                    },
                );
            });
            ui.muted_label("Preview memakai lagu dan video Generate. Pembakaran lirik dengan gaya ini tersedia melalui Render Old; Generate saat ini belum membakar lirik karaoke.");
        });
        if wide {
            ui.columns(2, |columns| {
                let mut settings = |ui: &mut egui::Ui| {
                    self.album_preview_playlist_card(ui);
                    self.karaoke_settings_card(ui);
                };
                if pinned {
                    egui::ScrollArea::vertical()
                        .id_salt("karaoke_settings_scroll")
                        .auto_shrink([false, false])
                        .show(&mut columns[0], settings);
                } else {
                    settings(&mut columns[0]);
                }
                generate_card(&mut columns[1], |ui| self.karaoke_preview_card(ui));
            });
        } else {
            self.album_preview_playlist_card(ui);
            generate_card(ui, |ui| self.karaoke_preview_card(ui));
            self.karaoke_settings_card(ui);
        }
    }

    fn karaoke_settings_card(&mut self, ui: &mut egui::Ui) {
        generate_card(ui, |ui| {
            card_header(
                ui,
                GenerateIcon::Sliders,
                "Gaya Lirik",
                "Pengaturan teks karaoke dan tata letak.",
            );
            ui.add_space(8.0);
            self.style_section(ui);
        });
    }

    /// Preview seperti Thumbnail / Main Menu: background + daftar lagu + contoh teks karaoke,
    /// lengkap dengan peringatan bila posisi karaoke menumpuk dengan judul / daftar lagu.
    fn karaoke_preview_card(&mut self, ui: &mut egui::Ui) {
        let p = theme::Palette::of(ui.ctx());
        let preview_path = self.thumbnail_preview_path();

        ui.horizontal(|ui| {
            card_header(
                ui,
                GenerateIcon::Play,
                "Preview Karaoke",
                "Lihat gaya dan posisi lirik pada video.",
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(video) = &self.generate.video {
                    ui.weak(format!("{}x{}", video.width, video.height));
                }
            });
        });
        ui.add_space(4.0);

        let outer_w = ui.available_width().max(260.0);
        let outer_h = outer_w * (9.0 / 16.0);
        let (outer_rect, _) =
            ui.allocate_exact_size(egui::vec2(outer_w, outer_h), egui::Sense::hover());
        let outer_painter = ui.painter_at(outer_rect);
        outer_painter.rect_filled(outer_rect, 4.0, egui::Color32::from_rgb(8, 10, 16));

        let target_aspect = self
            .generate
            .video
            .as_ref()
            .map(|video| video.width.max(1) as f32 / video.height.max(1) as f32)
            .unwrap_or(16.0 / 9.0);
        let (vp_w, vp_h) = if outer_w / outer_h > target_aspect {
            (outer_h * target_aspect, outer_h)
        } else {
            (outer_w, outer_w / target_aspect)
        };
        let viewport = egui::Rect::from_center_size(outer_rect.center(), egui::vec2(vp_w, vp_h));
        let vp_painter = ui.painter_at(viewport);

        // Background (frame video / gambar), atau pemandangan contoh jika belum ada background
        if let Some(path) = preview_path {
            egui::Image::new(format!("file://{}", path.display()))
                .fit_to_exact_size(viewport.size())
                .corner_radius(8)
                .paint_at(ui, viewport);
        } else {
            super::thumbnail_tab::draw_scenic_placeholder(&vp_painter, viewport);
        }

        let occupied = if self.thumbnail.tracklist_on_video && !self.generate.tracks.is_empty() {
            self.draw_thumbnail_overlay(&vp_painter, viewport, false)
        } else {
            Vec::new()
        };
        if self.style.show_intro {
            self.draw_karaoke_intro(&vp_painter, viewport);
        }
        let (sample, fraction, show_example_input) = self.karaoke_preview_line();
        let karaoke_rect = if sample.is_empty() {
            egui::Rect::NOTHING
        } else {
            self.draw_karaoke_overlay(&vp_painter, viewport, &sample, fraction)
        };
        let overlapping = occupied.iter().any(|r| r.intersects(karaoke_rect));
        if overlapping {
            draw_karaoke_overlap_marker(&vp_painter, viewport, karaoke_rect);
        }

        vp_painter.rect_stroke(
            viewport,
            3.0,
            egui::Stroke::new(1.0_f32, p.border),
            egui::StrokeKind::Inside,
        );

        ui.add_space(6.0);
        if overlapping {
            ui.colored_label(
                theme::WARNING,
                "⚠ Teks karaoke menumpuk dengan judul / daftar lagu. Ubah Posisi atau Jarak dari tepi.",
            );
        } else {
            ui.colored_label(theme::SUCCESS, "✓ Posisi karaoke aman, tidak menumpuk.");
        }
        if self.generate.video.is_none() {
            ui.muted_label("Pilih video di Generate untuk menyiapkan latar preview.");
        }

        if show_example_input {
            ui.add_space(6.0);
            ui.label("Teks contoh:");
            theme::text_field(
                ui,
                &mut self.karaoke_preview_text,
                "Preview teks karaoke Anda",
                theme::InputWidth::Fill,
            )
            .on_hover_text(
                "Coba teks panjang untuk memastikan lirik yang terbungkus 2 baris tetap tidak menumpuk",
            );
        }
    }

    fn draw_karaoke_intro(&self, painter: &egui::Painter, rect: egui::Rect) {
        let banner = &self.style.intro_banner;
        let (title, artist) = self
            .generate
            .tracks
            .get(self.visualizer_selected_track)
            .map(|track| {
                (
                    loopkaraoke::media::clean_proper_title(&track.title),
                    loopkaraoke::media::clean_artist(&track.artist),
                )
            })
            .unwrap_or_else(|| ("Judul Lagu".into(), "Nama Penyanyi".into()));
        let font = KARAOKE_FONTS[self.style.intro_font_preset].id;
        let family = egui::FontFamily::Name(font.into());
        let output_height = self
            .generate
            .video
            .as_ref()
            .map_or(1080, |video| video.height.max(1));
        let scale = rect.height() / output_height as f32;
        let [r, g, b] = banner.color;
        let color = egui::Color32::from_rgb(r, g, b);
        let painter = painter.with_clip_rect(rect);
        let layout = |text: String, size: u32| {
            let mut job = egui::text::LayoutJob::simple(
                text,
                egui::FontId::new((size as f32 * scale).max(1.0), family.clone()),
                color,
                rect.width(),
            );
            job.halign = egui::Align::Center;
            painter.layout_job(job)
        };
        let artist = (!artist.trim().is_empty() && !artist.eq_ignore_ascii_case(&title))
            .then(|| layout(artist, banner.size * 65 / 100));
        let title = layout(title, banner.size);
        let height = title.size().y + artist.as_ref().map_or(0.0, |text| text.size().y);
        let x = rect.left() + rect.width() * banner.x / 100.0;
        let y = rect.top() + rect.height() * banner.y / 100.0 - height / 2.0;
        let artist_y = y + title.size().y;
        painter.galley(egui::pos2(x - title.rect.center().x, y), title, color);
        if let Some(artist) = artist {
            painter.galley(
                egui::pos2(x - artist.rect.center().x, artist_y),
                artist,
                color,
            );
        }
    }

    fn karaoke_preview_line(&mut self) -> (String, f32, bool) {
        if self.generate.tracks.is_empty() {
            let custom = self.karaoke_preview_text.trim();
            let sample = if custom.is_empty() {
                "Preview teks karaoke Anda"
            } else {
                custom
            };
            return (sample.to_owned(), 0.45, true);
        }
        let lines = self
            .generate
            .tracks
            .get(self.visualizer_selected_track)
            .and_then(|track| {
                if let Some(studio) = self.lrc_studio.tracks.iter().find(|studio| {
                    track.path == std::path::Path::new(&studio.info.path) && studio.has_lyrics()
                }) {
                    return Some(studio.lines.as_slice());
                }
                let (path, _) = track.lrc.as_ref()?;
                if self
                    .karaoke_preview_lrc
                    .as_ref()
                    .is_none_or(|(cached, _)| cached != path)
                {
                    let lines = std::fs::read_to_string(path)
                        .map(|content| parse_lrc(&content))
                        .unwrap_or_default();
                    self.karaoke_preview_lrc = Some((path.clone(), lines));
                }
                self.karaoke_preview_lrc
                    .as_ref()
                    .map(|(_, lines)| lines.as_slice())
            });
        let Some(lines) = lines.filter(|lines| !lines.is_empty()) else {
            return (self.karaoke_preview_text.trim().to_owned(), 0.45, true);
        };
        let time = self.visualizer_time;
        let current = lines.iter().rev().find(|line| line.start <= time);
        match current.filter(|line| time < line.end) {
            Some(line) => (
                line.text.clone(),
                ((time - line.start) / (line.end - line.start)).clamp(0.0, 1.0) as f32,
                false,
            ),
            None => (String::new(), 0.0, false),
        }
    }

    /// Menggambar contoh teks karaoke sesuai gaya (font, warna, posisi, jarak tepi) pada area video.
    /// Skala mengikuti PlayResY resolusi output agar proporsinya sama dengan hasil render ASS.
    /// Mengembalikan area teks yang digambar.
    pub(super) fn draw_karaoke_overlay(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        text: &str,
        sung_fraction: f32,
    ) -> egui::Rect {
        let painter = painter.with_clip_rect(rect);
        let out_h = if self.tab == super::Tab::Karaoke {
            self.generate
                .video
                .as_ref()
                .map(|video| video.height)
                .unwrap_or(1080)
        } else {
            presets::dimensions(self.aspect_ratio, self.resolution).1
        };
        let scale = rect.height() / out_h.max(1) as f32;
        let style = &self.style;
        let color = |c: [u8; 3]| egui::Color32::from_rgb(c[0], c[1], c[2]);

        let font_family = if style.custom_font.is_some() {
            egui::FontFamily::Name("karaoke_custom".into())
        } else {
            let choice_id = KARAOKE_FONTS[style.font_preset.min(KARAOKE_FONTS.len() - 1)].id;
            if choice_id == "inter-semibold" {
                egui::FontFamily::Proportional
            } else {
                egui::FontFamily::Name(choice_id.into())
            }
        };
        let font = egui::FontId::new((style.font_size as f32 * 0.8 * scale).max(5.0), font_family);
        let side_margin = 30.0 * scale;
        let wrap_w = (rect.width() - side_margin * 2.0).max(20.0);
        let layout = |c: egui::Color32| {
            let mut job = egui::text::LayoutJob::simple(text.to_owned(), font.clone(), c, wrap_w);
            job.halign = egui::Align::Center;
            painter.layout_job(job)
        };

        let base = layout(color(style.normal));
        let center_x = rect.left() + rect.width() * (style.pos_x.clamp(0.0, 100.0) / 100.0);
        let center_y = rect.top() + rect.height() * (style.pos_y.clamp(0.0, 100.0) / 100.0);
        let origin = egui::pos2(
            center_x - base.rect.center().x,
            center_y - base.rect.center().y,
        );
        let text_rect = base.rect.translate(origin.to_vec2());

        let outline_w = (3.0 * scale).max(0.6);
        let shadow_d = (2.0 * scale).max(0.5);
        let shadow = color(style.shadow).gamma_multiply(0.6);
        painter.galley(
            origin + egui::vec2(outline_w + shadow_d, outline_w + shadow_d),
            layout(shadow),
            shadow,
        );
        let outline = color(style.outline);
        for (dx, dy) in [
            (-1.0, 0.0),
            (1.0, 0.0),
            (0.0, -1.0),
            (0.0, 1.0),
            (-0.7, -0.7),
            (0.7, -0.7),
            (-0.7, 0.7),
            (0.7, 0.7),
        ] {
            painter.galley(
                origin + egui::vec2(dx * outline_w, dy * outline_w),
                layout(outline),
                outline,
            );
        }
        painter.galley(origin, base, color(style.normal));

        // Bagian yang sedang dinyanyikan (efek wipe) memakai warna "Bernyanyi"
        let sung = egui::Rect::from_min_max(
            text_rect.min,
            egui::pos2(
                text_rect.left() + text_rect.width() * sung_fraction,
                text_rect.bottom(),
            ),
        );
        painter.with_clip_rect(sung.intersect(rect)).galley(
            origin,
            layout(color(style.singing)),
            color(style.singing),
        );

        text_rect.expand(outline_w)
    }

    fn standalone_lyrics_left(
        &mut self,
        ui: &mut egui::Ui,
        show_editor_after_whisper: bool,
        maximized: bool,
    ) {
        let p = theme::Palette::of(ui.ctx());
        let frame = egui::Frame::new()
            .fill(p.surface)
            .stroke(egui::Stroke::new(1.0_f32, p.border))
            .corner_radius(12)
            .inner_margin(egui::Margin::symmetric(16, 12));

        // Card 1: Pilih Multi Lagu & Pemutar Audio Mandiri
        frame.show(ui, |ui| {
            let max_w = ui.available_width();
            ui.set_width(max_w);
            ui.set_max_width(max_w);

            let total_tracks = self.lrc_studio.tracks.len();
            let completed_tracks = self
                .lrc_studio
                .tracks
                .iter()
                .filter(|t| t.has_lyrics())
                .count();

            ui.horizontal_wrapped(|ui| {
                card_header(ui, GenerateIcon::Music, "Lagu & Pratinjau", "");
                if total_tracks > 0 {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let badge_col = if completed_tracks == total_tracks {
                            p.success
                        } else {
                            p.accent
                        };
                        ui.colored_label(
                            badge_col,
                            format!("✓ {completed_tracks} / {total_tracks} lagu sudah ada lirik"),
                        );
                    });
                }
            });
            ui.add_space(4.0);

            ui.horizontal_wrapped(|ui| {
                let busy = self.lrc_studio.loading_audio;
                let btn_label = if busy {
                    "⏳ Memuat Lagu..."
                } else {
                    "🎵 Pilih Multi Berkas Audio..."
                };
                if ui
                    .add_enabled(!busy, theme::primary_button(btn_label))
                    .on_hover_text("Pilih satu atau banyak berkas lagu sekaligus (.mp3, .wav, .flac, .m4a, .ogg)")
                    .clicked()
                {
                    self.pick_standalone_audios();
                }

                if ui
                    .add_enabled(!busy, theme::StyledButton::secondary("📁 Tambah dari Folder..."))
                    .on_hover_text("Muat seluruh berkas audio dari sebuah folder")
                    .clicked()
                {
                    self.pick_standalone_audio_folder();
                }

                if total_tracks > 0
                    && ui
                        .add_enabled(!busy, theme::StyledButton::ghost("🗑 Kosongkan Lagu").small())
                        .on_hover_text("Hapus semua lagu dari daftar Studio Lirik")
                        .clicked()
                {
                    self.lrc_studio.clear_tracks();
                    self.storage = None;
                }
            });

            ui.add_space(6.0);

            if self.lrc_studio.tracks.is_empty() {
                ui.weak("Belum ada lagu dimuat. Klik '🎵 Pilih Multi Berkas Audio...' untuk menambahkan lagu.");
            } else {
                self.lrc_studio.selected_track = self
                    .lrc_studio
                    .selected_track
                    .min(self.lrc_studio.tracks.len() - 1);

                // Daftar Lagu dengan tanda status sudah/belum bikin lirik
                let list_frame = egui::Frame::new()
                    .fill(p.surface_alt)
                    .stroke(egui::Stroke::new(1.0_f32, p.border))
                    .corner_radius(4)
                    .inner_margin(egui::Margin::same(6));

                let mut select_idx: Option<usize> = None;
                let mut remove_idx: Option<usize> = None;
                let current_sel = self.lrc_studio.selected_track;
                let visible_track_rows = self.lrc_studio.tracks.len().min(5);
                let track_list_height = visible_track_rows as f32 * 42.0;

                list_frame.show(ui, |ui| {
                    let list_w = ui.available_width();
                    ui.set_width(list_w);
                    ui.set_max_width(list_w);
                    egui::ScrollArea::vertical()
                        .id_salt("standalone_tracks_list_scroll")
                        .min_scrolled_height(track_list_height)
                        .max_height(track_list_height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let row_w = (ui.available_width() - 10.0).max(120.0);
                            for (idx, trk) in self.lrc_studio.tracks.iter().enumerate() {
                                let is_sel = idx == current_sel;
                                let has_lrc = trk.has_lyrics();
                                let line_cnt = trk.lines.len();

                                let bg = if is_sel {
                                    p.accent.gamma_multiply(0.12)
                                } else if idx % 2 == 0 {
                                    p.surface
                                } else {
                                    p.surface_alt
                                };
                                let border = if is_sel {
                                    egui::Stroke::new(1.0_f32, p.accent)
                                } else {
                                    egui::Stroke::NONE
                                };

                                egui::Frame::new()
                                    .fill(bg)
                                    .stroke(border)
                                    .corner_radius(4)
                                    .inner_margin(egui::Margin::symmetric(6, 3))
                                    .show(ui, |ui| {
                                        ui.set_width(row_w);
                                        ui.set_max_width(row_w);
                                        ui.horizontal(|ui| {
                                            // Tanda status lirik
                                            if trk.transcribing {
                                                ui.spinner();
                                            } else if has_lrc {
                                                ui.colored_label(p.success, "✓")
                                                    .on_hover_text("Lagu ini sudah memiliki lirik");
                                            } else {
                                                ui.colored_label(p.text_muted, "○")
                                                    .on_hover_text("Lagu ini belum memiliki lirik");
                                            }

                                            let title_label = format!(
                                                "{:02}. {} ({})",
                                                idx + 1,
                                                trk.title,
                                                format_clock(trk.info.duration.round() as u64)
                                            );
                                            if ui.selectable_label(is_sel, title_label).clicked() {
                                                select_idx = Some(idx);
                                            }

                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if ui
                                                        .small_button("✕")
                                                        .on_hover_text("Hapus lagu ini dari daftar")
                                                        .clicked()
                                                    {
                                                        remove_idx = Some(idx);
                                                    }

                                                    if trk.transcribing {
                                                        ui.colored_label(
                                                            p.accent,
                                                            "⏳ Transkrip...",
                                                        );
                                                    } else if has_lrc {
                                                        let badge = if trk.saved_lrc_path.is_some()
                                                        {
                                                            format!("✓ Lirik ({line_cnt} baris • Tersimpan)")
                                                        } else {
                                                            format!("✓ Sudah Ada Lirik ({line_cnt} baris)")
                                                        };
                                                        ui.colored_label(p.success, badge);
                                                    } else {
                                                        ui.colored_label(
                                                            p.warning,
                                                            "Belum Ada Lirik",
                                                        );
                                                    }
                                                },
                                            );
                                        });
                                    });
                                ui.add_space(2.0);
                            }
                        });
                });

                if let Some(idx) = select_idx {
                    self.lrc_studio.select_track(idx);
                }
                if let Some(idx) = remove_idx {
                    self.lrc_studio.remove_track(idx);
                    self.storage = None;
                }
            }

            ui.add_space(6.0);

            // Audio Transport & Timeline Scrubber
            let max_dur = self.lrc_studio.duration();
            let cur_time = self.lrc_studio.current_time;
            let is_playing = self.lrc_studio.playing;
            let is_preparing = self.lrc_studio.loading_preview_id.is_some();
            let has_track = !self.lrc_studio.tracks.is_empty();

            use super::visualizer_playlist::{preview_timeline, transport_button, TransportIcon};
            ui.horizontal_wrapped(|ui| {
                let (icon, hint) = if is_preparing {
                    (TransportIcon::Stop, "Batal putar preview")
                } else if is_playing {
                    (TransportIcon::Pause, "Jeda preview")
                } else {
                    (TransportIcon::Play, "Putar preview")
                };
                if transport_button(ui, icon, has_track, hint).clicked() {
                    if is_playing || is_preparing {
                        self.lrc_studio.stop_playback();
                    } else {
                        self.start_standalone_playback();
                    }
                }
                if transport_button(ui, TransportIcon::Back, has_track, "Mundur 3 detik").clicked() {
                    self.lrc_studio.seek_to(cur_time - 3.0);
                }
                if transport_button(ui, TransportIcon::Forward, has_track, "Maju 3 detik").clicked() {
                    self.lrc_studio.seek_to(cur_time + 3.0);
                }
                if transport_button(ui, TransportIcon::Stop, has_track, "Hentikan dan kembali ke 0:00").clicked() {
                    self.lrc_studio.seek_to(0.0);
                    self.lrc_studio.stop_playback();
                }
                if is_preparing {
                    ui.spinner().on_hover_text("Menyiapkan preview...");
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.monospace(
                        egui::RichText::new(format!("{} / {}", clock(cur_time), clock(max_dur)))
                            .color(p.accent),
                    );
                });
            });

            let mut seek_pos = cur_time;
            if preview_timeline(ui, &mut seek_pos, max_dur, has_track).changed() {
                self.lrc_studio.seek_to(seek_pos);
            }

            ui.add_space(4.0);
            self.draw_standalone_karaoke_monitor(ui);
        });

        ui.add_space(8.0);

        // Card 2: Transkrip Awal dengan Whisper AI
        frame.show(ui, |ui| {
            let max_w = ui.available_width();
            ui.set_width(max_w);
            ui.set_max_width(max_w);
            ui.horizontal(|ui| {
                card_header(ui, GenerateIcon::Sliders, "Whisper AI", "Buat draf lirik dan timestamp secara otomatis.");
                theme::info(ui, help::WHISPER);
            });
            ui.add_space(2.0);
            ui.small(
                "Gunakan Whisper AI untuk mengambil perkiraan waktu (timestamp) otomatis dari lagu, lalu koreksi kata-kata yang salah di editor manual.",
            );
            ui.add_space(4.0);

            let previous_model = self.lrc_studio.whisper_model;
            if !ModelSize::ALL.contains(&self.lrc_studio.whisper_model) {
                self.lrc_studio.whisper_model = ModelSize::Base;
            }
            ui.horizontal_wrapped(|ui| {
                theme::label_with_info(ui, "Model:", help::MODEL);
                egui::ComboBox::from_id_salt("standalone_whisper_model")
                    .selected_text(self.lrc_studio.whisper_model.name())
                    .show_ui(ui, |ui| {
                        for model in ModelSize::ALL {
                            ui.selectable_value(
                                &mut self.lrc_studio.whisper_model,
                                model,
                                model.name(),
                            );
                        }
                    });
                theme::label_with_info(ui, "Bahasa:", help::LANGUAGE);
                egui::ComboBox::from_id_salt("standalone_whisper_lang")
                    .selected_text(LANGUAGES[self.lrc_studio.whisper_language].0)
                    .show_ui(ui, |ui| {
                        for (index, (label, _)) in LANGUAGES.iter().enumerate() {
                            ui.selectable_value(&mut self.lrc_studio.whisper_language, index, *label);
                        }
                    });
            });

            if previous_model != self.lrc_studio.whisper_model {
                self.save_settings();
            }
            let gpu_available = self
                .hardware
                .as_ref()
                .is_some_and(|hardware| hardware.gpu_whisper);
            let cur_transcribing = self
                .lrc_studio
                .current_track()
                .is_some_and(|t| t.transcribing);
            let can_transcribe_current =
                self.lrc_studio.current_track().is_some() && !cur_transcribing;
            let without_lyrics_count = self
                .lrc_studio
                .tracks
                .iter()
                .filter(|t| !t.has_lyrics() && !t.transcribing)
                .count();

            ui.horizontal_wrapped(|ui| {
                ui.add_enabled(
                    gpu_available,
                    egui::Checkbox::new(&mut self.lrc_studio.whisper_use_gpu, "Pakai GPU"),
                ).on_hover_text(help::GPU);

                let btn_text = if cur_transcribing {
                    "⏳ Sedang Mentranskrip..."
                } else {
                    "🤖 Transkrip Lagu Ini"
                };
                if ui
                    .add_enabled(can_transcribe_current, theme::primary_button(btn_text))
                    .on_hover_text("Jalankan Whisper AI pada lagu yang sedang dipilih")
                    .clicked()
                {
                    let idx = self.lrc_studio.selected_track;
                    self.transcribe_standalone_track(idx);
                }

                if ui
                    .add_enabled(
                        without_lyrics_count > 0,
                        theme::StyledButton::secondary(format!(
                            "🤖 Semua Belum Ber-lirik ({without_lyrics_count})"
                        )),
                    )
                    .on_hover_text("Jalankan Whisper AI secara berurutan untuk semua lagu yang belum memiliki lirik")
                    .clicked()
                {
                    for idx in 0..self.lrc_studio.tracks.len() {
                        if !self.lrc_studio.tracks[idx].has_lyrics()
                            && !self.lrc_studio.tracks[idx].transcribing
                        {
                            self.transcribe_standalone_track(idx);
                        }
                    }
                }
            });
        });

        if show_editor_after_whisper {
            ui.add_space(8.0);
            self.standalone_lyrics_right(ui, maximized, None);
        }

        ui.add_space(8.0);

        // Card 3: Input / Tempel Teks Lirik Asli (Suno / Udio / Manual) & Pengaturan Intro / Instrumen
        frame.show(ui, |ui| {
            let max_w = ui.available_width();
            ui.set_width(max_w);
            ui.set_max_width(max_w);
            card_header(ui, GenerateIcon::PlusCircle, "Teks & Jeda", "Tempel lirik asli dan atur intro atau instrumen.");
            ui.add_space(2.0);
            ui.small(
                "Atur agar lagu tidak langsung menampilkan lirik di 00:00 (beri \"...\" saat intro) serta otomatis memberi \"...\" saat instrumen di tengah lagu.",
            );
            ui.add_space(4.0);

            ui.horizontal_wrapped(|ui| {
                ui.checkbox(
                    &mut self.lrc_studio.auto_intro_marker,
                    "Beri Intro [00:00.00] ... (jangan langsung lirik di 00:00)",
                );
                if self.lrc_studio.auto_intro_marker {
                    ui.label("Mulai vokal:");
                    ui.add(
                        egui::DragValue::new(&mut self.lrc_studio.default_intro_seconds)
                            .speed(0.5)
                            .range(1.0..=60.0)
                            .suffix(" dtk"),
                    )
                    .on_hover_text(
                        "Waktu mulai baris vokal pertama saat membuat lirik manual (atau batas minimal intro awal)",
                    );
                }
            });

            ui.horizontal_wrapped(|ui| {
                ui.checkbox(
                    &mut self.lrc_studio.auto_interlude_marker,
                    "Sisipkan \"...\" otomatis saat jeda instrumen tengah",
                );
                if self.lrc_studio.auto_interlude_marker {
                    ui.label("Jeda ≥");
                    ui.add(
                        egui::DragValue::new(&mut self.lrc_studio.interlude_min_gap_seconds)
                            .speed(0.5)
                            .range(2.0..=30.0)
                            .suffix(" dtk"),
                    )
                    .on_hover_text(
                        "Jika jarak selesai vokal ke baris berikutnya melebihi durasi ini, otomatis sisipkan baris \"...\" (instrumen)",
                    );
                }
            });

            ui.checkbox(
                &mut self.lrc_studio.skip_section_headers,
                "Abaikan [Verse]/[Chorus] & ubah [Instrumental]/[Interlude]/[Solo] jadi \"...\"",
            );

            if let Some(track) = self.lrc_studio.current_track_mut() {
                ui.add(
                    egui::TextEdit::multiline(&mut track.raw_lyrics_input)
                        .hint_text("Tempel lirik lagu asli di sini (satu baris per kalimat)...\nContoh:\n[Intro]\nDi tengah malam yang sunyi\nKu dengar suara-Mu memanggil\n[Instrumental]\nBawalah hatiku pulang")
                        .background_color(p.bg)
                        .margin(egui::Margin::symmetric(10, 8))
                        .desired_width(f32::INFINITY)
                        .desired_rows(5),
                );
            } else {
                let mut empty = String::new();
                ui.add_enabled(
                    false,
                    egui::TextEdit::multiline(&mut empty)
                        .hint_text("Pilih atau muat berkas lagu terlebih dahulu di bagian 1...")
                        .background_color(p.bg)
                        .margin(egui::Margin::symmetric(10, 8))
                        .desired_width(f32::INFINITY)
                        .desired_rows(5),
                );
            }

            ui.add_space(4.0);
            let (has_raw, has_lines) = self
                .lrc_studio
                .current_track()
                .map(|t| (!t.raw_lyrics_input.trim().is_empty(), t.has_lyrics()))
                .unwrap_or((false, false));

            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        has_raw,
                        theme::primary_button("📋 Buat Daftar Baris Lirik dari Teks Ini"),
                    )
                    .on_hover_text("Pecah teks di atas menjadi baris-baris lirik (dengan [00:00.00] ... untuk intro)")
                    .clicked()
                {
                    self.import_raw_lyrics_to_studio(false);
                }

                if ui
                    .add_enabled(
                        has_raw && has_lines,
                        theme::StyledButton::secondary("🔄 Timpa Teks Saja (Pertahankan Timestamp & Jeda ...)"),
                    )
                    .on_hover_text("Ganti teks vokal hasil Whisper yang salah dengan teks asli di atas tanpa merusak waktu [mm:ss.xx] maupun baris Intro/Instrumen (...)")
                    .clicked()
                {
                    self.import_raw_lyrics_to_studio(true);
                }
            });
        });

        ui.add_space(8.0);

        // Card 4: Pratinjau & Ekspor Output .LRC
        frame.show(ui, |ui| {
            let max_w = ui.available_width();
            ui.set_width(max_w);
            ui.set_max_width(max_w);
            card_header(
                ui,
                GenerateIcon::Copy,
                "Hasil LRC",
                "Pratinjau, salin, dan simpan lirik bertimestamp.",
            );
            ui.add_space(4.0);

            let (mut lrc_preview, has_lines, saved_path) = self
                .lrc_studio
                .current_track()
                .map(|t| {
                    (
                        format_lrc_lines(&t.lines),
                        t.has_lyrics(),
                        t.saved_lrc_path.clone(),
                    )
                })
                .unwrap_or_else(|| (String::new(), false, None));

            ui.add(
                egui::TextEdit::multiline(&mut lrc_preview)
                    .hint_text(
                        "[00:00.00] Pratinjau isi berkas .LRC lagu aktif akan muncul di sini...",
                    )
                    .background_color(p.bg)
                    .margin(egui::Margin::symmetric(10, 8))
                    .desired_width(f32::INFINITY)
                    .desired_rows(4)
                    .interactive(false),
            );

            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        has_lines,
                        theme::primary_button("💾 Simpan sebagai Berkas .LRC..."),
                    )
                    .clicked()
                {
                    self.export_standalone_lrc_file();
                }

                if ui
                    .add_enabled(
                        has_lines,
                        theme::StyledButton::secondary("📋 Salin .LRC ke Clipboard"),
                    )
                    .clicked()
                {
                    ui.ctx().copy_text(lrc_preview);
                    self.info("Teks .LRC berhasil disalin ke clipboard.");
                }
            });

            if let Some(saved) = saved_path {
                ui.add_space(4.0);
                ui.colored_label(p.success, format!("✓ Tersimpan di: {}", saved.display()));
            }
        });
    }

    fn standalone_lyrics_right(
        &mut self,
        ui: &mut egui::Ui,
        maximized: bool,
        layout_bottom: Option<f32>,
    ) {
        let p = theme::Palette::of(ui.ctx());
        let frame = egui::Frame::new()
            .fill(p.surface)
            .stroke(egui::Stroke::new(1.0_f32, p.border))
            .corner_radius(12)
            .inner_margin(egui::Margin::symmetric(16, 12));

        frame.show(ui, |ui| {
            let max_w = ui.available_width();
            ui.set_width(max_w);
            ui.set_max_width(max_w);

            let (track_title, line_count) = self
                .lrc_studio
                .current_track()
                .map(|t| (t.title.clone(), t.lines.len()))
                .unwrap_or_else(|| ("Belum ada lagu dipilih".to_owned(), 0));

            ui.horizontal_wrapped(|ui| {
                card_header(ui, GenerateIcon::Music, "Editor & Sinkronisasi", "");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.small("Klik ⏱ saat vokal masuk, atau 🎵 saat masuk instrumen");
                });
            });
            ui.muted_label(format!("{track_title} · {line_count} baris"));
            ui.add_space(6.0);

            if self.lrc_studio.current_track().is_none() {
                ui.weak("Pilih atau tambahkan lagu di panel Lagu & Pratinjau.");
                return;
            }

            // Panel Tap Sync Cepat
            let tap_frame = egui::Frame::new()
                .fill(p.surface_alt)
                .stroke(egui::Stroke::new(1.0_f32, p.accent))
                .corner_radius(6)
                .inner_margin(egui::Margin::same(8));

            let mut do_tap = false;
            let mut do_insert_break_now = false;
            tap_frame.show(ui, |ui| {
                ui.set_width(ui.available_width());
                let cur_time = self.lrc_studio.current_time;
                let Some(track) = self.lrc_studio.current_track_mut() else {
                    return;
                };
                let count = track.lines.len();
                if count == 0 {
                    ui.weak(
                        "Belum ada baris lirik. Tempel teks di panel Teks & Jeda, jalankan Whisper AI, atau tambahkan baris baru.",
                    );
                } else {
                    track.active_row = track.active_row.min(count.saturating_sub(1));
                    let active_idx = track.active_row;
                    let target_text = track.lines[active_idx].text.clone();

                    ui.horizontal_wrapped(|ui| {
                        let tap_btn = theme::StyledButton::secondary(
                            egui::RichText::new(format!(
                                "⏱ Tandai Waktu Baris #{:02} ke [{}]",
                                active_idx + 1,
                                clock(cur_time)
                            ))
                            .size(13.5)
                            .strong(),
                        )
                        .min_size(egui::vec2(245.0, 32.0));

                        if ui
                            .add(tap_btn)
                            .on_hover_text("Set waktu mulai baris aktif ke posisi lagu sekarang dan otomatis pindah ke baris berikutnya")
                            .clicked()
                        {
                            do_tap = true;
                        }

                        let break_btn = theme::StyledButton::secondary(
                            egui::RichText::new(format!(
                                "🎵 Masuk Instrumen [...] di [{}]",
                                clock(cur_time)
                            ))
                            .size(12.5),
                        )
                        .min_size(egui::vec2(195.0, 32.0));

                        if ui
                            .add(break_btn)
                            .on_hover_text("Sisipkan baris jeda instrumen \"...\" pada waktu lagu sekarang (saat penyanyi berhenti menyanyi dan musik instrumen dimulai)")
                            .clicked()
                        {
                            do_insert_break_now = true;
                        }

                        if ui
                            .button("⏮")
                            .on_hover_text("Pindahkan target tap ke baris sebelumnya")
                            .clicked()
                        {
                            track.active_row = active_idx.saturating_sub(1);
                        }
                        if ui
                            .button("⏭")
                            .on_hover_text("Pindahkan target tap ke baris berikutnya")
                            .clicked()
                            && active_idx + 1 < count
                        {
                            track.active_row = active_idx + 1;
                        }
                    });
                    ui.small(format!(
                        "🎯 Target Baris #{:02}: \"{}\"",
                        active_idx + 1,
                        if target_text.trim().is_empty() {
                            "(baris kosong)"
                        } else if is_break_line_text(&target_text) {
                            "... (Intro / Jeda Instrumen)"
                        } else {
                            target_text.trim()
                        }
                    ));
                }
            });

            if do_tap {
                self.tap_sync_active_row();
            }
            if do_insert_break_now {
                self.insert_instrumental_break_at_current_time();
            }

            ui.add_space(6.0);

            // Toolbar Editor Baris
            let cur_time = self.lrc_studio.current_time;
            let add_intro = self.lrc_studio.auto_intro_marker;
            let min_vocal = self.lrc_studio.default_intro_seconds;
            let interlude_gap = self.lrc_studio.interlude_min_gap_seconds;

            if let Some(track) = self.lrc_studio.current_track_mut() {
                let toolbar_width = ui.available_width();
                ui.set_width(toolbar_width);
                ui.set_max_width(toolbar_width);
                let draw_primary = |ui: &mut egui::Ui, track: &mut StandaloneTrack| {
                    if ui
                        .button("🎵 + Intro [00:00] ...")
                        .on_hover_text("Pastikan di awal lagu (00:00.00) terdapat baris Intro \"...\" sehingga lirik tidak langsung tampil di 00:00")
                        .clicked()
                    {
                        if track.lines.is_empty() {
                            track.lines.push(LyricLine {
                                start: 0.0,
                                end: min_vocal.max(4.0),
                                text: "...".to_owned(),
                                words: Vec::new(),
                            });
                        } else {
                            track.apply_intro_and_interlude_markers(
                                true,
                                min_vocal.max(2.0),
                                false,
                                interlude_gap,
                            );
                        }
                    }

                    if ui
                        .add_enabled(
                            !track.lines.is_empty(),
                            theme::StyledButton::secondary("✨ Auto Intro & Jeda (...)"),
                        )
                        .on_hover_text(
                            "Otomatis tambahkan [00:00.00] ... di awal lagu dan sisipkan \"...\" pada setiap jeda instrumen panjang di tengah lagu",
                        )
                        .clicked()
                    {
                        track.apply_intro_and_interlude_markers(
                            add_intro,
                            min_vocal.max(2.0),
                            true,
                            interlude_gap,
                        );
                    }

                    if ui
                        .button("➕ Tambah Baris")
                        .on_hover_text("Tambahkan baris lirik baru di bagian akhir")
                        .clicked()
                    {
                        let start = track.lines.last().map(|l| l.end).unwrap_or(cur_time);
                        track.lines.push(LyricLine {
                            start,
                            end: start + 3.0,
                            text: String::new(),
                            words: Vec::new(),
                        });
                        track.active_row = track.lines.len().saturating_sub(1);
                    }

                };
                let draw_adjustments = |ui: &mut egui::Ui, track: &mut StandaloneTrack| {
                    if ui
                        .add_enabled(track.lines.len() > 1, theme::StyledButton::secondary("⇅ Urutkan Waktu"))
                        .on_hover_text("Urutkan baris lirik dari waktu paling awal ke paling akhir")
                        .clicked()
                    {
                        track.lines.sort_by(|a, b| a.start.total_cmp(&b.start));
                        track.recalculate_ends_and_words();
                    }

                    if ui
                        .add_enabled(!track.lines.is_empty(), theme::StyledButton::secondary("⏪ -0.5s"))
                        .on_hover_text("Majukan seluruh timestamp vokal 0.5 detik lebih awal")
                        .clicked()
                    {
                        for line in &mut track.lines {
                            if line.start > 0.0 || !is_break_line_text(&line.text) {
                                line.start = (line.start - 0.5).max(0.0);
                            }
                        }
                        track.recalculate_ends_and_words();
                    }

                    if ui
                        .add_enabled(!track.lines.is_empty(), theme::StyledButton::secondary("⏩ +0.5s"))
                        .on_hover_text("Mundurkan seluruh timestamp vokal 0.5 detik lebih lambat")
                        .clicked()
                    {
                        for (idx, line) in track.lines.iter_mut().enumerate() {
                            if !(idx == 0 && line.start == 0.0 && is_break_line_text(&line.text)) {
                                line.start += 0.5;
                            }
                        }
                        track.recalculate_ends_and_words();
                    }

                    if ui
                        .add_enabled(!track.lines.is_empty(), theme::StyledButton::secondary("🗑 Hapus Semua"))
                        .on_hover_text("Kosongkan seluruh daftar baris lirik lagu ini")
                        .clicked()
                    {
                        track.lines.clear();
                        track.active_row = 0;
                    }                };

                if maximized {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| draw_primary(ui, track));
                        ui.horizontal(|ui| draw_adjustments(ui, track));
                    });
                } else {
                    ui.horizontal(|ui| {
                        draw_primary(ui, track);
                        draw_adjustments(ui, track);
                    });
                }
            }

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(4.0);

            // Daftar Baris Lirik Editable
            enum LineAction {
                Select(usize),
                SetNow(usize),
                Seek(f64),
                InsertAfter(usize),
                InsertBreakAfter(usize),
                Remove(usize),
                Changed,
            }
            let mut action: Option<LineAction> = None;
            let active_row = self
                .lrc_studio
                .current_track()
                .map(|t| t.active_row)
                .unwrap_or(0);

            if let Some(track) = self.lrc_studio.current_track_mut() {
                let row_height = 80.0;
                let editor_height = if maximized {
                    layout_bottom
                        .map(|bottom| (bottom - ui.cursor().top() - 12.0).max(row_height))
                        .unwrap_or_else(|| ui.available_height().max(row_height))
                } else {
                    track.lines.len().min(5) as f32 * row_height
                };
                egui::ScrollArea::vertical()
                    .id_salt("standalone_lrc_editor_scroll")
                    .min_scrolled_height(editor_height)
                    .max_height(editor_height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let row_inner_w = (ui.available_width() - 16.0).max(160.0);
                        for (idx, line) in track.lines.iter_mut().enumerate() {
                            let is_playing_line = cur_time >= line.start && cur_time < line.end;
                            let is_target = idx == active_row;
                            let is_break = is_break_line_text(&line.text);

                            let row_bg = if is_target {
                                egui::Color32::from_rgb(36, 48, 72)
                            } else if is_playing_line {
                                egui::Color32::from_rgb(44, 42, 24)
                            } else if is_break {
                                egui::Color32::from_rgb(24, 34, 42)
                            } else if idx % 2 == 0 {
                                p.surface_alt
                            } else {
                                p.surface
                            };

                            let row_border = if is_target {
                                egui::Stroke::new(1.0_f32, p.accent)
                            } else if is_playing_line {
                                egui::Stroke::new(1.0_f32, p.warning)
                            } else {
                                egui::Stroke::NONE
                            };

                            egui::Frame::new()
                                .fill(row_bg)
                                .stroke(row_border)
                                .corner_radius(4)
                                .inner_margin(egui::Margin::symmetric(6, 4))
                                .show(ui, |ui| {
                                    let content_width = (row_inner_w - 12.0).max(120.0);
                                    ui.set_width(content_width);
                                    ui.set_max_width(content_width);
                                    ui.vertical(|ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        // Nomor baris / pilih target tap
                                        let num_label = if is_target {
                                            format!("🎯{:02}", idx + 1)
                                        } else {
                                            format!("#{:02}", idx + 1)
                                        };
                                        if ui
                                            .selectable_label(is_target, num_label)
                                            .on_hover_text(
                                                "Jadikan baris ini sebagai target Tap Sync",
                                            )
                                            .clicked()
                                        {
                                            action = Some(LineAction::Select(idx));
                                        }

                                        // Set ke waktu putar sekarang
                                        if ui
                                            .small_button("⏱")
                                            .on_hover_text(
                                                "Set waktu baris ini ke posisi lagu sekarang",
                                            )
                                            .clicked()
                                        {
                                            action = Some(LineAction::SetNow(idx));
                                        }

                                        // Lompat / dengarkan dari baris ini
                                        if ui
                                            .small_button("▶")
                                            .on_hover_text("Putar lagu dari waktu baris ini")
                                            .clicked()
                                        {
                                            action = Some(LineAction::Seek(line.start));
                                        }

                                        // Editor waktu [mm:ss.xx]
                                        let mut start_val = line.start;
                                        let drag = ui
                                            .add(
                                                egui::DragValue::new(&mut start_val)
                                                    .speed(0.05)
                                                    .range(0.0..=36000.0)
                                                    .custom_formatter(|v, _| {
                                                        format!("[{}]", clock(v))
                                                    }),
                                            )
                                            .on_hover_text("Geser kiri/kanan atau klik dua kali untuk mengetik detik");
                                        if drag.changed() {
                                            line.start = start_val.max(0.0);
                                            action = Some(LineAction::Changed);
                                        }

                                        if is_break {
                                            let tag = if idx == 0 && line.start == 0.0 {
                                                "🎵 Intro"
                                            } else {
                                                "🎵 Instrumen"
                                            };
                                            ui.colored_label(p.accent, tag);
                                        }

                                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                if ui
                                                    .small_button("🗑")
                                                    .on_hover_text("Hapus baris ini")
                                                    .clicked()
                                                {
                                                    action = Some(LineAction::Remove(idx));
                                                }
                                                if ui
                                                    .small_button("➕")
                                                    .on_hover_text(
                                                        "Sisipkan baris lirik baru di bawah baris ini",
                                                    )
                                                    .clicked()
                                                {
                                                    action = Some(LineAction::InsertAfter(idx));
                                                }
                                                if ui
                                                    .small_button("🎵")
                                                    .on_hover_text(
                                                        "Sisipkan baris jeda instrumen \"...\" di bawah baris ini",
                                                    )
                                                    .clicked()
                                                {
                                                    action =
                                                        Some(LineAction::InsertBreakAfter(idx));
                                                }
                                                });

                                    });
                                    {
                                        ui.add_space(3.0);
                                                let text_resp = ui.add_sized(
                                                    [ui.available_width(), 30.0],
                                                    egui::TextEdit::singleline(&mut line.text)
                                                        .hint_text("Ketik teks lirik atau \"...\" untuk jeda instrumen...")
                                                        .background_color(p.bg)
                                                        .margin(egui::Margin::symmetric(10, 5)),
                                                );
                                                if text_resp.clicked() {
                                                    action = Some(LineAction::Select(idx));
                                                }
                                                if text_resp.changed() {
                                                    action = Some(LineAction::Changed);
                                                }
                                    }
                                    });
                                });
                            ui.add_space(2.0);
                        }
                    });
            }

            if let Some(act) = action {
                match act {
                    LineAction::Select(idx) => {
                        if let Some(track) = self.lrc_studio.current_track_mut() {
                            track.active_row = idx;
                        }
                    }
                    LineAction::SetNow(idx) => {
                        if let Some(track) = self.lrc_studio.current_track_mut() {
                            track.lines[idx].start = cur_time;
                            track.active_row = idx;
                            track.recalculate_ends_and_words();
                        }
                    }
                    LineAction::Seek(t) => {
                        self.lrc_studio.seek_to(t);
                        if !self.lrc_studio.playing {
                            self.start_standalone_playback();
                        }
                    }
                    LineAction::InsertAfter(idx) => {
                        if let Some(track) = self.lrc_studio.current_track_mut() {
                            let new_start =
                                (track.lines[idx].start + 1.5).min(track.duration());
                            track.lines.insert(
                                idx + 1,
                                LyricLine {
                                    start: new_start,
                                    end: new_start + 2.5,
                                    text: String::new(),
                                    words: Vec::new(),
                                },
                            );
                            track.active_row = idx + 1;
                            track.recalculate_ends_and_words();
                        }
                    }
                    LineAction::InsertBreakAfter(idx) => {
                        if let Some(track) = self.lrc_studio.current_track_mut() {
                            let new_start =
                                (track.lines[idx].start + 2.5).min(track.duration());
                            track.lines.insert(
                                idx + 1,
                                LyricLine {
                                    start: new_start,
                                    end: new_start + 4.0,
                                    text: "...".to_owned(),
                                    words: Vec::new(),
                                },
                            );
                            track.recalculate_ends_and_words();
                        }
                    }
                    LineAction::Remove(idx) => {
                        if let Some(track) = self.lrc_studio.current_track_mut() {
                            track.lines.remove(idx);
                            if track.active_row >= track.lines.len() {
                                track.active_row = track.lines.len().saturating_sub(1);
                            }
                            track.recalculate_ends_and_words();
                        }
                    }
                    LineAction::Changed => {
                        if let Some(track) = self.lrc_studio.current_track_mut() {
                            track.recalculate_ends_and_words();
                        }
                    }
                }
            }
        });
    }

    fn draw_standalone_karaoke_monitor(&self, ui: &mut egui::Ui) {
        let p = theme::Palette::of(ui.ctx());
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 68.0), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, p.bg);
        painter.rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.0_f32, p.border),
            egui::StrokeKind::Inside,
        );

        let Some(track) = self.lrc_studio.current_track() else {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Pilih atau muat berkas lagu untuk melihat monitor lirik aktif",
                egui::FontId::proportional(11.5),
                p.text_muted,
            );
            return;
        };

        if track.lines.is_empty() {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Monitor Lirik Aktif (Baris lirik yang sedang dinyanyikan pada waktu lagu akan tampil di sini)",
                egui::FontId::proportional(11.5),
                p.text_muted,
            );
            return;
        }

        let t = self.lrc_studio.current_time;
        let active_idx = track.lines.iter().position(|l| t >= l.start && t < l.end);

        if let Some(idx) = active_idx {
            let cur_line = &track.lines[idx];
            let is_break = is_break_line_text(&cur_line.text);
            let display_text = if is_break {
                let kind = if idx == 0 && cur_line.start == 0.0 {
                    "Intro Musik"
                } else {
                    "Instrumen Musik"
                };
                format!("[{}] 🎵 ... ({kind})", clock(cur_line.start))
            } else {
                format!("[{}] {}", clock(cur_line.start), cur_line.text)
            };
            let color = if is_break {
                p.accent
            } else {
                egui::Color32::from_rgb(255, 215, 0)
            };

            painter.text(
                egui::pos2(rect.center().x, rect.center().y - 10.0),
                egui::Align2::CENTER_CENTER,
                display_text,
                egui::FontId::proportional(15.0),
                color,
            );
            if let Some(next_vocal) = track.lines[idx + 1..]
                .iter()
                .find(|l| !is_break_line_text(&l.text))
            {
                painter.text(
                    egui::pos2(rect.center().x, rect.center().y + 15.0),
                    egui::Align2::CENTER_CENTER,
                    format!(
                        "Vokal berikutnya [{}]: {}",
                        clock(next_vocal.start),
                        next_vocal.text
                    ),
                    egui::FontId::proportional(11.5),
                    p.text_muted,
                );
            }
        } else {
            // Waktu sedang berada di celah intro awal atau jeda instrumen antar-baris
            let next_vocal = track
                .lines
                .iter()
                .find(|l| l.start > t && !is_break_line_text(&l.text));
            let label = if track.lines.first().is_some_and(|first| t < first.start) {
                "🎵 ... (Intro Musik)"
            } else {
                "🎵 ... (Jeda Instrumen)"
            };
            painter.text(
                egui::pos2(rect.center().x, rect.center().y - 10.0),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(14.5),
                p.accent,
            );
            if let Some(next_line) = next_vocal {
                painter.text(
                    egui::pos2(rect.center().x, rect.center().y + 15.0),
                    egui::Align2::CENTER_CENTER,
                    format!(
                        "Vokal berikutnya [{}]: {}",
                        clock(next_line.start),
                        next_line.text
                    ),
                    egui::FontId::proportional(11.5),
                    p.text_muted,
                );
            }
        }
    }

    fn tap_sync_active_row(&mut self) {
        let now_sec = self.lrc_studio.current_time;
        let add_interlude = self.lrc_studio.auto_interlude_marker;
        let interlude_gap = self.lrc_studio.interlude_min_gap_seconds;
        let Some(track) = self.lrc_studio.current_track_mut() else {
            return;
        };
        if track.lines.is_empty() {
            return;
        }
        let idx = track.active_row.min(track.lines.len() - 1);
        track.lines[idx].start = now_sec;

        // Jika jarak dari selesainya baris vokal sebelumnya ke baris ini cukup panjang,
        // otomatis sisipkan baris "..." (jeda instrumen) di antaranya bila fitur aktif.
        if add_interlude && idx > 0 && !is_break_line_text(&track.lines[idx - 1].text) {
            let prev_start = track.lines[idx - 1].start;
            let word_cnt = track.lines[idx - 1].text.split_whitespace().count();
            let est_sing = (word_cnt as f64 * 0.45).clamp(2.0, 5.0);
            let prev_vocal_end = ((prev_start + est_sing) * 100.0).round() / 100.0;
            if now_sec - prev_vocal_end >= interlude_gap {
                track.lines.insert(
                    idx,
                    LyricLine {
                        start: prev_vocal_end,
                        end: now_sec,
                        text: "...".to_owned(),
                        words: Vec::new(),
                    },
                );
                let new_vocal_idx = idx + 1;
                if new_vocal_idx + 1 < track.lines.len() {
                    track.active_row = new_vocal_idx + 1;
                } else {
                    track.active_row = new_vocal_idx;
                }
                track.recalculate_ends_and_words();
                return;
            }
        }

        if idx + 1 < track.lines.len() {
            track.active_row = idx + 1;
        }
        track.recalculate_ends_and_words();
    }

    fn insert_instrumental_break_at_current_time(&mut self) {
        let now_sec = (self.lrc_studio.current_time * 100.0).round() / 100.0;
        let Some(track) = self.lrc_studio.current_track_mut() else {
            return;
        };
        let insert_idx = track.active_row.min(track.lines.len());
        track.lines.insert(
            insert_idx,
            LyricLine {
                start: now_sec,
                end: now_sec + 4.0,
                text: "...".to_owned(),
                words: Vec::new(),
            },
        );
        // Target tap tetap berada pada baris vokal setelah baris "..." yang baru disisipkan
        track.active_row = (insert_idx + 1).min(track.lines.len().saturating_sub(1));
        track.recalculate_ends_and_words();
    }

    fn pick_standalone_audios(&mut self) {
        let picked = rfd::FileDialog::new()
            .add_filter("Audio Lagu", &AUDIO_EXTENSIONS)
            .pick_files();
        let Some(paths) = picked else {
            return;
        };
        self.load_standalone_audios(paths);
    }

    fn pick_standalone_audio_folder(&mut self) {
        let Some(folder) = rfd::FileDialog::new().pick_folder() else {
            return;
        };
        let Ok(entries) = std::fs::read_dir(&folder) else {
            return;
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.is_file()
                    && p.extension()
                        .and_then(|ext| ext.to_str())
                        .is_some_and(|ext| AUDIO_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
            })
            .collect();
        paths.sort();
        if paths.is_empty() {
            self.warn("Tidak ada berkas audio yang ditemukan di folder tersebut.");
            return;
        }
        self.load_standalone_audios(paths);
    }

    pub(super) fn load_standalone_audios(&mut self, paths: Vec<PathBuf>) {
        if paths.is_empty() || self.lrc_studio.loading_audio {
            return;
        }
        let loaded: Vec<PathBuf> = self
            .lrc_studio
            .tracks
            .iter()
            .map(|track| PathBuf::from(&track.info.path))
            .collect();
        let paths = unique_standalone_audio_paths(&loaded, paths);
        if paths.is_empty() {
            self.info("Lagu yang dipilih sudah ada di Studio Lirik.");
            return;
        }
        self.lrc_studio.loading_audio = true;
        let tools = self.tools.clone();
        self.spawn(async move {
            let mut results = Vec::with_capacity(paths.len());
            for path in paths {
                results.push(probe_audio(&tools, &path).await);
            }
            Event::StandaloneAudios(results)
        });
    }

    fn start_standalone_playback(&mut self) {
        self.stop_visualizer_playback(false);
        self.stop_generate_preview();
        let Some(track) = self.lrc_studio.current_track() else {
            return;
        };
        let track_id = track.id;
        let source = PathBuf::from(&track.info.path);
        if self.lrc_studio.loading_preview_id == Some(track_id) {
            return;
        }
        if track
            .wav_preview
            .as_ref()
            .is_some_and(|preview| preview.path().is_file())
        {
            self.lrc_studio.start_playback();
            return;
        }
        if !self.ffmpeg_ready {
            self.warn("Menunggu FFmpeg siap untuk menyiapkan preview Lirik.");
            return;
        }
        self.lrc_studio.stop_playback();
        if self.audio_preview_session.is_none() {
            match loopkaraoke::karaoke::preview::PreviewSession::new(&self.paths.temp) {
                Ok(session) => self.audio_preview_session = Some(session),
                Err(error) => {
                    self.error(format!("Gagal menyiapkan folder preview Lirik: {error}"));
                    return;
                }
            }
        }
        let preview = self.audio_preview_session.as_ref().unwrap().create_file();
        self.lrc_studio.loading_preview_id = Some(track_id);
        let request_id = self.lrc_studio.preview_request_id;
        let tools = self.tools.clone();
        self.spawn(async move {
            let mut command = tools.ffmpeg();
            command
                .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
                .arg(&source)
                .args(["-vn", "-ac", "2", "-ar", "44100", "-acodec", "pcm_s16le"])
                .arg(preview.path());
            let result =
                match loopkaraoke::ffmpeg::run(&mut command, "FFmpeg (preview Lirik)").await {
                    Ok(_) if preview.path().is_file() => Ok(preview),
                    Ok(_) => Err(loopkaraoke::error::Error::internal(
                        "File preview Lirik tidak terbentuk.",
                    )),
                    Err(error) => Err(error),
                };
            Event::StandalonePreview {
                track_id,
                request_id,
                result,
            }
        });
    }

    fn transcribe_standalone_track(&mut self, index: usize) {
        if self.dependencies.operation.is_some() {
            self.warn("Tunggu pengelolaan dependensi selesai sebelum transkripsi.");
            return;
        }
        let Some(track) = self.lrc_studio.tracks.get_mut(index) else {
            return;
        };
        if track.transcribing {
            return;
        }
        let use_gpu = self.lrc_studio.whisper_use_gpu
            && self
                .hardware
                .as_ref()
                .is_some_and(|hardware| hardware.gpu_whisper);
        let options = TranscribeOptions {
            model: self.lrc_studio.whisper_model,
            language: LANGUAGES[self.lrc_studio.whisper_language]
                .1
                .map(str::to_owned),
            use_gpu,
        };
        track.transcribing = true;
        let track_id = track.id;
        let path = PathBuf::from(&track.info.path);
        let title = track.title.clone();
        let model_name = self.lrc_studio.whisper_model.name();
        self.log(
            super::LogLevel::Info,
            format!(
                "Studio Lirik: Memulai transkripsi Whisper AI untuk \"{title}\" (model: {model_name})..."
            ),
        );
        let (tools, models) = (self.tools.clone(), self.paths.models.clone());
        let (transcriber, slot) = (self.transcriber.clone(), self.transcribe_slot.clone());
        self.spawn(async move {
            let _permit = slot.acquire_owned().await;
            let result = transcriber
                .transcribe(&tools, &models, &path, options)
                .await;
            Event::StandaloneLyrics { track_id, result }
        });
    }

    fn import_raw_lyrics_to_studio(&mut self, keep_existing_timestamps: bool) {
        let skip_headers = self.lrc_studio.skip_section_headers;
        let add_intro = self.lrc_studio.auto_intro_marker;
        let intro_sec = self.lrc_studio.default_intro_seconds.max(1.0);
        let add_interlude = self.lrc_studio.auto_interlude_marker;
        let interlude_gap = self.lrc_studio.interlude_min_gap_seconds;

        let Some(track) = self.lrc_studio.current_track_mut() else {
            return;
        };
        let raw = track.raw_lyrics_input.trim().to_owned();
        if raw.is_empty() {
            return;
        }

        // First check if pasted text already has [mm:ss.xx] timestamps
        if !keep_existing_timestamps {
            let parsed_lrc = parse_lrc(&raw);
            if !parsed_lrc.is_empty() {
                track.lines = parsed_lrc;
                track.apply_intro_and_interlude_markers(
                    add_intro,
                    intro_sec.min(3.0),
                    add_interlude,
                    interlude_gap,
                );
                let count = track.lines.len();
                self.info(format!(
                    "{count} baris lirik berformat LRC berhasil dimuat."
                ));
                return;
            }
        }

        let mut clean_lines: Vec<String> = Vec::new();
        for raw_line in raw.lines().map(str::trim).filter(|l| !l.is_empty()) {
            if is_intro_section_header(raw_line) {
                continue;
            }
            if is_interlude_section_header(raw_line) || is_break_line_text(raw_line) {
                if !clean_lines
                    .last()
                    .is_some_and(|prev| is_break_line_text(prev))
                {
                    clean_lines.push("...".to_owned());
                }
                continue;
            }
            if skip_headers && raw_line.starts_with('[') && raw_line.ends_with(']') {
                let inner = &raw_line[1..raw_line.len() - 1];
                let looks_like_time = inner
                    .chars()
                    .all(|c| c.is_ascii_digit() || c == ':' || c == '.');
                if !looks_like_time {
                    continue;
                }
            }
            let stripped = if raw_line.starts_with('[') {
                if let Some(pos) = raw_line.find(']') {
                    raw_line[pos + 1..].trim().to_owned()
                } else {
                    raw_line.to_owned()
                }
            } else {
                raw_line.to_owned()
            };
            if !stripped.is_empty() {
                clean_lines.push(stripped);
            }
        }

        if clean_lines.is_empty() {
            self.error("Tidak ada baris teks lirik yang dapat dimuat.");
            return;
        }

        if keep_existing_timestamps && !track.lines.is_empty() {
            let pasted_has_breaks = clean_lines.iter().any(|l| is_break_line_text(l));
            if pasted_has_breaks {
                let existing_len = track.lines.len();
                for (idx, text) in clean_lines.into_iter().enumerate() {
                    if idx < existing_len {
                        track.lines[idx].text = text;
                    } else {
                        let last_end = track.lines.last().map(|l| l.end).unwrap_or(0.0);
                        track.lines.push(LyricLine {
                            start: last_end,
                            end: last_end + 3.0,
                            text,
                            words: Vec::new(),
                        });
                    }
                }
            } else {
                // Pertahankan baris Intro / Instrumen ("...") yang sudah ada, timpa hanya baris vokal
                let vocal_indices: Vec<usize> = track
                    .lines
                    .iter()
                    .enumerate()
                    .filter(|(_, l)| !is_break_line_text(&l.text))
                    .map(|(i, _)| i)
                    .collect();

                for (k, text) in clean_lines.into_iter().enumerate() {
                    if let Some(&line_idx) = vocal_indices.get(k) {
                        track.lines[line_idx].text = text;
                    } else {
                        let last_end = track.lines.last().map(|l| l.end).unwrap_or(0.0);
                        track.lines.push(LyricLine {
                            start: last_end,
                            end: last_end + 3.0,
                            text,
                            words: Vec::new(),
                        });
                    }
                }
            }
            track.recalculate_ends_and_words();
            self.info(
                "Teks lirik vokal berhasil diperbarui dengan tetap mempertahankan timestamp dan jeda Intro/Instrumen (...).",
            );
            return;
        }

        let total_dur = track.duration();
        let count = clean_lines.len();
        let start_offset = if add_intro { intro_sec } else { 0.0 };
        let usable_dur = (total_dur - start_offset).max(10.0);
        let step = (usable_dur / (count as f64 + 1.0)).clamp(1.5, 4.5);

        let mut new_lines: Vec<LyricLine> = Vec::with_capacity(count + 1);
        if add_intro {
            new_lines.push(LyricLine {
                start: 0.0,
                end: start_offset,
                text: "...".to_owned(),
                words: Vec::new(),
            });
        }

        let mut cur_t = start_offset;
        for text in clean_lines {
            let is_brk = is_break_line_text(&text);
            let line_dur = if is_brk { interlude_gap.max(6.0) } else { step };
            let end_t = cur_t + line_dur;
            let words = spread_words(&text, cur_t, line_dur);
            new_lines.push(LyricLine {
                start: (cur_t * 100.0).round() / 100.0,
                end: (end_t * 100.0).round() / 100.0,
                text,
                words,
            });
            cur_t = end_t;
        }

        track.lines = new_lines;
        track.recalculate_ends_and_words();
        track.active_row = track
            .lines
            .iter()
            .position(|l| !is_break_line_text(&l.text))
            .unwrap_or(0);
        let total_rows = track.lines.len();
        self.info(format!(
            "{total_rows} baris berhasil dibuat (termasuk Intro [00:00.00] ...). Putar lagu dan klik ⏱ saat vokal pertama mulai menyanyi."
        ));
    }

    fn pick_standalone_lrc_file(&mut self) {
        if self.lrc_studio.current_track().is_none() {
            return;
        }
        let picked = rfd::FileDialog::new()
            .add_filter("Berkas Lirik (.lrc, .txt)", &["lrc", "txt"])
            .pick_file();
        let Some(path) = picked else {
            return;
        };
        self.load_standalone_lrc_path(path);
    }

    pub(super) fn load_standalone_lrc_path(&mut self, path: PathBuf) {
        if self.lrc_studio.current_track().is_none() {
            self.warn("Pilih lagu sebelum mengimpor lirik.");
            return;
        }
        let add_intro = self.lrc_studio.auto_intro_marker;
        let intro_sec = self.lrc_studio.default_intro_seconds.min(3.0);
        let add_interlude = self.lrc_studio.auto_interlude_marker;
        let interlude_gap = self.lrc_studio.interlude_min_gap_seconds;
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let parsed = parse_lrc(&content);
                if let Some(track) = self.lrc_studio.current_track_mut() {
                    track.raw_lyrics_input = content;
                    if !parsed.is_empty() {
                        track.lines = parsed;
                        track.apply_intro_and_interlude_markers(
                            add_intro,
                            intro_sec,
                            add_interlude,
                            interlude_gap,
                        );
                        let count = track.lines.len();
                        self.info(format!(
                            "Berhasil memuat {count} baris lirik dari {}",
                            path.display()
                        ));
                        return;
                    }
                }
                self.import_raw_lyrics_to_studio(false);
            }
            Err(error) => self.error(format!("Gagal membaca berkas lirik: {error}")),
        }
    }

    fn export_standalone_lrc_file(&mut self) {
        if let Some(last) = self.lrc_studio.tracks.len().checked_sub(1) {
            let index = self.lrc_studio.selected_track.min(last);
            self.export_lrc_track(index);
        }
    }

    fn export_lrc_track(&mut self, index: usize) {
        let Some(track) = self.lrc_studio.tracks.get(index) else {
            return;
        };
        if track.lines.is_empty() {
            return;
        }
        let audio_path = PathBuf::from(&track.info.path);
        let stem = audio_path
            .file_stem()
            .and_then(|s| s.to_str())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(&track.title);
        let default_name = format!("{stem}.lrc");
        let mut dialog = rfd::FileDialog::new()
            .add_filter("LRC Lyrics (*.lrc)", &["lrc"])
            .set_file_name(&default_name);
        if let Some(parent) = audio_path.parent().filter(|p| p.is_dir()) {
            dialog = dialog.set_directory(parent);
        }
        let picked = dialog.save_file();
        let Some(path) = picked else {
            return;
        };
        match self.write_lrc_track(index, &path) {
            Ok(()) => {
                self.info(format!("Berkas .LRC berhasil disimpan: {}", path.display()));
            }
            Err(error) => self.error(format!("Gagal menyimpan berkas .LRC: {error}")),
        }
    }

    fn export_all_lrc_tracks(&mut self, indices: &[usize]) {
        let Some(folder) = rfd::FileDialog::new()
            .set_title("Folder untuk semua LRC")
            .pick_folder()
        else {
            return;
        };
        let mut names = std::collections::HashSet::new();
        let mut saved = 0;
        let mut failed = 0;
        for &index in indices {
            let track = &self.lrc_studio.tracks[index];
            let stem = std::path::Path::new(&track.info.path)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("Lirik");
            let mut name = format!("{stem}.lrc");
            for number in 2.. {
                if names.insert(name.to_lowercase()) {
                    break;
                }
                name = format!("{stem} ({number}).lrc");
            }
            let path = folder.join(name);
            match self.write_lrc_track(index, &path) {
                Ok(()) => saved += 1,
                Err(error) => {
                    failed += 1;
                    self.log(
                        super::LogLevel::Error,
                        format!("{}: {error}", path.display()),
                    );
                }
            }
        }
        if failed > 0 {
            self.warn(format!(
                "{saved} LRC disimpan, {failed} gagal. Lihat detail di log."
            ));
        } else {
            self.info(format!("{saved} LRC disimpan di {}", folder.display()));
        }
    }

    fn write_lrc_track(&mut self, index: usize, path: &std::path::Path) -> std::io::Result<()> {
        let track = &mut self.lrc_studio.tracks[index];
        std::fs::write(path, format_lrc_lines(&track.lines))?;
        self.karaoke_preview_lrc = None;
        track.saved_lrc_path = Some(path.to_owned());
        for generate_track in &mut self.generate.tracks {
            if generate_track.path == std::path::Path::new(&track.info.path) {
                generate_track.lrc = Some((path.to_owned(), track.lines.len()));
            }
        }
        Ok(())
    }

    // =========================================================================
    // Pengaturan gaya dan pratinjau untuk menu Karaoke.
    // =========================================================================

    pub(super) fn style_section(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for (index, (_, theme)) in THEMES.iter().enumerate() {
                if ui
                    .add(theme::StyledButton::secondary(theme.name).selected(self.theme == index))
                    .clicked()
                {
                    self.theme = index;
                    self.style.apply_theme(theme);
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            theme::label_with_info(ui, "Font Lirik:", help::FONT_CHOICE);
            let palette = theme::Palette::of(ui.ctx());
            egui::Frame::new()
                .fill(palette.bg)
                .stroke(egui::Stroke::new(1.0_f32, palette.border))
                .corner_radius(8)
                .show(ui, |ui| {
                    use super::generate_tab::{draw_stepper_button, paint_vertical_divider};

                    ui.horizontal_centered(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let size = &mut self.style.font_size;
                        if draw_stepper_button(ui, false, *size > 20, &palette, true).clicked() {
                            *size -= 1;
                        }
                        paint_vertical_divider(ui, palette.border);
                        ui.add_sized(
                            [58.0, theme::metrics::CONTROL_HEIGHT - 2.0],
                            egui::DragValue::new(size).range(20..=120).suffix(" px"),
                        )
                        .on_hover_text(help::FONT_SIZE);
                        paint_vertical_divider(ui, palette.border);
                        if draw_stepper_button(ui, true, *size < 120, &palette, false).clicked() {
                            *size += 1;
                        }
                    });
                });

            let current_font_label = if let Some(path) = &self.style.custom_font {
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            } else {
                let preset_idx = self.style.font_preset.min(KARAOKE_FONTS.len() - 1);
                KARAOKE_FONTS[preset_idx].label.to_string()
            };

            let font_width = ui.available_width().min(theme::metrics::INPUT_WIDTH);
            egui::ComboBox::from_id_salt("karaoke_font_select")
                .width(font_width)
                .truncate()
                .selected_text(current_font_label)
                .show_ui(ui, |ui| {
                    for (idx, font) in KARAOKE_FONTS.iter().enumerate() {
                        if ui
                            .selectable_label(
                                self.style.custom_font.is_none() && self.style.font_preset == idx,
                                font.label,
                            )
                            .clicked()
                        {
                            self.style.font_preset = idx;
                            self.style.custom_font = None;
                            self.reload_egui_fonts();
                        }
                    }
                });

            if ui
                .add(theme::StyledButton::secondary("📁 Unggah Font..."))
                .on_hover_text("Pilih berkas font (.ttf / .otf) dari komputer Anda")
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Font Files (*.ttf, *.otf)", &["ttf", "otf", "TTF", "OTF"])
                    .set_title("Pilih Berkas Font Lirik Karaoke")
                    .pick_file()
                {
                    self.style.custom_font = Some(path);
                    self.reload_egui_fonts();
                }
            }

            if self.style.custom_font.is_some()
                && ui
                    .add(theme::StyledButton::secondary("✔ Reset ke Default").small())
                    .clicked()
            {
                self.style.custom_font = None;
                self.reload_egui_fonts();
            }
        });
        ui.add_space(4.0);
        ui.separator();
        // The settings card itself is half-width in the maximized layout.
        if ui.available_width() >= 560.0 {
            ui.columns(2, |columns| {
                self.karaoke_position_style(&mut columns[0]);
                self.karaoke_advanced_style(&mut columns[1]);
            });
        } else {
            self.karaoke_position_style(ui);
            self.karaoke_advanced_style(ui);
        }
        ui.add_space(4.0);
        ui.separator();
        self.karaoke_intro_style(ui);
    }

    fn karaoke_position_style(&mut self, ui: &mut egui::Ui) {
        ui.label("Posisi Karaoke (Slider X & Y):")
            .on_hover_text(help::POSITION);
        ui.horizontal_wrapped(|ui| {
            ui.label("Preset Cepat:");
            if ui
                .add(theme::StyledButton::secondary("Bawah").selected(
                    (self.style.pos_x - 50.0).abs() < 1.0 && (self.style.pos_y - 85.0).abs() < 1.0,
                ))
                .clicked()
            {
                self.style.pos_x = 50.0;
                self.style.pos_y = 85.0;
                self.style.alignment = 2;
            }
            if ui
                .add(theme::StyledButton::secondary("Tengah").selected(
                    (self.style.pos_x - 50.0).abs() < 1.0 && (self.style.pos_y - 50.0).abs() < 1.0,
                ))
                .clicked()
            {
                self.style.pos_x = 50.0;
                self.style.pos_y = 50.0;
                self.style.alignment = 5;
            }
            if ui
                .add(theme::StyledButton::secondary("Atas").selected(
                    (self.style.pos_x - 50.0).abs() < 1.0 && (self.style.pos_y - 15.0).abs() < 1.0,
                ))
                .clicked()
            {
                self.style.pos_x = 50.0;
                self.style.pos_y = 15.0;
                self.style.alignment = 8;
            }
        });
        if theme::control_slider_reset(
            ui,
            "Posisi X (Horizontal)",
            &mut self.style.pos_x,
            0.0..=100.0,
            "%",
            "Reset ke 50%",
        ) {
            self.style.pos_x = 50.0;
        }
        if theme::control_slider_reset(
            ui,
            "Posisi Y (Vertikal)",
            &mut self.style.pos_y,
            0.0..=100.0,
            "%",
            "Reset ke 85%",
        ) {
            self.style.pos_y = 85.0;
        }
    }

    fn karaoke_advanced_style(&mut self, ui: &mut egui::Ui) {
        ui.label("Pengaturan lanjutan");
        ui.horizontal_wrapped(|ui| {
            theme::info(ui, help::COLORS);
            for (label, color) in [
                ("Normal", &mut self.style.normal),
                ("Bernyanyi", &mut self.style.singing),
                ("Tepi", &mut self.style.outline),
                ("Bayangan", &mut self.style.shadow),
            ] {
                let font = egui::TextStyle::Body.resolve(ui.style());
                let label_width = ui.fonts(|fonts| {
                    fonts
                        .layout_no_wrap(label.to_owned(), font, ui.visuals().text_color())
                        .size()
                        .x
                });
                let group_width =
                    label_width.ceil() + ui.spacing().item_spacing.x + ui.spacing().interact_size.x;
                if group_width > ui.available_size_before_wrap().x {
                    ui.end_row();
                }
                ui.horizontal(|ui| {
                    ui.add(egui::Label::new(label).wrap_mode(egui::TextWrapMode::Extend));
                    ui.color_edit_button_srgb(color);
                });
            }
        });
        let default_margin = super::StyleState::from_theme(&THEMES[0].1).margin_v;
        let margin = ui.scope(|ui| {
            theme::control_slider_reset(
                ui,
                "Jarak dari tepi (Margin)",
                &mut self.style.margin_v,
                0..=300,
                " px",
                &format!("Reset ke {default_margin} px"),
            )
        });
        margin.response.on_hover_text(help::MARGIN);
        if margin.inner {
            self.style.margin_v = default_margin;
        }
        ui.horizontal_wrapped(|ui| {
            theme::label_with_info(ui, "Efek wipe:", help::WIPE);
            for (smooth, label) in [(true, "Halus"), (false, "Per kata")] {
                if ui
                    .add(
                        theme::StyledButton::secondary(label)
                            .selected(self.style.smooth_wipe == smooth),
                    )
                    .clicked()
                {
                    self.style.smooth_wipe = smooth;
                }
            }
        });
    }

    fn karaoke_intro_style(&mut self, ui: &mut egui::Ui) {
        ui.label("Banner Pembuka");
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut self.style.show_intro, "Tampilkan judul dan penyanyi");
            theme::info(ui, help::INTRO);
        });
        if self.style.show_intro {
            ui.muted_label("Preview Karaoke menampilkan banner untuk penyetelan. Render Old menampilkannya selama 5 detik di awal setiap lagu.");
            ui.horizontal_wrapped(|ui| {
                ui.label("Font Banner");
                egui::ComboBox::from_id_salt("karaoke_intro_font")
                    .selected_text(KARAOKE_FONTS[self.style.intro_font_preset].label)
                    .show_ui(ui, |ui| {
                        for (index, font) in KARAOKE_FONTS.iter().enumerate() {
                            ui.selectable_value(
                                &mut self.style.intro_font_preset,
                                index,
                                font.label,
                            );
                        }
                    });
                ui.label("Ukuran");
                ui.add(
                    egui::DragValue::new(&mut self.style.intro_banner.size)
                        .range(20..=120)
                        .suffix(" px"),
                );
                ui.label("Warna");
                ui.color_edit_button_srgb(&mut self.style.intro_banner.color);
            });
            let banner = &mut self.style.intro_banner;
            if theme::control_slider_reset(
                ui,
                "Posisi X Banner",
                &mut banner.x,
                0.0..=100.0,
                "%",
                "Reset ke 50%",
            ) {
                banner.x = 50.0;
            }
            if theme::control_slider_reset(
                ui,
                "Posisi Y Banner",
                &mut banner.y,
                0.0..=100.0,
                "%",
                "Reset ke 15%",
            ) {
                banner.y = 15.0;
            }
        }
    }
}

#[cfg(test)]
mod studio_tests {
    use super::*;

    fn fixture_audio(path: &std::path::Path) -> AudioInfo {
        AudioInfo {
            path: path.to_string_lossy().into_owned(),
            filename: path.file_name().unwrap().to_string_lossy().into_owned(),
            title: "Test track".into(),
            artist: String::new(),
            duration: 60.0,
            duration_str: "1:00".into(),
            size_bytes: 0,
            size_mb: 0.0,
        }
    }

    #[test]
    fn imported_tracks_have_no_preview_and_pending_playback_is_invalidated_on_switch_or_stop() {
        let mut studio = StandaloneLyricsState::default();
        studio.add_audio_track(fixture_audio(std::path::Path::new("first.mp3")), None);
        studio.add_audio_track(fixture_audio(std::path::Path::new("second.m4a")), None);
        assert!(studio
            .tracks
            .iter()
            .all(|track| track.wav_preview.is_none()));
        assert_eq!(studio.tracks[0].info.path, "first.mp3");
        let first_id = studio.tracks[0].id;
        let request_id = studio.preview_request_id;
        studio.loading_preview_id = Some(first_id);
        assert!(studio.preview_request_matches(first_id, request_id));
        studio.seek_to(12.0);
        assert!(studio.preview_request_matches(first_id, request_id));
        assert!(!studio.playing);
        studio.select_track(1);
        assert!(!studio.preview_request_matches(first_id, request_id));
        assert!(studio.loading_preview_id.is_none());
        let second_id = studio.tracks[1].id;
        studio.loading_preview_id = Some(second_id);
        let request_id = studio.preview_request_id;
        studio.stop_playback();
        assert!(!studio.preview_request_matches(second_id, request_id));
        studio.start_playback();
        assert!(
            !studio.playing,
            "No silent playback timer without a ready preview"
        );
    }

    #[test]
    fn removing_and_clearing_tracks_release_previews_but_preserve_original_audio() {
        let root = std::env::temp_dir().join(format!("studio-cleanup-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let session =
            loopkaraoke::karaoke::preview::PreviewSession::new(&root.join("temp")).unwrap();
        let a = root.join("a.mp3");
        let b = root.join("b.m4a");
        std::fs::write(&a, b"original mp3").unwrap();
        std::fs::write(&b, b"original m4a").unwrap();
        let preview_a = session.create_file();
        let preview_b = session.create_file();
        let path_a = preview_a.path().to_path_buf();
        let path_b = preview_b.path().to_path_buf();
        std::fs::write(&path_a, b"preview a").unwrap();
        std::fs::write(&path_b, b"preview b").unwrap();
        let mut studio = StandaloneLyricsState::default();
        studio.add_audio_track(fixture_audio(&a), Some(preview_a));
        studio.add_audio_track(fixture_audio(&b), Some(preview_b));
        studio.selected_track = 1;
        studio.current_time = 12.0;
        let selected_id = studio.current_track().unwrap().id;
        studio.remove_track(0);
        assert!(!path_a.exists());
        assert!(path_b.exists());
        assert_eq!(studio.current_track().unwrap().id, selected_id);
        assert_eq!(studio.current_time, 12.0);
        studio.clear_tracks();
        assert!(!path_b.exists());
        assert!(studio.tracks.is_empty());
        assert_eq!(studio.current_time, 0.0);
        assert_eq!(std::fs::read(&a).unwrap(), b"original mp3");
        assert_eq!(std::fs::read(&b).unwrap(), b"original m4a");
        drop(studio);
        drop(session);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dropping_unreceived_preview_results_removes_preview_output() {
        let root = std::env::temp_dir().join(format!("studio-event-{}", uuid::Uuid::new_v4()));
        let session = loopkaraoke::karaoke::preview::PreviewSession::new(&root).unwrap();
        let preview = session.create_file();
        let path = preview.path().to_path_buf();
        std::fs::write(&path, b"queued preview").unwrap();
        let event = Event::StandalonePreview {
            track_id: 1,
            request_id: 1,
            result: Ok(preview),
        };
        drop(event);
        assert!(!path.exists());
        drop(session);
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn repeated_imports_skip_loaded_files_and_duplicate_paths_in_the_same_batch() {
        let root = std::env::temp_dir().join(format!("studio-dedup-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let loaded = root.join("loaded.mp3");
        let new = root.join("new.m4a");
        std::fs::write(&loaded, b"loaded").unwrap();
        std::fs::write(&new, b"new").unwrap();
        let paths = unique_standalone_audio_paths(
            std::slice::from_ref(&loaded),
            vec![
                loaded.clone(),
                new.clone(),
                root.join(".").join("new.m4a"),
                new.clone(),
            ],
        );
        assert_eq!(paths, vec![new]);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn imported_studio_audio_loads_companion_lrc_and_raw_text() {
        let dir = std::env::temp_dir().join(format!("studio-lrc-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("track.lrc");
        std::fs::write(&path, "[00:01.00]First line\n[00:05.50]Second line").unwrap();
        let mut studio = std::mem::ManuallyDrop::new(StandaloneLyricsState::default());
        studio.add_audio_track(
            AudioInfo {
                path: dir.join("track.mp3").to_string_lossy().into_owned(),
                filename: "track.mp3".into(),
                title: "Track".into(),
                artist: String::new(),
                duration: 60.0,
                duration_str: "1:00".into(),
                size_bytes: 0,
                size_mb: 0.0,
            },
            None,
        );
        let track = studio.current_track().unwrap();
        assert_eq!(track.lines.len(), 2);
        assert_eq!(track.raw_lyrics_input, "First line\nSecond line");
        assert_eq!(track.saved_lrc_path.as_ref(), Some(&path));
        // Avoid invoking the global audio transport from a parsing regression test.
        studio.tracks.clear();
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn exported_lrc_preserves_centisecond_timestamps_and_text() {
        let original = parse_lrc("[00:01.25]First line\n[01:05.50]Second line");
        let exported = format_lrc_lines(&original);
        let parsed = parse_lrc(&exported);
        assert_eq!(parsed.len(), original.len());
        for (expected, actual) in original.iter().zip(parsed.iter()) {
            assert!((expected.start - actual.start).abs() < 0.001);
            assert_eq!(expected.text, actual.text);
        }
    }
}
