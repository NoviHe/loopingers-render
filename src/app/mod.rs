mod background;
mod batch_render_tab;
mod dashboard_tab;
mod generate_tab;
mod help;
mod liquid_ring_gpu;
mod liquid_ring_motion;
mod lyrics;
mod playlist;
mod playlist_tools_tab;
mod project_outputs;
mod quick_tab;
mod render_tab;
mod spectrum_overlay;
mod spectrum_styles;
mod storage_tab;
pub mod theme;
mod thumbnail_tab;
mod visualizer_playback;
mod visualizer_playlist;
mod visualizer_preview_layers;
mod visualizer_settings;
mod visualizer_tab;

use std::{
    future::Future,
    path::PathBuf,
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

use eframe::egui;
use loopkaraoke::{
    config::{
        AppPaths, AppSettings, KaraokeBannerSettings, KaraokeLyricStyleSettings, KaraokeSettings,
        PagePinSettings,
    },
    demo::DemoSet,
    error::Result,
    ffmpeg::{self, DownloadProgress, Tools},
    hardware::HardwareInfo,
    karaoke::{
        ass::{StyleConfig, TracklistPosition},
        styles::{Theme, THEMES},
        transcribe::Transcriber,
        LyricLine,
    },
    media::{AudioInfo, VideoInfo, IMAGE_EXTENSIONS, VIDEO_EXTENSIONS},
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
    Dashboard,
    Project,
    Quick,
    Generate,
    BatchRender,
    Playlist,
    Lyrics,
    Karaoke,
    Visualizer,
    Thumbnail,
    Storage,
}

impl Tab {
    fn uses_generate_media(self) -> bool {
        matches!(self, Self::Generate | Self::Playlist)
    }

    const ALL: [Self; 11] = [
        Self::Dashboard,
        Self::Generate,
        Self::BatchRender,
        Self::Project,
        Self::Playlist,
        Self::Thumbnail,
        Self::Lyrics,
        Self::Karaoke,
        Self::Visualizer,
        Self::Quick,
        Self::Storage,
    ];

    fn help(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard: mulai membuat video full album dan lihat ringkasan project Generate.",
            Self::Project => "Galeri Render: lihat dan buka video serta thumbnail Generate dan Batch.",
            Self::Quick => {
                "Render Old: alur kerja terpadu input video/gambar, audio, lirik, dan render cepat."
            }
            Self::Generate => {
                "Generate: gabungkan video pendekmu dengan daftar lagu mu menjadi satu file video."
            }
            Self::BatchRender => help::TAB_BATCH_RENDER,
            Self::Playlist => help::TAB_PLAYLIST,
            Self::Lyrics => help::TAB_LYRICS,
            Self::Karaoke => {
                "Pengaturan gaya teks karaoke (font, warna, posisi, dan efek wipe) untuk lagu yang memiliki lirik LRC."
            }
            Self::Visualizer => help::TAB_VISUALIZER,
            Self::Thumbnail => help::TAB_THUMBNAIL,
            Self::Storage => help::TAB_STORAGE,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Project => "Galeri Render",
            Self::Quick => "Render Old",
            Self::Generate => "Generate",
            Self::BatchRender => "Batch Render",
            Self::Playlist => "Playlist",
            Self::Lyrics => "Lirik",
            Self::Karaoke => "Karaoke",
            Self::Visualizer => "Visualizer",
            Self::Thumbnail => "Thumbnail",
            Self::Storage => "Settings",
        }
    }

    fn icon(self) -> theme::SidebarIcon {
        match self {
            Self::Dashboard => theme::SidebarIcon::Dashboard,
            Self::Project => theme::SidebarIcon::Project,
            Self::Quick => theme::SidebarIcon::RenderOld,
            Self::Generate => theme::SidebarIcon::Generate,
            Self::BatchRender => theme::SidebarIcon::BatchRender,
            Self::Playlist => theme::SidebarIcon::Playlist,
            Self::Lyrics => theme::SidebarIcon::Lyrics,
            Self::Karaoke => theme::SidebarIcon::Karaoke,
            Self::Visualizer => theme::SidebarIcon::Visualizer,
            Self::Thumbnail => theme::SidebarIcon::Thumbnail,
            Self::Storage => theme::SidebarIcon::Settings,
        }
    }
}

