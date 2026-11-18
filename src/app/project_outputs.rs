//! Discover completed renders in the selected output folder, including batch bundles.
use super::{
    generate_tab::{card_header, generate_card, GenerateIcon},
    theme::{self, UiText},
    Event, StudioApp,
};
use eframe::egui;
use loopkaraoke::util::open_path;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime},
};

struct OutputEntry {
    video: PathBuf,
    thumbnail: Option<PathBuf>,
    modified: SystemTime,
    size: u64,
}

#[derive(Default)]
pub(super) struct OutputUiState {
    source: Option<PathBuf>,
    entries: Vec<OutputEntry>,
    error: Option<String>,
    checked: Option<Instant>,
    pending_delete: Option<PathBuf>,
    text_preview: Option<(String, String)>,
    used_bytes: u64,
    free_bytes: Option<u64>,
    pending_clear: Option<PathBuf>,
    pub(super) clearing: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

fn scan_outputs(root: &Path) -> Result<(Vec<OutputEntry>, u64), String> {
    if !root.exists() {
        return Ok((Vec::new(), 0));
    }
    let mut pending = vec![root.to_owned()];
    let mut entries = Vec::new();
    let mut used = 0_u64;
    while let Some(directory) = pending.pop() {
        for item in fs::read_dir(&directory).map_err(|e| format!("{}: {e}", directory.display()))? {
            let item = item.map_err(|e| e.to_string())?;
            let kind = item.file_type().map_err(|e| e.to_string())?;
            if kind.is_dir() {
                pending.push(item.path());
                continue;
            }
            if !kind.is_file() {
                continue;
            }
            used = used.saturating_add(item.metadata().map_err(|e| e.to_string())?.len());
            let path = item.path();
            if !path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("mp4"))
            {
                continue;
            }
            let stem = path.file_stem().unwrap_or_default().to_string_lossy();
            if stem.starts_with("video_") && !directory.join("playlist.txt").is_file() {
                continue; // Generate writes the playlist only after video rendering succeeds.
            }
            let code = stem.strip_prefix("video_").unwrap_or(&stem);
            let thumbnail = ["jpg", "jpeg", "png", "webp", "jfif"]
                .into_iter()
                .map(|ext| directory.join(format!("thumbnail_{code}.{ext}")))
                .chain(std::iter::once(directory.join("thumbnail.jpg")))
                .find(|p| p.is_file());
            let metadata = item.metadata().map_err(|e| e.to_string())?;
            entries.push(OutputEntry {
                video: path,
                thumbnail,
                modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                size: metadata.len(),
            });
        }
    }
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.modified));
    Ok((entries, used))
}

fn format_bytes(bytes: u64) -> String {
    let gib = 1024.0 * 1024.0 * 1024.0;
    if bytes as f64 >= gib {
        format!("{:.2} GB", bytes as f64 / gib)
    } else {
        format!("{:.2} MB", bytes as f64 / 1_048_576.0)
    }
}

fn batch_group<'a>(video: &'a Path, root: &Path) -> Option<&'a str> {
    video
        .parent()?
        .ancestors()
        .take_while(|path| path.starts_with(root))
        .filter_map(|path| path.file_name()?.to_str())
        .find(|name| name.starts_with("Batch_"))
}

fn batch_color(name: &str) -> egui::Color32 {
    const COLORS: [[u8; 3]; 6] = [
        [80, 160, 240],
        [164, 124, 240],
        [49, 184, 161],
        [227, 153, 66],
        [222, 115, 167],
        [89, 179, 203],
    ];
    let index = name.bytes().fold(0_usize, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as usize)
    }) % COLORS.len();
    let [r, g, b] = COLORS[index];
    egui::Color32::from_rgb(r, g, b)
}

#[cfg(windows)]
fn disk_free_bytes(path: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn GetDiskFreeSpaceExW(
            directory: *const u16,
            available: *mut u64,
            total: *mut u64,
            free: *mut u64,
        ) -> i32;
    }
    let mut existing = path;
    while !existing.exists() {
        existing = existing.parent()?;
    }
    let absolute = fs::canonicalize(existing).ok()?;
    let mut wide: Vec<u16> = absolute.as_os_str().encode_wide().collect();
    wide.push(0);
    let mut available = 0;
    // The buffer is NUL terminated, and every non-null output points to a u64.
    let success = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    (success != 0).then_some(available)
}

