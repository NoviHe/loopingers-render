use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use loopkaraoke::{
    config::AppPaths,
    demo,
    error::Result,
    ffmpeg::Tools,
    hardware::HardwareInfo,
    media::{format_clock, probe_video, VideoInfo},
    render::{
        effects::{AppliedEffect, EFFECTS},
        image_video::{ImageVideo, Motion, DEFAULT_BPM},
        looper::FitMode,
        presets::ASPECT_RATIOS,
    },
    util::random_id,
};

use super::{help::background as help, theme, Event, StudioApp};

pub(super) const VIDEO_EXTENSIONS: [&str; 5] = ["mp4", "mov", "mkv", "webm", "avi"];
pub(super) const IMAGE_EXTENSIONS: [&str; 5] = ["jpg", "jpeg", "png", "webp", "jfif"];
const IMAGE_CLIP_SECONDS: f64 = 10.0;
const IMAGE_CLIP_FPS: u32 = 30;

/// The chosen background: a video, or a clip generated from a still image.
pub struct Background {
    pub video: VideoInfo,
    pub image: Option<PathBuf>,
}

/// Real-time background preview player:
/// - If the imported file is a video (`bg.image.is_none()`), streams live frames from FFmpeg in a loop.
/// - If the imported file is an image (`bg.image.is_some()`), decodes the still image once into a GPU texture.
pub(super) struct VideoPreviewPlayer {
    pub stop_flag: Arc<AtomicBool>,
    pub paused: Arc<AtomicBool>,
    pub seek_request: Arc<Mutex<Option<f64>>>,
    pub latest_frame: Arc<Mutex<Option<(egui::ColorImage, f64)>>>,
    pub child_proc: Arc<Mutex<Option<Child>>>,
    pub texture: Option<egui::TextureHandle>,
    pub current_pts: f64,
    pub scrub_pts: Option<f64>,
    pub last_seek_sent: Instant,
    pub duration: f64,
}

