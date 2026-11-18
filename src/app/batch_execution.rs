//! Snapshot the batch plan and execute it through the shared render queue.
use eframe::egui;
use loopkaraoke::{
    media::probe_video,
    render::{
        self,
        batch::{BatchHandle, BatchItem, BatchStatus},
        encoder::VideoTarget,
        AutoThumbnailRequest, RenderContext, SimpleRenderRequest, VisualVariationMode,
    },
    util::open_path,
};

use super::super::{
    generate_tab::generate_card,
    theme::{self, UiText},
    StudioApp,
};

impl StudioApp {
    pub(in crate::app) fn start_batch_execution(&mut self) {
        if self.job_running() || !self.ffmpeg_ready {
            return;
        }
        let Some(hardware) = self.hardware.clone() else {
            return;
        };
        let Some(plan) = &self.batch_render.plan else {
            return;
        };
        if !self.batch_render.auto_thumbnail
            && plan
                .videos
                .iter()
                .any(|video| video.thumbnail.as_ref().is_none_or(|path| !path.is_file()))
        {
            self.batch_render.plan_error = Some(
                "Thumbnail belum lengkap. Pilih folder dengan nama yang cocok untuk setiap video atau aktifkan Auto Thumbnail."
                    .into(),
            );
            return;
        }
        let encoder = self.video_settings.resolve_encoder(&hardware);
        let output_dir = self
            .generate
            .output_dir
            .clone()
            .unwrap_or_else(|| self.paths.outputs.clone());
        let mut items = Vec::with_capacity(plan.videos.len());
        for video in &plan.videos {
            let Some(tracks) = video
                .order
                .iter()
                .map(|index| plan.tracks.get(*index))
                .collect::<Option<Vec<_>>>()
            else {
                self.batch_render.plan_error =
                    Some("Urutan lagu pada rencana tidak valid. Buat rencana baru.".into());
                return;
            };
            let titles: Vec<_> = tracks.iter().map(|track| track.title.clone()).collect();
            let thumbnail_path = video
                .thumbnail
                .as_ref()
                .filter(|path| path.is_file())
                .cloned();
            let auto_thumbnail = (self.batch_render.auto_thumbnail && thumbnail_path.is_none())
                .then(|| AutoThumbnailRequest {
                    spec: self.build_batch_thumbnail_spec(&titles),
                    timestamp: self.thumbnail.timestamp,
                });
            items.push(BatchItem {
                request: SimpleRenderRequest {
                    video_path: video.visual.clone(),
                    audio_files: tracks.iter().map(|track| track.path.clone()).collect(),
                    track_titles: titles,
                    thumbnail_path,
                    auto_thumbnail,
                    output_dir: output_dir.clone(),
                    size: (1920, 1080),
                    fps: 30,
                    encoder,
                    preset: self.video_settings.resolve_preset(encoder).to_owned(),
                    quality: self.video_settings.resolve_quality(encoder),
                    audio_bitrate: self.video_settings.resolve_audio_bitrate(
                        ["128k", "192k", "256k", "320k"]
                            .get(self.generate.audio_bitrate_idx)
                            .copied()
                            .unwrap_or("192k"),
                    ),
                    stream_copy_video: false,
                    youtube_standard: self.video_settings.target == VideoTarget::YouTubeStandard,
                    visual_mode: VisualVariationMode::from_index(
                        self.generate.visual_variation_idx,
                    ),
                    anti_duplicate: self.generate.anti_duplicate_idx == 0,
                    output_code: Some(video.output_code.clone()),
                    pre_resolved_stamp: None,
                    fade_out_secs: self.generate.fade_out_secs.clamp(0.0, 60.0),
                    crossfade_secs: plan.crossfade_secs,
                    normalize_volume: self.generate.volume_norm_idx == 0,
                    target_duration_secs: None,
                },
            });
        }
        if items.is_empty() {
            return;
        }
        self.batch_render.plan_error = None;
        let handle = BatchHandle::new(items.len());
        self.batch_render.execution_codes = plan
            .videos
            .iter()
            .map(|video| video.output_code.clone())
            .collect();
        self.batch_render.execution = Some(handle.clone());
        let context = RenderContext {
            paths: self.paths.clone(),
            tools: self.tools.clone(),
            hardware,
        };
        let settings = self.video_settings;
        let resolution = self.generate.resolution_idx;
        let sender = self.sender.clone();
        let ctx = self.ctx.clone();
        self.info(format!(
            "Mulai Batch: {} video ke {}",
            items.len(),
            output_dir.display()
        ));
        self.runtime.spawn(async move {
            // Probing runs on the async worker, never on the UI thread. Requests
            // and the playlist are already frozen, so edits cannot change this run.
            for item in &mut items {
                if handle.stop_requested() {
                    break;
                }
                if let Ok(video) = probe_video(&context.tools, &item.request.video_path).await {
                    let requested = batch_dimensions(video.width, video.height, resolution);
                    item.request.size = settings.resolve_target_size(requested);
                    item.request.fps = if video.fps.is_finite() && video.fps > 0.0 {
                        video.fps.round().clamp(1.0, 120.0) as u32
                    } else {
                        30
                    };
                    let stamp = render::build_anti_duplicate_stamp(
                        item.request.visual_mode,
                        item.request.anti_duplicate,
                    );
                    item.request.stream_copy_video = stamp.visual_variation
                        == render::looper::VisualVariation::None
                        && settings.can_stream_copy(
                            &video.codec,
                            &video.pix_fmt,
                            (video.width, video.height),
                            item.request.size,
                        );
                    item.request.pre_resolved_stamp = Some(stamp);
                }
            }
            let monitor = handle.clone();
            let shown = std::sync::atomic::AtomicBool::new(false);
            let logged = std::sync::atomic::AtomicUsize::new(0);
            let started = std::sync::atomic::AtomicUsize::new(0);
            let batch_warning_logged = std::sync::atomic::AtomicBool::new(false);
            render::batch::execute(context, handle, items, move || {
                use std::sync::atomic::Ordering;
                let snapshot = monitor.snapshot();
                let count = snapshot.finished.len();
                if snapshot.active_job.is_some() {
                    let number = snapshot.current + 1;
                    if number > started.swap(number, Ordering::Relaxed) {
                        let _ = sender.send(super::super::Event::BatchRenderDiagnostic(
                            super::super::LogLevel::Info,
                            format!(
                                "Batch Render, video {number}/{}: menyiapkan audio dan render.",
                                snapshot.total
                            ),
                        ));
                    }
                }
                if let Some(warning) = &snapshot.warning {
                    if !batch_warning_logged.swap(true, Ordering::Relaxed) {
                        let _ = sender.send(super::super::Event::BatchRenderDiagnostic(
                            super::super::LogLevel::Warn,
                            format!("Batch Render: {warning}"),
                        ));
                    }
                }
                if count > logged.swap(count, Ordering::Relaxed) {
                    if let Some(result) = snapshot.finished.last() {
                        if let Some(output) = &result.output {
                            let _ = sender.send(super::super::Event::BatchRenderDiagnostic(
                                super::super::LogLevel::Success,
                                format!(
                                    "Batch Render, video {count}/{} selesai: {}",
                                    snapshot.total,
                                    output.display()
                                ),
                            ));
                        }
                        for (level, detail) in [
                            (super::super::LogLevel::Error, result.error.as_deref()),
                            (super::super::LogLevel::Warn, result.warning.as_deref()),
                        ] {
                            if let Some(detail) = detail {
                                let _ = sender.send(super::super::Event::BatchRenderDiagnostic(
                                    level,
                                    format!("Batch Render, video {count}: {detail}"),
                                ));
                            }
                        }
                    }
                }
                if !shown.load(Ordering::Relaxed) {
                    let latest = snapshot.finished.last();
                    let detail = [
                        snapshot.warning.as_deref(),
                        latest.and_then(|item| item.error.as_deref()),
                        latest.and_then(|item| item.warning.as_deref()),
                    ]
                    .into_iter()
                    .flatten()
                    .find(|detail| super::super::is_storage_full(detail));
                    if let Some(detail) = detail {
                        shown.store(true, Ordering::Relaxed);
                        let _ = sender.send(super::super::Event::RenderStorageFull(format!(
                            "Batch Render: {detail}"
                        )));
                    }
                }
                ctx.request_repaint();
            })
            .await;
        });
    }