#[cfg(not(windows))]
fn disk_free_bytes(_path: &Path) -> Option<u64> {
    None
}

fn read_gallery_text(path: &Path) -> Result<String, String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > 2 * 1024 * 1024 {
        return Err("Berkas teks melebihi batas preview 2 MB.".into());
    }
    fs::read_to_string(path).map_err(|e| format!("Berkas tidak dapat dibaca: {e}"))
}

fn paint_gallery_thumbnail(ui: &egui::Ui, rect: egui::Rect, path: Option<&Path>) {
    let painter = ui.painter_at(rect);
    let palette = theme::Palette::of(ui.ctx());
    painter.rect_filled(rect, 5, palette.surface_alt);
    if let Some(path) = path {
        let uri = format!("file://{}", path.display());
        if let Ok(egui::load::TexturePoll::Ready { texture }) = ui.ctx().try_load_texture(
            &uri,
            egui::TextureOptions::LINEAR,
            egui::load::SizeHint::default(),
        ) {
            let scale = (rect.width() / texture.size.x).max(rect.height() / texture.size.y);
            let visible = rect.size() / (texture.size * scale);
            painter.image(
                texture.id,
                rect,
                egui::Rect::from_center_size(egui::pos2(0.5, 0.5), visible),
                egui::Color32::WHITE,
            );
            return;
        }
    }
    // Native placeholder also covers missing, loading and unreadable images.
    painter.circle_filled(rect.center(), 14.0, palette.accent.gamma_multiply(0.18));
    let c = rect.center();
    painter.add(egui::Shape::convex_polygon(
        vec![
            c + egui::vec2(-3.0, -6.0),
            c + egui::vec2(7.0, 0.0),
            c + egui::vec2(-3.0, 6.0),
        ],
        palette.accent,
        egui::Stroke::NONE,
    ));
    painter.rect_stroke(
        rect,
        5,
        egui::Stroke::new(1.0_f32, palette.border),
        egui::StrokeKind::Inside,
    );
}

fn delete_gallery_video(root: &Path, video: &Path) -> Result<(), String> {
    let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
    let resolved = fs::canonicalize(video).map_err(|e| e.to_string())?;
    let folder = resolved
        .parent()
        .ok_or_else(|| "Folder hasil tidak valid.".to_string())?;
    if !resolved.starts_with(&root)
        || folder == root
        || !folder.starts_with(&root)
        || !resolved
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("mp4"))
        || !fs::symlink_metadata(video)
            .map_err(|e| e.to_string())?
            .is_file()
    {
        return Err("Video harus berada di dalam folder output aktif.".into());
    }
    // Each render and Batch item owns a folder for its video and sidecar files.
    fs::remove_dir_all(folder).map_err(|e| e.to_string())
}

