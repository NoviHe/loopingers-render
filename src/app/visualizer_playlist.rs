use eframe::egui::{
    self, Align, Color32, FontId, Layout, Pos2, Rect, Response, Sense, Stroke, UiBuilder,
};

use super::{
    generate_tab::{
        accent_badge_colors, card_header, format_track_duration, generate_card,
        show_volume_slider_popup, square_icon_button, GenerateIcon,
    },
    lyrics,
    theme::{self, UiText},
    StudioApp,
};

const TRANSPORT_SIZE: f32 = 32.0;
const TIMELINE_THUMB_RADIUS: f32 = 5.0;

#[derive(Clone, Copy)]
pub(super) enum TransportIcon {
    Play,
    Pause,
    Back,
    Forward,
    Stop,
}

impl StudioApp {
    pub(super) fn update_album_preview(&mut self, ui: &egui::Ui) {
        self.sync_visualizer_selection();
        if self.visualizer_playing {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(16));
            let dt = ui.input(|i| i.stable_dt).min(0.1) as f64;
            self.visualizer_time += dt;

            let max_dur = self.visualizer_track_duration();
            if self.visualizer_time >= max_dur {
                self.visualizer_time = max_dur;
                self.stop_visualizer_playback(false);
            }
        }
    }

    pub(super) fn album_preview_playlist_card(&mut self, ui: &mut egui::Ui) {
        let karaoke = self.tab == super::Tab::Karaoke;
        let rows: Vec<_> = self
            .generate
            .tracks
            .iter()
            .map(|track| (track.id, track.title.clone(), track.duration))
            .collect();
        let has_tracks = !rows.is_empty();
        let selected = self
            .visualizer_selected_track
            .min(rows.len().saturating_sub(1));
        let duration = self.visualizer_track_duration().max(0.0);
        let loading = self.visualizer_loading_preview;
        let playing = self.visualizer_playing;
        let mut select_action = None;
        let mut toggle_play = false;
        let mut seek_action = None;
        let mut stop = false;
        let p = theme::Palette::of(ui.ctx());
        let maximized = ui
            .ctx()
            .input(|input| input.viewport().maximized.unwrap_or(false));
        if !has_tracks {
            self.visualizer_volume_popup_open = false;
        }
        let was_volume_open = self.visualizer_volume_popup_open;
        let mut speaker_rect = None;
        let mut speaker_clicked = false;

        generate_card(ui, |ui| {
            ui.horizontal(|ui| {
                let lrc_status = if karaoke {
                    let count = self
                        .generate
                        .tracks
                        .iter()
                        .filter(|track| track.lrc.is_some())
                        .count();
                    format!("{count} dari {} lagu memiliki LRC", rows.len())
                } else {
                    "Sinkron dengan Generate dan Batch Render.".into()
                };
                card_header(
                    ui,
                    GenerateIcon::Music,
                    if karaoke {
                        "Lirik Album"
                    } else {
                        "Playlist Album"
                    },
                    &lrc_status,
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let volume = self.visualizer_volume.round() as u32;
                    let icon = if volume == 0 {
                        GenerateIcon::SpeakerMuted
                    } else {
                        GenerateIcon::Speaker
                    };
                    let tooltip = if was_volume_open {
                        String::new()
                    } else {
                        format!("Atur volume preview ({volume}%)")
                    };
                    let speaker =
                        square_icon_button(ui, icon, has_tracks, was_volume_open, &tooltip);
                    speaker_rect = Some(speaker.rect);
                    speaker_clicked = speaker.clicked();

                    let total = rows
                        .iter()
                        .map(|(_, _, duration)| duration.max(0.0))
                        .sum::<f64>();
                    let (fill, border, foreground) = accent_badge_colors(ui.ctx(), &p);
                    egui::Frame::new()
                        .fill(fill)
                        .stroke(Stroke::new(1.0_f32, border))
                        .corner_radius(13)
                        .inner_margin(egui::Margin::symmetric(8, 4))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} lagu · {}",
                                    rows.len(),
                                    format_track_duration(total.round() as u64)
                                ))
                                .size(theme::metrics::FONT_SM)
                                .color(foreground)
                                .strong(),
                            );
                        });
                });
            });
            ui.add_space(8.0);
            if has_tracks {
                egui::Frame::new()
                    .fill(p.bg)
                    .stroke(Stroke::new(1.0_f32, p.border))
                    .corner_radius(theme::metrics::CONTROL_RADIUS)
                    .inner_margin(egui::Margin::symmetric(8, 2))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        egui::ScrollArea::vertical()
                            .id_salt("visualizer_playlist_rows")
                            .max_height(if maximized { 204.0 } else { 38.0 * 3.0 })
                            .scroll_source(if karaoke && maximized {
                                egui::containers::scroll_area::ScrollSource::SCROLL_BAR
                            } else {
                                egui::containers::scroll_area::ScrollSource::ALL
                            })
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing.y = 0.0;
                                for (index, (_, title, duration)) in rows.iter().enumerate() {
                                    let lrc_status = karaoke.then(|| {
                                        self.generate.tracks[index]
                                            .lrc
                                            .as_ref()
                                            .map(|(_, lines)| *lines)
                                    });
                                    if playlist_row(
                                        ui,
                                        index,
                                        title,
                                        *duration,
                                        selected == index,
                                        lrc_status,
                                    ) {
                                        select_action = Some((index, false));
                                    }
                                }
                            });
                    });
            } else {
                ui.muted_label(
                    "Tambahkan lagu melalui Playlist atau Generate untuk memutar preview album.",
                );
            }
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let current_loading = rows
                    .get(selected)
                    .is_some_and(|(id, _, _)| loading == Some(*id));
                if transport_button(
                    ui,
                    if playing {
                        TransportIcon::Pause
                    } else if current_loading {
                        TransportIcon::Stop
                    } else {
                        TransportIcon::Play
                    },
                    has_tracks,
                    if current_loading {
                        "Batal putar preview"
                    } else if playing {
                        "Jeda preview"
                    } else {
                        "Putar preview"
                    },
                )
                .clicked()
                {
                    toggle_play = true;
                }
                if transport_button(ui, TransportIcon::Back, has_tracks, "Mundur 3 detik").clicked()
                {
                    seek_action = Some(self.visualizer_time - 3.0);
                }
                if transport_button(ui, TransportIcon::Forward, has_tracks, "Maju 3 detik")
                    .clicked()
                {
                    seek_action = Some(self.visualizer_time + 3.0);
                }
                if transport_button(
                    ui,
                    TransportIcon::Stop,
                    has_tracks,
                    "Hentikan dan kembali ke 0:00",
                )
                .clicked()
                {
                    stop = true;
                }
                if current_loading {
                    ui.spinner();
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.monospace(
                        egui::RichText::new(format!(
                            "{} / {}",
                            preview_clock(self.visualizer_time),
                            preview_clock(duration)
                        ))
                        .color(p.accent),
                    );
                });
            });
            let mut time = self.visualizer_time;
            if preview_timeline(ui, &mut time, duration, has_tracks).changed() {
                seek_action = Some(time);
            }
        });
        if speaker_clicked {
            self.visualizer_volume_popup_open = !self.visualizer_volume_popup_open;
        }
        if self.visualizer_volume_popup_open {
            if let Some(rect) = speaker_rect {
                let changed = show_volume_slider_popup(
                    ui,
                    rect,
                    was_volume_open && !speaker_clicked,
                    &mut self.visualizer_volume_popup_open,
                    &mut self.visualizer_volume,
                );
                if changed && self.visualizer_playing {
                    lyrics::win_audio::set_volume(self.visualizer_volume);
                }
            }
        }
        if let Some((index, play)) = select_action {
            self.select_visualizer_track(index, play);
        }
        if toggle_play {
            self.toggle_visualizer_playback();
        }
        if let Some(time) = seek_action {
            self.seek_visualizer(time);
        }
        if stop {
            self.stop_visualizer_playback(true);
        }
    }
}

