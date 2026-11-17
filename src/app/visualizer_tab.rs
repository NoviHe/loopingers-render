use eframe::egui::{self, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use loopkaraoke::{
    karaoke::ass::{PlaylistDisplayMode, TracklistPosition},
    visualizer::{ParticleType, TextSourceMode, VisualizerPosition, VisualizerPreset},
};

use super::{help::visualizer as help, theme, StudioApp};

const GRADIENT_PALETTES: [(&str, [u8; 3], [u8; 3]); 8] = [
    ("Neon NFC", [0, 246, 255], [255, 43, 214]),
    ("Neon Cyan", [0, 51, 68], [0, 229, 255]),
    ("Neon Magenta", [42, 0, 48], [255, 43, 214]),
    ("Galaxy", [124, 92, 255], [0, 229, 255]),
    ("NCS", [0, 246, 255], [255, 230, 0]),
    ("Api (Fire)", [255, 69, 0], [255, 215, 0]),
    ("Rainbow", [255, 0, 128], [0, 255, 128]),
    ("SteelBlue", [70, 130, 180], [176, 224, 230]),
];

#[derive(Debug, Clone)]
pub struct VisualizerParticle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub size: f32,
    pub alpha: f32,
    pub color: Color32,
    pub life: f32,
    pub max_life: f32,
}

impl VisualizerParticle {
    pub fn new_random(
        p_type: ParticleType,
        w: f32,
        h: f32,
        col_a: Color32,
        col_b: Color32,
    ) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        let x = rng.gen_range(0.0..=w);
        let y = rng.gen_range(0.0..=h);

        match p_type {
            ParticleType::Fire => {
                let size = rng.gen_range(2.0..=6.0);
                let vy = -rng.gen_range(1.5..=4.5);
                let vx = rng.gen_range(-0.8..=0.8);
                let color = if rng.gen_bool(0.6) {
                    Color32::from_rgb(255, rng.gen_range(100..=220), 0)
                } else {
                    Color32::from_rgb(255, rng.gen_range(40..=80), 0)
                };
                Self {
                    x,
                    y: h + rng.gen_range(0.0..=20.0),
                    vx,
                    vy,
                    size,
                    alpha: rng.gen_range(0.5..=1.0),
                    color,
                    life: 0.0,
                    max_life: rng.gen_range(1.5..=3.5),
                }
            }
            ParticleType::Snow => {
                let size = rng.gen_range(2.0..=5.0);
                let vy = rng.gen_range(0.6..=1.8);
                let vx = rng.gen_range(-0.4..=0.4);
                Self {
                    x,
                    y: rng.gen_range(-20.0..=0.0),
                    vx,
                    vy,
                    size,
                    alpha: rng.gen_range(0.4..=0.85),
                    color: Color32::from_white_alpha(240),
                    life: 0.0,
                    max_life: rng.gen_range(3.0..=6.0),
                }
            }
            ParticleType::NeonRain => {
                let size = rng.gen_range(2.0..=4.0);
                let vy = rng.gen_range(4.0..=8.0);
                let vx = rng.gen_range(-0.5..=0.5);
                let color = if rng.gen_bool(0.5) { col_a } else { col_b };
                Self {
                    x,
                    y: rng.gen_range(-40.0..=0.0),
                    vx,
                    vy,
                    size,
                    alpha: rng.gen_range(0.5..=0.9),
                    color,
                    life: 0.0,
                    max_life: rng.gen_range(1.2..=2.5),
                }
            }
            ParticleType::NcsRise => {
                let size = rng.gen_range(2.5..=6.5);
                let vy = -rng.gen_range(1.0..=3.2);
                let vx = rng.gen_range(-1.2..=1.2);
                let color = if rng.gen_bool(0.5) { col_a } else { col_b };
                Self {
                    x,
                    y: h * 0.7 + rng.gen_range(-20.0..=20.0),
                    vx,
                    vy,
                    size,
                    alpha: rng.gen_range(0.4..=0.95),
                    color,
                    life: 0.0,
                    max_life: rng.gen_range(2.0..=4.5),
                }
            }
            ParticleType::Sparkles => {
                let size = rng.gen_range(2.0..=5.5);
                let color = if rng.gen_bool(0.7) {
                    Color32::from_white_alpha(255)
                } else {
                    col_a
                };
                Self {
                    x,
                    y,
                    vx: rng.gen_range(-0.3..=0.3),
                    vy: rng.gen_range(-0.4..=0.4),
                    size,
                    alpha: rng.gen_range(0.3..=1.0),
                    color,
                    life: 0.0,
                    max_life: rng.gen_range(1.5..=4.0),
                }
            }
            _ => {
                // Dust
                let size = rng.gen_range(1.5..=3.5);
                Self {
                    x,
                    y,
                    vx: rng.gen_range(-0.2..=0.2),
                    vy: rng.gen_range(-0.5..=-0.1),
                    size,
                    alpha: rng.gen_range(0.2..=0.6),
                    color: Color32::from_gray(210),
                    life: 0.0,
                    max_life: rng.gen_range(2.5..=5.0),
                }
            }
        }
    }
}