    pub(super) fn batch_execution_actions(&mut self, ui: &mut egui::Ui) {
        let Some(handle) = &self.batch_render.execution else {
            return;
        };
        if handle.is_running() {
            let stopping = handle.stop_requested();
            if ui
                .add_enabled(
                    !stopping,
                    theme::StyledButton::danger(if stopping { "Membatalkan…" } else { "Batal" }),
                )
                .on_hover_text("Hentikan antrean setelah video yang sedang dirender selesai.")
                .clicked()
            {
                handle.stop_after_current();
            }
        } else if let Some(directory) = handle.snapshot().directory {
            if ui
                .add(theme::StyledButton::secondary("Buka Folder Output"))
                .clicked()
            {
                if let Err(error) = open_path(&directory) {
                    self.error(format!("Gagal membuka folder output: {error}"));
                }
            }
        }
    }

    pub(in crate::app) fn batch_execution_progress(&mut self, ui: &mut egui::Ui) {
        let Some(handle) = &self.batch_render.execution else {
            return;
        };
        let snapshot = handle.snapshot();
        generate_card(ui, |ui| {
            if let Some(warning) = &snapshot.warning {
                ui.colored_label(theme::Palette::of(ui.ctx()).danger, warning);
            }
            let active = snapshot.active_job.as_ref().map(|job| job.snapshot());
            let progress = active.as_ref().map_or_else(
                || {
                    if snapshot
                        .finished
                        .last()
                        .is_some_and(|result| result.output.is_some())
                    {
                        1.0
                    } else {
                        0.0
                    }
                },
                |job| (job.progress_percent as f32 / 100.0).clamp(0.0, 1.0),
            );
            let processed = snapshot.finished.len();
            let status = match snapshot.status {
                BatchStatus::Queued => "Menyiapkan",
                BatchStatus::Running => "Merender",
                BatchStatus::Completed => "Selesai",
                BatchStatus::Stopped => "Dihentikan",
            };
            let palette = theme::Palette::of(ui.ctx());
            egui::Frame::new()
                .fill(palette.surface_alt)
                .stroke(egui::Stroke::new(1.0_f32, palette.border))
                .corner_radius(8)
                .inner_margin(14)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{status} — {processed}/{} video diproses",
                                snapshot.total,
                            ))
                            .strong(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(format!("{:.0}%", progress * 100.0))
                                    .strong()
                                    .color(palette.accent),
                            );
                        });
                    });
                    ui.add_space(10.0);
                    batch_gradient_bar(ui, progress);
                    ui.add_space(8.0);
                });
            match snapshot.status {
                BatchStatus::Queued => {
                    ui.spinner();
                    ui.muted_label("Menyiapkan antrean dan membaca video...");
                }
                BatchStatus::Running => {
                    if let Some(job) = active {
                        ui.label(format!(
                            "Video {} / {} - {}",
                            snapshot.current + 1,
                            snapshot.total,
                            job.stage.label()
                        ));
                    }
                }
                BatchStatus::Completed => {
                    ui.label("Batch selesai.");
                }
                BatchStatus::Stopped => {
                    ui.label("Batch dihentikan; hasil yang selesai tetap tersimpan.");
                }
            }
        });
    }
}