/// Results that background tasks hand back to the UI thread.
enum Event {
    RenderStorageFull(String),
    BatchRenderDiagnostic(LogLevel, String),
    Ffmpeg(Result<Tools>),
    Hardware(HardwareInfo),
    Background(Result<Background>),
    Audios(Vec<Result<AudioInfo>>),
    StandaloneAudios(Vec<Result<AudioInfo>>),
    StandalonePreview {
        track_id: u32,
        request_id: u64,
        result: Result<loopkaraoke::karaoke::preview::PreviewFile>,
    },
    StandaloneLyrics {
        track_id: u32,
        result: Result<Vec<LyricLine>>,
    },
    Demo(Result<DemoSet>),
    Thumbnail(Result<PathBuf>),
    ThumbnailFrame {
        source: PathBuf,
        result: Result<PathBuf>,
    },
    GenerateVideo(Result<VideoInfo>),
    GenerateAudios(Vec<Result<AudioInfo>>),
    GeneratePreview {
        track_id: u32,
        request_id: u64,
        result: Result<loopkaraoke::karaoke::preview::PreviewFile>,
    },
    VisualizerPreview {
        track_id: u32,
        request_id: u64,
        result: Result<loopkaraoke::karaoke::preview::PreviewFile>,
    },
    SpectrumAnalyzed {
        track_id: u32,
        request_id: u64,
        result: Result<loopkaraoke::spectrum::SpectrumData>,
    },
    Storage(StorageStats),
    Dependencies(loopkaraoke::dependencies::DependencyReport),
    DependencyTools {
        directory: Option<PathBuf>,
        result: Result<Tools>,
    },
    DependencyModel {
        result: Result<()>,
        message: String,
    },
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
    path: PathBuf,
    title: String,
    artist: String,
    duration: f64,
    lyrics: Vec<LyricLine>,
    has_lrc: bool,
}

impl Track {
    fn from_info(info: AudioInfo, lyrics: Vec<LyricLine>, has_lrc: bool) -> Self {
        let has_lrc = has_lrc || !lyrics.is_empty();
        Self {
            path: PathBuf::from(info.path),
            title: info.title,
            artist: info.artist,
            duration: info.duration,
            lyrics,
            has_lrc,
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
    intro_banner: loopkaraoke::karaoke::ass::IntroBanner,
    intro_font_preset: usize,
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
            intro_banner: loopkaraoke::karaoke::ass::IntroBanner {
                font: String::new(),
                size: 48,
                color: [255, 255, 255],
                x: 50.0,
                y: 15.0,
            },
            intro_font_preset: 0,
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
            intro_banner: Some(loopkaraoke::karaoke::ass::IntroBanner {
                font: lyrics::karaoke_font_ass_name(self.intro_font_preset, &None),
                ..self.intro_banner.clone()
            }),
            wipe_mode: Some(if self.smooth_wipe { "smooth" } else { "jump" }.to_owned()),
            ..Default::default()
        }
    }
}

pub struct StudioApp {
    project_outputs: project_outputs::OutputUiState,
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
    pub auto_generate_thumbnail: bool,
    pub(super) show_tracklist_visual: bool,
    pub(super) show_karaoke_lyrics: bool,
    pub(super) karaoke_preview_text: String,
    karaoke_preview_pinned: bool,
    visualizer_preview_pinned: bool,
    thumbnail_preview_pinned: bool,
    lyrics_header_pinned: bool,
    playlist_header_pinned: bool,
    karaoke_preview_lrc: Option<(PathBuf, Vec<loopkaraoke::karaoke::LyricLine>)>,
    pub(super) tracklist_position: TracklistPosition,
    pub(super) generate: generate_tab::GenerateState,
    pub(super) batch_render: batch_render_tab::BatchRenderState,
    pub(super) thumbnail: thumbnail_tab::ThumbnailState,
    spectrum_overlay: spectrum_overlay::SpectrumOverlayState,
    visualizer_preview: visualizer_preview_layers::VisualizerPreviewState,
    pub(super) visualizer_playing: bool,
    pub(super) visualizer_time: f64,
    pub(super) visualizer_selected_track: usize,
    pub(super) visualizer_volume: f32,
    pub(super) visualizer_volume_popup_open: bool,
    pub(super) visualizer_loading_preview: Option<u32>,
    pub(super) visualizer_preview_request: u64,
    pub(super) visualizer_active_track_id: Option<u32>,
    pub(super) lrc_studio: lyrics::StandaloneLyricsState,
    audio_preview_session: Option<loopkaraoke::karaoke::preview::PreviewSession>,
    storage: Option<StorageStats>,
    dependency_settings: loopkaraoke::config::DependencySettings,
    dependencies: storage_tab::DependencyUiState,
    logs: Vec<LogEntry>,
    show_logs: bool,
    log_auto_scroll: bool,
    pub log_height: f32,
    sidebar_compact: bool,
    pub(super) dark_mode: bool,
    pub(super) video_settings: VideoRenderSettings,
    storage_full_message: Option<String>,
    last_job_status: Option<JobStatus>,
    last_job_milestone: u32,
}

