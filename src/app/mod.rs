mod background;
mod generate_tab;
mod help;
mod lyrics;
mod playlist;
mod quick_tab;
mod render_tab;
mod storage_tab;
pub mod theme;
mod thumbnail_tab;
mod visualizer_tab;

use std::{
    future::Future,
    path::PathBuf,
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

use eframe::egui;
use loopkaraoke::{
    config::{AppPaths, AppSettings},
    demo::DemoSet,
    error::Result,
    ffmpeg::{self, DownloadProgress, Tools},
    hardware::HardwareInfo,
    karaoke::{
        ass::{PlaylistDisplayMode, StyleConfig, TracklistPosition},
        styles::{Theme, THEMES},
        transcribe::{ModelSize, Transcriber},
        LyricLine,
    },
    media::{AudioInfo, VideoInfo},
    render::{
        encoder::{Encoder, VideoRenderSettings},
        image_video::Motion,
        job::{JobHandle, JobStatus},
        looper::FitMode,
        presets::RESOLUTION_LABELS,
    },
    storage::{self, CleanupOptions, StorageStats},
};
use theme::{StyledButton, UiMetrics, UiText};
use tokio::sync::Semaphore;

use background::Background;

const STARTUP_TEMP_MAX_AGE: Duration = Duration::from_secs(24 * 3600);
const FALLBACK_FONTS: [&str; 7] = [
    "segoeui.ttf",
    "seguisym.ttf",
    "seguiemj.ttf",
    "YuGothR.ttc",
    "msgothic.ttc",
    "malgun.ttf",
    "msyh.ttc",
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Tab {
    Quick,
    Generate,
    Background,
    Playlist,
    Lyrics,
    Karaoke,
    Visualizer,
    Render,
    Thumbnail,
    Storage,
}

impl Tab {
    const ALL: [Self; 10] = [
        Self::Quick,
        Self::Generate,
        Self::Background,
        Self::Playlist,
        Self::Lyrics,
        Self::Karaoke,
        Self::Visualizer,
        Self::Render,
        Self::Thumbnail,
        Self::Storage,
    ];

    fn help(self) -> &'static str {
        match self {
            Self::Quick => {
                "Dashboard: alur kerja terpadu input video/gambar, audio, lirik, dan render cepat."
            }
            Self::Generate => {
                "Generate: gabungkan video pendekmu dengan daftar lagu mu menjadi satu file video."
            }
            Self::Background => help::TAB_BACKGROUND,
            Self::Playlist => help::TAB_PLAYLIST,
            Self::Lyrics => help::TAB_LYRICS,
            Self::Karaoke => {
                "Pengaturan gaya teks karaoke (font, warna, posisi, dan efek wipe) untuk lagu yang memiliki lirik LRC."
            }
            Self::Visualizer => help::TAB_VISUALIZER,
            Self::Render => help::TAB_RENDER,
            Self::Thumbnail => help::TAB_THUMBNAIL,
            Self::Storage => help::TAB_STORAGE,
        }
    }

    /// The next step of the main workflow, if this tab is part of it.
    fn next(self) -> Option<Self> {
        match self {
            Self::Quick | Self::Generate | Self::Lyrics | Self::Karaoke => None,
            Self::Background => Some(Self::Playlist),
            Self::Playlist => Some(Self::Visualizer),
            Self::Visualizer => Some(Self::Render),
            _ => None,
        }
    }

    fn previous(self) -> Option<Self> {
        match self {
            Self::Background => Some(Self::Quick),
            Self::Playlist => Some(Self::Background),
            Self::Visualizer => Some(Self::Playlist),
            Self::Render => Some(Self::Visualizer),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Quick => "Dashboard",
            Self::Generate => "Generate",
            Self::Background => "Background",
            Self::Playlist => "Playlist",
            Self::Lyrics => "Lirik",
            Self::Karaoke => "Karaoke",
            Self::Visualizer => "Visualizer",
            Self::Render => "Render",
            Self::Thumbnail => "Thumbnail",
            Self::Storage => "Settings",
        }
    }

    fn icon(self) -> theme::SidebarIcon {
        match self {
            Self::Quick => theme::SidebarIcon::Dashboard,
            Self::Generate => theme::SidebarIcon::Generate,
            Self::Background => theme::SidebarIcon::Background,
            Self::Playlist => theme::SidebarIcon::Playlist,
            Self::Lyrics => theme::SidebarIcon::Lyrics,
            Self::Karaoke => theme::SidebarIcon::Karaoke,
            Self::Visualizer => theme::SidebarIcon::Visualizer,
            Self::Render => theme::SidebarIcon::Render,
            Self::Thumbnail => theme::SidebarIcon::Thumbnail,
            Self::Storage => theme::SidebarIcon::Settings,
        }
    }
}

/// Results that background tasks hand back to the UI thread.
enum Event {
    Ffmpeg(Result<Tools>),
    Hardware(HardwareInfo),
    Background(Result<Background>),
    Audios(Vec<Result<AudioInfo>>),
    Lyrics {
        track: u32,
        result: Result<Vec<LyricLine>>,
    },
    StandaloneAudios(Vec<(Result<AudioInfo>, Option<PathBuf>)>),
    StandaloneLyrics {
        track_id: u32,
        result: Result<Vec<LyricLine>>,
    },
    Demo(Result<DemoSet>),
    Thumbnail(Result<PathBuf>),
    ThumbnailFrame(Result<PathBuf>),
    GenerateVideo(Result<VideoInfo>),
    GenerateAudios(Vec<Result<AudioInfo>>),
    GeneratePreview {
        track_id: u32,
        result: Result<PathBuf>,
    },
    Storage(StorageStats),
    Cleaned(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Success,
    Warn,
    Error,
}

impl LogLevel {
    pub fn label(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Success => "OK",
            Self::Warn => "WARN",
            Self::Error => "ERR",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub time: String,
    pub level: LogLevel,
    pub message: String,
}

const TOAST_LIFETIME_SECS: f64 = 4.5;

pub(super) struct Notice {
    pub text: String,
    pub is_error: bool,
    pub shown_at: Instant,
}

impl Notice {
    pub(super) fn new(text: impl Into<String>, is_error: bool) -> Self {
        Self {
            text: text.into(),
            is_error,
            shown_at: Instant::now(),
        }
    }
}

struct Track {
    id: u32,
    path: PathBuf,
    title: String,
    artist: String,
    duration: f64,
    lyrics: Vec<LyricLine>,
    has_lrc: bool,
    transcribing: bool,
}

impl Track {
    fn from_info(id: u32, info: AudioInfo, lyrics: Vec<LyricLine>, has_lrc: bool) -> Self {
        let has_lrc = has_lrc || !lyrics.is_empty();
        Self {
            id,
            path: PathBuf::from(info.path),
            title: info.title,
            artist: info.artist,
            duration: info.duration,
            lyrics,
            has_lrc,
            transcribing: false,
        }
    }
}

struct StyleState {
    normal: [u8; 3],
    singing: [u8; 3],
    outline: [u8; 3],
    shadow: [u8; 3],
    font_size: u32,
    font_preset: usize,
    custom_font: Option<PathBuf>,
    alignment: u8,
    margin_v: i32,
    pos_x: f32,
    pos_y: f32,
    smooth_wipe: bool,
    show_intro: bool,
}

fn rgb(hex: &str) -> [u8; 3] {
    let value = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0xFF_FF_FF);
    [(value >> 16) as u8, (value >> 8) as u8, value as u8]
}

fn hex(color: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", color[0], color[1], color[2])
}

impl StyleState {
    fn from_theme(theme: &Theme) -> Self {
        Self {
            normal: rgb(theme.normal_color),
            singing: rgb(theme.singing_color),
            outline: rgb(theme.outline_color),
            shadow: rgb(theme.shadow_color),
            font_size: 48,
            font_preset: 0,
            custom_font: None,
            alignment: 2,
            margin_v: 70,
            pos_x: 50.0,
            pos_y: 85.0,
            smooth_wipe: true,
            show_intro: false,
        }
    }

    fn apply_theme(&mut self, theme: &Theme) {
        self.normal = rgb(theme.normal_color);
        self.singing = rgb(theme.singing_color);
        self.outline = rgb(theme.outline_color);
        self.shadow = rgb(theme.shadow_color);
    }

    fn to_config(&self) -> StyleConfig {
        let font_name = lyrics::karaoke_font_ass_name(self.font_preset, &self.custom_font);
        StyleConfig {
            font_name: Some(font_name),
            font_path: self.custom_font.clone(),
            font_size: Some(self.font_size),
            normal_color: Some(hex(self.normal)),
            singing_color: Some(hex(self.singing)),
            outline_color: Some(hex(self.outline)),
            shadow_color: Some(hex(self.shadow)),
            alignment: Some(self.alignment),
            margin_v: Some(self.margin_v),
            pos_x: Some(self.pos_x),
            pos_y: Some(self.pos_y),
            show_intro: Some(self.show_intro),
            wipe_mode: Some(if self.smooth_wipe { "smooth" } else { "jump" }.to_owned()),
            ..Default::default()
        }
    }
}

pub struct StudioApp {
    ctx: egui::Context,
    paths: AppPaths,
    tools: Tools,
    runtime: tokio::runtime::Runtime,
    transcriber: Arc<Transcriber>,
    transcribe_slot: Arc<Semaphore>,
    sender: mpsc::Sender<Event>,
    events: mpsc::Receiver<Event>,
    pending: usize,
    notice: Option<Notice>,
    hardware: Option<HardwareInfo>,
    ffmpeg_ready: bool,
    ffmpeg_error: Option<String>,
    ffmpeg_download: Arc<DownloadProgress>,
    tab: Tab,

    background: Option<Background>,
    video_preview: Option<background::VideoPreviewPlayer>,
    show_bg_settings: bool,
    aspect_ratio: &'static str,
    fit: FitMode,
    motion: Motion,
    beat_pulse: bool,
    visual_effect: usize,

    tracks: Vec<Track>,
    next_track_id: u32,
    timestamps_text: String,

    selected_track: usize,
    lrc_text: String,
    whisper_model: ModelSize,
    language: usize,
    use_gpu: bool,
    theme: usize,
    style: StyleState,

    resolution: &'static str,
    fps: u32,
    encoder: Encoder,
    preset: String,
    quality: u32,
    job: Option<JobHandle>,

    pub custom_filename: String,
    pub quick_selected_track: Option<usize>,
    pub show_tab_background: bool,
    pub show_tab_playlist: bool,
    pub show_tab_lyrics: bool,
    pub show_tab_render: bool,
    pub show_tab_thumbnail: bool,
    pub show_tab_visualizer: bool,
    pub auto_generate_thumbnail: bool,
    pub(super) playlist_mode: PlaylistDisplayMode,
    pub(super) show_tracklist_visual: bool,
    pub(super) show_karaoke_lyrics: bool,
    pub(super) karaoke_preview_text: String,
    pub(super) tracklist_position: TracklistPosition,
    pub(super) tracklist_font_size: u32,
    pub(super) tracklist_title: String,
    pub(super) generate: generate_tab::GenerateState,
    pub(super) thumbnail: thumbnail_tab::ThumbnailState,
    pub(super) visualizer: loopkaraoke::visualizer::VisualizerSpec,
    pub(super) visualizer_playing: bool,
    pub(super) visualizer_time: f64,
    pub(super) visualizer_selected_track: usize,
    pub(super) visualizer_volume: f32,
    pub(super) visualizer_active_tab: usize,
    pub(super) visualizer_particles: Vec<visualizer_tab::VisualizerParticle>,
    pub(super) lrc_studio: lyrics::StandaloneLyricsState,
    storage: Option<StorageStats>,
    logs: Vec<LogEntry>,
    show_logs: bool,
    log_auto_scroll: bool,
    pub log_height: f32,
    sidebar_compact: bool,
    pub(super) dark_mode: bool,
    pub(super) video_settings: VideoRenderSettings,
    last_job_status: Option<JobStatus>,
    last_job_milestone: u32,
}

impl StudioApp {
    pub fn new(
        ctx: egui::Context,
        paths: AppPaths,
        tools: Tools,
        runtime: tokio::runtime::Runtime,
        transcriber: Arc<Transcriber>,
    ) -> Self {
        let settings = AppSettings::load(&paths);
        ctx.set_theme(if settings.dark_mode {
            egui::ThemePreference::Dark
        } else {
            egui::ThemePreference::Light
        });
        let resolution = RESOLUTION_LABELS
            .iter()
            .copied()
            .find(|label| *label == settings.resolution)
            .unwrap_or("1080p");
        let fps = if settings.fps == 60 { 60 } else { 30 };

        let (sender, events) = mpsc::channel();
        let mut app = Self {
            ctx,
            paths,
            tools,
            runtime,
            transcriber,
            transcribe_slot: Arc::new(Semaphore::new(1)),
            sender,
            events,
            pending: 0,
            notice: None,
            hardware: None,
            ffmpeg_ready: false,
            ffmpeg_error: None,
            ffmpeg_download: Arc::new(DownloadProgress::default()),
            tab: Tab::Quick,
            background: None,
            video_preview: None,
            show_bg_settings: false,
            aspect_ratio: "16:9",
            fit: FitMode::Crop,
            motion: Motion::Static,
            beat_pulse: false,
            visual_effect: 0,
            tracks: Vec::new(),
            next_track_id: 1,
            timestamps_text: String::new(),
            selected_track: 0,
            lrc_text: String::new(),
            whisper_model: ModelSize::Base,
            language: 0,
            use_gpu: true,
            theme: 0,
            style: StyleState::from_theme(&THEMES[0].1),
            resolution,
            fps,
            encoder: Encoder::CpuX264,
            preset: "veryfast".to_owned(),
            quality: 22,
            job: None,
            custom_filename: String::new(),
            quick_selected_track: None,
            show_tab_background: settings.show_tab_background,
            show_tab_playlist: settings.show_tab_playlist,
            show_tab_lyrics: settings.show_tab_lyrics,
            show_tab_visualizer: settings.show_tab_visualizer,
            show_tab_render: settings.show_tab_render,
            show_tab_thumbnail: settings.show_tab_thumbnail,
            auto_generate_thumbnail: settings.auto_generate_thumbnail,
            playlist_mode: PlaylistDisplayMode::Both,
            show_tracklist_visual: true,
            show_karaoke_lyrics: true,
            karaoke_preview_text: String::new(),
            tracklist_position: TracklistPosition::TopLeft,
            tracklist_font_size: 26,
            tracklist_title: "DAFTAR LAGU".to_owned(),
            generate: generate_tab::GenerateState::from_settings(&settings.generate),
            thumbnail: thumbnail_tab::ThumbnailState::default(),
            visualizer: loopkaraoke::visualizer::VisualizerSpec::default(),
            visualizer_playing: false,
            visualizer_time: 0.0,
            visualizer_selected_track: 0,
            visualizer_volume: 100.0,
            visualizer_active_tab: 0,
            visualizer_particles: Vec::new(),
            lrc_studio: lyrics::StandaloneLyricsState::default(),
            storage: None,
            logs: vec![LogEntry {
                time: chrono::Local::now().format("%H:%M:%S").to_string(),
                level: LogLevel::Info,
                message: "LoopKaraoke Studio siap digunakan.".to_owned(),
            }],
            show_logs: true,
            log_auto_scroll: true,
            log_height: 84.0,
            sidebar_compact: false,
            dark_mode: settings.dark_mode,
            video_settings: settings.video,
            last_job_status: None,
            last_job_milestone: 0,
        };
        app.sync_encoder_with_video_settings();
        app.start_up();
        app
    }

    pub(super) fn save_settings(&mut self) {
        let settings = AppSettings {
            dark_mode: self.dark_mode,
            video: self.video_settings,
            show_tab_background: self.show_tab_background,
            show_tab_playlist: self.show_tab_playlist,
            show_tab_lyrics: self.show_tab_lyrics,
            show_tab_visualizer: self.show_tab_visualizer,
            show_tab_render: self.show_tab_render,
            show_tab_thumbnail: self.show_tab_thumbnail,
            resolution: self.resolution.to_owned(),
            fps: self.fps,
            auto_generate_thumbnail: self.auto_generate_thumbnail,
            generate: self.generate.to_settings(),
        };
        if let Err(error) = settings.save(&self.paths) {
            self.warn(format!("Gagal menyimpan pengaturan: {error}"));
        }
    }

    pub(super) fn sync_encoder_with_video_settings(&mut self) {
        let hardware = self.hardware.clone().unwrap_or_default();
        let encoder = self.video_settings.resolve_encoder(&hardware);
        self.encoder = encoder;
        self.preset = self.video_settings.resolve_preset(encoder).to_owned();
        self.quality = self.video_settings.resolve_quality(encoder);
    }

    pub(super) fn has_any_track_lyrics(&self) -> bool {
        self.tracks
            .iter()
            .any(|t| t.has_lrc || !t.lyrics.is_empty())
    }

    pub(super) fn tab_visible(&self, tab: Tab) -> bool {
        match tab {
            Tab::Quick | Tab::Generate => true,
            Tab::Storage => true,
            Tab::Background => self.show_tab_background,
            Tab::Playlist => self.show_tab_playlist,
            Tab::Lyrics => self.show_tab_lyrics,
            Tab::Karaoke => true,
            Tab::Visualizer => self.show_tab_visualizer,
            Tab::Render => self.show_tab_render,
            Tab::Thumbnail => self.show_tab_thumbnail,
        }
    }

    fn start_up(&mut self) {
        self.install_ffmpeg();
    }

    /// Finds FFmpeg, downloading it first if it is missing.
    fn install_ffmpeg(&mut self) {
        self.ffmpeg_error = None;
        let tools_dir = self.paths.data_dir.join("tools");
        let progress = self.ffmpeg_download.clone();
        self.spawn(
            async move { Event::Ffmpeg(ffmpeg::ensure_installed(tools_dir, progress).await) },
        );
    }

    /// Work that needs FFmpeg, started once it is available.
    fn after_ffmpeg_ready(&mut self) {
        let tools = self.tools.clone();
        self.spawn(async move {
            Event::Hardware(loopkaraoke::hardware::HardwareInfo::detect(&tools).await)
        });

        let paths = self.paths.clone();
        self.spawn(async move {
            let options = CleanupOptions {
                clean_temp: true,
                min_age: Some(STARTUP_TEMP_MAX_AGE),
                ..CleanupOptions::default()
            };
            let removed =
                tokio::task::spawn_blocking(move || storage::cleanup(&paths, &options).temp)
                    .await
                    .unwrap_or(0);
            Event::Cleaned(format!("{removed} berkas sementara lama dibersihkan"))
        });
    }

    /// Runs `work` on the async runtime and delivers its event to the UI thread.
    fn spawn(&mut self, work: impl Future<Output = Event> + Send + 'static) {
        self.pending += 1;
        let sender = self.sender.clone();
        let ctx = self.ctx.clone();
        self.runtime.spawn(async move {
            let _ = sender.send(work.await);
            ctx.request_repaint();
        });
    }

    pub fn log(&mut self, level: LogLevel, text: impl Into<String>) {
        let time = chrono::Local::now().format("%H:%M:%S").to_string();
        let message = text.into();
        self.logs.push(LogEntry {
            time,
            level,
            message,
        });
        if self.logs.len() > 1000 {
            self.logs.drain(0..self.logs.len() - 1000);
        }
    }

    pub fn info(&mut self, text: impl Into<String>) {
        let msg = text.into();
        self.log(LogLevel::Info, &msg);
        self.notice = Some(Notice::new(msg, false));
    }

    pub fn error(&mut self, text: impl Into<String>) {
        let msg = text.into();
        self.log(LogLevel::Error, &msg);
        self.notice = Some(Notice::new(msg, true));
    }

    pub fn warn(&mut self, text: impl Into<String>) {
        let msg = text.into();
        self.log(LogLevel::Warn, &msg);
        self.notice = Some(Notice::new(msg, false));
    }

    fn notify_success(&mut self, log_text: impl Into<String>, toast_text: impl Into<String>) {
        self.log(LogLevel::Success, log_text);
        self.notice = Some(Notice::new(toast_text, false));
    }

    fn handle(&mut self, event: Event) {
        self.pending = self.pending.saturating_sub(1);
        match event {
            Event::Ffmpeg(Ok(tools)) => {
                self.tools = tools;
                self.ffmpeg_ready = true;
                self.log(
                    LogLevel::Success,
                    "FFmpeg berhasil dimuat dan siap digunakan.",
                );
                self.after_ffmpeg_ready();
            }
            Event::Ffmpeg(Err(error)) => {
                self.ffmpeg_error = Some(error.to_string());
                self.error(format!("FFmpeg gagal dimuat: {error}"));
            }
            Event::Hardware(hardware) => {
                self.use_gpu = hardware.gpu_whisper;
                self.hardware = Some(hardware.clone());
                self.sync_encoder_with_video_settings();
                let info_msg = if hardware.has_gpu() {
                    format!(
                        "Hardware terdeteksi: {} ({}) — Encoder: {}",
                        hardware.gpu_name,
                        hardware.vendor(),
                        self.encoder.name()
                    )
                } else {
                    format!(
                        "Hardware: GPU tidak terdeteksi — menggunakan CPU ({})",
                        self.encoder.name()
                    )
                };
                self.log(LogLevel::Info, info_msg);
            }
            Event::Background(Ok(background)) => {
                if let Some(aspect) = background::known_aspect(&background.video.aspect_ratio) {
                    self.aspect_ratio = aspect;
                }
                let bg_msg = format!(
                    "Background siap: {} ({}x{}, {:.1}s, {:.0} FPS)",
                    background.video.filename,
                    background.video.width,
                    background.video.height,
                    background.video.duration,
                    background.video.fps
                );
                self.notify_success(
                    bg_msg,
                    format!("Background siap: {}", background.video.filename),
                );
                self.background = Some(background);
                self.extract_thumbnail_frame();
                self.start_background_preview();
            }
            Event::Background(Err(error)) => {
                self.error(format!("Background gagal dimuat: {error}"));
            }
            Event::Audios(results) => {
                let mut failed = Vec::new();
                let mut added_count = 0;
                let mut lrc_count = 0;
                for result in results {
                    match result {
                        Ok(info) => {
                            let companion = playlist::find_companion_lrc(
                                std::path::Path::new(&info.path),
                                info.duration,
                            );
                            let (lyrics, has_lrc) = match companion {
                                Some((lrc_path, parsed_lyrics)) => {
                                    lrc_count += 1;
                                    let lrc_name = lrc_path
                                        .file_name()
                                        .map(|n| n.to_string_lossy().into_owned())
                                        .unwrap_or_else(|| "file.lrc".to_owned());
                                    self.log(
                                        LogLevel::Success,
                                        format!(
                                            "Lirik LRC ditemukan untuk \"{}\": {} ({} baris)",
                                            info.title,
                                            lrc_name,
                                            parsed_lyrics.len()
                                        ),
                                    );
                                    (parsed_lyrics, true)
                                }
                                None => (Vec::new(), false),
                            };
                            self.log(
                                LogLevel::Info,
                                format!(
                                    "Lagu dimuat: \"{}\" oleh \"{}\" ({})",
                                    info.title,
                                    if info.artist.is_empty() {
                                        "Unknown"
                                    } else {
                                        &info.artist
                                    },
                                    info.duration_str
                                ),
                            );
                            self.add_track_with_lrc(info, lyrics, has_lrc);
                            added_count += 1;
                        }
                        Err(error) => failed.push(error.to_string()),
                    }
                }
                if failed.is_empty() {
                    let msg = if lrc_count > 0 {
                        format!(
                            "{added_count} lagu berhasil ditambahkan ({lrc_count} lagu ber-LRC)."
                        )
                    } else {
                        format!("{added_count} lagu berhasil ditambahkan ke playlist.")
                    };
                    self.notify_success(&msg, msg.clone());
                } else {
                    self.warn(format!("Sebagian lagu gagal dimuat: {}", failed.join("; ")));
                }
            }
            Event::Lyrics { track, result } => {
                let Some(entry) = self.tracks.iter_mut().find(|entry| entry.id == track) else {
                    return;
                };
                entry.transcribing = false;
                let title = entry.title.clone();
                match result {
                    Ok(lyrics) => {
                        let count = lyrics.len();
                        entry.lyrics = lyrics;
                        let msg =
                            format!("Berhasil mendeteksi {count} baris lirik untuk \"{title}\"");
                        self.notify_success(&msg, msg.clone());
                    }
                    Err(error) => self.error(format!("Transkripsi \"{title}\" gagal: {error}")),
                }
            }
            Event::StandaloneAudios(items) => {
                self.lrc_studio.loading_audio = false;
                let mut added = 0;
                let mut errors = Vec::new();
                for (info, wav_path) in items {
                    match info {
                        Ok(audio) => {
                            self.lrc_studio.add_audio_track(audio, wav_path);
                            added += 1;
                        }
                        Err(error) => errors.push(error.to_string()),
                    }
                }
                if added > 0 {
                    let msg = format!("{added} berkas lagu berhasil dimuat ke Studio Lirik.");
                    self.notify_success(&msg, msg.clone());
                }
                if !errors.is_empty() {
                    self.warn(format!(
                        "Sebagian lagu gagal dimuat di Studio Lirik: {}",
                        errors.join("; ")
                    ));
                }
            }
            Event::StandaloneLyrics { track_id, result } => {
                let add_intro = self.lrc_studio.auto_intro_marker;
                let min_vocal = self.lrc_studio.default_intro_seconds.clamp(1.5, 4.0);
                let add_interlude = self.lrc_studio.auto_interlude_marker;
                let interlude_gap = self.lrc_studio.interlude_min_gap_seconds;
                let Some(track) = self.lrc_studio.tracks.iter_mut().find(|t| t.id == track_id)
                else {
                    return;
                };
                track.transcribing = false;
                let title = track.title.clone();
                match result {
                    Ok(lyrics) => {
                        track.apply_whisper_lines(
                            lyrics,
                            add_intro,
                            min_vocal,
                            add_interlude,
                            interlude_gap,
                        );
                        let count = track.lines.len();
                        let msg = format!(
                            "Whisper AI selesai untuk \"{title}\" ({count} baris termasuk penanda intro/jeda). Lagu kini bertanda ✓ Sudah Ada Lirik."
                        );
                        self.notify_success(&msg, msg.clone());
                    }
                    Err(error) => {
                        self.error(format!("Transkripsi Whisper \"{title}\" gagal: {error}"));
                    }
                }
            }
            Event::Demo(Ok(set)) => {
                self.background = Some(Background {
                    video: set.video,
                    image: None,
                });
                self.extract_thumbnail_frame();
                self.start_background_preview();
                self.tracks.clear();
                for track in set.tracks {
                    self.add_track(track.info, track.lyrics);
                }
                let msg = "Contoh demo dimuat: video latar dan 2 lagu dengan lirik.";
                self.notify_success(msg, msg);
            }
            Event::Demo(Err(error)) => self.error(format!("Demo gagal dibuat: {error}")),
            Event::Thumbnail(result) => {
                self.thumbnail.busy = false;
                match result {
                    Ok(path) => {
                        let msg = format!("Thumbnail tersimpan: {}", path.display());
                        self.notify_success(&msg, msg.clone());
                        self.thumbnail.result = Some(path);
                    }
                    Err(error) => self.error(format!("Thumbnail gagal: {error}")),
                }
            }
            Event::ThumbnailFrame(result) => {
                self.thumbnail.extracting_frame = false;
                match result {
                    Ok(path) => {
                        self.thumbnail.preview_frame = Some(path);
                        self.log(
                            LogLevel::Info,
                            "Frame background thumbnail berhasil diambil.",
                        );
                    }
                    Err(error) => self.error(format!("Gagal mengambil frame video: {error}")),
                }
            }
            Event::GenerateVideo(Ok(video)) => {
                let msg = format!(
                    "Video Generate siap: {} ({}x{}, {:.1}s, {:.0} FPS)",
                    video.filename, video.width, video.height, video.duration, video.fps
                );
                self.notify_success(msg, format!("Video Generate siap: {}", video.filename));
                self.generate.video = Some(video);
            }
            Event::GenerateVideo(Err(error)) => {
                self.error(format!("Video Generate gagal dimuat: {error}"));
            }
            Event::GenerateAudios(results) => {
                let mut failed = Vec::new();
                let mut added_count = 0;
                for result in results {
                    match result {
                        Ok(info) => {
                            self.log(
                                LogLevel::Info,
                                format!(
                                    "Lagu Generate ditambahkan: {} ({})",
                                    info.filename, info.duration_str
                                ),
                            );
                            self.generate.push_audio(info);
                            added_count += 1;
                        }
                        Err(error) => failed.push(error.to_string()),
                    }
                }
                if failed.is_empty() && added_count > 0 {
                    let msg = format!("{added_count} lagu ditambahkan ke Playlist Album.");
                    self.notify_success(&msg, msg.clone());
                } else if !failed.is_empty() {
                    self.warn(format!("Sebagian lagu gagal dimuat: {}", failed.join("; ")));
                }
            }
            Event::GeneratePreview { track_id, result } => {
                if self.generate.loading_preview_id != Some(track_id) {
                    return;
                }
                self.generate.loading_preview_id = None;
                match result {
                    Ok(wav) => {
                        lyrics::win_audio::play_from_with_volume(
                            &wav,
                            0.0,
                            self.generate.preview_volume,
                        );
                        self.generate.playing_track_id = Some(track_id);
                    }
                    Err(error) => self.warn(format!("Gagal memutar pratinjau lagu: {error}")),
                }
            }
            Event::Storage(stats) => {
                self.log(
                    LogLevel::Info,
                    format!(
                        "Statistik penyimpanan: Total penggunaan {} MB",
                        stats.total_app_mb
                    ),
                );
                self.storage = Some(stats);
            }
            Event::Cleaned(message) => self.info(message),
        }
    }

    fn add_track(&mut self, info: AudioInfo, lyrics: Vec<LyricLine>) {
        let has_lrc = !lyrics.is_empty();
        self.add_track_with_lrc(info, lyrics, has_lrc);
    }

    fn add_track_with_lrc(&mut self, info: AudioInfo, lyrics: Vec<LyricLine>, has_lrc: bool) {
        let id = self.next_track_id;
        self.next_track_id += 1;
        self.tracks
            .push(Track::from_info(id, info, lyrics, has_lrc));
    }

    fn total_duration(&self) -> f64 {
        self.tracks.iter().map(|track| track.duration).sum()
    }

    /// Whether the user has finished the step behind `tab`, shown as a dot on the tab.
    fn step_done(&self, tab: Tab) -> bool {
        match tab {
            Tab::Quick => self.background.is_some() && !self.tracks.is_empty(),
            Tab::Generate => false,
            Tab::Background => self.background.is_some(),
            Tab::Playlist => !self.tracks.is_empty(),
            Tab::Lyrics => {
                !self.tracks.is_empty() && self.tracks.iter().all(|track| !track.lyrics.is_empty())
            }
            Tab::Karaoke => self.show_karaoke_lyrics && self.has_any_track_lyrics(),
            Tab::Visualizer => self.visualizer.enabled,
            Tab::Render => self
                .job
                .as_ref()
                .is_some_and(|job| matches!(job.snapshot().status, JobStatus::Completed)),
            Tab::Thumbnail => self.thumbnail.result.is_some(),
            Tab::Storage => false,
        }
    }

    /// Routes dropped files to the step they belong to, by extension.
    fn handle_drop(&mut self, files: Vec<egui::DroppedFile>) {
        if !self.ffmpeg_ready {
            return;
        }
        let on_generate = self.tab == Tab::Generate;
        let mut audios = Vec::new();
        for path in files.into_iter().filter_map(|file| file.path) {
            if path.is_dir() {
                if on_generate {
                    self.load_generate_audio_folder(path);
                } else {
                    self.load_audio_folder(path);
                }
                continue;
            }
            let extension = path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(str::to_ascii_lowercase)
                .unwrap_or_default();
            let is = |list: &[&str]| list.contains(&extension.as_str());
            if is(&background::VIDEO_EXTENSIONS) {
                if on_generate {
                    self.load_generate_video(path);
                } else {
                    if self.tab != Tab::Quick {
                        self.tab = Tab::Background;
                    }
                    self.load_video(path);
                }
            } else if is(&background::IMAGE_EXTENSIONS) {
                if on_generate {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.display().to_string());
                    self.generate.thumbnail_path = Some(path);
                    self.info(format!("Thumbnail Generate dipilih: {name}"));
                } else {
                    if self.tab != Tab::Quick {
                        self.tab = Tab::Background;
                    }
                    self.load_image(path);
                }
            } else if is(&playlist::AUDIO_EXTENSIONS) {
                audios.push(path);
            } else if !on_generate && is(&["lrc", "txt"]) && !self.tracks.is_empty() {
                let lrc_stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default();
                let matched_idx = self.tracks.iter().position(|t| {
                    t.path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .is_some_and(|s| s.eq_ignore_ascii_case(lrc_stem))
                });
                match std::fs::read_to_string(&path) {
                    Ok(text) => {
                        if let Some(idx) = matched_idx {
                            let parsed = loopkaraoke::karaoke::lrc::parse_lrc(&text);
                            self.tracks[idx].lyrics = parsed;
                            self.tracks[idx].has_lrc = true;
                            let title = self.tracks[idx].title.clone();
                            self.info(format!("File LRC dihubungkan ke lagu \"{title}\"."));
                        } else {
                            self.apply_lrc(&text);
                        }
                    }
                    Err(error) => self.error(format!("File lirik tidak terbaca: {error}")),
                }
            } else if !is(&["lrc", "txt"]) {
                self.warn(format!("Format file tidak didukung: {}", path.display()));
            }
        }
        if !audios.is_empty() {
            if on_generate {
                self.load_generate_audios(audios);
            } else {
                if self.tab != Tab::Quick {
                    self.tab = Tab::Playlist;
                }
                self.load_audios(audios);
            }
        }
    }

    fn job_running(&self) -> bool {
        self.job
            .as_ref()
            .is_some_and(|job| job.snapshot().is_running())
    }

    pub(super) fn pick_background_dynamic(&mut self) {
        let picked = rfd::FileDialog::new()
            .add_filter(
                "Media Background (Video & Gambar)",
                &[
                    "mp4", "mov", "mkv", "webm", "avi", "jpg", "jpeg", "png", "webp", "jfif",
                ],
            )
            .add_filter("Video", &background::VIDEO_EXTENSIONS)
            .add_filter("Gambar", &background::IMAGE_EXTENSIONS)
            .pick_file();
        if let Some(path) = picked {
            self.load_media_dynamic(path);
        }
    }

    pub(super) fn load_media_dynamic(&mut self, path: PathBuf) {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        if background::VIDEO_EXTENSIONS.contains(&ext.as_str()) {
            self.load_video(path);
        } else if background::IMAGE_EXTENSIONS.contains(&ext.as_str()) {
            self.load_image(path);
        } else {
            self.warn(format!("Format berkas tidak didukung: {}", path.display()));
        }
    }

    pub(super) fn pick_audio_files(&mut self) {
        let picked = rfd::FileDialog::new()
            .add_filter("Audio", &playlist::AUDIO_EXTENSIONS)
            .pick_files();
        if let Some(paths) = picked {
            self.load_audios(paths);
        }
    }

    pub(super) fn pick_audio_folder(&mut self) {
        let picked = rfd::FileDialog::new().pick_folder();
        if let Some(folder) = picked {
            self.load_audio_folder(folder);
        }
    }

    pub(super) fn load_audio_folder(&mut self, folder: PathBuf) {
        let mut audios = Vec::new();
        collect_audios_in_folder(&folder, &mut audios);
        if audios.is_empty() {
            self.warn(format!(
                "Tidak ada file audio ditemukan di folder: {}",
                folder.display()
            ));
        } else {
            let count = audios.len();
            self.log(
                LogLevel::Info,
                format!(
                    "Memindai folder: {count} audio ditemukan di {}",
                    folder.display()
                ),
            );
            self.load_audios(audios);
        }
    }

    pub(super) fn shuffle_tracks(&mut self) {
        if self.tracks.len() <= 1 {
            return;
        }
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        self.tracks.shuffle(&mut rng);
        self.selected_track = 0;
        self.timestamps_text.clear();
        self.quick_selected_track = None;
        self.log(
            LogLevel::Success,
            format!(
                "Urutan {} lagu di playlist berhasil diacak (shuffle).",
                self.tracks.len()
            ),
        );
    }
}

pub(super) fn collect_audios_in_folder(folder: &std::path::Path, out: &mut Vec<PathBuf>) {
    let mut found = Vec::new();
    if let Ok(entries) = std::fs::read_dir(folder) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(str::to_ascii_lowercase)
                    .unwrap_or_default();
                if playlist::AUDIO_EXTENSIONS.contains(&ext.as_str()) {
                    found.push(path);
                }
            }
        }
    }
    found.sort();
    out.extend(found);
}