impl Drop for VideoPreviewPlayer {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Ok(mut guard) = self.child_proc.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

impl VideoPreviewPlayer {
    pub fn seek(&mut self, target_pts: f64) {
        let clamped = if self.duration > 0.0 {
            target_pts.clamp(0.0, (self.duration - 0.05).max(0.0))
        } else {
            target_pts.max(0.0)
        };
        self.current_pts = clamped;
        self.last_seek_sent = Instant::now();
        if let Ok(mut guard) = self.seek_request.lock() {
            *guard = Some(clamped);
        }
        if let Ok(mut guard) = self.child_proc.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    pub fn start(ctx: egui::Context, ffmpeg_path: PathBuf, bg: &Background) -> Self {
        let is_video = bg.image.is_none();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let paused = Arc::new(AtomicBool::new(false));
        let seek_request = Arc::new(Mutex::new(None));
        let latest_frame = Arc::new(Mutex::new(None));
        let child_proc = Arc::new(Mutex::new(None));

        let src_w = bg.video.width.max(2) as f64;
        let src_h = bg.video.height.max(2) as f64;
        let max_dim = if is_video { 640.0 } else { 960.0 };
        let scale = (max_dim / src_w).min(max_dim / src_h).min(1.0);
        let pw = (((src_w * scale).round() as u32).max(2)) & !1;
        let ph = (((src_h * scale).round() as u32).max(2)) & !1;

        let fps = if bg.video.fps > 1.0 {
            bg.video.fps.clamp(12.0, 30.0)
        } else {
            24.0
        };
        let duration = bg.video.duration;
        let input_path = bg.image.as_ref().map_or_else(
            || bg.video.path.clone(),
            |p| p.to_string_lossy().into_owned(),
        );

        let stop_clone = stop_flag.clone();
        let paused_clone = paused.clone();
        let seek_clone = seek_request.clone();
        let frame_clone = latest_frame.clone();
        let child_clone = child_proc.clone();

        std::thread::spawn(move || {
            let frame_bytes = (pw as usize) * (ph as usize) * 4;
            let mut buf = vec![0u8; frame_bytes];

            if !is_video {
                // Decode single static image frame via FFmpeg
                let mut cmd = Command::new(&ffmpeg_path);
                cmd.args([
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-i",
                    &input_path,
                    "-frames:v",
                    "1",
                    "-vf",
                    &format!("scale={pw}:{ph}:flags=lanczos"),
                    "-an",
                    "-pix_fmt",
                    "rgba",
                    "-f",
                    "rawvideo",
                    "pipe:1",
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null());

                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    cmd.creation_flags(0x0800_0000);
                }

                if let Ok(mut child) = cmd.spawn() {
                    if let Some(mut stdout) = child.stdout.take() {
                        if stdout.read_exact(&mut buf).is_ok() {
                            let img = egui::ColorImage::from_rgba_unmultiplied(
                                [pw as usize, ph as usize],
                                &buf,
                            );
                            if let Ok(mut guard) = frame_clone.lock() {
                                *guard = Some((img, 0.0));
                            }
                            ctx.request_repaint();
                        }
                    }
                    let _ = child.kill();
                    let _ = child.wait();
                }
                return;
            }

            // Stream video frames continuously in a loop, supporting real-time seeking
            let frame_interval = Duration::from_secs_f64(1.0 / fps);
            let mut start_offset: f64 = 0.0;
            let mut force_single_frame = false;

            while !stop_clone.load(Ordering::Relaxed) {
                if let Ok(mut guard) = seek_clone.lock() {
                    if let Some(req) = guard.take() {
                        start_offset = req;
                        force_single_frame = true;
                    }
                }

                let mut cmd = Command::new(&ffmpeg_path);
                cmd.args(["-hide_banner", "-loglevel", "error"]);
                if start_offset > 0.001 {
                    cmd.args(["-ss", &format!("{start_offset:.3}")]);
                } else {
                    cmd.args(["-stream_loop", "-1"]);
                }
                cmd.args([
                    "-i",
                    &input_path,
                    "-vf",
                    &format!("fps={fps:.2},scale={pw}:{ph}:flags=fast_bilinear"),
                    "-an",
                    "-pix_fmt",
                    "rgba",
                    "-f",
                    "rawvideo",
                    "pipe:1",
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null());

                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    cmd.creation_flags(0x0800_0000);
                }

                let Ok(mut child) = cmd.spawn() else {
                    break;
                };
                let Some(mut stdout) = child.stdout.take() else {
                    let _ = child.kill();
                    let _ = child.wait();
                    break;
                };
                if let Ok(mut guard) = child_clone.lock() {
                    *guard = Some(child);
                }

                let mut frame_idx: u64 = 0;
                let mut decoded_any = false;
                let mut broke_for_seek = false;

                while !stop_clone.load(Ordering::Relaxed) {
                    if seek_clone.lock().is_ok_and(|g| g.is_some()) {
                        broke_for_seek = true;
                        break;
                    }
                    if paused_clone.load(Ordering::Relaxed) && !force_single_frame {
                        std::thread::sleep(Duration::from_millis(20));
                        continue;
                    }
                    let tick = Instant::now();
                    if stdout.read_exact(&mut buf).is_err() {
                        break;
                    }
                    if stop_clone.load(Ordering::Relaxed) {
                        break;
                    }
                    decoded_any = true;
                    force_single_frame = false;

                    let img =
                        egui::ColorImage::from_rgba_unmultiplied([pw as usize, ph as usize], &buf);
                    let raw_pts = start_offset + (frame_idx as f64 / fps);
                    let pts = if duration > 0.0 {
                        raw_pts % duration
                    } else {
                        raw_pts
                    };
                    frame_idx = frame_idx.wrapping_add(1);

                    if seek_clone.lock().is_ok_and(|g| g.is_none()) {
                        if let Ok(mut guard) = frame_clone.lock() {
                            *guard = Some((img, pts));
                        }
                        ctx.request_repaint();
                    } else {
                        broke_for_seek = true;
                        break;
                    }

                    if start_offset > 0.001 && duration > 0.0 && raw_pts >= duration {
                        start_offset = 0.0;
                        break;
                    }

                    let elapsed = tick.elapsed();
                    if let Some(wait) = frame_interval.checked_sub(elapsed) {
                        std::thread::sleep(wait);
                    }
                }

                if let Ok(mut guard) = child_clone.lock() {
                    if let Some(mut c) = guard.take() {
                        let _ = c.kill();
                        let _ = c.wait();
                    }
                }

                if broke_for_seek || seek_clone.lock().is_ok_and(|g| g.is_some()) {
                    continue;
                }

                start_offset = 0.0;
                if !decoded_any {
                    break;
                }
            }
        });

        Self {
            stop_flag,
            paused,
            seek_request,
            latest_frame,
            child_proc,
            texture: None,
            current_pts: 0.0,
            scrub_pts: None,
            last_seek_sent: Instant::now(),
            duration,
        }
    }
}

pub fn known_aspect(label: &str) -> Option<&'static str> {
    ASPECT_RATIOS
        .iter()
        .copied()
        .find(|aspect| *aspect == label)
}

struct ImageJob {
    image: PathBuf,
    motion: Motion,
    beat_pulse: bool,
    aspect_ratio: String,
    resolution: String,
    effect_id: String,
}

async fn build_image_background(
    tools: Tools,
    paths: AppPaths,
    hardware: Option<HardwareInfo>,
    job: ImageJob,
) -> Result<Background> {
    let hardware = match hardware {
        Some(hardware) => hardware,
        None => HardwareInfo::detect(&tools).await,
    };
    let output = paths
        .temp
        .join(format!("video_from_img_{}.mp4", random_id(8)));
    let effect = AppliedEffect::resolve(&job.effect_id, &paths.effects);
    let video = ImageVideo {
        image: &job.image,
        output: &output,
        duration: IMAGE_CLIP_SECONDS,
        fps: IMAGE_CLIP_FPS,
        aspect_ratio: &job.aspect_ratio,
        resolution: &job.resolution,
        motion: job.motion,
        beat_pulse: job.beat_pulse,
        bpm: DEFAULT_BPM,
        effect: &effect,
    }
    .render(&tools, &hardware)
    .await
    .map_err(|error| error.context("Gagal mengonversi gambar ke video background"))?;
    Ok(Background {
        video,
        image: Some(job.image),
    })
}

impl StudioApp {
    pub(super) fn background_tab(&mut self, ui: &mut egui::Ui) {
        theme::heading_with_info(ui, "Video atau Gambar Background", help::HEADING);
        ui.label("Video diulang otomatis sepanjang total durasi lagu. Gambar diubah menjadi video bergerak.");
        ui.add_space(4.0);
        ui.columns(2, |columns| {
            self.background_source(&mut columns[0]);
            self.draw_live_background_preview(&mut columns[1]);
        });
    }