impl StudioApp {
    pub(super) fn visualizer_tab(&mut self, ui: &mut egui::Ui) {
        // Continuous repaint when preview is playing or neon frame/particles are active
        if self.visualizer_playing
            || self.visualizer.bg_neon_frame
            || self.visualizer.particle != ParticleType::None
        {
            ui.ctx().request_repaint();
            if self.visualizer_playing {
                let dt = ui.input(|i| i.stable_dt).min(0.1) as f64;
                self.visualizer_time += dt;

                // Loop playback within selected track duration
                let max_dur = if !self.tracks.is_empty() {
                    self.tracks[self.visualizer_selected_track.min(self.tracks.len() - 1)].duration
                } else {
                    180.0
                };
                if self.visualizer_time > max_dur {
                    self.visualizer_time = 0.0;
                }
            }
        }

        theme::heading_with_info(ui, "Visualizer Musik", help::HEADING);
        ui.label("Studio Visualizer Musik reaktif audio (ala VIZ) dengan spektrum frekuensi, teks dinamis, logo, partikel, dan efek latar belakang.");
        ui.add_space(4.0);

        // Header / Audio Player Bar
        self.render_visualizer_player_bar(ui);
        ui.add_space(6.0);

        // 2 Columns: Sisi Kiri = Stage Pratinjau 16:9, Sisi Kanan = Panel Editor VIZ
        ui.columns(2, |columns| {
            // Kolom Kiri: Canvas Preview & Quick Action
            self.render_visualizer_preview(&mut columns[0]);

            // Kolom Kanan: Panel Editor (Viz, Teks, BG, Overlay, Partikel)
            self.render_visualizer_editor(&mut columns[1]);
        });
    }