impl eframe::App for StudioApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let bg = if self.dark_mode {
            theme::PALETTE_DARK.bg
        } else {
            theme::Palette::light().bg
        };
        bg.to_normalized_gamma_f32()
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(event) = self.events.try_recv() {
            self.handle(event);
        }
        let dropped = ctx.input(|input| input.raw.dropped_files.clone());
        if !dropped.is_empty() {
            self.handle_drop(dropped);
        }

        // Monitor render job progress and log milestones
        if let Some(job) = &self.job {
            let snapshot = job.snapshot();
            if self.last_job_status != Some(snapshot.status) {
                self.last_job_status = Some(snapshot.status);
                match snapshot.status {
                    JobStatus::Preparing => {
                        self.log(
                            LogLevel::Info,
                            "Render: Menyiapkan gabungan audio, subtitle ASS, dan filter graph...",
                        );
                    }
                    JobStatus::Rendering => {
                        self.log(
                            LogLevel::Info,
                            format!(
                                "Render: Proses encode video dimulai menggunakan {}",
                                snapshot.encoder_name
                            ),
                        );
                    }
                    JobStatus::Completed => {
                        if let Some(output) = &snapshot.output_file {
                            self.notify_success(
                                format!("Render: Selesai 100%! Berkas video: {}", output.display()),
                                format!("Render selesai: {}", output.display()),
                            );
                        } else {
                            self.notify_success("Render: Selesai 100%!", "Render selesai 100%!");
                        }
                    }
                    JobStatus::Failed => {
                        self.error(format!("Render: Gagal - {}", snapshot.error_message));
                    }
                    _ => {}
                }
            }

            if snapshot.status == JobStatus::Rendering {
                let milestone = (snapshot.progress_percent / 20.0).floor() as u32 * 20;
                if milestone > 0 && milestone < 100 && milestone != self.last_job_milestone {
                    self.last_job_milestone = milestone;
                    self.log(
                        LogLevel::Info,
                        format!(
                            "Render progres: {}% • {} / {} • kec {} • {:.0} fps • sisa {}s",
                            milestone,
                            loopkaraoke::media::format_clock(snapshot.current_time_sec as u64),
                            loopkaraoke::media::format_clock(snapshot.total_time_sec as u64),
                            snapshot.speed,
                            snapshot.fps,
                            snapshot.eta_seconds as u64
                        ),
                    );
                }
            }
        }

        let p = theme::Palette::of(ctx);

        egui::TopBottomPanel::bottom("status")
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(p.surface)
                    .stroke(egui::Stroke::new(1.0_f32, p.border))
                    .corner_radius(egui::CornerRadius::same(theme::metrics::RADIUS_MD as u8))
                    .inner_margin(egui::Margin::symmetric(10, 4))
                    .outer_margin(egui::Margin {
                        left: theme::GAP,
                        right: theme::GAP,
                        top: theme::GAP,
                        bottom: theme::GAP,
                    }),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 2.0);
                ui.spacing_mut().interact_size.y = 20.0;
                ui.spacing_mut().button_padding = egui::vec2(6.0, 2.0);
                self.footer(ui);
                if self.show_logs {
                    self.log_console(ui);
                }
            });

        let sidebar_width = if self.sidebar_compact {
            theme::SIDEBAR_COMPACT_WIDTH
        } else {
            let max_label_width = Tab::ALL
                .into_iter()
                .filter(|&tab| self.tab_visible(tab))
                .map(|tab| {
                    ctx.fonts(|fonts| {
                        fonts
                            .layout_no_wrap(
                                tab.label().to_owned(),
                                egui::FontId::proportional(theme::metrics::FONT_MD),
                                p.text,
                            )
                            .size()
                            .x
                    })
                })
                .fold(0.0_f32, f32::max);
            // 6px left gap + 2px border + 16px inner_margin + 34px icon offset + longest label + 16px (1 rem)
            (24.0 + 34.0 + max_label_width + 16.0)
                .ceil()
                .max(theme::SIDEBAR_WIDTH)
        };
        egui::SidePanel::left("sidebar")
            .resizable(false)
            .exact_width(sidebar_width)
            .show_separator_line(false)
            .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                left: theme::GAP,
                right: 0,
                top: theme::GAP,
                bottom: 0,
            }))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(p.surface)
                    .stroke(egui::Stroke::new(1.0_f32, p.border))
                    .corner_radius(egui::CornerRadius::same(theme::metrics::RADIUS_MD as u8))
                    .inner_margin(egui::Margin::symmetric(8, 12))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.set_min_height(ui.available_height());

                        let is_dark = ctx.style().visuals.dark_mode;
                        if self.sidebar_compact {
                            ui.vertical_centered(|ui| {
                                let resp = ui
                                    .add(sidebar_logo_image(is_dark).sense(egui::Sense::click()))
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .on_hover_text("Lebarkan sidebar");
                                if resp.clicked() {
                                    self.sidebar_compact = false;
                                }
                            });
                        } else {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                ui.add_space(4.0);
                                ui.add(sidebar_logo_image(is_dark));
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if theme::sidebar_toggle_button(ui) {
                                            self.sidebar_compact = true;
                                        }
                                    },
                                );
                            });
                        }

                        ui.add_space(4.0);
                        let sep_y = ui.cursor().min.y;
                        let sep_rect = ui.max_rect();
                        ui.painter().line_segment(
                            [
                                egui::pos2(sep_rect.min.x + 2.0, sep_y),
                                egui::pos2(sep_rect.max.x - 2.0, sep_y),
                            ],
                            egui::Stroke::new(1.0_f32, p.border),
                        );
                        ui.add_space(6.0);

                        ui.spacing_mut().item_spacing.y = 4.0;
                        for tab in Tab::ALL {
                            if !self.tab_visible(tab) {
                                continue;
                            }
                            if theme::sidebar_tab(
                                ui,
                                tab.icon(),
                                tab.label(),
                                self.tab == tab,
                                self.step_done(tab),
                                self.sidebar_compact,
                                tab.help(),
                            ) {
                                self.tab = tab;
                            }
                        }
                    });
            });

        let content_rect = egui::CentralPanel::default()
            .frame(theme::panel_frame(
                ctx,
                if self.tab == Tab::Generate { 12 } else { 16 },
            ))
            .show(ctx, |ui| {
                let panel_rect = ui.max_rect();
                if !self.ffmpeg_ready {
                    self.ffmpeg_setup_panel(ui);
                    return panel_rect;
                }
                if !self.tab_visible(self.tab) {
                    self.tab = Tab::Quick;
                }
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.tab {
                        Tab::Quick => self.quick_tab(ui),
                        Tab::Generate => self.generate_tab(ui),
                        Tab::Background => self.background_tab(ui),
                        Tab::Playlist => self.playlist_tab(ui),
                        Tab::Lyrics => self.lyrics_tab(ui),
                        Tab::Karaoke => self.karaoke_tab(ui),
                        Tab::Visualizer => self.visualizer_tab(ui),
                        Tab::Render => self.render_tab(ui),
                        Tab::Thumbnail => self.thumbnail_tab(ui),
                        Tab::Storage => self.storage_tab(ui),
                    });
                panel_rect
            })
            .inner;

        self.render_toast(ctx, content_rect);

        if ctx.input(|input| !input.raw.hovered_files.is_empty()) {
            drop_overlay(ctx);
        }

        if self.job_running() || self.pending > 0 {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }
}