fn playlist_row(
    ui: &mut egui::Ui,
    index: usize,
    title: &str,
    duration: f64,
    selected: bool,
    lrc_status: Option<Option<usize>>,
) -> bool {
    let p = theme::Palette::of(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 38.0), Sense::hover());
    let row_response = ui.interact(
        rect,
        ui.make_persistent_id(("visualizer_playlist_select", index)),
        Sense::click(),
    );
    if selected || row_response.hovered() {
        ui.painter()
            .rect_filled(rect, 4, p.surface_alt.gamma_multiply(0.45));
    }
    let cy = rect.center().y;
    ui.painter().text(
        egui::pos2(rect.left() + 18.0, cy),
        egui::Align2::CENTER_CENTER,
        format!("{}", index + 1),
        FontId::proportional(theme::metrics::FONT_SM),
        p.text_muted,
    );
    let duration = format_track_duration(duration.max(0.0).round() as u64);
    let duration_galley = ui.fonts(|fonts| {
        fonts.layout_no_wrap(
            duration,
            FontId::proportional(theme::metrics::FONT_SM),
            p.text_muted,
        )
    });
    let duration_x = rect.right() - duration_galley.size().x - 12.0;
    ui.painter().galley(
        egui::pos2(duration_x, cy - duration_galley.size().y / 2.0),
        duration_galley,
        p.text_muted,
    );
    let mut title_right = duration_x - 10.0;
    if let Some(lines) = lrc_status {
        let status_left = (title_right - 120.0).max(rect.left() + 34.0);
        if status_left < title_right {
            let status_rect = Rect::from_min_max(
                egui::pos2(status_left, rect.top()),
                egui::pos2(title_right, rect.bottom()),
            );
            let mut status_ui = ui.new_child(
                UiBuilder::new()
                    .id_salt(("karaoke_lrc_status", index))
                    .max_rect(status_rect)
                    .layout(Layout::right_to_left(Align::Center)),
            );
            status_ui.set_clip_rect(ui.clip_rect().intersect(status_rect));
            if let Some(lines) = lines {
                status_ui.small(egui::RichText::new(format!("{lines} baris")).color(p.text_muted));
                super::playlist::draw_lrc_badge(&mut status_ui, lines);
            } else {
                status_ui.small(egui::RichText::new("Belum ada LRC").color(p.text_muted));
            }
            title_right = status_left - 6.0;
        }
    }
    let title_rect = Rect::from_min_max(
        egui::pos2(rect.left() + 34.0, rect.top()),
        egui::pos2(title_right.max(rect.left() + 34.0), rect.bottom()),
    );
    let mut title_ui = ui.new_child(
        UiBuilder::new()
            .id_salt(("visualizer_playlist_title", index))
            .max_rect(title_rect)
            .layout(Layout::left_to_right(Align::Center)),
    );
    title_ui
        .add(
            egui::Label::new(egui::RichText::new(title).color(if selected {
                p.accent_hover
            } else {
                p.text_strong
            }))
            .truncate(),
        )
        .on_hover_text(title);
    row_response
        .on_hover_text("Pilih lagu untuk preview")
        .clicked()
}

