mod background;
mod help;
mod lyrics;
mod playlist;
mod quick_tab;
mod render_tab;
mod storage_tab;
pub mod theme;
mod thumbnail_tab;

use std::{
    future::Future,
    path::PathBuf,
    sync::{mpsc, Arc},
    time::Duration,
};

use eframe::egui;
use loopkaraoke::{
    config::AppPaths,
    demo::DemoSet,
    error::Result,
    ffmpeg::{self, DownloadProgress, Tools},
    hardware::HardwareInfo,
    karaoke::{
        ass::StyleConfig,
        styles::{Theme, THEMES},
        transcribe::{ModelSize, Transcriber},
        LyricLine,
    },
    media::AudioInfo,
    render::{
        encoder::Encoder,
        image_video::Motion,
        job::{JobHandle, JobStatus},
        looper::FitMode,
    },
    storage::{self, CleanupOptions, StorageStats},
};
use tokio::sync::Semaphore;

use background::Background;

const STARTUP_TEMP_MAX_AGE: Duration = Duration::from_secs(24 * 3600);
const FALLBACK_FONTS: [&str; 5] = [
    "segoeui.ttf",
    "YuGothR.ttc",
    "msgothic.ttc",
    "malgun.ttf",
    "msyh.ttc",
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Tab {
    Quick,
    Background,
    Playlist,
    Lyrics,
    Render,
    Thumbnail,
    Storage,
}

impl Tab {
    const ALL: [Self; 7] = [
        Self::Quick,
        Self::Background,
        Self::Playlist,
        Self::Lyrics,
        Self::Render,
        Self::Thumbnail,
        Self::Storage,
    ];

    fn help(self) -> &'static str {
        match self {
            Self::Quick => {
                "Main Menu: alur kerja terpadu input video/gambar, audio, lirik, dan render cepat."
            }
            Self::Background => help::TAB_BACKGROUND,
            Self::Playlist => help::TAB_PLAYLIST,
            Self::Lyrics => help::TAB_LYRICS,
            Self::Render => help::TAB_RENDER,
            Self::Thumbnail => help::TAB_THUMBNAIL,
            Self::Storage => help::TAB_STORAGE,
        }
    }

    /// The next step of the main workflow, if this tab is part of it.
    fn next(self) -> Option<Self> {
        match self {
            Self::Quick => None,
            Self::Background => Some(Self::Playlist),
            Self::Playlist => Some(Self::Lyrics),
            Self::Lyrics => Some(Self::Render),
            _ => None,
        }
    }

    fn previous(self) -> Option<Self> {
        match self {
            Self::Background => Some(Self::Quick),
            Self::Playlist => Some(Self::Background),
            Self::Lyrics => Some(Self::Playlist),
            Self::Render => Some(Self::Lyrics),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Quick => "Main Menu",
            Self::Background => "Background",
            Self::Playlist => "Playlist",
            Self::Lyrics => "Lirik & Gaya",
            Self::Render => "Render",
            Self::Thumbnail => "Thumbnail",
            Self::Storage => "Settings",
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
    Demo(Result<DemoSet>),
    Thumbnail(Result<PathBuf>),
    ThumbnailFrame(Result<PathBuf>),
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

struct Notice {
    text: String,
    is_error: bool,
}

struct Track {
    id: u32,
    path: PathBuf,
    title: String,
    artist: String,
    duration: f64,
    lyrics: Vec<LyricLine>,
    transcribing: bool,
}

impl Track {
    fn from_info(id: u32, info: AudioInfo, lyrics: Vec<LyricLine>) -> Self {
        Self {
            id,
            path: PathBuf::from(info.path),
            title: info.title,
            artist: info.artist,
            duration: info.duration,
            lyrics,
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
    alignment: u8,
    margin_v: i32,
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
            alignment: 2,
            margin_v: 70,
            smooth_wipe: true,
            show_intro: true,
        }
    }

    fn apply_theme(&mut self, theme: &Theme) {
        self.normal = rgb(theme.normal_color);
        self.singing = rgb(theme.singing_color);
        self.outline = rgb(theme.outline_color);
        self.shadow = rgb(theme.shadow_color);
    }

    fn to_config(&self) -> StyleConfig {
        StyleConfig {
            font_name: None,
            font_size: Some(self.font_size),
            normal_color: Some(hex(self.normal)),
            singing_color: Some(hex(self.singing)),
            outline_color: Some(hex(self.outline)),
            shadow_color: Some(hex(self.shadow)),
            alignment: Some(self.alignment),
            margin_v: Some(self.margin_v),
            show_intro: Some(self.show_intro),
            wipe_mode: Some(if self.smooth_wipe { "smooth" } else { "jump" }.to_owned()),
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
    pub auto_generate_thumbnail: bool,
    pub(super) thumbnail: thumbnail_tab::ThumbnailState,
    storage: Option<StorageStats>,
    logs: Vec<LogEntry>,
    show_logs: bool,
    log_auto_scroll: bool,
    pub log_height: f32,
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
            aspect_ratio: "16:9",
            fit: FitMode::Crop,
            motion: Motion::ZoomIn,
            beat_pulse: true,
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
            resolution: "1080p",
            fps: 30,
            encoder: Encoder::CpuX264,
            preset: "veryfast".to_owned(),
            quality: 22,
            job: None,
            custom_filename: String::new(),
            quick_selected_track: None,
            show_tab_background: false,
            show_tab_playlist: false,
            show_tab_lyrics: false,
            show_tab_render: false,
            show_tab_thumbnail: true,
            auto_generate_thumbnail: true,
            thumbnail: thumbnail_tab::ThumbnailState::default(),
            storage: None,
            logs: vec![LogEntry {
                time: chrono::Local::now().format("%H:%M:%S").to_string(),
                level: LogLevel::Info,
                message: "LoopKaraoke Studio siap digunakan.".to_owned(),
            }],
            show_logs: true,
            log_auto_scroll: true,
            log_height: 120.0,
            last_job_status: None,
            last_job_milestone: 0,
        };
        app.start_up();
        app
    }

    pub(super) fn tab_visible(&self, tab: Tab) -> bool {
        match tab {
            Tab::Quick => true,
            Tab::Storage => true,
            Tab::Background => self.show_tab_background,
            Tab::Playlist => self.show_tab_playlist,
            Tab::Lyrics => self.show_tab_lyrics,
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
        self.notice = Some(Notice {
            text: msg,
            is_error: false,
        });
    }

    pub fn error(&mut self, text: impl Into<String>) {
        let msg = text.into();
        self.log(LogLevel::Error, &msg);
        self.notice = Some(Notice {
            text: msg,
            is_error: true,
        });
    }

    pub fn warn(&mut self, text: impl Into<String>) {
        let msg = text.into();
        self.log(LogLevel::Warn, &msg);
        self.notice = Some(Notice {
            text: msg,
            is_error: false,
        });
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
                self.encoder = Encoder::best_available(&hardware);
                self.preset = self.encoder.default_preset().to_owned();
                self.use_gpu = hardware.gpu_whisper;
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
                self.hardware = Some(hardware);
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
                self.log(LogLevel::Success, &bg_msg);
                self.notice = Some(Notice {
                    text: format!("Background siap: {}", background.video.filename),
                    is_error: false,
                });
                self.background = Some(background);
            }
            Event::Background(Err(error)) => {
                self.error(format!("Background gagal dimuat: {error}"));
            }
            Event::Audios(results) => {
                let mut failed = Vec::new();
                let mut added_count = 0;
                for result in results {
                    match result {
                        Ok(info) => {
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
                            self.add_track(info, Vec::new());
                            added_count += 1;
                        }
                        Err(error) => failed.push(error.to_string()),
                    }
                }
                if failed.is_empty() {
                    self.log(
                        LogLevel::Success,
                        format!("{added_count} lagu berhasil ditambahkan ke playlist."),
                    );
                    self.notice = Some(Notice {
                        text: "Lagu ditambahkan.".to_owned(),
                        is_error: false,
                    });
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
                        self.log(LogLevel::Success, &msg);
                        self.notice = Some(Notice {
                            text: msg,
                            is_error: false,
                        });
                    }
                    Err(error) => self.error(format!("Transkripsi \"{title}\" gagal: {error}")),
                }
            }
            Event::Demo(Ok(set)) => {
                self.background = Some(Background {
                    video: set.video,
                    image: None,
                });
                self.tracks.clear();
                for track in set.tracks {
                    self.add_track(track.info, track.lyrics);
                }
                let msg = "Contoh demo dimuat: video latar dan 2 lagu dengan lirik.";
                self.log(LogLevel::Success, msg);
                self.notice = Some(Notice {
                    text: msg.to_owned(),
                    is_error: false,
                });
            }
            Event::Demo(Err(error)) => self.error(format!("Demo gagal dibuat: {error}")),
            Event::Thumbnail(result) => {
                self.thumbnail.busy = false;
                match result {
                    Ok(path) => {
                        let msg = format!("Thumbnail tersimpan: {}", path.display());
                        self.log(LogLevel::Success, &msg);
                        self.notice = Some(Notice {
                            text: msg,
                            is_error: false,
                        });
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
        let id = self.next_track_id;
        self.next_track_id += 1;
        self.tracks.push(Track::from_info(id, info, lyrics));
    }

    fn total_duration(&self) -> f64 {
        self.tracks.iter().map(|track| track.duration).sum()
    }

    /// Whether the user has finished the step behind `tab`, shown as a dot on the tab.
    fn step_done(&self, tab: Tab) -> bool {
        match tab {
            Tab::Quick => self.background.is_some() && !self.tracks.is_empty(),
            Tab::Background => self.background.is_some(),
            Tab::Playlist => !self.tracks.is_empty(),
            Tab::Lyrics => {
                !self.tracks.is_empty() && self.tracks.iter().all(|track| !track.lyrics.is_empty())
            }
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
        let mut audios = Vec::new();
        for path in files.into_iter().filter_map(|file| file.path) {
            let extension = path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(str::to_ascii_lowercase)
                .unwrap_or_default();
            let is = |list: &[&str]| list.contains(&extension.as_str());
            if is(&background::VIDEO_EXTENSIONS) {
                self.tab = Tab::Background;
                self.load_video(path);
            } else if is(&background::IMAGE_EXTENSIONS) {
                self.tab = Tab::Background;
                self.load_image(path);
            } else if is(&playlist::AUDIO_EXTENSIONS) {
                audios.push(path);
            } else if is(&["lrc", "txt"]) && !self.tracks.is_empty() {
                self.tab = Tab::Lyrics;
                match std::fs::read_to_string(&path) {
                    Ok(text) => self.apply_lrc(&text),
                    Err(error) => self.error(format!("File lirik tidak terbaca: {error}")),
                }
            } else {
                self.warn(format!("Format file tidak didukung: {}", path.display()));
            }
        }
        if !audios.is_empty() {
            self.tab = Tab::Playlist;
            self.load_audios(audios);
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
        if let Ok(entries) = std::fs::read_dir(&folder) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(str::to_ascii_lowercase)
                        .unwrap_or_default();
                    if playlist::AUDIO_EXTENSIONS.contains(&ext.as_str()) {
                        audios.push(path);
                    }
                }
            }
        }
        if audios.is_empty() {
            self.warn(format!(
                "Tidak ada file audio ditemukan di folder: {}",
                folder.display()
            ));
        } else {
            audios.sort();
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

impl eframe::App for StudioApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        theme::APP_BG.to_normalized_gamma_f32()
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
                            self.log(
                                LogLevel::Success,
                                format!("Render: Selesai 100%! Berkas video: {}", output.display()),
                            );
                        } else {
                            self.log(LogLevel::Success, "Render: Selesai 100%!");
                        }
                    }
                    JobStatus::Failed => {
                        self.log(
                            LogLevel::Error,
                            format!("Render: Gagal - {}", snapshot.error_message),
                        );
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

        egui::TopBottomPanel::top("top")
            .exact_height(theme::HEADER_HEIGHT)
            .frame(
                egui::Frame::new()
                    .fill(theme::HEADER_BG)
                    .inner_margin(egui::Margin::symmetric(14, 0)),
            )
            .show(ctx, |ui| {
                let bounds = ui.max_rect();
                ui.painter().line_segment(
                    [bounds.left_bottom(), bounds.right_bottom()],
                    egui::Stroke::new(1.0_f32, egui::Color32::BLACK),
                );
                ui.horizontal_centered(|ui| {
                    ui.label(egui::RichText::new("Loopingers Render").size(14.0).strong());
                    ui.add_space(18.0);
                    for tab in Tab::ALL {
                        if !self.tab_visible(tab) {
                            continue;
                        }
                        if theme::header_tab(
                            ui,
                            tab.label(),
                            self.tab == tab,
                            self.step_done(tab),
                            tab.help(),
                        ) {
                            self.tab = tab;
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        match &self.hardware {
                            Some(hardware) if hardware.has_gpu() => {
                                ui.label(format!("GPU: {}", hardware.gpu_name));
                            }
                            Some(_) => {
                                ui.label("GPU NVIDIA/AMD tidak ditemukan — mode CPU");
                            }
                            None => {
                                ui.label("Mendeteksi hardware...");
                            }
                        }
                    });
                });
            });

        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(theme::PANEL_BG)
                    .inner_margin(egui::Margin::symmetric(14, 8))
                    .outer_margin(egui::Margin {
                        left: theme::GAP,
                        right: theme::GAP,
                        top: 0,
                        bottom: theme::GAP,
                    }),
            )
            .show(ctx, |ui| {
                self.footer(ui);
                if self.show_logs {
                    ui.add_space(6.0);
                    self.log_console(ui);
                }
            });

        egui::CentralPanel::default()
            .frame(theme::panel_frame(16))
            .show(ctx, |ui| {
                if !self.ffmpeg_ready {
                    self.ffmpeg_setup_panel(ui);
                    return;
                }
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.tab {
                        Tab::Quick => self.quick_tab(ui),
                        Tab::Background => self.background_tab(ui),
                        Tab::Playlist => self.playlist_tab(ui),
                        Tab::Lyrics => self.lyrics_tab(ui),
                        Tab::Render => self.render_tab(ui),
                        Tab::Thumbnail => self.thumbnail_tab(ui),
                        Tab::Storage => self.storage_tab(ui),
                    });
            });

        if ctx.input(|input| !input.raw.hovered_files.is_empty()) {
            drop_overlay(ctx);
        }

        if self.job_running() || self.pending > 0 {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
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
                .small_button(toggle_label)
                .on_hover_text("Buka/tutup jendela log aktivitas")
                .clicked()
            {
                self.show_logs = !self.show_logs;
            }
            if let Some(notice) = &self.notice {
                let color = if notice.is_error {
                    theme::DANGER
                } else {
                    theme::SUCCESS
                };
                ui.colored_label(color, &notice.text);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.footer_actions(ui);
            });
        });
    }

    fn log_console(&mut self, ui: &mut egui::Ui) {
        // Interactive drag handle to resize log height freely
        let (handle_rect, handle_response) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 10.0), egui::Sense::drag());

        if handle_response.hovered() || handle_response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }

        if handle_response.dragged() {
            let delta = handle_response.drag_delta().y;
            // Dragging up (negative y) increases height; dragging down decreases height
            self.log_height = (self.log_height - delta).clamp(50.0, 500.0);
        }

        if handle_response.double_clicked() {
            self.log_height = 120.0;
        }

        // Draw resize grip visual indicator
        let grip_color = if handle_response.hovered() || handle_response.dragged() {
            theme::FOCUS
        } else {
            theme::FIELD_BORDER
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
        ui.add_space(2.0);

        ui.horizontal(|ui| {
            ui.monospace(
                egui::RichText::new("LOG AKTIVITAS")
                    .size(11.0)
                    .color(theme::TEXT_DIM)
                    .strong(),
            );
            ui.label(
                egui::RichText::new(format!(
                    "• {} entri ({:.0}px)",
                    self.logs.len(),
                    self.log_height
                ))
                .size(11.0)
                .color(theme::TEXT_DIM),
            );
            ui.checkbox(&mut self.log_auto_scroll, "Auto-scroll");

            ui.add_space(4.0);
            ui.weak("Ukuran:");
            if ui
                .small_button("S")
                .on_hover_text("Tinggi Kecil (70px)")
                .clicked()
            {
                self.log_height = 70.0;
            }
            if ui
                .small_button("M")
                .on_hover_text("Tinggi Sedang (130px)")
                .clicked()
            {
                self.log_height = 130.0;
            }
            if ui
                .small_button("L")
                .on_hover_text("Tinggi Besar (260px)")
                .clicked()
            {
                self.log_height = 260.0;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Sembunyikan").clicked() {
                    self.show_logs = false;
                }
                if ui.small_button("Bersihkan").clicked() {
                    self.logs.clear();
                }
                if ui.small_button("Salin Semua").clicked() {
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
                    self.notice = Some(Notice {
                        text: "Log berhasil disalin ke clipboard.".to_owned(),
                        is_error: false,
                    });
                }
            });
        });

        ui.add_space(2.0);

        let console_frame = egui::Frame::new()
            .fill(egui::Color32::from_rgb(12, 14, 18))
            .stroke(egui::Stroke::new(1.0_f32, theme::FIELD_BORDER))
            .corner_radius(egui::CornerRadius::same(4))
            .inner_margin(egui::Margin::symmetric(8, 6));

        console_frame.show(ui, |ui| {
            let mut scroll = egui::ScrollArea::vertical()
                .max_height(self.log_height)
                .min_scrolled_height(self.log_height)
                .auto_shrink([false, false]);

            if self.log_auto_scroll {
                scroll = scroll.stick_to_bottom(true);
            }

            scroll.show(ui, |ui| {
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
        vec!["inter-semibold".to_owned(), "inter".to_owned()],
    );
    fonts.families.insert(
        egui::FontFamily::Name(theme::MEDIUM.into()),
        vec!["inter-medium".to_owned(), "inter".to_owned()],
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