fn draw_toast_icon(painter: &egui::Painter, c: egui::Pos2, is_error: bool, color: egui::Color32) {
    let stroke = egui::Stroke::new(1.7_f32, color);
    if is_error {
        painter.line_segment(
            [
                egui::pos2(c.x - 4.0, c.y - 4.0),
                egui::pos2(c.x + 4.0, c.y + 4.0),
            ],
            stroke,
        );
        painter.line_segment(
            [
                egui::pos2(c.x + 4.0, c.y - 4.0),
                egui::pos2(c.x - 4.0, c.y + 4.0),
            ],
            stroke,
        );
    } else {
        painter.add(egui::Shape::line(
            vec![
                egui::pos2(c.x - 4.5, c.y + 0.5),
                egui::pos2(c.x - 1.2, c.y + 3.8),
                egui::pos2(c.x + 4.8, c.y - 3.5),
            ],
            stroke,
        ));
    }
}

fn drop_overlay(ctx: &egui::Context) {
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("drop_overlay"),
    ));
    let screen = ctx.screen_rect();
    painter.rect_filled(screen, 0.0, egui::Color32::from_black_alpha(190));
    painter.rect_stroke(
        screen.shrink(12.0),
        6.0,
        egui::Stroke::new(2.0_f32, theme::ACCENT),
        egui::StrokeKind::Inside,
    );
    painter.text(
        screen.center(),
        egui::Align2::CENTER_CENTER,
        "Lepaskan file di sini\nVideo atau gambar untuk background, audio untuk playlist, .lrc untuk lirik",
        egui::FontId::proportional(16.0),
        egui::Color32::WHITE,
    );
}