impl Drop for StudioApp {
    fn drop(&mut self) {
        // Release MCI's handle before either playlist's owned preview files drop.
        lyrics::win_audio::stop();
    }
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
        let whisper_model = settings.settings.dependencies.selected_whisper_model();
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

        let audio_preview_session = loopkaraoke::karaoke::preview::PreviewSession::new(&paths.temp);
        let preview_error = audio_preview_session
            .as_ref()
            .err()
            .map(ToString::to_string);
        let (sender, events) = mpsc::channel();
        let mut app = Self {
            project_outputs: project_outputs::OutputUiState::default(),
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
            tab: Tab::Dashboard,
            background: None,
            video_preview: None,
            show_bg_settings: false,
            aspect_ratio: "16:9",
            fit: FitMode::Crop,
            motion: Motion::Static,
            beat_pulse: false,
            visual_effect: 0,
            tracks: Vec::new(),
            theme: settings.karaoke.lyric_style.theme.min(THEMES.len() - 1),
            style: {
                let mut style = StyleState::from_theme(&THEMES[0].1);
                let saved = &settings.karaoke.lyric_style;
                style.normal = saved.normal;
                style.singing = saved.singing;
                style.outline = saved.outline;
                style.shadow = saved.shadow;
                style.font_size = saved.font_size.clamp(20, 120);
                style.font_preset = saved.font_preset.min(lyrics::KARAOKE_FONTS.len() - 1);
                style.custom_font = saved.custom_font.clone().filter(|path| path.is_file());
                style.alignment = saved.alignment.clamp(1, 9);
                style.margin_v = saved.margin.clamp(0, 300);
                style.pos_x = saved.x.clamp(0.0, 100.0);
                style.pos_y = saved.y.clamp(0.0, 100.0);
                style.smooth_wipe = saved.smooth_wipe;
                let banner = settings.karaoke.banner;
                style.show_intro = banner.enabled;
                style.intro_font_preset = banner.font_preset.min(lyrics::KARAOKE_FONTS.len() - 1);
                style.intro_banner.size = banner.size.clamp(20, 120);
                style.intro_banner.color = banner.color;
                style.intro_banner.x = banner.x.clamp(0.0, 100.0);
                style.intro_banner.y = banner.y.clamp(0.0, 100.0);
                style
            },
            resolution,
            fps,
            encoder: Encoder::CpuX264,
            preset: "veryfast".to_owned(),
            quality: 22,
            job: None,
            custom_filename: String::new(),
            quick_selected_track: None,
            auto_generate_thumbnail: settings.auto_generate_thumbnail,
            show_tracklist_visual: true,
            show_karaoke_lyrics: true,
            karaoke_preview_text: settings.karaoke.preview_text,
            karaoke_preview_pinned: settings.page_pins.karaoke,
            visualizer_preview_pinned: settings.page_pins.visualizer,
            thumbnail_preview_pinned: settings.page_pins.thumbnail,
            lyrics_header_pinned: settings.page_pins.lyrics,
            playlist_header_pinned: settings.page_pins.playlist,
            karaoke_preview_lrc: None,
            tracklist_position: TracklistPosition::TopLeft,
            generate: generate_tab::GenerateState {
                output_dir: settings.settings.storage.output_dir,
                ..generate_tab::GenerateState::from_settings(&settings.generate)
            },
            batch_render: batch_render_tab::BatchRenderState::default(),
            thumbnail: thumbnail_tab::ThumbnailState::default(),
            spectrum_overlay: spectrum_overlay::SpectrumOverlayState::new(
                settings.visualizer.spectrum,
            ),
            visualizer_preview: visualizer_preview_layers::VisualizerPreviewState::new(
                settings.visualizer.preview,
            ),
            visualizer_playing: false,
            visualizer_time: 0.0,
            visualizer_selected_track: 0,
            visualizer_volume: 100.0,
            visualizer_volume_popup_open: false,
            visualizer_loading_preview: None,
            visualizer_preview_request: 0,
            visualizer_active_track_id: None,
            lrc_studio: lyrics::StandaloneLyricsState::default(),
            audio_preview_session: audio_preview_session.ok(),
            storage: None,
            dependency_settings: settings.settings.dependencies,
            dependencies: storage_tab::DependencyUiState::default(),
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
            video_settings: settings.settings.video,
            storage_full_message: None,
            last_job_status: None,
            last_job_milestone: 0,
        };
        app.lrc_studio.whisper_model = whisper_model;
        if app.style.custom_font.is_some() {
            app.reload_egui_fonts();
        }
        app.sync_encoder_with_video_settings();
        if let Some(error) = preview_error {
            app.warn(format!("Folder preview audio belum siap: {error}"));
        }
        app.start_up();
        app
    }

    fn karaoke_banner_settings(&self) -> KaraokeBannerSettings {
        let banner = &self.style.intro_banner;
        KaraokeBannerSettings {
            enabled: self.style.show_intro,
            font_preset: self.style.intro_font_preset,
            size: banner.size,
            color: banner.color,
            x: banner.x,
            y: banner.y,
        }
    }

    fn page_pin_settings(&self) -> PagePinSettings {
        PagePinSettings {
            karaoke: self.karaoke_preview_pinned,
            visualizer: self.visualizer_preview_pinned,
            thumbnail: self.thumbnail_preview_pinned,
            lyrics: self.lyrics_header_pinned,
            playlist: self.playlist_header_pinned,
        }
    }

    fn karaoke_settings(&self) -> KaraokeSettings {
        KaraokeSettings {
            lyric_style: KaraokeLyricStyleSettings {
                theme: self.theme,
                normal: self.style.normal,
                singing: self.style.singing,
                outline: self.style.outline,
                shadow: self.style.shadow,
                font_size: self.style.font_size,
                font_preset: self.style.font_preset,
                custom_font: self.style.custom_font.clone(),
                alignment: self.style.alignment,
                margin: self.style.margin_v,
                x: self.style.pos_x,
                y: self.style.pos_y,
                smooth_wipe: self.style.smooth_wipe,
            },
            banner: self.karaoke_banner_settings(),
            preview_text: self.karaoke_preview_text.clone(),
        }
    }

    pub(super) fn save_settings(&mut self) {
        let mut dependencies = self.dependency_settings.clone();
        dependencies.whisper_model = Some(self.lrc_studio.whisper_model.name().to_owned());
        let settings = AppSettings {
            settings: loopkaraoke::config::SettingsPage {
                video: self.video_settings,
                storage: loopkaraoke::config::StorageSettings {
                    output_dir: self.generate.output_dir.clone(),
                },
                dependencies,
            },
            dark_mode: self.dark_mode,
            resolution: self.resolution.to_owned(),
            fps: self.fps,
            auto_generate_thumbnail: self.auto_generate_thumbnail,
            generate: self.generate.to_settings(),
            visualizer: loopkaraoke::config::VisualizerSettings {
                spectrum: self.spectrum_overlay.settings.clone(),
                preview: self.visualizer_preview.settings.clone(),
            },
            karaoke: self.karaoke_settings(),
            page_pins: self.page_pin_settings(),
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

    fn start_up(&mut self) {
        self.install_ffmpeg();
    }

    /// Finds FFmpeg, downloading it first if it is missing.
    fn install_ffmpeg(&mut self) {
        self.ffmpeg_error = None;
        self.dependencies.operation = Some("Menyiapkan FFmpeg dan FFprobe...".to_owned());
        self.dependencies.error = None;
        let tools_dir = self.paths.data_dir.join("tools");
        let progress = self.ffmpeg_download.clone();
        let custom_dir = self.dependency_settings.tools_dir.clone();
        self.spawn(async move {
            let result = if let Some(directory) = custom_dir {
                match Tools::from_directory(&directory) {
                    Ok(tools) => tools.check().await.map(|()| tools),
                    Err(error) => Err(error),
                }
            } else {
                ffmpeg::ensure_installed(tools_dir, progress).await
            };
            Event::Ffmpeg(result)
        });
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
        match event {
            Event::RenderStorageFull(detail) => {
                self.log(LogLevel::Error, &detail);
                self.storage_full_message = Some(detail);
                return;
            }
            Event::BatchRenderDiagnostic(level, detail) => {
                self.log(level, detail);
                return;
            }
            _ => {}
        }
        self.pending = self.pending.saturating_sub(1);
        match event {
            Event::RenderStorageFull(_) | Event::BatchRenderDiagnostic(..) => unreachable!(),
            Event::Ffmpeg(Ok(tools)) => {
                self.dependencies.operation = None;
                self.dependencies.report = None;
                self.tools = tools;
                self.ffmpeg_ready = true;
                self.log(
                    LogLevel::Success,
                    "FFmpeg berhasil dimuat dan siap digunakan.",
                );
                self.after_ffmpeg_ready();
            }
            Event::Ffmpeg(Err(error)) => {
                self.dependencies.operation = None;
                self.dependencies.error = Some(error.to_string());
                self.ffmpeg_error = Some(error.to_string());
                self.error(format!("FFmpeg gagal dimuat: {error}"));
            }
            Event::Dependencies(report) => {
                self.dependencies.refreshing = false;
                self.dependencies.report = Some(report);
            }
            Event::DependencyTools { directory, result } => {
                self.dependencies.operation = None;
                match result {
                    Ok(tools) => {
                        self.tools = tools;
                        self.ffmpeg_ready = true;
                        self.ffmpeg_error = None;
                        self.dependency_settings.tools_dir = directory;
                        self.save_settings();
                        self.info("Lokasi FFmpeg dan FFprobe berhasil diterapkan.");
                        let tools = self.tools.clone();
                        self.spawn(
                            async move { Event::Hardware(HardwareInfo::detect(&tools).await) },
                        );
                    }
                    Err(error) => {
                        self.dependencies.error = Some(error.to_string());
                        self.error(format!("Dependensi gagal disiapkan: {error}"));
                    }
                }
                self.refresh_dependencies();
            }
            Event::DependencyModel { result, message } => {
                self.dependencies.operation = None;
                self.dependencies.downloading_model = None;
                match result {
                    Ok(()) => self.info(message),
                    Err(error) => {
                        self.dependencies.error = Some(error.to_string());
                        self.error(format!("Pengelolaan model gagal: {error}"));
                    }
                }
                self.storage = None;
                self.refresh_dependencies();
            }
            Event::Hardware(hardware) => {
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
            Event::StandaloneAudios(items) => {
                self.lrc_studio.loading_audio = false;
                self.storage = None;
                let mut added = 0;
                let mut errors = Vec::new();
                for info in items {
                    match info {
                        Ok(audio) => {
                            self.lrc_studio.add_audio_track(audio, None);
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
            Event::StandalonePreview {
                track_id,
                request_id,
                result,
            } => {
                if !self
                    .lrc_studio
                    .preview_request_matches(track_id, request_id)
                {
                    return;
                }
                self.lrc_studio.loading_preview_id = None;
                self.storage = None;
                match result {
                    Ok(preview) => {
                        if let Some(track) = self.lrc_studio.current_track_mut() {
                            track.wav_preview = Some(preview);
                        }
                        self.lrc_studio.start_playback();
                    }
                    Err(error) => self.warn(format!("Gagal menyiapkan preview Lirik: {error}")),
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
            Event::ThumbnailFrame { source, result } => {
                self.thumbnail.extracting_frame = false;
                match result {
                    Ok(path) => {
                        self.thumbnail.preview_frame = Some(path);
                        self.thumbnail.preview_source = Some(source);
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
            Event::SpectrumAnalyzed {
                track_id,
                request_id,
                result,
            } => {
                self.accept_spectrum_analysis(track_id, request_id, result);
            }
            Event::VisualizerPreview {
                track_id,
                request_id,
                result,
            } => {
                if self.visualizer_loading_preview != Some(track_id)
                    || self.visualizer_preview_request != request_id
                    || self
                        .generate
                        .tracks
                        .get(self.visualizer_selected_track)
                        .map(|track| track.id)
                        != Some(track_id)
                {
                    return;
                }
                self.visualizer_loading_preview = None;
                self.storage = None;
                match result {
                    Ok(preview) => {
                        lyrics::win_audio::play_from_with_volume(
                            preview.path(),
                            self.visualizer_time,
                            self.visualizer_volume,
                        );
                        self.generate.previews.insert(track_id, preview);
                        self.visualizer_playing = true;
                    }
                    Err(error) => self.warn(format!("Gagal memutar preview Visualizer: {error}")),
                }
            }
            Event::GeneratePreview {
                track_id,
                request_id,
                result,
            } => {
                if !self.generate.preview_request_matches(track_id, request_id) {
                    return;
                }
                self.storage = None;
                self.generate.loading_preview_id = None;
                match result {
                    Ok(wav) => {
                        lyrics::win_audio::play_from_with_volume(
                            wav.path(),
                            0.0,
                            self.generate.preview_volume,
                        );
                        self.generate.playing_track_id = Some(track_id);
                        self.generate.previews.insert(track_id, wav);
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
        self.tracks.push(Track::from_info(info, lyrics, has_lrc));
    }

    fn total_duration(&self) -> f64 {
        self.tracks.iter().map(|track| track.duration).sum()
    }

    /// Whether the user has finished the step behind `tab`, shown as a dot on the tab.
    fn step_done(&self, tab: Tab) -> bool {
        match tab {
            Tab::Quick => self.background.is_some() && !self.tracks.is_empty(),
            Tab::Dashboard | Tab::Generate | Tab::BatchRender => false,
            Tab::Playlist => false,
            Tab::Lyrics => false,
            Tab::Karaoke => false,
            Tab::Visualizer => false,
            Tab::Thumbnail => false,
            Tab::Storage => false,
            Tab::Project => false,
        }
    }

    /// Routes dropped files to the step they belong to, by extension.
    fn handle_drop(&mut self, files: Vec<egui::DroppedFile>) {
        if !self.ffmpeg_ready {
            return;
        }
        if self.tab == Tab::Thumbnail {
            for path in files.into_iter().filter_map(|file| file.path) {
                let extension = path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                if IMAGE_EXTENSIONS.contains(&extension.as_str()) {
                    self.set_thumbnail_image(path);
                } else {
                    self.warn("Thumbnail menerima gambar JPG, PNG, WebP, atau JFIF. Pilih video melalui Generate.");
                }
            }
            return;
        }
        if self.tab == Tab::Lyrics {
            let mut audios = Vec::new();
            for path in files.into_iter().filter_map(|file| file.path) {
                if path.is_dir() {
                    collect_audios_in_folder(&path, &mut audios);
                    continue;
                }
                let extension = path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                if playlist::AUDIO_EXTENSIONS.contains(&extension.as_str()) {
                    audios.push(path);
                } else if matches!(extension.as_str(), "lrc" | "txt") {
                    self.load_standalone_lrc_path(path);
                } else {
                    self.warn(format!(
                        "Format file tidak didukung di Studio Lirik: {}",
                        path.display()
                    ));
                }
            }
            if !audios.is_empty() {
                self.load_standalone_audios(audios);
            }
            return;
        }
        let on_generate = self.tab.uses_generate_media() || self.tab == Tab::BatchRender;
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
            if is(&VIDEO_EXTENSIONS) {
                if on_generate {
                    self.load_generate_video(path);
                } else {
                    if self.tab != Tab::Quick {
                        self.tab = Tab::Quick;
                    }
                    self.load_video(path);
                }
            } else if is(&IMAGE_EXTENSIONS) {
                if on_generate {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.display().to_string());
                    self.generate.thumbnail_path = Some(path);
                    self.info(format!("Thumbnail Generate dipilih: {name}"));
                } else {
                    if self.tab != Tab::Quick {
                        self.tab = Tab::Quick;
                    }
                    self.load_image(path);
                }
            } else if is(&playlist::AUDIO_EXTENSIONS) {
                audios.push(path);
            } else if !is(&["lrc", "txt"]) {
                self.warn(format!("Format file tidak didukung: {}", path.display()));
            }
        }
        if !audios.is_empty() {
            if on_generate {
                self.load_generate_audios(audios);
            } else {
                if self.tab != Tab::Quick {
                    self.tab = Tab::Quick;
                }
                self.load_audios(audios);
            }
        }
    }

    fn job_running(&self) -> bool {
        self.project_outputs
            .clearing
            .load(std::sync::atomic::Ordering::Acquire)
            || self.batch_render.is_running()
            || self
                .generate
                .job
                .as_ref()
                .is_some_and(|job| job.snapshot().is_running())
            || self
                .job
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
            .add_filter("Video", &VIDEO_EXTENSIONS)
            .add_filter("Gambar", &IMAGE_EXTENSIONS)
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
        if VIDEO_EXTENSIONS.contains(&ext.as_str()) {
            self.load_video(path);
        } else if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
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

fn is_storage_full(detail: &str) -> bool {
    let detail = detail.to_ascii_lowercase();
    detail.contains("no space left on device")
        || detail.contains("disk full")
        || detail.contains("not enough space on the disk")
        || detail.contains("not enough space on disk")
        || (cfg!(windows)
            && (detail.contains("(os error 112)") || detail.contains("(os error 39)")))
        || (cfg!(unix) && detail.contains("(os error 28)"))
}

impl eframe::App for StudioApp {
    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        if let Some(gl) = gl {
            self.spectrum_overlay.destroy_gpu(gl);
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let bg = if self.dark_mode {
            theme::PALETTE_DARK.bg
        } else {
            theme::Palette::light().bg
        };
        bg.to_normalized_gamma_f32()
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if self.batch_render.is_running() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if self.tab == Tab::Visualizer {
            self.spectrum_overlay.ensure_gpu(frame.gl());
        }
        if self.tab != Tab::Visualizer {
            self.spectrum_overlay.cancel_analysis();
        }
        if !matches!(self.tab, Tab::Visualizer | Tab::Karaoke)
            && (self.visualizer_playing || self.visualizer_loading_preview.is_some())
        {
            self.stop_visualizer_playback(false);
        }
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
                        self.log(LogLevel::Info, format!(
                            "Waktu render: total {:.1}s; audio {:.1}s; video {:.1}s; finalisasi {:.1}s; encoder {}",
                            snapshot.timings.total_elapsed_seconds, snapshot.timings.audio_seconds,
                            snapshot.timings.video_seconds, snapshot.timings.finalizing_seconds,
                            snapshot.encoder_name,
                        ));
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
                        let detail = format!("Render gagal: {}", snapshot.error_message);
                        if is_storage_full(&detail) {
                            self.handle(Event::RenderStorageFull(detail));
                        } else {
                            self.error(detail);
                        }
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

        let saved_preferences = (self.karaoke_settings(), self.page_pin_settings());
        let content_rect = egui::CentralPanel::default()
            .frame(theme::panel_frame(ctx, 16))
            .show(ctx, |ui| {
                let panel_rect = ui.max_rect();
                if !self.ffmpeg_ready && !matches!(self.tab, Tab::Storage | Tab::Project) {
                    self.ffmpeg_setup_panel(ui);
                    return panel_rect;
                }
                let maximized = ui.input(|input| input.viewport().maximized.unwrap_or(false));
                let pinned_tab: Option<fn(&mut Self, &mut egui::Ui)> = match self.tab {
                    Tab::Karaoke
                        if self.karaoke_preview_pinned && Self::karaoke_has_wide_layout(ui) =>
                    {
                        Some(Self::karaoke_tab)
                    }
                    Tab::Visualizer if maximized && self.visualizer_preview_pinned => {
                        Some(Self::visualizer_tab)
                    }
                    Tab::Thumbnail
                        if Self::karaoke_has_wide_layout(ui) && self.thumbnail_preview_pinned =>
                    {
                        Some(Self::thumbnail_tab)
                    }
                    Tab::Lyrics if maximized && self.lyrics_header_pinned => Some(Self::lyrics_tab),
                    Tab::Playlist if maximized && self.playlist_header_pinned => {
                        Some(Self::playlist_tools_tab)
                    }
                    _ => None,
                };
                if let Some(draw) = pinned_tab {
                    theme::apply_control_style(ui);
                    draw(self, ui);
                    return panel_rect;
                }
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        theme::apply_control_style(ui);
                        match self.tab {
                            Tab::Dashboard => self.dashboard_tab(ui),
                            Tab::Project => self.project_outputs_tab(ui),
                            Tab::Quick => self.quick_tab(ui),
                            Tab::Generate => self.generate_tab(ui),
                            Tab::BatchRender => self.batch_render_tab(ui),
                            Tab::Playlist => self.playlist_tools_tab(ui),
                            Tab::Lyrics => self.lyrics_tab(ui),
                            Tab::Karaoke => self.karaoke_tab(ui),
                            Tab::Visualizer => self.visualizer_tab(ui),
                            Tab::Thumbnail => self.thumbnail_tab(ui),
                            Tab::Storage => self.storage_tab(ui),
                        }
                    });
                panel_rect
            })
            .inner;

        if saved_preferences != (self.karaoke_settings(), self.page_pin_settings()) {
            self.save_settings();
        }
        self.render_toast(ctx, content_rect);
        if let Some(detail) = &self.storage_full_message {
            let modal = egui::Modal::new(egui::Id::new("render_storage_full")).show(ctx, |ui| {
                ui.set_max_width(520.0);
                ui.heading("Penyimpanan penuh");
                ui.label(
                    "Kosongkan ruang pada drive output atau folder sementara sebelum mencoba lagi.",
                );
                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .show(ui, |ui| {
                        ui.label(detail);
                    });
                ui.button("Tutup").clicked()
            });
            if modal.inner || modal.should_close() {
                self.storage_full_message = None;
            }
        }

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
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.metadata_label(self.hardware_status_text());
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
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&entry.message)
                                        .monospace()
                                        .size(11.0)
                                        .color(theme::TEXT),
                                )
                                .wrap(),
                            );
                        });
                    }
                }
            });
        });
    }

    fn footer_actions(&mut self, ui: &mut egui::Ui) {
        if self.tab == Tab::Quick {
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
    // A font-data name must also be bound as a family before a preview can use it.
    for alias in ["inter", "inter-medium", "inter-semibold", "jetbrains-mono"] {
        let mut stack = fonts.families[&egui::FontFamily::Proportional].clone();
        stack.retain(|name| name != alias);
        stack.insert(0, alias.to_owned());
        fonts
            .families
            .insert(egui::FontFamily::Name(alias.into()), stack);
    }

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
        let mut stack = fonts.families[&egui::FontFamily::Name("inter-semibold".into())].clone();
        if let Ok(bytes) = std::fs::read(font_dir.join(filename)) {
            fonts.font_data.insert(
                alias.to_owned(),
                Arc::new(egui::FontData::from_owned(bytes)),
            );
            stack.insert(0, alias.to_owned());
        }
        fonts
            .families
            .insert(egui::FontFamily::Name(alias.into()), stack);
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
    fn thumbnail_font_presets_can_be_rendered_before_and_after_font_reload() {
        let ctx = egui::Context::default();
        let custom = [(
            "track_custom",
            include_bytes!("../../assets/fonts/Inter-Regular.ttf").to_vec(),
        )];
        for extras in [&[][..], &custom[..], &[][..]] {
            ctx.set_fonts(load_font_definitions(extras));
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    for choice in thumbnail_tab::TITLE_FONTS
                        .iter()
                        .chain(thumbnail_tab::SUBTITLE_FONTS.iter())
                    {
                        ui.label(
                            egui::RichText::new("Lorem Ipsum")
                                .family(egui::FontFamily::Name(choice.id.into())),
                        );
                    }
                    if !extras.is_empty() {
                        ui.label(
                            egui::RichText::new("Custom Font")
                                .family(egui::FontFamily::Name("track_custom".into())),
                        );
                    }
                });
            });
        }
    }

    #[test]
    fn playlist_imports_use_generate_media_without_changing_legacy_routes() {
        assert!(Tab::Playlist.uses_generate_media());
        assert!(Tab::Generate.uses_generate_media());
        for tab in [Tab::Quick, Tab::BatchRender] {
            assert!(!tab.uses_generate_media());
        }
    }

    #[test]
    fn removed_background_and_render_pages_are_absent_from_sidebar_navigation() {
        assert!(Tab::ALL
            .iter()
            .all(|tab| !matches!(tab.label(), "Background" | "Render")));
    }

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
