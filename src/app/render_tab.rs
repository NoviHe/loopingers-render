use eframe::egui;
use loopkaraoke::{
    hardware::HardwareInfo,
    media::format_clock,
    render::{
        self,
        effects::EFFECTS,
        encoder::{Encoder, VideoTarget},
        job::{JobHandle, JobStatus},
        presets::{self, RESOLUTION_LABELS},
        request::{ImageSource, RenderRequest, TrackData},
    },
    util::open_path,
};

use super::{help::render as help, theme, StudioApp};

impl StudioApp {
    pub(super) fn render_tab(&mut self, ui: &mut egui::Ui) {
        theme::heading_with_info(ui, "Render & Ekspor Video", help::HEADING);
        ui.add_space(4.0);
        ui.columns(2, |columns| {
            self.render_settings(&mut columns[0]);
            self.render_summary(&mut columns[1]);
        });
    }

    /// Why rendering cannot start yet, in words the user can act on.
    pub(super) fn render_blocker(&self) -> Option<&'static str> {
        if self.background.is_none() {
            Some("Pilih background di langkah 1")
        } else if self.tracks.is_empty() {
            Some("Tambahkan lagu di langkah 2")
        } else if self.hardware.is_none() {
            Some("Mendeteksi hardware...")
        } else {
            None
        }
    }

    fn render_settings(&mut self, ui: &mut egui::Ui) {
        theme::strong_with_info(ui, "Resolusi", help::RESOLUTION);
        ui.horizontal_wrapped(|ui| {
            for label in RESOLUTION_LABELS {
                ui.radio_value(&mut self.resolution, label, label);
            }
        });
        ui.add_space(4.0);
        theme::strong_with_info(ui, "Frame rate", help::FPS);
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.fps, 30, "30 FPS");
            ui.radio_value(&mut self.fps, 60, "60 FPS");
        });
        ui.add_space(4.0);

        let hardware = self.hardware.clone().unwrap_or_default();
        theme::strong_with_info(ui, "Encoder", help::ENCODER);
        ui.label(format!(
            "{} — {} ({})",
            self.encoder.name(),
            hardware.gpu_name,
            hardware.vendor()
        ));
        ui.add_space(2.0);
        egui::CollapsingHeader::new("Pengaturan lanjutan")
            .default_open(false)
            .show(ui, |ui| self.encoder_section(ui, &hardware));
        ui.add_space(4.0);

        theme::strong_with_info(ui, "Mode Daftar Lagu & Visual", help::PRESET);
        ui.horizontal_wrapped(|ui| {
            for mode in loopkaraoke::karaoke::ass::PlaylistDisplayMode::ALL {
                if ui
                    .selectable_label(self.playlist_mode == mode, mode.label())
                    .clicked()
                {
                    self.playlist_mode = mode;
                    if mode.includes_visual()
                        && !self.show_tracklist_visual
                        && !self.visualizer.enabled
                    {
                        self.show_tracklist_visual = true;
                        self.visualizer.enabled = true;
                    }
                }
            }
        });
        if self.playlist_mode.includes_visual() {
            ui.checkbox(
                &mut self.show_tracklist_visual,
                format!(
                    "Sertakan Daftar Judul Lagu (Tracklist) di Layar ({})",
                    self.tracklist_position.label()
                ),
            );
            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut self.visualizer.enabled,
                    "Sertakan Visualizer Musik pada Video",
                );
                if self.visualizer.enabled {
                    ui.colored_label(
                        theme::FOCUS,
                        format!("(Bentuk: {})", self.visualizer.preset.label()),
                    );
                }
            });
        }
    }

    fn render_summary(&mut self, ui: &mut egui::Ui) {
        let (aspect_ratio, resolution) = self.resolved_render_format();
        let (width, height) = presets::dimensions(aspect_ratio, resolution);
        let without_lyrics = self
            .tracks
            .iter()
            .filter(|track| track.lyrics.is_empty())
            .count();
        theme::strong_with_info(ui, "Ringkasan", help::START);
        ui.label(format!(
            "Keluaran {width}x{height} • {} lagu • total {}",
            self.tracks.len(),
            format_clock(self.total_duration().round() as u64)
        ));
        ui.label(format!(
            "Mode Daftar Lagu: {}",
            self.playlist_mode.short_label()
        ));
        if self.playlist_mode.includes_lyrics() && without_lyrics > 0 {
            ui.colored_label(
                theme::WARNING,
                format!("{without_lyrics} lagu belum punya lirik; hanya banner judul/daftar lagu yang tampil."),
            );
        }
        if let Some(reason) = self.render_blocker() {
            ui.colored_label(theme::WARNING, reason);
        } else if self.job.is_none() {
            ui.small("Semua siap. Tekan Mulai Render di kanan bawah.");
        }

        if let Some(job) = self.job.clone() {
            ui.add_space(8.0);
            self.job_section(ui, &job);
        }
    }

    fn encoder_section(&mut self, ui: &mut egui::Ui, hardware: &HardwareInfo) {
        let previous = self.encoder;
        let best = Encoder::best_available(hardware);
        ui.horizontal(|ui| {
            if ui.button("Otomatis (terbaik)").clicked() {
                self.encoder = best;
                self.preset = best.default_preset().to_owned();
            }
            theme::info(ui, help::AUTO_ENCODER);
        });
        ui.horizontal_wrapped(|ui| {
            for encoder in Encoder::ALL {
                ui.add_enabled_ui(encoder.is_available(hardware), |ui| {
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
            ui.add(egui::Slider::new(&mut self.quality, 15..=35).text("Kualitas"));
            theme::info(ui, help::QUALITY);
        });
        ui.small("Angka kualitas makin kecil, gambar makin tajam, file makin besar.");
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