impl StudioApp {
    fn hardware_status_text(&self) -> String {
        match &self.hardware {
            Some(hardware) if hardware.has_gpu() => format!("GPU: {}", hardware.gpu_name),
            Some(_) => "GPU NVIDIA/AMD tidak ditemukan — mode CPU".to_owned(),
            None => "Mendeteksi hardware...".to_owned(),
        }
    }

    /// Bottom bar: status on the left, progress and the step's main action on the right.
    fn footer(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if self.pending > 0 {
                ui.spinner();
                ui.label("Memproses...");
            }
            let toggle_label = if self.show_logs {
                format!("📋 Log ({}) ▼", self.logs.len())
            } else {
                format!("📋 Log ({}) ▲", self.logs.len())
            };
            if ui
                .add(StyledButton::ghost(toggle_label).small())
                .on_hover_text("Buka/tutup jendela log aktivitas")
                .clicked()
            {
                self.show_logs = !self.show_logs;
            }
            if !self.show_logs {
                ui.metadata_label(self.hardware_status_text());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.footer_actions(ui);
            });
        });
    }

    fn render_toast(&mut self, ctx: &egui::Context, content_rect: egui::Rect) {
        let Some(notice) = &self.notice else {
            return;
        };
        let elapsed = notice.shown_at.elapsed().as_secs_f64();
        if elapsed >= TOAST_LIFETIME_SECS {
            self.notice = None;
            return;
        }
        let fade_window = 0.4;
        let remaining = TOAST_LIFETIME_SECS - elapsed;
        let alpha = (remaining / fade_window).clamp(0.0, 1.0) as f32;
        if remaining <= fade_window {
            ctx.request_repaint_after(Duration::from_millis(33));
        } else {
            let until_fade = ((remaining - fade_window) * 1000.0).ceil().max(50.0) as u64;
            ctx.request_repaint_after(Duration::from_millis(until_fade));
        }

        let p = theme::Palette::of(ctx);
        let dark = self.dark_mode;
        let is_error = notice.is_error;
        let text = notice.text.clone();
        let max_text_w = (content_rect.width() - 96.0).clamp(180.0, 320.0);

        let mut dismissed = false;
        let toast_pos = content_rect.max - egui::vec2(8.0, 8.0);
        egui::Area::new(egui::Id::new("bottom_right_toast"))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::RIGHT_BOTTOM)
            .fixed_pos(toast_pos)
            .show(ctx, |ui| {
                let frame_resp = egui::Frame::new()
                    .fill(p.surface.gamma_multiply(alpha))
                    .stroke(egui::Stroke::new(1.0_f32, p.border.gamma_multiply(alpha)))
                    .corner_radius(egui::CornerRadius::same(12))
                    .inner_margin(egui::Margin::symmetric(14, 10))
                    .shadow(egui::Shadow {
                        offset: [0, 4],
                        blur: 14,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(if dark { 115 } else { 30 })
                            .gamma_multiply(alpha),
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;
                            let (badge_rect, _) = ui
                                .allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::hover());
                            let (badge_fill, icon_color) = if is_error {
                                if dark {
                                    (egui::Color32::from_rgb(56, 26, 30), theme::DANGER)
                                } else {
                                    (egui::Color32::from_rgb(254, 226, 226), theme::DANGER)
                                }
                            } else if dark {
                                (
                                    egui::Color32::from_rgb(28, 52, 48),
                                    egui::Color32::from_rgb(110, 231, 183),
                                )
                            } else {
                                (
                                    egui::Color32::from_rgb(220, 252, 231),
                                    egui::Color32::from_rgb(21, 128, 61),
                                )
                            };
                            let painter = ui.painter();
                            painter.rect_filled(
                                badge_rect,
                                egui::CornerRadius::same(8),
                                badge_fill.gamma_multiply(alpha),
                            );
                            draw_toast_icon(
                                painter,
                                badge_rect.center(),
                                is_error,
                                icon_color.gamma_multiply(alpha),
                            );

                            ui.scope(|ui| {
                                ui.set_max_width(max_text_w);
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(&text)
                                            .size(theme::metrics::FONT_MD)
                                            .color(p.text_strong.gamma_multiply(alpha)),
                                    )
                                    .wrap(),
                                );
                            });
                        });
                    });
                if frame_resp.response.interact(egui::Sense::click()).clicked() {
                    dismissed = true;
                }
            });

        if dismissed {
            self.notice = None;
        }
    }

    fn log_console(&mut self, ui: &mut egui::Ui) {
        // Interactive drag handle to resize log height freely
        let (handle_rect, handle_response) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 4.0), egui::Sense::drag());

        if handle_response.hovered() || handle_response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }

        if handle_response.dragged() {
            let delta = handle_response.drag_delta().y;
            // Dragging up (negative y) increases height; dragging down decreases height
            self.log_height = (self.log_height - delta).clamp(44.0, 400.0);
        }

        if handle_response.double_clicked() {
            self.log_height = 84.0;
        }

        let p = theme::Palette::of(ui.ctx());
        // Draw resize grip visual indicator
        let grip_color = if handle_response.hovered() || handle_response.dragged() {
            p.accent_hover
        } else {
            p.border
        };
        let center_y = handle_rect.center().y;
        let center_x = handle_rect.center().x;
        ui.painter().line_segment(
            [
                egui::pos2(center_x - 32.0, center_y),
                egui::pos2(center_x + 32.0, center_y),
            ],
            egui::Stroke::new(2.0_f32, grip_color),
        );

        ui.horizontal(|ui| {
            ui.monospace(
                egui::RichText::new("LOG AKTIVITAS")
                    .size(11.0)
                    .color(p.text_muted)
                    .strong(),
            );
            ui.metadata_label(format!(
                "• {} entri ({:.0}px)",
                self.logs.len(),
                self.log_height
            ));
            ui.checkbox(&mut self.log_auto_scroll, "Auto-scroll");

            ui.space1();
            ui.metadata_label("Ukuran:");
            if ui
                .add(StyledButton::ghost("S").small())
                .on_hover_text("Tinggi Kecil (56px)")
                .clicked()
            {
                self.log_height = 56.0;
            }
            if ui
                .add(StyledButton::ghost("M").small())
                .on_hover_text("Tinggi Sedang (84px)")
                .clicked()
            {
                self.log_height = 84.0;
            }
            if ui
                .add(StyledButton::ghost("L").small())
                .on_hover_text("Tinggi Besar (160px)")
                .clicked()
            {
                self.log_height = 160.0;
            }
            ui.space2();
            ui.metadata_label(format!("• {}", self.hardware_status_text()));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(StyledButton::ghost("Sembunyikan").small()).clicked() {
                    self.show_logs = false;
                }
                if ui.add(StyledButton::ghost("Bersihkan").small()).clicked() {
                    self.logs.clear();
                }
                if ui
                    .add(StyledButton::secondary("Salin Semua").small())
                    .clicked()
                {
                    let text = self
                        .logs
                        .iter()
                        .map(|entry| {
                            format!(
                                "[{}] [{}] {}",
                                entry.time,
                                entry.level.label(),
                                entry.message
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    ui.ctx().copy_text(text);
                    self.notice = Some(Notice::new("Log berhasil disalin ke clipboard.", false));
                }
            });
        });

        let console_frame = egui::Frame::new()
            .fill(p.bg)
            .stroke(egui::Stroke::new(1.0_f32, p.border))
            .corner_radius(egui::CornerRadius::same(theme::metrics::RADIUS_SM as u8))
            .inner_margin(egui::Margin::symmetric(8, 4));

        console_frame.show(ui, |ui| {
            let mut scroll = egui::ScrollArea::vertical()
                .max_height(self.log_height)
                .min_scrolled_height(self.log_height)
                .auto_shrink([false, false]);

            if self.log_auto_scroll {
                scroll = scroll.stick_to_bottom(true);
            }

            scroll.show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 1.0);
                ui.spacing_mut().interact_size.y = 14.0;
                if self.logs.is_empty() {
                    ui.weak("Belum ada log aktivitas.");
                } else {
                    for entry in &self.logs {
                        ui.horizontal(|ui| {
                            ui.monospace(
                                egui::RichText::new(format!("[{}]", entry.time))
                                    .size(11.0)
                                    .color(egui::Color32::from_gray(120)),
                            );
                            let (badge_color, badge_text) = match entry.level {
                                LogLevel::Info => (egui::Color32::from_rgb(100, 180, 255), "INFO"),
                                LogLevel::Success => (theme::SUCCESS, "OK  "),
                                LogLevel::Warn => (theme::WARNING, "WARN"),
                                LogLevel::Error => (theme::DANGER, "ERR "),
                            };
                            ui.monospace(
                                egui::RichText::new(format!("[{badge_text}]"))
                                    .size(11.0)
                                    .color(badge_color)
                                    .strong(),
                            );
                            ui.monospace(
                                egui::RichText::new(&entry.message)
                                    .size(11.0)
                                    .color(theme::TEXT),
                            );
                        });
                    }
                }
            });
        });
    }

    fn footer_actions(&mut self, ui: &mut egui::Ui) {
        if self.tab == Tab::Render || self.tab == Tab::Quick {
            let blocker = self.render_blocker();
            let running = self.job_running();
            if ui
                .add_enabled(
                    blocker.is_none() && !running,
                    theme::primary_button("Mulai Render"),
                )
                .clicked()
            {
                self.start_render();
            }
            if let (Some(reason), false) = (blocker, running) {
                ui.colored_label(theme::WARNING, reason);
            }
        } else if let Some(next) = self.tab.next() {
            if ui
                .add(theme::primary_button(format!("Lanjut: {}", next.label())))
                .clicked()
            {
                self.tab = next;
            }
        }
        if let Some(previous) = self.tab.previous() {
            if ui.button("Kembali").clicked() {
                self.tab = previous;
            }
        }
        if let Some(job) = self.job.as_ref().filter(|job| job.snapshot().is_running()) {
            let snapshot = job.snapshot();
            ui.add(
                egui::ProgressBar::new((snapshot.progress_percent / 100.0) as f32)
                    .desired_width(200.0)
                    .show_percentage(),
            );
            ui.label("Render");
        }
    }
}