pub(super) fn transport_button(
    ui: &mut egui::Ui,
    icon: TransportIcon,
    enabled: bool,
    tooltip: &str,
) -> Response {
    let response = ui
        .add_enabled_ui(enabled, |ui| {
            ui.add_sized(
                [TRANSPORT_SIZE, TRANSPORT_SIZE],
                theme::StyledButton::secondary(""),
            )
        })
        .inner;
    let p = theme::Palette::of(ui.ctx());
    paint_transport_icon(
        ui.painter(),
        response.rect.center(),
        icon,
        if enabled {
            p.text_strong
        } else {
            p.text_muted.gamma_multiply(0.5)
        },
    );
    response.on_hover_text(tooltip)
}

fn paint_transport_icon(
    painter: &egui::Painter,
    center: Pos2,
    icon: TransportIcon,
    color: Color32,
) {
    let stroke = Stroke::new(1.5_f32, color);
    let triangle = |x: f32, forward: bool| {
        let d = if forward { 1.0 } else { -1.0 };
        painter.add(egui::Shape::convex_polygon(
            vec![
                center + egui::vec2(x - 3.0 * d, -5.0),
                center + egui::vec2(x + 4.0 * d, 0.0),
                center + egui::vec2(x - 3.0 * d, 5.0),
            ],
            color,
            Stroke::NONE,
        ));
    };
    match icon {
        TransportIcon::Play => triangle(0.0, true),
        TransportIcon::Pause => {
            for x in [-3.0, 3.0] {
                painter.line_segment(
                    [center + egui::vec2(x, -5.0), center + egui::vec2(x, 5.0)],
                    stroke,
                );
            }
        }
        TransportIcon::Back => {
            triangle(-3.0, false);
            triangle(3.0, false);
        }
        TransportIcon::Forward => {
            triangle(-3.0, true);
            triangle(3.0, true);
        }
        TransportIcon::Stop => {
            painter.rect_filled(
                Rect::from_center_size(center, egui::vec2(9.0, 9.0)),
                1,
                color,
            );
        }
    }
}