fn clear_gallery_output(root: &Path, protected: &Path) -> Result<usize, String> {
    let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
    let protected = fs::canonicalize(protected).map_err(|e| e.to_string())?;
    if root.parent().is_none() || protected.starts_with(&root) || !root.is_dir() {
        return Err("Folder output tidak boleh berupa akar disk atau folder aplikasi.".into());
    }
    if std::env::current_exe()
        .ok()
        .and_then(|exe| fs::canonicalize(exe).ok())
        .is_some_and(|exe| exe.starts_with(&root))
    {
        return Err("Folder output tidak boleh berisi aplikasi yang sedang berjalan.".into());
    }
    let mut pending = vec![root.clone()];
    let mut files = Vec::new();
    let mut directories = Vec::new();
    while let Some(directory) = pending.pop() {
        let resolved = fs::canonicalize(&directory).map_err(|e| e.to_string())?;
        if !resolved.starts_with(&root) {
            return Err("Subfolder berada di luar output aktif.".into());
        }
        for entry in fs::read_dir(&directory).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            if kind.is_dir() {
                let path = entry.path();
                directories.push(path.clone());
                pending.push(path);
            } else if kind.is_file() {
                files.push(entry.path());
            }
        }
    }
    let count = files.len();
    for file in files {
        let resolved = fs::canonicalize(&file).map_err(|e| e.to_string())?;
        if !resolved.starts_with(&root) {
            return Err("Berkas berada di luar output aktif.".into());
        }
        fs::remove_file(resolved).map_err(|e| e.to_string())?;
    }
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for directory in directories {
        let resolved = fs::canonicalize(&directory).map_err(|e| e.to_string())?;
        if resolved == root || !resolved.starts_with(&root) {
            return Err("Subfolder berada di luar output aktif.".into());
        }
        // Remove only now-empty directories, deepest first; keep the selected
        // output root itself intact.
        fs::remove_dir(resolved).map_err(|e| e.to_string())?;
    }
    Ok(count)
}

fn gallery_row_label(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    rect: egui::Rect,
    label: egui::Label,
) -> egui::Response {
    // The parent already reserved the full row. Keep label layout local.
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(id)
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.set_clip_rect(ui.clip_rect().intersect(rect));
    child.add(label)
}

impl StudioApp {
    pub(super) fn project_outputs_tab(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 8.0;
        theme::page_header(
            ui,
            "Galeri Render",
            "Kelola hasil render video dan thumbnail dari Generate dan Batch Render.",
        );
        ui.add_space(4.0);
        self.project_output_card(ui);
    }