impl StudioApp {
    fn ffmpeg_setup_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            ui.heading("Menyiapkan FFmpeg");
            if let Some(error) = self.ffmpeg_error.clone() {
                ui.colored_label(theme::DANGER, error);
                ui.label("Periksa koneksi internet, atau letakkan ffmpeg.exe dan ffprobe.exe di folder aplikasi.");
                if ui.button("Coba lagi").clicked() {
                    self.install_ffmpeg();
                }
                return;
            }
            let downloaded = self.ffmpeg_download.downloaded.load(std::sync::atomic::Ordering::Relaxed);
            let total = self.ffmpeg_download.total.load(std::sync::atomic::Ordering::Relaxed);
            if downloaded == 0 {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Memeriksa FFmpeg...");
                });
            } else {
                ui.label("FFmpeg belum terpasang, sedang diunduh otomatis (hanya sekali).");
                let megabytes = |bytes: u64| bytes as f64 / (1024.0 * 1024.0);
                let fraction = if total > 0 { downloaded as f32 / total as f32 } else { 0.0 };
                ui.add(egui::ProgressBar::new(fraction).desired_width(420.0).text(format!(
                    "{:.0} / {:.0} MB",
                    megabytes(downloaded),
                    megabytes(total)
                )));
            }
        });
    }
}