fn batch_gradient_bar(ui: &mut egui::Ui, progress: f32) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 6.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 3, theme::Palette::of(ui.ctx()).border);
    let width = rect.width() * progress;
    if width <= 0.0 {
        return;
    }
    let palette = theme::Palette::of(ui.ctx());
    let start = palette.accent.lerp_to_gamma(egui::Color32::BLACK, 0.32);
    let end = palette.accent.lerp_to_gamma(egui::Color32::WHITE, 0.55);
    let fill = egui::Rect::from_min_size(rect.min, egui::vec2(width, rect.height()));
    if width <= 6.0 {
        painter.rect_filled(fill, 3, palette.accent);
        return;
    }
    let mut mesh = egui::Mesh::default();
    for i in 0..=2 {
        let x = rect.left() + 3.0 + (width - 6.0) * i as f32 / 2.0;
        let color = [start, palette.accent, end][i as usize];
        mesh.colored_vertex(egui::pos2(x, rect.top()), color);
        mesh.colored_vertex(egui::pos2(x, rect.bottom()), color);
        if i > 0 {
            let base = (i - 1) * 2;
            mesh.add_triangle(base, base + 1, base + 2);
            mesh.add_triangle(base + 1, base + 3, base + 2);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
    painter.circle_filled(egui::pos2(fill.left() + 3.0, fill.center().y), 3.0, start);
    painter.circle_filled(egui::pos2(fill.right() - 3.0, fill.center().y), 3.0, end);
}

fn batch_dimensions(width: u32, height: u32, resolution: usize) -> (u32, u32) {
    let dimensions = match resolution {
        1 => (640, 360),
        2 => (1280, 720),
        3 => (1920, 1080),
        4 => (2560, 1440),
        5 => (3840, 2160),
        _ => return (width, height),
    };
    if height > width {
        (dimensions.1, dimensions.0)
    } else {
        dimensions
    }
}

#[cfg(test)]
mod tests {
    use super::batch_dimensions;
    #[test]
    fn batch_resolutions_follow_generate_orientation_and_source_mode() {
        assert_eq!(batch_dimensions(1080, 1920, 2), (720, 1280));
        assert_eq!(batch_dimensions(1280, 720, 3), (1920, 1080));
        assert_eq!(batch_dimensions(1440, 1080, 0), (1440, 1080));
    }
}