    fn background_source(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui.add(theme::primary_button("Pilih Video...")).clicked() {
                let picked = rfd::FileDialog::new()
                    .add_filter("Video", &VIDEO_EXTENSIONS)
                    .pick_file();
                if let Some(path) = picked {
                    self.load_video(path);
                }
            }
            theme::info(ui, help::PICK_VIDEO);
            if ui.button("Pilih Gambar...").clicked() {
                let picked = rfd::FileDialog::new()
                    .add_filter("Gambar", &IMAGE_EXTENSIONS)
                    .pick_file();
                if let Some(path) = picked {
                    self.load_image(path);
                }
            }
            theme::info(ui, help::PICK_IMAGE);
        });
        ui.add_space(6.0);

        if self.background.is_none() {
            let frame = egui::Frame::new()
                .fill(theme::FIELD_BG)
                .stroke(egui::Stroke::new(1.0_f32, theme::FIELD_BORDER))
                .corner_radius(4)
                .inner_margin(egui::Margin::same(12));
            frame.show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.vertical_centered(|ui| {
                    ui.add_space(14.0);
                    ui.label("Seret video atau gambar ke jendela ini");
                    ui.small("atau pakai tombol di atas");
                    ui.add_space(14.0);
                });
            });
            return;
        }

        self.background_effects_card(ui, "bg_tab");
    }

    /// Kelompok pengaturan Background Video/Gambar (termasuk Efek Visual, Rasio, Mode Tampilan, & Gerakan)
    /// yang muncul ketika berkas background sudah dipilih.
    pub(super) fn background_effects_card(&mut self, ui: &mut egui::Ui, id_prefix: &str) {
        let Some(bg) = &self.background else {
            return;
        };
        let is_image = bg.image.is_some();
        let image_path = bg.image.clone();
        let filename = bg.image.as_deref().and_then(Path::file_name).map_or_else(
            || bg.video.filename.clone(),
            |name| name.to_string_lossy().into_owned(),
        );
        let specs = format!(
            "{}x{} • {:.1}s • {:.0} FPS • Rasio {}",
            bg.video.width, bg.video.height, bg.video.duration, bg.video.fps, bg.video.aspect_ratio
        );

        let card = egui::Frame::new()
            .fill(theme::ROW_ALT)
            .stroke(egui::Stroke::new(1.0_f32, theme::FIELD_BORDER))
            .corner_radius(4)
            .inner_margin(egui::Margin::same(8));

        card.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                if is_image {
                    ui.colored_label(theme::SUCCESS, "🖼 Gambar:");
                } else {
                    ui.colored_label(theme::SUCCESS, "🎬 Video:");
                }
                ui.strong(&filename);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let toggle_label = if self.show_bg_settings {
                        "⚙ Sembunyikan Pengaturan ▲"
                    } else {
                        "⚙ Pengaturan & Efek ▼"
                    };
                    if ui
                        .small_button(toggle_label)
                        .on_hover_text(
                            "Tampilkan atau sembunyikan pengaturan efek visual, rasio layar, dan mode tampilan",
                        )
                        .clicked()
                    {
                        self.show_bg_settings = !self.show_bg_settings;
                    }
                });
            });
            ui.small(specs);

            if !self.show_bg_settings {
                let summary = if is_image {
                    format!(
                        "Rasio: {} • Tampilan: {} • Efek: {} • Gerakan: {}",
                        self.aspect_ratio,
                        self.fit.label(),
                        EFFECTS[self.visual_effect].name,
                        self.motion.label()
                    )
                } else {
                    format!(
                        "Rasio: {} • Tampilan: {} • Efek: {}",
                        self.aspect_ratio,
                        self.fit.label(),
                        EFFECTS[self.visual_effect].name
                    )
                };
                ui.small(egui::RichText::new(summary).color(theme::TEXT_DIM));
                return;
            }

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // Efek Visual di dalam kelompok Background Video / Gambar
            ui.horizontal(|ui| {
                theme::label_with_info(ui, "✨ Efek Visual:", help::EFFECT);
                egui::ComboBox::from_id_salt(format!("{id_prefix}_visual_effect"))
                    .selected_text(EFFECTS[self.visual_effect].name)
                    .show_ui(ui, |ui| {
                        for (index, effect) in EFFECTS.iter().enumerate() {
                            ui.selectable_value(&mut self.visual_effect, index, effect.name)
                                .on_hover_text(effect.desc);
                        }
                    });
            });
            ui.small(EFFECTS[self.visual_effect].desc);
            ui.add_space(4.0);

            // Rasio Layar & Mode Tampilan
            ui.horizontal_wrapped(|ui| {
                theme::label_with_info(ui, "Rasio Layar:", help::ASPECT);
                for aspect in ASPECT_RATIOS {
                    ui.radio_value(&mut self.aspect_ratio, aspect, aspect);
                }
            });
            ui.horizontal_wrapped(|ui| {
                theme::label_with_info(ui, "Mode Tampilan:", help::FIT);
                for fit in FitMode::ALL {
                    ui.radio_value(&mut self.fit, fit, fit.label());
                }
            });

            // Gerakan Gambar (khusus jika berkas adalah gambar)
            if let Some(img_path) = image_path {
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);
                ui.horizontal_wrapped(|ui| {
                    theme::label_with_info(ui, "Gerakan Gambar:", help::MOTION);
                    for motion in Motion::ALL {
                        ui.radio_value(&mut self.motion, motion, motion.label());
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(&mut self.beat_pulse, "Denyut mengikuti ketukan musik");
                    theme::info(ui, help::BEAT_PULSE);
                    if ui
                        .small_button("🔄 Render Ulang Klip Gambar")
                        .on_hover_text(help::APPLY_IMAGE)
                        .clicked()
                    {
                        self.load_image(img_path);
                    }
                });
            }
        });
    }

    pub(super) fn start_background_preview(&mut self) {
        self.video_preview = None;
        let Some(bg) = &self.background else {
            return;
        };
        let ffmpeg_path = PathBuf::from(self.tools.ffmpeg().as_std().get_program());
        let ctx = self.ctx.clone();
        self.video_preview = Some(VideoPreviewPlayer::start(ctx, ffmpeg_path, bg));
    }

    pub(super) fn poll_background_texture(
        &mut self,
        ctx: &egui::Context,
    ) -> Option<(egui::TextureId, f64, f64, bool)> {
        if self.video_preview.is_none() && self.background.is_some() {
            self.start_background_preview();
        }
        let player = self.video_preview.as_mut()?;
        if let Ok(mut guard) = player.latest_frame.lock() {
            if let Some((color_image, pts)) = guard.take() {
                if player.scrub_pts.is_none()
                    && player.seek_request.lock().is_ok_and(|g| g.is_none())
                {
                    player.current_pts = pts;
                }
                if let Some(tex) = &mut player.texture {
                    tex.set(color_image, egui::TextureOptions::LINEAR);
                } else {
                    player.texture = Some(ctx.load_texture(
                        "live_background_preview_tex",
                        color_image,
                        egui::TextureOptions::LINEAR,
                    ));
                }
            }
        }
        let is_paused = player.paused.load(Ordering::Relaxed);
        let display_pts = player.scrub_pts.unwrap_or(player.current_pts);
        player
            .texture
            .as_ref()
            .map(|t| (t.id(), display_pts, player.duration, is_paused))
    }

    /// Stage Pratinjau Background Live:
    /// - Jika berkas yang diimpor adalah Video: memutar video asli secara langsung.
    /// - Jika berkas yang diimpor adalah Gambar: menampilkan gambar secara langsung.
    pub(super) fn draw_live_background_preview(&mut self, ui: &mut egui::Ui) {
        let stage_frame = egui::Frame::new()
            .fill(theme::ROW_ALT)
            .stroke(egui::Stroke::new(1.0_f32, theme::FIELD_BORDER))
            .corner_radius(6)
            .inner_margin(egui::Margin::same(10));

        let is_image = self
            .background
            .as_ref()
            .is_some_and(|bg| bg.image.is_some());
        let preview_info = self.poll_background_texture(ui.ctx());

        stage_frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                if self.background.is_none() {
                    ui.strong("🎬 Stage Pratinjau Background");
                } else if is_image {
                    ui.strong("🖼 Pratinjau Gambar Background");
                } else {
                    ui.strong("🎬 Pratinjau Video Background");
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.background.is_some() {
                        let kind = if is_image { "GAMBAR" } else { "VIDEO" };
                        ui.colored_label(
                            theme::SUCCESS,
                            format!(
                                "• {kind} ({}) • {}",
                                self.aspect_ratio, EFFECTS[self.visual_effect].name
                            ),
                        );
                    } else {
                        ui.weak("Menunggu berkas...");
                    }
                });
            });
            ui.add_space(4.0);

            let avail_w = ui.available_width().max(260.0);
            let outer_w = avail_w;
            let outer_h = outer_w * (9.0 / 16.0);
            let sense = if self.background.is_some() && !is_image {
                egui::Sense::click()
            } else {
                egui::Sense::hover()
            };
            let (outer_rect, resp) = ui.allocate_exact_size(Vec2::new(outer_w, outer_h), sense);

            if !is_image && resp.clicked() {
                if let Some(player) = &self.video_preview {
                    let cur = player.paused.load(Ordering::Relaxed);
                    player.paused.store(!cur, Ordering::Relaxed);
                }
            }
            if !is_image && self.background.is_some() {
                resp.on_hover_text("Klik untuk Putar / Jeda video pratinjau");
            }

            let outer_painter = ui.painter_at(outer_rect);
            outer_painter.rect_filled(outer_rect, 4.0, Color32::from_rgb(8, 10, 16));

            let Some(bg_ref) = &self.background else {
                outer_painter.text(
                    Pos2::new(outer_rect.center().x, outer_rect.center().y - 10.0),
                    egui::Align2::CENTER_CENTER,
                    "🎬 / 🖼 Belum Ada Background Dipilih",
                    FontId::proportional(15.0),
                    Color32::from_white_alpha(180),
                );
                outer_painter.text(
                    Pos2::new(outer_rect.center().x, outer_rect.center().y + 14.0),
                    egui::Align2::CENTER_CENTER,
                    "Pilih berkas video atau gambar di sisi kiri untuk melihat pratinjau",
                    FontId::proportional(12.0),
                    Color32::from_white_alpha(110),
                );
                return;
            };

            let src_w = bg_ref.video.width.max(1) as f32;
            let src_h = bg_ref.video.height.max(1) as f32;
            let src_aspect = src_w / src_h;

            // Only request continuous repaint on UI timer when animation is needed
            let has_active_effect = self.visual_effect != 0;
            let has_image_motion = is_image && (self.motion != Motion::Static || self.beat_pulse);
            if has_active_effect || has_image_motion {
                ui.ctx().request_repaint();
            }
            let t = ui.input(|i| i.time) as f32;

            // Compute target viewport rect based on chosen aspect_ratio
            let target_aspect = match self.aspect_ratio {
                "9:16" => 9.0 / 16.0,
                "1:1" => 1.0,
                "4:3" => 4.0 / 3.0,
                _ => 16.0 / 9.0,
            };
            let (vp_w, vp_h) = if outer_w / outer_h > target_aspect {
                (outer_h * target_aspect, outer_h)
            } else {
                (outer_w, outer_w / target_aspect)
            };
            let viewport = Rect::from_center_size(outer_rect.center(), Vec2::new(vp_w, vp_h));
            let vp_aspect = vp_w / vp_h.max(1.0);

            // Motion & Beat Pulse only apply to images when explicitly enabled by the user.
            // Videos play naturally without any breathing/zooming effect.
            let (zoom, pan_x, pan_y): (f32, f32, f32) = if is_image {
                let cycle = (t * 0.25) % 1.0;
                let pulse_zoom = if self.beat_pulse {
                    (t * 5.5).sin().abs() * 0.025
                } else {
                    0.0
                };
                match self.motion {
                    Motion::ZoomIn => (1.0 + cycle * 0.16 + pulse_zoom, 0.0, 0.0),
                    Motion::ZoomOut => (1.16 - cycle * 0.16 + pulse_zoom, 0.0, 0.0),
                    Motion::PanLeft => (1.12 + pulse_zoom, (0.5 - cycle) * 0.08, 0.0),
                    Motion::PanRight => (1.12 + pulse_zoom, (cycle - 0.5) * 0.08, 0.0),
                    Motion::Static => (1.0 + pulse_zoom, 0.0, 0.0),
                }
            } else {
                (1.0, 0.0, 0.0)
            };

            let inv_zoom = (1.0 / zoom).clamp(0.5, 1.0);
            let uv_cx = (0.5 + pan_x).clamp(inv_zoom * 0.5, 1.0 - inv_zoom * 0.5);
            let uv_cy = (0.5 + pan_y).clamp(inv_zoom * 0.5, 1.0 - inv_zoom * 0.5);
            let uncropped_uv =
                Rect::from_center_size(Pos2::new(uv_cx, uv_cy), Vec2::new(inv_zoom, inv_zoom));

            let vp_painter = ui.painter_at(viewport);

            // Render Background Video or Image according to FitMode (Crop, Fit/Letterbox, BlurPad)
            if let Some((tex_id, _, _, _)) = preview_info {
                let (base_uv_w, base_uv_h) = if vp_aspect > src_aspect {
                    (1.0, (src_aspect / vp_aspect).clamp(0.05, 1.0))
                } else {
                    ((vp_aspect / src_aspect).clamp(0.05, 1.0), 1.0)
                };
                let crop_w = base_uv_w * inv_zoom;
                let crop_h = base_uv_h * inv_zoom;
                let crop_cx = (0.5 + pan_x).clamp(crop_w * 0.5, 1.0 - crop_w * 0.5);
                let crop_cy = (0.5 + pan_y).clamp(crop_h * 0.5, 1.0 - crop_h * 0.5);
                let crop_uv =
                    Rect::from_center_size(Pos2::new(crop_cx, crop_cy), Vec2::new(crop_w, crop_h));

                match self.fit {
                    FitMode::Crop => {
                        vp_painter.image(tex_id, viewport, crop_uv, Color32::WHITE);
                    }
                    FitMode::Fit => {
                        vp_painter.rect_filled(viewport, 3.0, Color32::BLACK);
                        let (fg_w, fg_h) = if vp_aspect > src_aspect {
                            (vp_h * src_aspect, vp_h)
                        } else {
                            (vp_w, vp_w / src_aspect)
                        };
                        let fg_rect =
                            Rect::from_center_size(viewport.center(), Vec2::new(fg_w, fg_h));
                        vp_painter.image(tex_id, fg_rect, uncropped_uv, Color32::WHITE);
                    }
                    FitMode::BlurPad => {
                        let blur_uv = Rect::from_center_size(
                            Pos2::new(0.5, 0.5),
                            Vec2::new(base_uv_w * 0.65, base_uv_h * 0.65),
                        );
                        vp_painter.image(
                            tex_id,
                            viewport,
                            blur_uv,
                            Color32::from_rgb(120, 120, 135),
                        );
                        vp_painter.rect_filled(viewport, 3.0, Color32::from_black_alpha(65));
                        let (fg_w, fg_h) = if vp_aspect > src_aspect {
                            (vp_h * src_aspect, vp_h)
                        } else {
                            (vp_w, vp_w / src_aspect)
                        };
                        let fg_rect =
                            Rect::from_center_size(viewport.center(), Vec2::new(fg_w, fg_h));
                        vp_painter.image(tex_id, fg_rect, uncropped_uv, Color32::WHITE);
                    }
                }
            } else {
                vp_painter.rect_filled(viewport, 3.0, Color32::from_rgb(18, 24, 36));
                vp_painter.text(
                    viewport.center(),
                    egui::Align2::CENTER_CENTER,
                    if is_image {
                        "⏳ Memuat gambar pratinjau..."
                    } else {
                        "⏳ Memuat pemutar video pratinjau..."
                    },
                    FontId::proportional(13.0),
                    Color32::from_white_alpha(180),
                );
            }

            // Render Live Visual Effect (EFFECTS[self.visual_effect])
            self.draw_visual_effect_preview(&vp_painter, viewport, t);

            // Render Tracklist Overlay matching Thumbnail style if checked
            if self.show_tracklist_visual && !self.tracks.is_empty() {
                self.draw_thumbnail_overlay(&vp_painter, viewport, false);
            }

            // Border outline around target aspect ratio viewport
            vp_painter.rect_stroke(
                viewport,
                3.0,
                Stroke::new(1.2_f32, theme::FOCUS),
                egui::StrokeKind::Inside,
            );

            // Video Player Transport & Duration Slider (when background is a video)
            if !is_image {
                if let Some(player) = &mut self.video_preview {
                    let dur = player.duration.max(0.1);
                    let is_paused = player.paused.load(Ordering::Relaxed);
                    let mut pos = player
                        .scrub_pts
                        .unwrap_or(player.current_pts)
                        .clamp(0.0, dur);

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        let play_icon = if is_paused { "▶" } else { "⏸" };
                        if ui
                            .button(play_icon)
                            .on_hover_text(if is_paused {
                                "Putar video"
                            } else {
                                "Jeda video"
                            })
                            .clicked()
                        {
                            player.paused.store(!is_paused, Ordering::Relaxed);
                        }

                        ui.monospace(format_clock(pos as u64));

                        let avail_slider_w = (ui.available_width() - 55.0).max(80.0);
                        ui.spacing_mut().slider_width = avail_slider_w;
                        let slider_resp = ui
                            .add(
                                egui::Slider::new(&mut pos, 0.0..=dur)
                                    .show_value(false)
                                    .trailing_fill(true),
                            )
                            .on_hover_text(format!(
                                "Geser untuk pindah durasi video ({:.1}s / {:.1}s)",
                                pos, dur
                            ));

                        ui.monospace(format_clock(dur as u64));

                        if slider_resp.dragged() {
                            player.scrub_pts = Some(pos);
                            player.current_pts = pos;
                            self.thumbnail.timestamp = pos;
                            if player.last_seek_sent.elapsed() >= Duration::from_millis(120) {
                                player.seek(pos);
                            }
                        } else if slider_resp.drag_stopped() || slider_resp.changed() {
                            player.scrub_pts = None;
                            self.thumbnail.timestamp = pos;
                            player.seek(pos);
                        }
                    });
                }
            }

            // Footer info
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.small(format!(
                    "Rasio: {} • Tampilan: {} • Efek: {}",
                    self.aspect_ratio,
                    self.fit.label(),
                    EFFECTS[self.visual_effect].name
                ));
                if is_image {
                    ui.small(format!("• Gerakan: {}", self.motion.label()));
                }
            });
        });
    }

    /// Merender simulasi live dari 50 Efek Visual (`EFFECTS`) langsung di atas kanvas pratinjau.
    pub(super) fn draw_visual_effect_preview(&self, painter: &egui::Painter, rect: Rect, t: f32) {
        let effect_id = EFFECTS.get(self.visual_effect).map_or("none", |e| e.id);
        if effect_id == "none" {
            return;
        }

        let w = rect.width();
        let h = rect.height();

        match effect_id {
            "rain" | "drizzle" => {
                let count = if effect_id == "rain" { 55 } else { 28 };
                let speed = if effect_id == "rain" { 1.6 } else { 0.9 };
                let len = if effect_id == "rain" {
                    h * 0.09
                } else {
                    h * 0.05
                };
                for i in 0..count {
                    let seed = i as f32 * 1.37;
                    let x_norm = ((seed * 7.13).sin().abs() + (t * 0.12 * speed)) % 1.0;
                    let y_norm = ((seed * 3.91).cos().abs() + (t * speed)) % 1.0;
                    let p1 = Pos2::new(rect.left() + x_norm * w, rect.top() + y_norm * h);
                    let p2 = Pos2::new(p1.x - len * 0.22, (p1.y + len).min(rect.bottom()));
                    painter.line_segment(
                        [p1, p2],
                        Stroke::new(
                            1.2_f32,
                            Color32::from_rgba_premultiplied(180, 210, 255, 130),
                        ),
                    );
                }
            }
            "snow" | "snowflakes" => {
                let count = 42;
                for i in 0..count {
                    let seed = i as f32 * 2.17;
                    let x_norm =
                        ((seed * 5.3).sin().abs() + (t * 0.15 + seed).sin() * 0.04).rem_euclid(1.0);
                    let y_norm = ((seed * 4.1).cos().abs() + t * 0.35) % 1.0;
                    let pos = Pos2::new(rect.left() + x_norm * w, rect.top() + y_norm * h);
                    let r = 1.8 + (seed.sin().abs() * 2.8);
                    painter.circle_filled(pos, r, Color32::from_white_alpha(200));
                }
            }
            "fire" | "fireworks" => {
                let count = 40;
                for i in 0..count {
                    let seed = i as f32 * 1.93;
                    let x_norm =
                        ((seed * 6.1).sin().abs() + (t * 1.2 + seed).sin() * 0.03).rem_euclid(1.0);
                    let y_norm = 1.0 - (((seed * 3.7).cos().abs() + t * 0.55) % 1.0);
                    let pos = Pos2::new(rect.left() + x_norm * w, rect.top() + y_norm * h);
                    let col = if i % 3 == 0 {
                        Color32::from_rgb(255, 220, 90)
                    } else if i % 2 == 0 {
                        Color32::from_rgb(255, 110, 30)
                    } else {
                        Color32::from_rgb(255, 50, 80)
                    };
                    painter.circle_filled(pos, 2.2 + (seed.cos().abs() * 2.0), col);
                }
            }
            "bokeh" | "bubbles" | "balloons" => {
                for i in 0..18 {
                    let seed = i as f32 * 3.11;
                    let x_norm =
                        ((seed * 4.7).sin().abs() + (t * 0.08 + seed).sin() * 0.05).rem_euclid(1.0);
                    let y_norm = 1.0 - (((seed * 2.9).cos().abs() + t * 0.18) % 1.0);
                    let pos = Pos2::new(rect.left() + x_norm * w, rect.top() + y_norm * h);
                    let radius = h * (0.025 + (seed.sin().abs() * 0.04));
                    let col = match effect_id {
                        "bubbles" => Color32::from_rgba_premultiplied(160, 230, 255, 70),
                        "balloons" => Color32::from_rgba_premultiplied(255, 120, 180, 110),
                        _ => Color32::from_rgba_premultiplied(255, 200, 120, 65),
                    };
                    painter.circle_filled(pos, radius, col);
                    painter.circle_stroke(
                        pos,
                        radius,
                        Stroke::new(1.0_f32, Color32::from_white_alpha(90)),
                    );
                }
            }
            "sparkles" | "stars" | "shooting_stars" | "gold_rain" | "dust" | "film_dust" => {
                let is_gold = matches!(effect_id, "gold_rain" | "dust");
                for i in 0..38 {
                    let seed = i as f32 * 2.71;
                    let x_norm = ((seed * 8.3).sin().abs() + t * 0.03) % 1.0;
                    let y_norm = ((seed * 5.9).cos().abs()
                        + if effect_id == "gold_rain" {
                            t * 0.45
                        } else {
                            t * 0.05
                        })
                        % 1.0;
                    let twinkle = ((t * 4.5 + seed * 3.0).sin().abs() * 230.0) as u8;
                    let col = if is_gold {
                        Color32::from_rgba_premultiplied(255, 215, 90, twinkle)
                    } else {
                        Color32::from_white_alpha(twinkle)
                    };
                    let pos = Pos2::new(rect.left() + x_norm * w, rect.top() + y_norm * h);
                    painter.circle_filled(pos, 1.8 + (seed.sin().abs() * 1.6), col);
                }
            }
            "fireflies" => {
                for i in 0..24 {
                    let seed = i as f32 * 2.53;
                    let x_norm =
                        ((seed * 4.3).sin().abs() + (t * 0.4 + seed).sin() * 0.08).rem_euclid(1.0);
                    let y_norm =
                        ((seed * 6.7).cos().abs() + (t * 0.3 + seed).cos() * 0.08).rem_euclid(1.0);
                    let glow = ((t * 3.0 + seed).sin().abs() * 220.0) as u8;
                    let pos = Pos2::new(rect.left() + x_norm * w, rect.top() + y_norm * h);
                    painter.circle_filled(
                        pos,
                        3.2,
                        Color32::from_rgba_premultiplied(170, 255, 70, glow),
                    );
                }
            }
            "hearts" | "petals" | "rose_petals" | "leaves" | "confetti" => {
                for i in 0..30 {
                    let seed = i as f32 * 1.79;
                    let x_norm =
                        ((seed * 5.1).sin().abs() + (t * 0.5 + seed).sin() * 0.06).rem_euclid(1.0);
                    let y_norm = ((seed * 3.3).cos().abs() + t * 0.28) % 1.0;
                    let pos = Pos2::new(rect.left() + x_norm * w, rect.top() + y_norm * h);
                    let col = match effect_id {
                        "hearts" | "petals" => Color32::from_rgb(255, 165, 205),
                        "rose_petals" => Color32::from_rgb(220, 40, 75),
                        "leaves" => Color32::from_rgb(235, 130, 35),
                        _ => {
                            if i % 3 == 0 {
                                Color32::from_rgb(0, 230, 255)
                            } else if i % 2 == 0 {
                                Color32::from_rgb(255, 220, 40)
                            } else {
                                Color32::from_rgb(255, 70, 190)
                            }
                        }
                    };
                    painter.circle_filled(pos, 3.0, col);
                }
            }
            "matrix" => {
                let cols = 22;
                for c in 0..cols {
                    let x = rect.left() + (c as f32 + 0.5) * (w / cols as f32);
                    let y_head = rect.top() + (((c as f32 * 3.7).sin().abs() + t * 0.65) % 1.0) * h;
                    let tail = (y_head - h * 0.22).max(rect.top());
                    painter.line_segment(
                        [Pos2::new(x, tail), Pos2::new(x, y_head)],
                        Stroke::new(2.0_f32, Color32::from_rgba_premultiplied(0, 255, 90, 150)),
                    );
                }
            }
            "color_grading" => {
                painter.rect_filled(rect, 3.0, Color32::from_rgba_premultiplied(20, 45, 65, 38));
            }
            "warm" | "light_leak" | "rays" => {
                painter.rect_filled(
                    rect,
                    3.0,
                    Color32::from_rgba_premultiplied(255, 165, 60, 42),
                );
            }
            "cool" | "mist" | "smoke" => {
                painter.rect_filled(
                    rect,
                    3.0,
                    Color32::from_rgba_premultiplied(65, 130, 220, 42),
                );
            }
            "vintage" | "sepia" => {
                painter.rect_filled(
                    rect,
                    3.0,
                    Color32::from_rgba_premultiplied(160, 120, 65, 58),
                );
            }
            "bw" => {
                painter.rect_filled(rect, 3.0, Color32::from_rgba_premultiplied(30, 30, 35, 90));
            }
            "dreamy" | "bloom" | "aurora" | "nebula" | "synthwave" => {
                let pulse = ((t * 2.0).sin().abs() * 35.0) as u8 + 25;
                painter.rect_filled(
                    rect,
                    3.0,
                    Color32::from_rgba_premultiplied(170, 90, 255, pulse),
                );
            }
            "cinema" => {
                let bar_h = h * 0.10;
                painter.rect_filled(
                    Rect::from_min_size(rect.min, Vec2::new(w, bar_h)),
                    0.0,
                    Color32::BLACK,
                );
                painter.rect_filled(
                    Rect::from_min_size(
                        Pos2::new(rect.left(), rect.bottom() - bar_h),
                        Vec2::new(w, bar_h),
                    ),
                    0.0,
                    Color32::BLACK,
                );
            }
            "pulse" | "lightning" => {
                let flash = ((t * 5.0).sin().max(0.0).powf(3.0) * 90.0) as u8;
                painter.rect_filled(rect, 3.0, Color32::from_white_alpha(flash));
            }
            "vhs" | "glitch" => {
                let rows = 18;
                for r in 0..rows {
                    let y = rect.top() + (r as f32 / rows as f32) * h;
                    painter.line_segment(
                        [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                        Stroke::new(1.0_f32, Color32::from_black_alpha(65)),
                    );
                }
                if effect_id == "glitch" && ((t * 6.0) as i32 % 4 == 0) {
                    let gy = rect.top() + ((t * 13.0).sin().abs() * 0.8 + 0.1) * h;
                    let glitch_rect = Rect::from_center_size(
                        Pos2::new(rect.center().x, gy),
                        Vec2::new(w, h * 0.05),
                    );
                    painter.rect_filled(
                        glitch_rect,
                        0.0,
                        Color32::from_rgba_premultiplied(0, 240, 255, 75),
                    );
                }
            }
            _ => {
                // Default subtle ambient shimmer for remaining effects
                let shimmer = ((t * 2.5).sin().abs() * 35.0) as u8 + 15;
                painter.rect_filled(
                    rect,
                    3.0,
                    Color32::from_rgba_premultiplied(100, 180, 255, shimmer),
                );
            }
        }
    }

    pub(super) fn load_video(&mut self, path: PathBuf) {
        let tools = self.tools.clone();
        self.spawn(async move {
            Event::Background(
                probe_video(&tools, &path)
                    .await
                    .map(|video| Background { video, image: None }),
            )
        });
    }

    pub(super) fn load_image(&mut self, image: PathBuf) {
        let job = ImageJob {
            image,
            motion: self.motion,
            beat_pulse: self.beat_pulse,
            aspect_ratio: self.aspect_ratio.to_owned(),
            resolution: self.resolution.to_owned(),
            effect_id: EFFECTS[self.visual_effect].id.to_owned(),
        };
        let (tools, paths, hardware) = (
            self.tools.clone(),
            self.paths.clone(),
            self.hardware.clone(),
        );
        self.spawn(async move {
            Event::Background(build_image_background(tools, paths, hardware, job).await)
        });
    }

    #[allow(dead_code)]
    pub(super) fn load_demo(&mut self) {
        let (tools, dir) = (self.tools.clone(), self.paths.temp.join("demo"));
        self.spawn(async move { Event::Demo(demo::generate(&tools, &dir).await) });
    }
}
