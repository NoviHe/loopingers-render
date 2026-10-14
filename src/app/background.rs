use std::path::{Path, PathBuf};

use eframe::egui;
use loopkaraoke::{
    config::AppPaths,
    demo,
    error::Result,
    ffmpeg::Tools,
    hardware::HardwareInfo,
    media::{probe_video, VideoInfo},
    render::{
        effects::{AppliedEffect, EFFECTS},
        image_video::{ImageVideo, Motion, DEFAULT_BPM},
        looper::FitMode,
        presets::ASPECT_RATIOS,
    },
    util::{open_path, random_id},
};

use super::{help::background as help, theme, Event, StudioApp};

const VIDEO_EXTENSIONS: [&str; 5] = ["mp4", "mov", "mkv", "webm", "avi"];
const IMAGE_EXTENSIONS: [&str; 4] = ["jpg", "jpeg", "png", "webp"];
const IMAGE_CLIP_SECONDS: f64 = 10.0;
const IMAGE_CLIP_FPS: u32 = 30;

/// The chosen background: a video, or a clip generated from a still image.
pub struct Background {
    pub video: VideoInfo,
    pub image: Option<PathBuf>,
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
        theme::heading_with_info(
            ui,
            "Langkah 1 — Video atau Gambar Background",
            help::HEADING,
        );
        ui.label("Video diulang otomatis sepanjang total durasi lagu. Gambar diubah menjadi video bergerak.");
        ui.add_space(4.0);
        ui.columns(2, |columns| {
            self.background_source(&mut columns[0]);
            self.background_options(&mut columns[1]);
        });
    }

    fn background_source(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui.button("Pilih Video...").clicked() {
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
            if ui.button("Coba Demo Sampel").clicked() {
                self.load_demo();
            }
            theme::info(ui, help::DEMO);
        });
        ui.add_space(4.0);

        if let Some(background) = &self.background {
            let video = &background.video;
            let source = background
                .image
                .as_deref()
                .and_then(Path::file_name)
                .map_or_else(
                    || video.filename.clone(),
                    |name| name.to_string_lossy().into_owned(),
                );
            let summary = format!(
                "{source}\n{}x{} • {} • {:.0} fps • {:.1} MB",
                video.width, video.height, video.duration_str, video.fps, video.size_mb
            );
            let path = PathBuf::from(&video.path);
            ui.group(|ui| {
                ui.label(summary);
                ui.horizontal(|ui| {
                    if ui.button("Buka di pemutar").clicked() {
                        let _ = open_path(&path);
                    }
                    theme::info(ui, help::OPEN_PLAYER);
                });
            });
        } else {
            ui.label("Belum ada background. Pilih video, gambar, atau coba demo.");
        }
    }

    fn background_options(&mut self, ui: &mut egui::Ui) {
        theme::strong_with_info(ui, "Rasio layar", help::ASPECT);
        ui.horizontal_wrapped(|ui| {
            for aspect in ASPECT_RATIOS {
                ui.radio_value(&mut self.aspect_ratio, aspect, aspect);
            }
        });

        theme::strong_with_info(ui, "Mode tampilan", help::FIT);
        ui.horizontal(|ui| {
            for fit in FitMode::ALL {
                ui.radio_value(&mut self.fit, fit, fit.label());
            }
        });

        theme::strong_with_info(ui, "Efek visual", help::EFFECT);
        egui::ComboBox::from_id_salt("visual_effect")
            .selected_text(EFFECTS[self.visual_effect].name)
            .show_ui(ui, |ui| {
                for (index, effect) in EFFECTS.iter().enumerate() {
                    ui.selectable_value(&mut self.visual_effect, index, effect.name)
                        .on_hover_text(effect.desc);
                }
            });
        ui.small(EFFECTS[self.visual_effect].desc);

        if let Some(image) = self.background.as_ref().and_then(|bg| bg.image.clone()) {
            ui.add_space(4.0);
            theme::strong_with_info(ui, "Gerakan gambar", help::MOTION);
            ui.horizontal_wrapped(|ui| {
                for motion in Motion::ALL {
                    ui.radio_value(&mut self.motion, motion, motion.label());
                }
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.beat_pulse, "Denyut mengikuti ketukan musik");
                theme::info(ui, help::BEAT_PULSE);
            });
            ui.horizontal(|ui| {
                if ui.button("Terapkan ke background gambar").clicked() {
                    self.load_image(image);
                }
                theme::info(ui, help::APPLY_IMAGE);
            });
        }
    }

    fn load_video(&mut self, path: PathBuf) {
        let tools = self.tools.clone();
        self.spawn(async move {
            Event::Background(
                probe_video(&tools, &path)
                    .await
                    .map(|video| Background { video, image: None }),
            )
        });
    }

    fn load_image(&mut self, image: PathBuf) {
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

    fn load_demo(&mut self) {
        let (tools, dir) = (self.tools.clone(), self.paths.temp.join("demo"));
        self.spawn(async move { Event::Demo(demo::generate(&tools, &dir).await) });
    }
}