    fn render_visualizer_player_bar(&mut self, ui: &mut egui::Ui) {
        let frame = egui::Frame::new()
            .fill(theme::PANEL_BG)
            .stroke(egui::Stroke::new(1.0_f32, theme::FIELD_BORDER))
            .corner_radius(6)
            .inner_margin(egui::Margin::same(10));

        frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                // Play / Pause Button
                let play_icon = if self.visualizer_playing {
                    "⏸ Jeda"
                } else {
                    "▶ Putar"
                };
                if ui.button(egui::RichText::new(play_icon).strong()).clicked() {
                    self.visualizer_playing = !self.visualizer_playing;
                }

                // Track Selector from Playlist
                if self.tracks.is_empty() {
                    ui.label("🎵 Lagu Contoh (Demo Mode • Belum ada lagu di Playlist)");
                } else {
                    let cur_idx = self.visualizer_selected_track.min(self.tracks.len() - 1);
                    let title = &self.tracks[cur_idx].title;
                    egui::ComboBox::from_id_salt("viz_track_select")
                        .selected_text(format!("🎵 {}", title))
                        .width(220.0)
                        .show_ui(ui, |ui| {
                            for (idx, trk) in self.tracks.iter().enumerate() {
                                if ui
                                    .selectable_label(
                                        self.visualizer_selected_track == idx,
                                        &trk.title,
                                    )
                                    .clicked()
                                {
                                    self.visualizer_selected_track = idx;
                                    self.visualizer_time = 0.0;
                                }
                            }
                        });
                }

                // Time Seek Slider
                let cur_time = self.visualizer_time;
                let max_dur = if !self.tracks.is_empty() {
                    self.tracks[self.visualizer_selected_track.min(self.tracks.len() - 1)].duration
                } else {
                    180.0
                };
                let cur_str = format!(
                    "{}:{:02}",
                    (cur_time / 60.0) as u32,
                    (cur_time % 60.0) as u32
                );
                let dur_str = format!("{}:{:02}", (max_dur / 60.0) as u32, (max_dur % 60.0) as u32);

                ui.label(format!("{cur_str} / {dur_str}"));
                let mut seek_val = cur_time;
                if ui
                    .add(egui::Slider::new(&mut seek_val, 0.0..=max_dur).show_value(false))
                    .changed()
                {
                    self.visualizer_time = seek_val;
                }

                // Volume
                ui.label("🔊");
                ui.add(
                    egui::Slider::new(&mut self.visualizer_volume, 0.0..=100.0)
                        .show_value(false)
                        .text("%"),
                );

                // Status Badge
                ui.separator();
                ui.colored_label(
                    if self.visualizer.enabled {
                        theme::SUCCESS
                    } else {
                        theme::TEXT_DIM
                    },
                    if self.visualizer.enabled {
                        "● Terpasang ke Render"
                    } else {
                        "○ Pratinjau Saja"
                    },
                );

                // Quick Toggle Enable for Render
                ui.checkbox(&mut self.visualizer.enabled, "Sertakan di Video");
            });
        });
    }

    fn render_visualizer_preview(&mut self, ui: &mut egui::Ui) {
        let frame = egui::Frame::new()
            .fill(theme::PANEL_BG)
            .stroke(egui::Stroke::new(1.0_f32, theme::FIELD_BORDER))
            .corner_radius(6)
            .inner_margin(egui::Margin::same(12));

        frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.strong("Stage Pratinjau Visualizer (16:9)");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button("🎬 Export Video Visualizer (.mp4)")
                        .on_hover_text("Render video visualizer audio MP4")
                        .clicked()
                    {
                        // Switch to render tab or trigger render
                        self.visualizer.enabled = true;
                        self.tab = super::Tab::Quick;
                    }
                });
            });
            ui.add_space(4.0);

            // 16:9 Preview Canvas
            self.draw_visualizer_canvas(ui);

            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("Bentuk: {}", self.visualizer.preset.label()));
                ui.separator();
                ui.label(format!("Posisi: {}", self.visualizer.position.label()));
                ui.separator();
                ui.label(format!("Ukuran: {:.0}%", self.visualizer.size));
                ui.separator();
                ui.label(format!("Sensitivitas: {:.0}%", self.visualizer.sensitivity));
            });
        });
    }

    fn draw_visualizer_canvas(&mut self, ui: &mut egui::Ui) {
        let avail_w = ui.available_width().max(280.0);
        let canvas_w = avail_w;
        let canvas_h = canvas_w * (9.0 / 16.0);

        let (rect, _response) =
            ui.allocate_exact_size(Vec2::new(canvas_w, canvas_h), egui::Sense::hover());

        // 1. Draw Background (Live playing video or static image)
        let preview_info = self.poll_background_texture(ui.ctx());
        let painter = ui.painter_at(rect);
        if let Some((tex_id, _, _, _)) = preview_info {
            let full_uv = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));
            painter.image(tex_id, rect, full_uv, Color32::WHITE);
        } else {
            // Cyber aesthetic dark background
            painter.rect_filled(rect, 4.0, Color32::from_rgb(10, 14, 22));
            // Subtle ambient glow
            painter.circle_filled(
                Pos2::new(rect.center().x, rect.center().y),
                canvas_h * 0.4,
                Color32::from_rgba_premultiplied(15, 35, 60, 40),
            );
        }

        // 2. Background Dimming
        if self.visualizer.bg_dim > 0.01 {
            let dim_alpha = (self.visualizer.bg_dim.clamp(0.0, 1.0) * 255.0) as u8;
            painter.rect_filled(rect, 4.0, Color32::from_black_alpha(dim_alpha));
        }

        // 3. Background Tint
        if self.visualizer.bg_tint_amount > 0.01 {
            let tc = self.visualizer.bg_tint_color;
            let tint_alpha = (self.visualizer.bg_tint_amount.clamp(0.0, 1.0) * 180.0) as u8;
            painter.rect_filled(
                rect,
                4.0,
                Color32::from_rgba_premultiplied(tc[0], tc[1], tc[2], tint_alpha),
            );
        }

        let t = self.visualizer_time as f32;
        let col_a = Color32::from_rgb(
            self.visualizer.color_a[0],
            self.visualizer.color_a[1],
            self.visualizer.color_a[2],
        );
        let col_b = Color32::from_rgb(
            self.visualizer.color_b[0],
            self.visualizer.color_b[1],
            self.visualizer.color_b[2],
        );

        // 4. Neon RGB Frame Border
        if self.visualizer.bg_neon_frame {
            let border_w = self.visualizer.bg_neon_width * (canvas_w / 1920.0).max(0.5);
            let hue = (t * 60.0) % 360.0;
            let rgb_frame_color = hsl_to_rgb(hue, 1.0, 0.55);
            painter.rect_stroke(
                rect.shrink(border_w / 2.0),
                4.0,
                Stroke::new(border_w, rgb_frame_color),
                egui::StrokeKind::Middle,
            );
        }

        // 4.5. Global Visual Effect Preview (Hujan, Salju, Sakura, VHS, dll.)
        self.draw_visual_effect_preview(&painter, rect, t);

        // 5. Particles Update & Render
        if self.visualizer.particle != ParticleType::None {
            self.update_and_render_particles(&painter, rect, col_a, col_b);
        }

        // 6. Audio Reactive Visualizer Spectrum Rendering
        self.draw_visualizer_spectrum(&painter, rect, t, col_a, col_b);

        // 7. Logo / Overlay Layer (if any)
        if self.visualizer.show_overlay {
            if let Some(path) = &self.visualizer.overlay_path {
                if path.is_file() {
                    let overlay_scale = (self.visualizer.overlay_size / 100.0).clamp(0.1, 1.0);
                    let mut logo_size = canvas_h * 0.45 * overlay_scale;
                    if self.visualizer.overlay_pulse_beat && self.visualizer_playing {
                        let beat = (t * 8.0).sin().abs() * 0.08;
                        logo_size *= 1.0 + beat;
                    }
                    let logo_rect = Rect::from_center_size(rect.center(), Vec2::splat(logo_size));
                    ui.put(
                        logo_rect,
                        egui::Image::new(format!("file://{}", path.display()))
                            .fit_to_exact_size(logo_rect.size())
                            .corner_radius(4),
                    );
                }
            }
        }

        // 8. Dynamic Text Layer
        if self.visualizer.show_text {
            let text = if self.visualizer.text_mode == TextSourceMode::Custom
                && !self.visualizer.text_custom.trim().is_empty()
            {
                self.visualizer.text_custom.clone()
            } else if !self.tracks.is_empty() {
                let cur = self.visualizer_selected_track.min(self.tracks.len() - 1);
                let trk = &self.tracks[cur];
                if trk.artist.is_empty() {
                    trk.title.clone()
                } else {
                    format!("{} - {}", trk.title, trk.artist)
                }
            } else {
                "Loopingers Audio Visualizer • VIZ Studio".to_string()
            };

            let font_sz = self.visualizer.text_size * (canvas_w / 1920.0).max(0.4);
            let font = FontId::proportional(font_sz.max(10.0));
            let text_color = Color32::from_rgb(
                self.visualizer.text_color[0],
                self.visualizer.text_color[1],
                self.visualizer.text_color[2],
            );

            let y_pos = match self.visualizer.text_position {
                1 => rect.center().y + canvas_h * 0.25, // Tengah bawah
                2 => rect.bottom() - canvas_h * 0.10,   // Bawah
                _ => rect.top() + canvas_h * 0.12,      // Atas
            };
            let text_pos = Pos2::new(rect.center().x, y_pos);

            // Glow / Shadow
            if self.visualizer.text_glow {
                let glow_col =
                    Color32::from_rgba_premultiplied(col_a.r(), col_a.g(), col_a.b(), 110);
                painter.text(
                    Pos2::new(text_pos.x + 1.5, text_pos.y + 1.5),
                    egui::Align2::CENTER_CENTER,
                    &text,
                    font.clone(),
                    glow_col,
                );
            }
            painter.text(
                text_pos,
                egui::Align2::CENTER_CENTER,
                &text,
                font,
                text_color,
            );
        }

        // 9. Visual Tracklist Overlay (Daftar Lagu di Layar)
        if self.playlist_mode.includes_visual()
            && self.show_tracklist_visual
            && !self.tracks.is_empty()
        {
            self.draw_tracklist_overlay(&painter, rect, col_a);
        }

        // 10. Karaoke Lyrics Preview on Stage (jika mode menyertakan Lirik Lagu)
        if self.playlist_mode.includes_lyrics() && !self.tracks.is_empty() {
            self.draw_lyrics_overlay(&painter, rect, col_a);
        }
    }

    pub(super) fn draw_tracklist_overlay(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        accent_col: Color32,
    ) {
        let total = self.tracks.len();
        let active_idx = self.visualizer_selected_track.min(total.saturating_sub(1));
        let max_items = 10;
        let (start_idx, end_idx) = if total <= max_items {
            (0, total)
        } else {
            let half = max_items / 2;
            let mut s = active_idx.saturating_sub(half);
            if s + max_items > total {
                s = total - max_items;
            }
            (s, s + max_items)
        };

        let scale = (rect.width() / 1920.0).max(0.38);
        let item_sz = (self.tracklist_font_size as f32 * scale).clamp(9.0, 22.0);
        let line_h = item_sz * 1.38;
        let pad_x = rect.width() * 0.03;
        let pad_y = rect.height() * 0.05;

        let row_count =
            (end_idx - start_idx) + usize::from(!self.tracklist_title.trim().is_empty());
        let block_h = row_count as f32 * line_h;

        let (base_x, mut cur_y, align) = match self.tracklist_position {
            TracklistPosition::TopLeft => (
                rect.left() + pad_x,
                rect.top() + pad_y,
                egui::Align2::LEFT_TOP,
            ),
            TracklistPosition::TopRight => (
                rect.right() - pad_x,
                rect.top() + pad_y,
                egui::Align2::RIGHT_TOP,
            ),
            TracklistPosition::MiddleLeft => (
                rect.left() + pad_x,
                rect.center().y - block_h * 0.5,
                egui::Align2::LEFT_TOP,
            ),
            TracklistPosition::MiddleRight => (
                rect.right() - pad_x,
                rect.center().y - block_h * 0.5,
                egui::Align2::RIGHT_TOP,
            ),
            TracklistPosition::Center => (
                rect.center().x,
                rect.center().y - block_h * 0.5,
                egui::Align2::CENTER_TOP,
            ),
        };

        let header_title = self.tracklist_title.trim();
        if !header_title.is_empty() {
            let hdr_font = FontId::proportional(item_sz * 1.1);
            let hdr_text = format!("🎵 {header_title}");
            painter.text(
                Pos2::new(base_x + 1.0, cur_y + 1.0),
                align,
                &hdr_text,
                hdr_font.clone(),
                Color32::from_black_alpha(180),
            );
            painter.text(
                Pos2::new(base_x, cur_y),
                align,
                &hdr_text,
                hdr_font,
                accent_col,
            );
            cur_y += line_h * 1.1;
        }

        for i in start_idx..end_idx {
            let trk = &self.tracks[i];
            let short_title = if trk.title.chars().count() > 30 {
                format!("{}...", trk.title.chars().take(27).collect::<String>())
            } else {
                trk.title.clone()
            };
            let is_active = i == active_idx;
            let line_text = if is_active {
                format!("▶ {:02}. {short_title}", i + 1)
            } else {
                format!("   {:02}. {short_title}", i + 1)
            };
            let font = FontId::proportional(if is_active { item_sz * 1.06 } else { item_sz });
            let col = if is_active {
                accent_col
            } else {
                Color32::from_white_alpha(190)
            };

            painter.text(
                Pos2::new(base_x + 1.0, cur_y + 1.0),
                align,
                &line_text,
                font.clone(),
                Color32::from_black_alpha(180),
            );
            painter.text(Pos2::new(base_x, cur_y), align, &line_text, font, col);
            cur_y += line_h;
        }
    }

    pub(super) fn draw_lyrics_overlay(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        accent_col: Color32,
    ) {
        let cur = self
            .visualizer_selected_track
            .min(self.tracks.len().saturating_sub(1));
        let track = &self.tracks[cur];
        if track.lyrics.is_empty() {
            return;
        }

        let t = self.visualizer_time;
        let active_line = track
            .lyrics
            .iter()
            .find(|l| t >= l.start && t <= l.end)
            .or_else(|| track.lyrics.iter().rfind(|l| l.start <= t))
            .or_else(|| track.lyrics.first());

        if let Some(line) = active_line {
            let scale = (rect.width() / 1920.0).max(0.4);
            let font_sz = (self.style.font_size as f32 * scale).clamp(11.0, 28.0);
            let font = FontId::proportional(font_sz);
            let y = rect.bottom() - rect.height() * 0.09;
            let pos = Pos2::new(rect.center().x, y);

            let singing_col = Color32::from_rgb(
                self.style.singing[0],
                self.style.singing[1],
                self.style.singing[2],
            );
            let draw_col = if self.visualizer_playing {
                singing_col
            } else {
                accent_col
            };

            painter.text(
                Pos2::new(pos.x + 1.5, pos.y + 1.5),
                egui::Align2::CENTER_CENTER,
                &line.text,
                font.clone(),
                Color32::from_black_alpha(200),
            );
            painter.text(pos, egui::Align2::CENTER_CENTER, &line.text, font, draw_col);
        }
    }

    pub(super) fn update_and_render_particles(
        &mut self,
        painter: &egui::Painter,
        rect: Rect,
        col_a: Color32,
        col_b: Color32,
    ) {
        let max_count = self.visualizer.particle_density as usize;
        let w = rect.width();
        let h = rect.height();

        // Spawn new particles if below density
        while self.visualizer_particles.len() < max_count {
            self.visualizer_particles
                .push(VisualizerParticle::new_random(
                    self.visualizer.particle,
                    w,
                    h,
                    col_a,
                    col_b,
                ));
        }

        let dt = 0.025; // Approx frame step
        let mut i = 0;
        while i < self.visualizer_particles.len() {
            let p = &mut self.visualizer_particles[i];
            p.x += p.vx;
            p.y += p.vy;
            p.life += dt;

            if p.life >= p.max_life
                || p.x < -20.0
                || p.x > w + 20.0
                || p.y < -30.0
                || p.y > h + 30.0
            {
                // Respawn particle
                self.visualizer_particles[i] =
                    VisualizerParticle::new_random(self.visualizer.particle, w, h, col_a, col_b);
            } else {
                let screen_pos = Pos2::new(rect.left() + p.x, rect.top() + p.y);
                let alpha_ratio = (1.0 - (p.life / p.max_life)).clamp(0.0, 1.0) * p.alpha;
                let draw_color = Color32::from_rgba_premultiplied(
                    (p.color.r() as f32 * alpha_ratio) as u8,
                    (p.color.g() as f32 * alpha_ratio) as u8,
                    (p.color.b() as f32 * alpha_ratio) as u8,
                    (255.0 * alpha_ratio) as u8,
                );

                painter.circle_filled(screen_pos, p.size, draw_color);
                i += 1;
            }
        }
    }

    pub(super) fn draw_visualizer_spectrum(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        t: f32,
        col_a: Color32,
        col_b: Color32,
    ) {
        let spec = &self.visualizer;
        let sens = spec.sensitivity / 60.0;
        let scale = (spec.size / 100.0).clamp(0.2, 1.0);
        let center = rect.center();

        match spec.preset {
            VisualizerPreset::RadialNcs | VisualizerPreset::RadialPulse => {
                let is_pulse = spec.preset == VisualizerPreset::RadialPulse;
                let num_bars = 48;
                let base_radius = rect.height() * 0.22 * scale;
                let spin_angle = t * 1.5 + (spec.rotation_deg.to_radians());

                // Inner circle ring
                let pulse_beat =
                    (t * 7.0).sin().abs() * 0.08 * if self.visualizer_playing { 1.0 } else { 0.0 };
                let current_radius = base_radius * (1.0 + pulse_beat);

                painter.circle_stroke(center, current_radius, Stroke::new(2.0_f32, col_a));
                if is_pulse {
                    painter.circle_stroke(
                        center,
                        current_radius * 0.7,
                        Stroke::new(1.0_f32, col_b),
                    );
                }

                for i in 0..num_bars {
                    let angle = spin_angle + (i as f32 * std::f32::consts::TAU / num_bars as f32);
                    let wave = ((t * 4.5 + i as f32 * 0.4).sin().abs() * 0.55
                        + (t * 2.2 + (i as f32 * 0.9).cos()).abs() * 0.45)
                        .powf(1.3)
                        * sens;
                    let spike_len = rect.height() * 0.18 * wave.clamp(0.08, 1.2) * scale;

                    let p1 = Pos2::new(
                        center.x + angle.cos() * current_radius,
                        center.y + angle.sin() * current_radius,
                    );
                    let p2 = Pos2::new(
                        center.x + angle.cos() * (current_radius + spike_len),
                        center.y + angle.sin() * (current_radius + spike_len),
                    );

                    let ratio = (i as f32 / num_bars as f32).clamp(0.0, 1.0);
                    let line_col = lerp_color(col_a, col_b, ratio);
                    painter.line_segment([p1, p2], Stroke::new(2.5 * scale, line_col));

                    // Peak dot
                    let dot_pos = Pos2::new(
                        center.x + angle.cos() * (current_radius + spike_len + 3.0),
                        center.y + angle.sin() * (current_radius + spike_len + 3.0),
                    );
                    painter.circle_filled(dot_pos, 1.5 * scale, Color32::WHITE);
                }
            }
            VisualizerPreset::BarsRound | VisualizerPreset::BarsDense => {
                let num_bars = if spec.preset == VisualizerPreset::BarsDense {
                    64
                } else {
                    36
                };
                let viz_width = rect.width() * 0.85 * scale;
                let start_x = center.x - viz_width / 2.0;
                let bar_width = (viz_width / num_bars as f32) * 0.75;
                let base_y = match spec.position {
                    VisualizerPosition::Bottom => rect.bottom() - rect.height() * 0.08,
                    VisualizerPosition::Center => center.y + rect.height() * 0.15 * scale,
                    VisualizerPosition::Full => rect.bottom() - rect.height() * 0.05,
                };
                let max_h = rect.height() * 0.40 * scale;

                for i in 0..num_bars {
                    let ratio = i as f32 / num_bars as f32;
                    let x = start_x + (i as f32 * viz_width / num_bars as f32);
                    let wave = ((t * 4.0 + i as f32 * 0.35).sin().abs() * 0.6
                        + (t * 2.0 + (i as f32 * 0.8).cos()).abs() * 0.4)
                        .powf(1.2)
                        * sens;
                    let h = (max_h * wave.clamp(0.05, 1.0)).max(3.0);

                    let bar_col = lerp_color(col_a, col_b, ratio);
                    let bar_rect =
                        Rect::from_min_size(Pos2::new(x, base_y - h), Vec2::new(bar_width, h));
                    painter.rect_filled(bar_rect, 2.5, bar_col);

                    // Peak marker
                    let peak_rect = Rect::from_min_size(
                        Pos2::new(x, base_y - h - 3.0),
                        Vec2::new(bar_width, 2.0),
                    );
                    painter.rect_filled(peak_rect, 1.0, Color32::WHITE);
                }
            }
            VisualizerPreset::LedClassic | VisualizerPreset::LedMirror => {
                let num_cols = 28;
                let leds_per_col = 14;
                let viz_width = rect.width() * 0.80 * scale;
                let start_x = center.x - viz_width / 2.0;
                let col_width = (viz_width / num_cols as f32) * 0.70;
                let base_y = match spec.position {
                    VisualizerPosition::Bottom => rect.bottom() - rect.height() * 0.08,
                    _ => center.y + rect.height() * 0.18 * scale,
                };
                let led_h = (rect.height() * 0.35 * scale) / leds_per_col as f32;

                for i in 0..num_cols {
                    let col_ratio = i as f32 / num_cols as f32;
                    let x = start_x + (i as f32 * viz_width / num_cols as f32);
                    let wave = ((t * 4.2 + (col_ratio * 3.0).sin() * 2.0).sin().abs() * 0.6
                        + (t * 1.8 + i as f32 * 0.5).cos().abs() * 0.4)
                        * sens;
                    let active_leds = (wave.clamp(0.05, 1.0) * leds_per_col as f32) as usize;

                    for l in 0..leds_per_col {
                        let y = base_y - (l as f32 * led_h);
                        let led_rect = Rect::from_min_size(
                            Pos2::new(x, y - led_h * 0.7),
                            Vec2::new(col_width, led_h * 0.7),
                        );
                        let is_on = l < active_leds;
                        let led_col = if is_on {
                            let led_ratio = l as f32 / leds_per_col as f32;
                            lerp_color(col_a, col_b, led_ratio)
                        } else {
                            Color32::from_rgba_premultiplied(35, 45, 55, 30)
                        };
                        painter.rect_filled(led_rect, 1.5, led_col);
                    }
                }
            }
            VisualizerPreset::WaveGraph => {
                let points_count = 60;
                let width_span = rect.width() * 0.88 * scale;
                let start_x = center.x - width_span / 2.0;
                let base_y = match spec.position {
                    VisualizerPosition::Bottom => rect.bottom() - rect.height() * 0.15,
                    _ => center.y,
                };

                let mut points = Vec::new();
                for i in 0..=points_count {
                    let x_ratio = i as f32 / points_count as f32;
                    let x = start_x + x_ratio * width_span;
                    let wave = (t * 4.0 + x_ratio * 8.0).sin() * 0.5
                        + (t * 2.5 + x_ratio * 16.0).cos() * 0.35
                        + (t * 6.0 + x_ratio * 4.0).sin() * 0.15;
                    let y = base_y - (wave * rect.height() * 0.15 * sens * scale);
                    points.push(Pos2::new(x, y));
                }

                for w in 0..points.len() - 1 {
                    let ratio = w as f32 / points.len() as f32;
                    let col = lerp_color(col_a, col_b, ratio);
                    painter.line_segment([points[w], points[w + 1]], Stroke::new(3.0 * scale, col));
                }
            }
            VisualizerPreset::SpiralGalaxy
            | VisualizerPreset::CyberGrid
            | VisualizerPreset::Starburst => {
                // Multi-arm spiral / starburst
                let num_arms = 6;
                let nodes_per_arm = 12;
                let spin = t * 2.0 + spec.rotation_deg.to_radians();

                for a in 0..num_arms {
                    let base_angle = spin + (a as f32 * std::f32::consts::TAU / num_arms as f32);
                    for n in 1..=nodes_per_arm {
                        let r = (n as f32 / nodes_per_arm as f32) * rect.height() * 0.32 * scale;
                        let angle = base_angle + (n as f32 * 0.25);
                        let x = center.x + angle.cos() * r;
                        let y = center.y + angle.sin() * r;
                        let node_ratio = n as f32 / nodes_per_arm as f32;
                        let node_col = lerp_color(col_a, col_b, node_ratio);
                        painter.circle_filled(
                            Pos2::new(x, y),
                            (2.0 + node_ratio * 3.0) * scale,
                            node_col,
                        );
                    }
                }
                painter.circle_filled(center, 4.0 * scale, Color32::WHITE);
            }
        }
    }

    fn render_visualizer_editor(&mut self, ui: &mut egui::Ui) {
        let frame = egui::Frame::new()
            .fill(theme::PANEL_BG)
            .stroke(egui::Stroke::new(1.0_f32, theme::FIELD_BORDER))
            .corner_radius(6)
            .inner_margin(egui::Margin::same(12));

        frame.show(ui, |ui| {
            ui.set_width(ui.available_width());

            // Tab bar ala VIZ (Viz, Teks, BG, Overlay, Partikel)
            ui.horizontal_wrapped(|ui| {
                let tabs = ["1. Viz", "2. Teks", "3. BG", "4. Overlay", "5. Partikel"];
                for (idx, name) in tabs.iter().enumerate() {
                    let is_active = self.visualizer_active_tab == idx;
                    if ui.selectable_label(is_active, *name).clicked() {
                        self.visualizer_active_tab = idx;
                    }
                }
            });
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            match self.visualizer_active_tab {
                0 => self.panel_viz_settings(ui),
                1 => self.panel_text_settings(ui),
                2 => self.panel_bg_settings(ui),
                3 => self.panel_overlay_settings(ui),
                _ => self.panel_particle_settings(ui),
            }
        });
    }

    fn panel_viz_settings(&mut self, ui: &mut egui::Ui) {
        ui.strong("Pengaturan Visualizer");
        ui.add_space(4.0);

        // Preset Dropdown
        theme::label_with_info(ui, "Bentuk Visualizer (Preset):", help::PRESET);
        egui::ComboBox::from_id_salt("viz_preset_select")
            .selected_text(self.visualizer.preset.label())
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for p in VisualizerPreset::ALL {
                    ui.selectable_value(&mut self.visualizer.preset, p, p.label());
                }
            });
        ui.add_space(6.0);

        // Quick Position Buttons
        theme::label_with_info(ui, "Posisi Visualizer:", help::POSITION);
        ui.horizontal_wrapped(|ui| {
            for pos in VisualizerPosition::ALL {
                if ui
                    .selectable_label(self.visualizer.position == pos, pos.label())
                    .clicked()
                {
                    self.visualizer.position = pos;
                }
            }
            if ui
                .button("Rata 0°")
                .on_hover_text("Reset sudut rotasi ke 0 derajat")
                .clicked()
            {
                self.visualizer.rotation_deg = 0.0;
            }
        });
        ui.add_space(6.0);

        // Sliders: Size, Sensitivity, Rotation
        theme::label_with_info(ui, "Ukuran (Scale):", help::SIZE);
        ui.add(egui::Slider::new(&mut self.visualizer.size, 20.0..=100.0).text("%"));

        theme::label_with_info(ui, "Sensitivitas Audio (Gain):", help::SENSITIVITY);
        ui.add(egui::Slider::new(&mut self.visualizer.sensitivity, 10.0..=100.0).text("%"));

        theme::label_with_info(ui, "Rotasi:", help::ROTATION);
        ui.add(egui::Slider::new(&mut self.visualizer.rotation_deg, 0.0..=360.0).text("°"));
        ui.add_space(8.0);

        // Colors
        theme::label_with_info(ui, "Warna Gradien Spektrum:", help::COLORS);
        ui.horizontal(|ui| {
            ui.label("Warna A:");
            ui.color_edit_button_srgb(&mut self.visualizer.color_a);
            ui.label("Warna B:");
            ui.color_edit_button_srgb(&mut self.visualizer.color_b);
        });
        ui.add_space(4.0);

        ui.label("Palet Cepat:");
        ui.horizontal_wrapped(|ui| {
            for (name, col_a, col_b) in GRADIENT_PALETTES {
                if ui.small_button(name).clicked() {
                    self.visualizer.color_a = col_a;
                    self.visualizer.color_b = col_b;
                }
            }
        });
    }

    fn panel_text_settings(&mut self, ui: &mut egui::Ui) {
        theme::label_with_info(ui, "Pengaturan Teks & Judul", help::TEXT);
        ui.add_space(4.0);

        ui.checkbox(
            &mut self.visualizer.show_text,
            "Tampilkan Teks pada Visualizer",
        );
        ui.add_space(4.0);

        if self.visualizer.show_text {
            ui.horizontal(|ui| {
                for mode in TextSourceMode::ALL {
                    ui.radio_value(&mut self.visualizer.text_mode, mode, mode.label());
                }
            });
            ui.add_space(4.0);

            if self.visualizer.text_mode == TextSourceMode::Custom {
                ui.label("Tulis Teks Kustom:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.visualizer.text_custom)
                        .hint_text("Contoh: Nightcore Beats • YouTube Channel")
                        .desired_width(f32::INFINITY),
                );
            } else {
                ui.colored_label(
                    theme::SUCCESS,
                    "✓ Judul dan nama artis diambil otomatis dari daftar Playlist lagu aktif.",
                );
            }
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                ui.label("Posisi:");
                ui.radio_value(&mut self.visualizer.text_position, 0, "Atas");
                ui.radio_value(&mut self.visualizer.text_position, 1, "Tengah");
                ui.radio_value(&mut self.visualizer.text_position, 2, "Bawah");
            });

            ui.horizontal(|ui| {
                ui.label("Warna Teks:");
                ui.color_edit_button_srgb(&mut self.visualizer.text_color);
                ui.checkbox(&mut self.visualizer.text_glow, "Efek Neon Glow");
            });

            ui.label("Ukuran Huruf:");
            ui.add(egui::Slider::new(
                &mut self.visualizer.text_size,
                16.0..=120.0,
            ));
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);
        ui.strong("📋 Mode Daftar Lagu & Tracklist Visual");
        ui.small("Pilih apakah lagu ditampilkan sebagai Visual, Lirik Lagu, atau Keduanya:");
        ui.horizontal_wrapped(|ui| {
            for mode in PlaylistDisplayMode::ALL {
                if ui
                    .selectable_label(self.playlist_mode == mode, mode.label())
                    .clicked()
                {
                    self.playlist_mode = mode;
                    if mode.includes_visual() && !self.show_tracklist_visual {
                        self.show_tracklist_visual = true;
                    }
                }
            }
        });

        if self.playlist_mode.includes_visual() {
            ui.add_space(4.0);
            ui.checkbox(
                &mut self.show_tracklist_visual,
                "Tampilkan Daftar Judul Lagu (Tracklist) di Layar Video",
            );
            if self.show_tracklist_visual {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Posisi Tracklist:");
                    for pos in TracklistPosition::ALL {
                        ui.selectable_value(&mut self.tracklist_position, pos, pos.label());
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Header:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.tracklist_title)
                            .hint_text("DAFTAR LAGU")
                            .desired_width(120.0),
                    );
                });
                ui.label("Ukuran Font Daftar Lagu:");
                ui.add(egui::Slider::new(&mut self.tracklist_font_size, 16..=48).text("px"));
            }
        }
    }

    fn panel_bg_settings(&mut self, ui: &mut egui::Ui) {
        ui.strong("Latar Belakang (Background) & Efek");
        ui.add_space(4.0);

        if let Some(bg) = &self.background {
            let name = bg.image.as_ref().and_then(|p| p.file_name()).map_or_else(
                || bg.video.filename.clone(),
                |n| n.to_string_lossy().to_string(),
            );
            ui.colored_label(theme::SUCCESS, format!("✓ Background Terhubung: {name}"));
        } else {
            ui.colored_label(
                theme::WARNING,
                "Belum ada background dipilih di tab Background / Main Menu.",
            );
        }
        ui.add_space(6.0);

        theme::label_with_info(ui, "Efek Visual:", help::BG_FX);
        ui.label("Gelap (Dim):");
        ui.add(
            egui::Slider::new(&mut self.visualizer.bg_dim, 0.0..=0.8)
                .text("%")
                .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
        );

        ui.label("Keburaman (Blur):");
        ui.add(egui::Slider::new(&mut self.visualizer.bg_blur, 0.0..=20.0));

        ui.horizontal(|ui| {
            ui.label("Warna Nuansa (Tint):");
            ui.color_edit_button_srgb(&mut self.visualizer.bg_tint_color);
            ui.add(
                egui::Slider::new(&mut self.visualizer.bg_tint_amount, 0.0..=0.7).text("jumlah"),
            );
        });
        ui.add_space(6.0);

        ui.checkbox(
            &mut self.visualizer.bg_pulse,
            "Getaran Ketukan Musik (Beat Pulse)",
        );
        ui.checkbox(
            &mut self.visualizer.bg_neon_frame,
            "Bingkai Neon RGB Berputar",
        );
        if self.visualizer.bg_neon_frame {
            ui.label("Ketebalan Bingkai Neon:");
            ui.add(egui::Slider::new(
                &mut self.visualizer.bg_neon_width,
                4.0..=28.0,
            ));
        }
    }

    fn panel_overlay_settings(&mut self, ui: &mut egui::Ui) {
        theme::label_with_info(ui, "Logo & Overlay Gambar", help::OVERLAY);
        ui.add_space(4.0);

        ui.checkbox(
            &mut self.visualizer.show_overlay,
            "Tampilkan Overlay / Logo",
        );
        ui.add_space(6.0);

        if self.visualizer.show_overlay {
            ui.horizontal(|ui| {
                if ui.button("📁 Pilih Logo / Overlay...").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Gambar", &["png", "jpg", "jpeg", "webp", "gif"])
                        .pick_file()
                    {
                        self.visualizer.overlay_path = Some(path);
                    }
                }
                if self.visualizer.overlay_path.is_some() && ui.button("Hapus").clicked() {
                    self.visualizer.overlay_path = None;
                }
            });

            if let Some(path) = &self.visualizer.overlay_path {
                ui.label(format!(
                    "Berkas: {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ));
            }
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                ui.label("Lapisan:");
                ui.radio_value(
                    &mut self.visualizer.overlay_depth_behind,
                    false,
                    "Depan Visualizer",
                );
                ui.radio_value(
                    &mut self.visualizer.overlay_depth_behind,
                    true,
                    "Belakang Visualizer",
                );
            });

            ui.checkbox(
                &mut self.visualizer.overlay_knockout_black,
                "Hapus Latar Hitam (Knockout Black)",
            );
            ui.checkbox(
                &mut self.visualizer.overlay_pulse_beat,
                "Getar Mengikuti Ketukan (Beat Pulse)",
            );

            ui.label("Ukuran Logo:");
            ui.add(egui::Slider::new(&mut self.visualizer.overlay_size, 10.0..=100.0).text("%"));

            ui.label("Opacity (Transparansi):");
            ui.add(egui::Slider::new(&mut self.visualizer.overlay_opacity, 10.0..=100.0).text("%"));
        }
    }

    fn panel_particle_settings(&mut self, ui: &mut egui::Ui) {
        ui.strong("Partikel & Efek Atmosfer");
        ui.add_space(4.0);

        theme::label_with_info(ui, "Jenis Partikel:", help::PARTICLES);
        egui::ComboBox::from_id_salt("viz_particle_select")
            .selected_text(self.visualizer.particle.label())
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for p in ParticleType::ALL {
                    if ui
                        .selectable_label(self.visualizer.particle == p, p.label())
                        .clicked()
                    {
                        self.visualizer.particle = p;
                        self.visualizer_particles.clear();
                    }
                }
            });
        ui.add_space(6.0);

        if self.visualizer.particle != ParticleType::None {
            ui.label("Kerapatan / Jumlah Partikel:");
            if ui
                .add(egui::Slider::new(
                    &mut self.visualizer.particle_density,
                    10..=150,
                ))
                .changed()
            {
                self.visualizer_particles.clear();
            }
        }
    }
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgb(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
    )
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> Color32 {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r, g, b) = match h as i32 {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    Color32::from_rgb(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}