/// Sidebar brand symbol paired with the active dark or light surface palette.
fn sidebar_logo_image(dark_mode: bool) -> egui::Image<'static> {
    let (uri, bytes): (&str, &'static [u8]) = if dark_mode {
        (
            "bytes://logo-header-dark.png",
            include_bytes!("../../assets/logo/final-symbol-on-dark-64.png"),
        )
    } else {
        (
            "bytes://logo-header-light.png",
            include_bytes!("../../assets/logo/final-symbol-64.png"),
        )
    };
    egui::Image::from_bytes(uri, bytes).fit_to_exact_size(egui::vec2(28.0, 28.0))
}

/// Inter for the interface and JetBrains Mono for numbers (both SIL OFL, see `assets/fonts`), with
/// Windows system fonts as fallbacks so non-Latin titles and lyrics are drawn.
pub fn load_font_definitions(extra_fonts: &[(&str, Vec<u8>)]) -> egui::FontDefinitions {
    let bundled: [(&str, &'static [u8]); 4] = [
        (
            "inter",
            include_bytes!("../../assets/fonts/Inter-Regular.ttf"),
        ),
        (
            "inter-medium",
            include_bytes!("../../assets/fonts/Inter-Medium.ttf"),
        ),
        (
            "inter-semibold",
            include_bytes!("../../assets/fonts/Inter-SemiBold.ttf"),
        ),
        (
            "jetbrains-mono",
            include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf"),
        ),
    ];
    let mut fonts = egui::FontDefinitions::default();
    for (name, bytes) in bundled {
        fonts.font_data.insert(
            name.to_owned(),
            Arc::new(egui::FontData::from_static(bytes)),
        );
    }
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "inter".to_owned());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "jetbrains-mono".to_owned());
    fonts.families.insert(
        egui::FontFamily::Name(theme::SEMIBOLD.into()),
        vec![
            "inter-semibold".to_owned(),
            "inter".to_owned(),
            "NotoEmoji-Regular".to_owned(),
            "emoji-icon-font".to_owned(),
        ],
    );
    fonts.families.insert(
        egui::FontFamily::Name(theme::MEDIUM.into()),
        vec![
            "inter-medium".to_owned(),
            "inter".to_owned(),
            "NotoEmoji-Regular".to_owned(),
            "emoji-icon-font".to_owned(),
        ],
    );

    let windows_root =
        std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
    let font_dir = windows_root.join("Fonts");
    for name in FALLBACK_FONTS {
        let Ok(bytes) = std::fs::read(font_dir.join(name)) else {
            continue;
        };
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(egui::FontData::from_owned(bytes)));
        for stack in fonts.families.values_mut() {
            stack.push(name.to_owned());
        }
    }

    let thumbnail_system_fonts: [(&str, &str); 9] = [
        ("impact", "impact.ttf"),
        ("ariblk", "ariblk.ttf"),
        ("segoesc", "segoesc.ttf"),
        ("bahnschrift", "bahnschrift.ttf"),
        ("segoeuib", "segoeuib.ttf"),
        ("trebucbd", "trebucbd.ttf"),
        ("georgiab", "georgiab.ttf"),
        ("comicbd", "comicbd.ttf"),
        ("ariali", "ariali.ttf"),
    ];
    for (alias, filename) in thumbnail_system_fonts {
        if let Ok(bytes) = std::fs::read(font_dir.join(filename)) {
            fonts.font_data.insert(
                alias.to_owned(),
                Arc::new(egui::FontData::from_owned(bytes)),
            );
            fonts.families.insert(
                egui::FontFamily::Name(alias.into()),
                vec![
                    alias.to_owned(),
                    "inter-semibold".to_owned(),
                    "inter".to_owned(),
                ],
            );
        }
    }

    for (custom_name, custom_bytes) in extra_fonts {
        fonts.font_data.insert(
            (*custom_name).to_owned(),
            Arc::new(egui::FontData::from_owned(custom_bytes.clone())),
        );
        fonts.families.insert(
            egui::FontFamily::Name((*custom_name).into()),
            vec![
                (*custom_name).to_owned(),
                "inter-semibold".to_owned(),
                "inter".to_owned(),
            ],
        );
    }

    fonts
}