    pub(super) fn project_output_card(&mut self, ui: &mut egui::Ui) {
        let source = self
            .generate
            .output_dir
            .clone()
            .unwrap_or_else(|| self.paths.outputs.clone());
        let refresh_due = self.project_outputs.source.as_ref() != Some(&source)
            || self
                .project_outputs
                .checked
                .is_none_or(|t| t.elapsed() > Duration::from_secs(5));
        if self
            .project_outputs
            .clearing
            .load(std::sync::atomic::Ordering::Acquire)
        {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        } else if refresh_due {
            self.refresh_project_outputs(&source);
        }
        let mut refresh = false;
        let mut open_error = None;
        let mut delete = None;
        let mut text_file = None;
        let can_delete = !self.job_running()
            && self.pending == 0
            && !self
                .project_outputs
                .clearing
                .load(std::sync::atomic::Ordering::Acquire);
        let mut clear_all = false;
        generate_card(ui, |ui| {
            let palette = theme::Palette::of(ui.ctx());
            let wide = ui.available_width() >= 780.0;
            let mut actions = |ui: &mut egui::Ui| {
                if ui
                    .add(theme::StyledButton::secondary("Buka Folder Output"))
                    .clicked()
                {
                    if let Err(e) = open_path(&source) {
                        open_error = Some(e.to_string());
                    }
                }
                refresh = ui
                    .add(theme::StyledButton::secondary("Refresh Hasil"))
                    .clicked();
                egui::Frame::new()
                    .fill(palette.accent.gamma_multiply(0.12))
                    .stroke(egui::Stroke::new(
                        1.0_f32,
                        palette.accent.gamma_multiply(0.4),
                    ))
                    .corner_radius(12)
                    .inner_margin(egui::Margin::symmetric(9, 4))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} video",
                                self.project_outputs.entries.len()
                            ))
                            .strong()
                            .color(palette.accent),
                        );
                    });
            };
            if wide {
                ui.horizontal(|ui| {
                    card_header(
                        ui,
                        GenerateIcon::Folder,
                        "Hasil Render",
                        "Video Generate dan Batch dari folder output aktif.",
                    );
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        &mut actions,
                    );
                });
            } else {
                card_header(
                    ui,
                    GenerateIcon::Folder,
                    "Hasil Render",
                    "Video Generate dan Batch dari folder output aktif.",
                );
                ui.add_space(6.0);
                ui.horizontal_wrapped(&mut actions);
            }
            ui.add_space(10.0);
            egui::Frame::new()
                .fill(palette.bg)
                .stroke(egui::Stroke::new(1.0_f32, palette.border))
                .corner_radius(8)
                .inner_margin(egui::Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Output")
                                .strong()
                                .color(palette.text_muted),
                        );
                        ui.add_space(6.0);
                        let usage = format!(
                            "{} output / {} sisa disk",
                            format_bytes(self.project_outputs.used_bytes),
                            self.project_outputs
                                .free_bytes
                                .map(format_bytes)
                                .unwrap_or_else(|| "—".into())
                        );
                        let reserved = ui.fonts(|fonts| {
                            fonts
                                .layout_no_wrap(
                                    usage.clone(),
                                    egui::FontId::proportional(12.0),
                                    palette.text_muted,
                                )
                                .size()
                                .x
                        }) + 48.0;
                        let path_width = (ui.available_width() - reserved).max(40.0);
                        ui.allocate_ui_with_layout(
                            egui::vec2(path_width, 18.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(source.display().to_string())
                                            .color(palette.text),
                                    )
                                    .truncate(),
                                )
                                .on_hover_text(source.display().to_string());
                            },
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if !self.project_outputs.entries.is_empty() {
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(26.0, 26.0),
                                    egui::Sense::hover(),
                                );
                                let response = ui.interact(
                                    rect,
                                    ui.id().with("clear_output"),
                                    if can_delete {
                                        egui::Sense::click()
                                    } else {
                                        egui::Sense::hover()
                                    },
                                );
                                if can_delete && response.hovered() {
                                    let fill = if response.is_pointer_button_down_on() {
                                        palette.danger_active
                                    } else {
                                        palette.danger_hover
                                    };
                                    ui.painter().rect_filled(rect, 6.0, fill);
                                    response
                                        .clone()
                                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                                }
                                super::generate_tab::draw_generate_icon(
                                    ui.painter(),
                                    rect.center(),
                                    GenerateIcon::Trash,
                                    if can_delete && response.hovered() {
                                        palette.on_danger
                                    } else if can_delete {
                                        palette.danger
                                    } else {
                                        palette.text_muted
                                    },
                                );
                                clear_all = response
                                    .on_hover_text("Hapus semua berkas output")
                                    .clicked();
                            }
                            ui.label(
                                egui::RichText::new(usage)
                                    .size(12.0)
                                    .color(palette.text_muted),
                            );
                        });
                    });
                });
            ui.add_space(6.0);
            if let Some(error) = &self.project_outputs.error {
                ui.colored_label(theme::Palette::of(ui.ctx()).danger, error);
            }
            if self.project_outputs.entries.is_empty() {
                ui.muted_label("Belum ada hasil render di folder ini. Pilih folder output yang benar melalui Settings.");
            }
            if !self.project_outputs.entries.is_empty() {
                egui::Frame::new()
                    .fill(palette.bg)
                    .stroke(egui::Stroke::new(1.0_f32, palette.border))
                    .corner_radius(8)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for (index, entry) in self.project_outputs.entries.iter().enumerate() {
                            let (row, response) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), 72.0),
                                egui::Sense::hover(),
                            );
                            if response.hovered() {
                                ui.painter().rect_filled(row, 0, palette.surface_alt);
                            }
                            let actions_left = row.right() - 144.0;
                            let image_rect = egui::Rect::from_min_size(
                                egui::pos2(row.left() + 42.0, row.top() + 9.0),
                                egui::vec2(96.0, 54.0),
                            );
                            paint_gallery_thumbnail(ui, image_rect, entry.thumbnail.as_deref());
                            if let Some(thumbnail) = &entry.thumbnail {
                                if ui
                                    .interact(
                                        image_rect,
                                        ui.id().with(("gallery_thumbnail", index)),
                                        egui::Sense::click(),
                                    )
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .on_hover_text("Buka thumbnail")
                                    .clicked()
                                {
                                    if let Err(error) = open_path(thumbnail) {
                                        open_error = Some(error.to_string());
                                    }
                                }
                            }
                            ui.painter().text(
                                egui::pos2(row.left() + 24.0, row.center().y),
                                egui::Align2::CENTER_CENTER,
                                (index + 1).to_string(),
                                egui::FontId::proportional(13.0),
                                palette.text_muted,
                            );
                            let title_rect = egui::Rect::from_min_max(
                                egui::pos2(row.left() + 150.0, row.top()),
                                egui::pos2(
                                    (actions_left - 86.0).max(row.left() + 152.0),
                                    if batch_group(&entry.video, &source).is_some() {
                                        row.top() + 35.0
                                    } else {
                                        row.bottom()
                                    },
                                ),
                            );
                            if let Some(group) = batch_group(&entry.video, &source) {
                                let color = batch_color(group);
                                ui.painter().rect_filled(
                                    egui::Rect::from_min_size(
                                        row.min,
                                        egui::vec2(3.0, row.height()),
                                    ),
                                    1,
                                    color,
                                );
                                let text = group.replace('_', " · ");
                                let width = ui
                                    .fonts(|fonts| {
                                        fonts
                                            .layout_no_wrap(
                                                text.clone(),
                                                egui::FontId::proportional(11.0),
                                                color,
                                            )
                                            .size()
                                            .x
                                            + 16.0
                                    })
                                    .min(title_rect.width());
                                let badge = egui::Rect::from_min_size(
                                    egui::pos2(title_rect.left(), row.top() + 38.0),
                                    egui::vec2(width, 22.0),
                                );
                                ui.painter()
                                    .rect_filled(badge, 6, color.gamma_multiply(0.12));
                                ui.painter().rect_stroke(
                                    badge,
                                    6,
                                    egui::Stroke::new(1.0_f32, color.gamma_multiply(0.4)),
                                    egui::StrokeKind::Inside,
                                );
                                gallery_row_label(
                                    ui,
                                    ("batch_badge", index),
                                    badge.shrink2(egui::vec2(7.0, 0.0)),
                                    egui::Label::new(
                                        egui::RichText::new(text).size(11.0).color(color),
                                    )
                                    .truncate(),
                                )
                                .on_hover_text(format!("Grup batch yang sama: {group}"));
                            }
                            gallery_row_label(
                                ui,
                                ("video_title", index),
                                title_rect,
                                egui::Label::new(
                                    egui::RichText::new(
                                        entry
                                            .video
                                            .file_name()
                                            .unwrap_or_default()
                                            .to_string_lossy(),
                                    )
                                    .color(palette.text_strong),
                                )
                                .truncate(),
                            )
                            .on_hover_text(entry.video.display().to_string());
                            ui.painter().text(
                                egui::pos2(actions_left - 12.0, row.center().y),
                                egui::Align2::RIGHT_CENTER,
                                format!("{:.1} MB", entry.size as f64 / 1_048_576.0),
                                egui::FontId::proportional(13.0),
                                palette.text_muted,
                            );
                            let mut target = None;
                            for (slot, icon, label) in [
                                (0, GenerateIcon::Play, "Buka Video"),
                                (1, GenerateIcon::Folder, "Buka Folder"),
                                (2, GenerateIcon::Music, "Buka Playlist"),
                                (3, GenerateIcon::Copy, "Buka Timestamp"),
                                (4, GenerateIcon::Trash, "Delete"),
                            ] {
                                let rect = egui::Rect::from_center_size(
                                    egui::pos2(
                                        actions_left + 14.0 + slot as f32 * 26.0,
                                        row.center().y,
                                    ),
                                    egui::vec2(24.0, 26.0),
                                );
                                if super::generate_tab::row_icon_button(
                                    ui,
                                    (label, index),
                                    rect,
                                    icon,
                                    if slot == 4 { can_delete } else { true },
                                    label,
                                ) {
                                    if slot == 4 {
                                        delete = Some(entry.video.clone());
                                        continue;
                                    }
                                    if slot == 2 || slot == 3 {
                                        if let Some(folder) = entry.video.parent() {
                                            text_file = Some((
                                                if slot == 2 { "Playlist" } else { "Timestamp" }
                                                    .to_owned(),
                                                folder.join(if slot == 2 {
                                                    "playlist.txt"
                                                } else {
                                                    "Timestamp.txt"
                                                }),
                                            ));
                                        }
                                        continue;
                                    }
                                    target = match slot {
                                        0 => Some(entry.video.as_path()),
                                        1 => entry.video.parent(),
                                        _ => entry.thumbnail.as_deref(),
                                    };
                                }
                            }
                            if let Some(target) = target {
                                if let Err(error) = open_path(target) {
                                    open_error = Some(error.to_string());
                                }
                            }
                            if index + 1 < self.project_outputs.entries.len() {
                                ui.painter().line_segment(
                                    [row.left_bottom(), row.right_bottom()],
                                    egui::Stroke::new(1.0_f32, palette.border),
                                );
                            }
                        }
                    });
            }
        });
        if let Some((title, path)) = text_file {
            self.project_outputs.text_preview = Some((
                title,
                read_gallery_text(&path).unwrap_or_else(|error| error),
            ));
        }
        if let Some((title, text)) = &mut self.project_outputs.text_preview {
            let mut open = true;
            egui::Window::new(title.clone())
                .id(egui::Id::new("gallery_text_preview"))
                .open(&mut open)
                .default_width(620.0)
                .default_height(400.0)
                .resizable(true)
                .show(ui.ctx(), |ui| {
                    if ui
                        .add(theme::StyledButton::secondary("Salin Teks"))
                        .clicked()
                    {
                        ui.ctx().copy_text(text.clone());
                    }
                    ui.separator();
                    egui::ScrollArea::both().show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(text)
                                .code_editor()
                                .desired_width(f32::INFINITY)
                                .interactive(false),
                        );
                    });
                });
            if !open {
                self.project_outputs.text_preview = None;
            }
        }
        if let Some(video) = delete {
            self.project_outputs.pending_delete = Some(video);
        }
        if clear_all {
            self.project_outputs.pending_clear = Some(source.clone());
        }
        let bulk = self.project_outputs.pending_clear.is_some();
        if let Some(video) = self
            .project_outputs
            .pending_clear
            .clone()
            .or_else(|| self.project_outputs.pending_delete.clone())
        {
            let mut confirm = false;
            let mut cancel = false;
            let p = theme::Palette::of(ui.ctx());
            let width = (ui.ctx().screen_rect().width() - 64.0).clamp(240.0, 440.0);
            egui::Modal::new(egui::Id::new("delete_gallery_video"))
                .frame(
                    egui::Frame::new()
                        .fill(p.surface)
                        .stroke(egui::Stroke::new(1.0_f32, p.border))
                        .corner_radius(14)
                        .inner_margin(24),
                )
                .show(ui.ctx(), |ui| {
                    ui.set_width(width);
                    ui.spacing_mut().item_spacing.y = 10.0;
                    ui.horizontal(|ui| {
                        let (icon, _) =
                            ui.allocate_exact_size(egui::vec2(42.0, 42.0), egui::Sense::hover());
                        ui.painter()
                            .rect_filled(icon, 12, p.danger.gamma_multiply(0.12));
                        super::generate_tab::draw_generate_icon(
                            ui.painter(),
                            icon.center(),
                            GenerateIcon::Trash,
                            p.danger,
                        );
                        ui.add_space(4.0);
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(if bulk { "Hapus semua hasil render?" } else { "Hapus hasil render ini?" })
                                    .size(18.0)
                                    .strong()
                                    .color(p.text_strong),
                            );
                            ui.muted_label("Konfirmasi penghapusan hasil render");
                        });
                    });
                    ui.add_space(4.0);
                    egui::Frame::new()
                        .fill(p.bg)
                        .stroke(egui::Stroke::new(1.0_f32, p.border))
                        .corner_radius(8)
                        .inner_margin(12)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(
                                egui::RichText::new(
                                    video.file_name().unwrap_or_default().to_string_lossy(),
                                )
                                .strong()
                                .color(p.text_strong),
                            );
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(
                                        video.parent().unwrap_or(&source).display().to_string(),
                                    )
                                    .size(12.0)
                                    .color(p.text_muted),
                                )
                                .truncate(),
                            )
                            .on_hover_text(video.display().to_string());
                        });
                    ui.label(
                        egui::RichText::new(
                            if bulk { "Semua berkas dalam folder output ini akan dihapus permanen dan tidak dapat dikembalikan." } else { "Folder hasil render ini beserta seluruh isinya akan dihapus permanen dan tidak dapat dikembalikan." },
                        )
                        .color(p.danger),
                    );
                    ui.muted_label(
                        if bulk { "Termasuk video, thumbnail, playlist, timestamp, laporan, dan seluruh berkas di subfolder batch." } else { "Mencakup video, thumbnail, playlist, timestamp, dan laporan render." },
                    );
                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        confirm = ui
                            .add_enabled(can_delete, theme::StyledButton::danger(if bulk { "Hapus Semua" } else { "Hapus Hasil" }))
                            .clicked();
                        cancel = ui.add(theme::StyledButton::secondary("Batal")).clicked();
                    });
                    cancel |= ui.input(|input| input.key_pressed(egui::Key::Escape));
                });
            if confirm {
                if bulk {
                    let protected = self.paths.data_dir.clone();
                    let flag = self.project_outputs.clearing.clone();
                    flag.store(true, std::sync::atomic::Ordering::Release);
                    self.spawn(async move {
                        let result = tokio::task::spawn_blocking(move || {
                            clear_gallery_output(&video, &protected)
                        })
                        .await;
                        flag.store(false, std::sync::atomic::Ordering::Release);
                        Event::Cleaned(match result {
                            Ok(Ok(count)) => format!("{count} berkas output dihapus."),
                            Ok(Err(error)) => format!("Penghapusan output terhenti: {error}"),
                            Err(error) => format!("Penghapusan output gagal: {error}"),
                        })
                    });
                    self.project_outputs.checked = None;
                    self.storage = None;
                } else {
                    match delete_gallery_video(&source, &video) {
                        Ok(()) => {
                            refresh = true;
                            self.storage = None;
                        }
                        Err(error) => open_error = Some(error),
                    }
                }
            }
            if confirm || cancel {
                self.project_outputs.pending_delete = None;
                self.project_outputs.pending_clear = None;
            }
        }
        if refresh {
            self.refresh_project_outputs(&source);
        }
        if let Some(error) = open_error {
            self.error(format!("Gagal membuka hasil: {error}"));
        }
    }

    fn refresh_project_outputs(&mut self, source: &Path) {
        self.project_outputs.source = Some(source.to_owned());
        self.project_outputs.checked = Some(Instant::now());
        match scan_outputs(source) {
            Ok((entries, used)) => {
                self.project_outputs.entries = entries;
                self.project_outputs.used_bytes = used;
                self.project_outputs.error = None;
            }
            Err(error) => {
                self.project_outputs.entries.clear();
                self.project_outputs.used_bytes = 0;
                self.project_outputs.error = Some(error);
            }
        }
        self.project_outputs.free_bytes = disk_free_bytes(source);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn bulk_delete_removes_nested_output_files_and_preserves_outside_files() {
        let base = std::env::temp_dir().join(format!("clear-gallery-{}", uuid::Uuid::new_v4()));
        let output = base.join("outputs");
        let nested = output.join("Batch_001/01_abc");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("video_abc.mp4"), b"video").unwrap();
        std::fs::write(nested.join("thumbnail_abc.jpg"), b"thumbnail").unwrap();
        let empty = output.join("folder-kosong/subfolder");
        std::fs::create_dir_all(&empty).unwrap();
        std::fs::write(base.join("keep.txt"), b"keep").unwrap();
        assert!(super::clear_gallery_output(&base, &base).is_err());
        assert_eq!(super::clear_gallery_output(&output, &base).unwrap(), 2);
        assert!(output.exists());
        assert_eq!(std::fs::read_dir(&output).unwrap().count(), 0);
        assert!(base.join("keep.txt").is_file());
        assert_eq!(super::scan_outputs(&output).unwrap().1, 0);
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn batch_markers_match_across_video_subfolders_and_exclude_other_outputs() {
        let root = std::path::Path::new("outputs");
        let first = std::path::Path::new("outputs/Batch_2026-11-18_001/01_abc/video_abc.mp4");
        let second = std::path::Path::new("outputs/Batch_2026-11-18_001/02_def/video_def.mp4");
        assert_eq!(
            super::batch_group(first, root),
            super::batch_group(second, root)
        );
        assert_eq!(
            super::batch_group(first, root),
            Some("Batch_2026-11-18_001")
        );
        assert!(
            super::batch_group(std::path::Path::new("outputs/20261118/video_xyz.mp4"), root)
                .is_none()
        );
        assert!(super::batch_group(first, std::path::Path::new("other-output")).is_none());
    }

    #[test]
    fn text_preview_reads_sidecars_and_bounds_large_files() {
        let root = std::env::temp_dir().join(format!("gallery-text-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let playlist = root.join("playlist.txt");
        std::fs::write(&playlist, "1. Lagu Pertama\n2. Lagu Kedua").unwrap();
        assert!(super::read_gallery_text(&playlist)
            .unwrap()
            .contains("Lagu Kedua"));
        assert!(super::read_gallery_text(&root.join("missing.txt")).is_err());
        std::fs::File::create(root.join("large.txt"))
            .unwrap()
            .set_len(2 * 1024 * 1024 + 1)
            .unwrap();
        assert!(super::read_gallery_text(&root.join("large.txt")).is_err());
        assert_eq!(super::format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(super::format_bytes(1024 * 1024 * 1024), "1.00 GB");
        #[cfg(windows)]
        assert!(super::disk_free_bytes(&root).is_some_and(|free| free > 0));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn delete_removes_the_render_folder_and_its_sidecars() {
        let root = std::env::temp_dir().join(format!("gallery-delete-{}", uuid::Uuid::new_v4()));
        let output = root.join("outputs");
        let render_folder = output.join("20261118_120000");
        std::fs::create_dir_all(&render_folder).unwrap();
        let video = render_folder.join("video_abc123.mp4");
        let thumbnail = render_folder.join("thumbnail_abc123.png");
        let playlist = render_folder.join("playlist.txt");
        let outside = root.join("original.mp4");
        for path in [&video, &thumbnail, &playlist, &outside] {
            std::fs::write(path, b"keep").unwrap();
        }
        assert!(super::delete_gallery_video(&output, &outside).is_err());
        assert!(super::delete_gallery_video(&output, &thumbnail).is_err());
        assert!(super::delete_gallery_video(&output, &video).is_ok());
        assert!(!render_folder.exists());
        assert!(outside.exists());
        std::fs::create_dir_all(&output).unwrap();
        let direct_video = output.join("video_direct.mp4");
        std::fs::write(&direct_video, b"keep").unwrap();
        assert!(super::delete_gallery_video(&output, &direct_video).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn finds_generate_and_nested_batch_outputs_with_matching_thumbnails() {
        let root = std::env::temp_dir().join(format!("project-outputs-{}", uuid::Uuid::new_v4()));
        for folder in ["20261118", "Batch_2026-11-18_001/01_abc123"] {
            let dir = root.join(folder);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("video_abc123.mp4"), b"video").unwrap();
            std::fs::write(dir.join("thumbnail_abc123.png"), b"thumbnail").unwrap();
            std::fs::write(dir.join("playlist.txt"), b"playlist").unwrap();
        }
        let unfinished = root.join("render-in-progress");
        std::fs::create_dir(&unfinished).unwrap();
        std::fs::write(unfinished.join("video_pending.mp4"), b"partial").unwrap();
        let (entries, used) = super::scan_outputs(&root).unwrap();
        assert!(used > 0);
        assert_eq!(entries.len(), 2);
        assert!(entries
            .iter()
            .all(|e| e.thumbnail.as_ref().unwrap().is_file()));
        std::fs::remove_dir_all(root).unwrap();
    }
}