fn timeline_value(pointer_x: f32, rail: Rect, duration: f64) -> f64 {
    if !duration.is_finite() || duration <= 0.0 || rail.width() <= 0.0 {
        return 0.0;
    }
    ((pointer_x - rail.left()) / rail.width()).clamp(0.0, 1.0) as f64 * duration
}

pub(super) fn preview_timeline(
    ui: &mut egui::Ui,
    time: &mut f64,
    duration: f64,
    enabled: bool,
) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), TRANSPORT_SIZE),
        if enabled && duration > 0.0 {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        },
    );
    let duration = if duration.is_finite() {
        duration.max(0.0)
    } else {
        0.0
    };
    let rail = Rect::from_min_max(
        egui::pos2(rect.left() + TIMELINE_THUMB_RADIUS, rect.center().y - 2.0),
        egui::pos2(
            (rect.right() - TIMELINE_THUMB_RADIUS).max(rect.left() + TIMELINE_THUMB_RADIUS),
            rect.center().y + 2.0,
        ),
    );
    let previous = *time;
    if enabled && duration > 0.0 {
        if response.clicked() || response.dragged() {
            response.request_focus();
            if let Some(pointer) = response.interact_pointer_pos() {
                *time = timeline_value(pointer.x, rail, duration);
            }
        }
        if response.has_focus() {
            ui.input(|input| {
                if input.key_pressed(egui::Key::ArrowLeft) {
                    *time -= 3.0;
                }
                if input.key_pressed(egui::Key::ArrowRight) {
                    *time += 3.0;
                }
                if input.key_pressed(egui::Key::Home) {
                    *time = 0.0;
                }
                if input.key_pressed(egui::Key::End) {
                    *time = duration;
                }
            });
        }
        *time = time.clamp(0.0, duration);
        if *time != previous {
            response.mark_changed();
        }
    }
    let p = theme::Palette::of(ui.ctx());
    ui.painter().rect_filled(rail, 2, p.bg);
    let fraction = if duration > 0.0 {
        (*time / duration).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    let x = rail.left() + rail.width() * fraction;
    let accent = if enabled {
        p.accent
    } else {
        p.text_muted.gamma_multiply(0.35)
    };
    ui.painter().rect_filled(
        Rect::from_min_max(rail.min, egui::pos2(x, rail.bottom())),
        2,
        accent,
    );
    ui.painter().circle_filled(
        egui::pos2(x, rail.center().y),
        TIMELINE_THUMB_RADIUS,
        accent,
    );
    response.on_hover_text(format!(
        "{} / {}",
        preview_clock(*time),
        preview_clock(duration)
    ))
}

fn preview_clock(time: f64) -> String {
    let total = (time.max(0.0) * 100.0).round() as u64;
    format!(
        "{:02}:{:02}.{:02}",
        total / 6000,
        (total / 100) % 60,
        total % 100
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeline_keeps_large_hitbox_with_small_thumb_and_supports_keyboard_seek() {
        let ctx = egui::Context::default();
        let mut time = 5.0;
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let response = preview_timeline(ui, &mut time, 120.0, true);
                assert_eq!(response.rect.height(), 32.0);
                response.request_focus();
            });
        });
        assert!(output.shapes.iter().any(
            |shape| matches!(&shape.shape, egui::Shape::Circle(circle) if circle.radius == 5.0)
        ));
        let input = egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::ArrowRight,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::default(),
            }],
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let response = preview_timeline(ui, &mut time, 120.0, true);
                assert!(response.changed());
            });
        });
        assert_eq!(time, 8.0);
    }

    #[test]
    fn timeline_maps_pointer_to_track_time_and_clamps_outside_rail() {
        let rail = Rect::from_min_max(egui::pos2(10.0, 0.0), egui::pos2(110.0, 4.0));
        assert_eq!(timeline_value(60.0, rail, 120.0), 60.0);
        assert_eq!(timeline_value(-10.0, rail, 120.0), 0.0);
        assert_eq!(timeline_value(150.0, rail, 120.0), 120.0);
        assert_eq!(timeline_value(60.0, rail, 0.0), 0.0);
        assert_eq!(timeline_value(60.0, rail, f64::INFINITY), 0.0);
    }
}