pub fn install_fonts(ctx: &egui::Context) {
    let fonts = load_font_definitions(&[]);
    ctx.set_fonts(fonts);
}

pub fn register_custom_fonts(ctx: &egui::Context, extra_fonts: &[(&str, Vec<u8>)]) {
    let fonts = load_font_definitions(extra_fonts);
    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ab_glyph::{Font, FontArc, ScaleFont};

    #[test]
    fn all_ui_symbols_have_outline_glyphs() {
        let defs = load_font_definitions(&[]);
        let prop_stack = defs
            .families
            .get(&egui::FontFamily::Proportional)
            .expect("Proportional family");
        let parsed: Vec<(String, FontArc)> = prop_stack
            .iter()
            .filter_map(|name| {
                let data = defs.font_data.get(name)?;
                let font = FontArc::try_from_vec(data.font.to_vec()).ok()?;
                Some((name.clone(), font))
            })
            .collect();

        let candidates = [
            '\u{1F5BC}', // 🖼
            '\u{1F3AC}', // 🎬
            '\u{1F4F7}', // 📷
            '\u{1F4F8}', // 📸
            '\u{1F304}', // 🌄
            '\u{1F3A8}', // 🎨
            '\u{1F399}', // 🎙
            '\u{1F39A}', // 🎚
            '\u{1F3A4}', // 🎤
            '\u{1F3B5}', // 🎵
            '\u{1F441}', // 👁
            '\u{1F4A1}', // 💡
            '\u{1F4C1}', // 📁
            '\u{1F4C2}', // 📂
            '\u{1F4CA}', // 📊
            '\u{1F4CB}', // 📋
            '\u{1F4DD}', // 📝
            '\u{1F500}', // 🔀
            '\u{1F504}', // 🔄
            '\u{1F50A}', // 🔊
            '\u{1F680}', // 🚀
            '\u{23F1}',  // ⏱
            '\u{23F3}',  // ⏳
            '\u{23F8}',  // ⏸
            '\u{25B6}',  // ▶
            '\u{2699}',  // ⚙
            '\u{26A0}',  // ⚠
            '\u{2728}',  // ✨
            '\u{2795}',  // ➕
        ];

        for ch in candidates {
            let has_outline = parsed.iter().any(|(_, font)| {
                let gid = font.glyph_id(ch);
                gid.0 != 0
                    && font
                        .as_scaled(20.0)
                        .outline_glyph(gid.with_scale(20.0))
                        .is_some()
            });
            assert!(has_outline, "Missing outline glyph for U+{:04X}", ch as u32);
        }
    }
}
