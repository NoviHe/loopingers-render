use eframe::egui;
use loopkaraoke::{
    media::format_clock,
    render::{
        self,
        effects::EFFECTS,
        encoder::Encoder,
        job::{JobHandle, JobStatus},
        presets::{self, RESOLUTION_LABELS},
        request::{ImageSource, RenderRequest, TrackData},
    },
    util::open_path,
};

use super::{help::render as help, theme, StudioApp};

impl StudioApp {
    pub(super) fn render_tab(&mut self, ui: &mut egui::Ui) {
        theme::heading_with_info(ui, "Langkah 4 — Render & Ekspor Video", help::HEADING);
        ui.add_space(4.0);
        ui.columns(2, |columns| {
            self.render_settings(&mut columns[0]);
            self.render_summary(&mut columns[1]);
        });
    }

    fn render_settings(&mut self, ui: &mut egui::Ui) {
        theme::strong_with_info(ui, "Resolusi", help::RESOLUTION);
        ui.horizontal(|ui| {
            for label in RESOLUTION_LABELS {
                ui.radio_value(&mut self.resolution, label, label);
            }
        });
        theme::strong_with_info(ui, "Frame rate", help::FPS);
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.fps, 30, "30 FPS");
            ui.radio_value(&mut self.fps, 60, "60 FPS");
        });

        self.encoder_section(ui);
    }

    fn render_summary(&mut self, ui: &mut egui::Ui) {
        let (width, height) = presets::dimensions(self.aspect_ratio, self.resolution);
        let without_lyrics = self
            .tracks
            .iter()
            .filter(|track| track.lyrics.is_empty())
            .count();
        ui.label(format!(
            "Keluaran {width}x{height} • {} lagu • total {}",
            self.tracks.len(),
            format_clock(self.total_duration().round() as u64)
        ));
        if without_lyrics > 0 {
            ui.colored_label(
                super::theme::WARNING,
                format!("{without_lyrics} lagu belum punya lirik; hanya banner judul yang tampil."),
            );
        }

        ui.add_space(8.0);
        let ready = self.background.is_some()
            && !self.tracks.is_empty()
            && self.hardware.is_some()
            && !self.job_running();
        if ui
            .add_enabled(ready, egui::Button::new("Mulai Render"))
            .clicked()
        {
            self.start_render();
        }
        theme::info(ui, help::START);
        if self.background.is_none() || self.tracks.is_empty() {
            ui.small("Pilih background (langkah 1) dan tambahkan lagu (langkah 2) dulu.");
        }

        if let Some(job) = self.job.clone() {
            ui.add_space(8.0);
            self.job_section(ui, &job);
        }
    }

    fn encoder_section(&mut self, ui: &mut egui::Ui) {
        let hardware = self.hardware.clone().unwrap_or_default();
        let previous = self.encoder;

        let best = Encoder::best_available(&hardware);
        theme::strong_with_info(ui, "Encoder", help::ENCODER);
        ui.horizontal(|ui| {
            ui.label(format!(
                "Terdeteksi: {} ({}) — pilihan terbaik: {}",
                hardware.gpu_name,
                hardware.vendor(),
                best.name()
            ));
            if ui.button("Otomatis (terbaik)").clicked() {
                self.encoder = best;
                self.preset = best.default_preset().to_owned();
            }
            theme::info(ui, help::AUTO_ENCODER);
        });
        ui.horizontal_wrapped(|ui| {
            for encoder in Encoder::ALL {
                ui.add_enabled_ui(encoder.is_available(&hardware), |ui| {
                    ui.radio_value(&mut self.encoder, encoder, encoder.name());
                });
            }
        });
        if self.encoder != previous {
            self.preset = self.encoder.default_preset().to_owned();
        }

        let presets = self.encoder.presets();
        ui.horizontal(|ui| {
            theme::label_with_info(ui, "Preset:", help::PRESET);
            egui::ComboBox::from_id_salt("encoder_preset")
                .selected_text(self.preset.clone())
                .show_ui(ui, |ui| {
                    for preset in presets {
                        ui.selectable_value(&mut self.preset, (*preset).to_owned(), *preset);
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.add(
                egui::Slider::new(&mut self.quality, 15..=35)
                    .text("Kualitas (CQ/CRF, makin kecil makin tajam)"),
            );
            theme::info(ui, help::QUALITY);
        });
    }

    fn job_section(&mut self, ui: &mut egui::Ui, job: &JobHandle) {
        let snapshot = job.snapshot();
        match snapshot.status {
            JobStatus::Queued | JobStatus::Preparing => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Menyiapkan audio, subtitle, dan background...");
                });
            }
            JobStatus::Rendering => {
                ui.add(
                    egui::ProgressBar::new((snapshot.progress_percent / 100.0) as f32)
                        .show_percentage(),
                );
                ui.label(format!(
                    "{} / {} • kecepatan {} • {:.0} fps • sisa {} detik",
                    format_clock(snapshot.current_time_sec as u64),
                    format_clock(snapshot.total_time_sec as u64),
                    snapshot.speed,
                    snapshot.fps,
                    snapshot.eta_seconds as u64
                ));
                ui.label(format!("Encoder: {}", snapshot.encoder_name));
            }
            JobStatus::Failed => {
                ui.colored_label(
                    super::theme::DANGER,
                    format!("Render gagal: {}", snapshot.error_message),
                );
            }
            JobStatus::Completed => {
                ui.colored_label(super::theme::SUCCESS, "Render selesai 100%");
                if let Some(output) = &snapshot.output_file {
                    ui.label(output.display().to_string());
                    ui.horizontal(|ui| {
                        if ui.button("Buka Video").clicked() {
                            let _ = open_path(output);
                        }
                        if ui.button("Buka Folder").clicked() {
                            let _ = open_path(&self.paths.outputs);
                        }
                    });
                }
            }
        }
    }

    fn start_render(&mut self) {
        let (Some(background), Some(hardware)) = (&self.background, &self.hardware) else {
            return;
        };
        let request = RenderRequest {
            video_path: background.video.path.clone().into(),
            image_source: background.image.clone().map(|path| ImageSource {
                path,
                motion: self.motion,
            }),
            tracks: self
                .tracks
                .iter()
                .map(|track| TrackData {
                    path: track.path.clone(),
                    title: track.title.clone(),
                    artist: track.artist.clone(),
                    duration: track.duration,
                    lyrics: track.lyrics.clone(),
                })
                .collect(),
            aspect_ratio: self.aspect_ratio.to_owned(),
            resolution: self.resolution.to_owned(),
            fit: self.fit,
            fps: self.fps,
            style: self.style.to_config(),
            encoder: self.encoder,
            preset: self.preset.clone(),
            quality: self.quality,
            beat_pulse: self.beat_pulse,
            visual_effect: EFFECTS[self.visual_effect].id.to_owned(),
        };
        let context = render::RenderContext {
            paths: self.paths.clone(),
            tools: self.tools.clone(),
            hardware: hardware.clone(),
        };

        let job = JobHandle::new();
        self.job = Some(job.clone());
        let ctx = self.ctx.clone();
        self.runtime.spawn(async move {
            render::execute(&context, &job, request).await;
            ctx.request_repaint();
        });
    }
}
