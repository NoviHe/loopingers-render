mod background;
mod help;
mod lyrics;
mod playlist;
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
enum Tab {
    Background,
    Playlist,
    Lyrics,
    Render,
    Thumbnail,
    Storage,
}

impl Tab {
    const ALL: [Self; 6] = [
        Self::Background,
        Self::Playlist,
        Self::Lyrics,
        Self::Render,
        Self::Thumbnail,
        Self::Storage,
    ];

    fn help(self) -> &'static str {
        match self {
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
            Self::Background => Some(Self::Playlist),
            Self::Playlist => Some(Self::Lyrics),
            Self::Lyrics => Some(Self::Render),
            _ => None,
        }
    }

    fn previous(self) -> Option<Self> {
        match self {
            Self::Playlist => Some(Self::Background),
            Self::Lyrics => Some(Self::Playlist),
            Self::Render => Some(Self::Lyrics),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Background => "1. Background",
            Self::Playlist => "2. Playlist",
            Self::Lyrics => "3. Lirik & Gaya",
            Self::Render => "4. Render",
            Self::Thumbnail => "Thumbnail",
            Self::Storage => "Penyimpanan",
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
    Storage(StorageStats),
    Cleaned(String),
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

    thumbnail: thumbnail_tab::ThumbnailState,
    storage: Option<StorageStats>,
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
            tab: Tab::Background,
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
            thumbnail: thumbnail_tab::ThumbnailState::default(),
            storage: None,
        };
        app.start_up();
        app
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

    fn info(&mut self, text: impl Into<String>) {
        self.notice = Some(Notice {
            text: text.into(),
            is_error: false,
        });
    }

    fn error(&mut self, text: impl Into<String>) {
        self.notice = Some(Notice {
            text: text.into(),
            is_error: true,
        });
    }

    fn handle(&mut self, event: Event) {
        self.pending = self.pending.saturating_sub(1);
        match event {
            Event::Ffmpeg(Ok(tools)) => {
                self.tools = tools;
                self.ffmpeg_ready = true;
                self.after_ffmpeg_ready();
            }
            Event::Ffmpeg(Err(error)) => self.ffmpeg_error = Some(error.to_string()),
            Event::Hardware(hardware) => {
                self.encoder = Encoder::best_available(&hardware);
                self.preset = self.encoder.default_preset().to_owned();
                self.use_gpu = hardware.gpu_whisper;
                self.hardware = Some(hardware);
            }
            Event::Background(Ok(background)) => {
                if let Some(aspect) = background::known_aspect(&background.video.aspect_ratio) {
                    self.aspect_ratio = aspect;
                }
                self.info(format!("Background siap: {}", background.video.filename));
                self.background = Some(background);
            }
            Event::Background(Err(error)) => {
                self.error(format!("Background gagal dimuat: {error}"))
            }
            Event::Audios(results) => {
                let mut failed = Vec::new();
                for result in results {
                    match result {
                        Ok(info) => self.add_track(info, Vec::new()),
                        Err(error) => failed.push(error.to_string()),
                    }
                }
                if failed.is_empty() {
                    self.info("Lagu ditambahkan.");
                } else {
                    self.error(format!("Sebagian lagu gagal dimuat: {}", failed.join("; ")));
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
                        self.info(format!(
                            "Berhasil mendeteksi {count} baris lirik untuk \"{title}\""
                        ));
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
                self.info("Contoh demo dimuat: video latar dan 2 lagu dengan lirik.");
            }
            Event::Demo(Err(error)) => self.error(format!("Demo gagal dibuat: {error}")),
            Event::Thumbnail(result) => {
                self.thumbnail.busy = false;
                match result {
                    Ok(path) => {
                        self.info(format!("Thumbnail tersimpan: {}", path.display()));
                        self.thumbnail.result = Some(path);
                    }
                    Err(error) => self.error(format!("Thumbnail gagal: {error}")),
                }
            }
            Event::Storage(stats) => self.storage = Some(stats),
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
                self.error(format!("Format file tidak didukung: {}", path.display()));
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
                    egui::Stroke::new(1.0, egui::Color32::BLACK),
                );
                ui.horizontal_centered(|ui| {
                    ui.label(
                        egui::RichText::new("LoopKaraoke Studio")
                            .size(14.0)
                            .strong(),
                    );
                    ui.add_space(18.0);
                    for tab in Tab::ALL {
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
            .show(ctx, |ui| self.footer(ui));

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
        egui::Stroke::new(2.0, theme::ACCENT),
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

    fn footer_actions(&mut self, ui: &mut egui::Ui) {
        if self.tab == Tab::Render {
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
pub fn install_fonts(ctx: &egui::Context) {
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
    ctx.set_fonts(fonts);
}
