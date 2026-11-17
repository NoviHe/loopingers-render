use eframe::egui;
use loopkaraoke::{
    media::format_clock,
    render::{
        self,
        effects::EFFECTS,
        encoder::VideoTarget,
        job::{JobHandle, JobStatus},
        presets,
        request::{ImageSource, RenderRequest, TrackData},
    },
    util::open_path,
};

use super::{theme, StudioApp};

impl StudioApp {
    /// Why rendering cannot start yet, in words the user can act on.
    pub(super) fn render_blocker(&self) -> Option<&'static str> {
        if self.background.is_none() {
            Some("Pilih video atau gambar di Render Old")
        } else if self.tracks.is_empty() {
            Some("Tambahkan lagu di Render Old")
        } else if self.hardware.is_none() {
            Some("Mendeteksi hardware...")
        } else {
            None
        }
    }

    pub(super) fn job_section(&mut self, ui: &mut egui::Ui, job: &JobHandle) {
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
                    theme::DANGER,
                    format!("Render gagal: {}", snapshot.error_message),
                );
            }
            JobStatus::Completed => {
                ui.colored_label(theme::SUCCESS, "Render selesai 100%");
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

    fn resolved_render_format(&self) -> (&'static str, &'static str) {
        if self.video_settings.target == VideoTarget::YouTubeStandard {
            let res = if self.resolution == "720p" {
                "720p"
            } else {
                "1080p"
            };
            ("16:9", res)
        } else {
            (self.aspect_ratio, self.resolution)
        }
    }

    pub(super) fn start_render(&mut self) {
        let (Some(background), Some(hardware)) = (&self.background, &self.hardware) else {
            return;
        };
        let (aspect_ratio, resolution) = self.resolved_render_format();
        let encoder = if self.video_settings.target == VideoTarget::YouTubeStandard {
            self.video_settings.resolve_encoder(hardware)
        } else {
            self.encoder
        };
        let preset = if self.video_settings.target == VideoTarget::YouTubeStandard {
            self.video_settings.resolve_preset(encoder).to_owned()
        } else {
            self.preset.clone()
        };
        let audio_bitrate = self.video_settings.resolve_audio_bitrate("");
        let request = RenderRequest {
            custom_filename: self.custom_filename.clone(),
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
            aspect_ratio: aspect_ratio.to_owned(),
            resolution: resolution.to_owned(),
            fit: self.fit,
            fps: self.fps,
            style: self.build_style_config(),
            encoder,
            preset,
            quality: self.quality,
            audio_bitrate: audio_bitrate.clone(),
            youtube_standard: self.video_settings.target == VideoTarget::YouTubeStandard,
            beat_pulse: self.beat_pulse,
            visual_effect: EFFECTS[self.visual_effect].id.to_owned(),
            visualizer: None,
            tracklist_overlay: if self.show_tracklist_visual && !self.tracks.is_empty() {
                Some(self.build_thumbnail_spec(true))
            } else {
                None
            },
        };
        let context = render::RenderContext {
            paths: self.paths.clone(),
            tools: self.tools.clone(),
            hardware: hardware.clone(),
        };

        let job = JobHandle::new();
        self.job = Some(job.clone());
        let (width, height) = presets::dimensions(aspect_ratio, resolution);
        self.log(
            super::LogLevel::Info,
            format!(
                "Mulai render video: {width}x{height} @ {} FPS • Encoder: {} • Audio: {audio_bitrate} • Efek: {}",
                self.fps,
                encoder.name(),
                EFFECTS[self.visual_effect].name
            ),
        );
        if self.auto_generate_thumbnail {
            self.generate_thumbnail();
        }

        let ctx = self.ctx.clone();
        self.runtime.spawn(async move {
            render::execute(&context, &job, request).await;
            ctx.request_repaint();
        });
    }
}
