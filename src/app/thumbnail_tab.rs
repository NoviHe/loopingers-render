use std::path::PathBuf;

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Vec2};
use loopkaraoke::{
    thumbnail::{self, Align, ThumbnailSpec},
    util::{open_path, random_id},
};

use super::{help::thumbnail as help, hex, theme, Event, LogLevel, StudioApp};

const COLUMN_MODES: [(&str, &str); 3] = [("auto", "Otomatis"), ("1", "1 kolom"), ("2", "2 kolom")];

pub struct FontChoice {
    pub id: &'static str,
    pub label: &'static str,
    pub filename: &'static str,
}

pub const TITLE_FONTS: [FontChoice; 8] = [
    FontChoice {
        id: "impact",
        label: "Impact (Standar Tebal YouTube)",
        filename: "impact.ttf",
    },
    FontChoice {
        id: "ariblk",
        label: "Arial Black (Ultra Bold)",
        filename: "ariblk.ttf",
    },
    FontChoice {
        id: "bahnschrift",
        label: "Bahnschrift (Modern DIN)",
        filename: "bahnschrift.ttf",
    },
    FontChoice {
        id: "segoeuib",
        label: "Segoe UI Bold (Elegan)",
        filename: "segoeuib.ttf",
    },
    FontChoice {
        id: "trebucbd",
        label: "Trebuchet MS Bold",
        filename: "trebucbd.ttf",
    },
    FontChoice {
        id: "georgiab",
        label: "Georgia Bold (Serif Klasik)",
        filename: "georgiab.ttf",
    },
    FontChoice {
        id: "comicbd",
        label: "Comic Sans Bold",
        filename: "comicbd.ttf",
    },
    FontChoice {
        id: "inter-semibold",
        label: "Inter Bold (Bawaan Aplikasi)",
        filename: "",
    },
];

pub const SUBTITLE_FONTS: [FontChoice; 8] = [
    FontChoice {
        id: "segoesc",
        label: "Segoe Script (Kaligrafi / Cursive)",
        filename: "segoesc.ttf",
    },
    FontChoice {
        id: "ariali",
        label: "Arial Italic",
        filename: "ariali.ttf",
    },
    FontChoice {
        id: "georgiab",
        label: "Georgia Italic (Serif)",
        filename: "georgiai.ttf",
    },
    FontChoice {
        id: "impact",
        label: "Impact",
        filename: "impact.ttf",
    },
    FontChoice {
        id: "segoeuib",
        label: "Segoe UI",
        filename: "segoeui.ttf",
    },
    FontChoice {
        id: "bahnschrift",
        label: "Bahnschrift",
        filename: "bahnschrift.ttf",
    },
    FontChoice {
        id: "comicbd",
        label: "Comic Sans",
        filename: "comic.ttf",
    },
    FontChoice {
        id: "inter",
        label: "Inter Regular (Bawaan Aplikasi)",
        filename: "",
    },
];

pub const DEMO_TRACKS: [&str; 19] = [
    "He Who Reigns Majestic In All The Earth",
    "Grace Like The Dawn",
    "Dawn Arrived",
    "The Dawn Has Arrived",
    "Child Of Gracious Light",
    "Anchor Of My Soul Morning Praise",
    "Glory To Your Name",
    "New Mercies At The Breaking Day",
    "Dawn Of Faithful Love",
    "Upon This Truth I Stand",
    "I Speak Jesus Morning Praise",
    "Promise Keeper Light In Darkness",
    "Awakened",
    "My Song Of Praise Will Rise",
    "The Love That Found My Heart",
    "The Dawn Has Arrived (Reprise)",
    "My Soul Will Sing Morning Praise",
    "Beauty Of This Dawn",
    "Light Over The Mountain Range",
];

pub struct ThumbnailState {
    pub title: String,
    pub subtitle: String,
    pub timestamp: f64,
    pub column_mode: usize,
    pub mirrored_numbers: bool,
    pub title_size: u32,
    pub track_size: u32,
    pub subtitle_size: u32,
    pub vignette: f32,
    pub title_color: [u8; 3],
    pub track_color: [u8; 3],
    pub subtitle_color: [u8; 3],
    pub title_y_pct: f32,
    pub subtitle_y_pct: f32,
    pub title_font_preset: usize,
    pub title_custom_font: Option<PathBuf>,
    pub subtitle_font_preset: usize,
    pub subtitle_custom_font: Option<PathBuf>,
    pub preview_frame: Option<PathBuf>,
    pub extracting_frame: bool,
    pub busy: bool,
    pub result: Option<PathBuf>,
}

impl Default for ThumbnailState {
    fn default() -> Self {
        Self {
            title: "Powerful Worship".to_owned(),
            subtitle: "Hillsong Worship".to_owned(),
            timestamp: 0.0,
            column_mode: 0,
            mirrored_numbers: true,
            title_size: 120,
            track_size: 36,
            subtitle_size: 80,
            vignette: 0.55,
            title_color: [255, 255, 255],
            track_color: [255, 255, 255],
            subtitle_color: [255, 215, 0], // Gold #FFD700
            title_y_pct: 0.055,
            subtitle_y_pct: 0.82,
            title_font_preset: 0, // Impact
            title_custom_font: None,
            subtitle_font_preset: 0, // Segoe Script
            subtitle_custom_font: None,
            preview_frame: None,
            extracting_frame: false,
            busy: false,
            result: None,
        }
    }
}

impl ThumbnailState {
    pub fn reset_worship_preset(&mut self) {
        self.title = "Powerful Worship".to_owned();
        self.subtitle = "Hillsong Worship".to_owned();
        self.column_mode = 0;
        self.mirrored_numbers = true;
        self.title_size = 120;
        self.track_size = 36;
        self.subtitle_size = 80;
        self.vignette = 0.55;
        self.title_color = [255, 255, 255];
        self.track_color = [255, 255, 255];
        self.subtitle_color = [255, 215, 0];
        self.title_y_pct = 0.055;
        self.subtitle_y_pct = 0.82;
        self.title_font_preset = 0;
        self.title_custom_font = None;
        self.subtitle_font_preset = 0;
        self.subtitle_custom_font = None;
    }
}

pub fn resolve_font_path(custom: &Option<PathBuf>, choice: &FontChoice) -> Option<PathBuf> {
    if let Some(path) = custom {
        if path.is_file() {
            return Some(path.clone());
        }
    }
    if choice.filename.is_empty() {
        return None;
    }
    let windows_root =
        std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
    let candidate = windows_root.join("Fonts").join(choice.filename);
    if candidate.is_file() {
        Some(candidate)
    } else {
        None
    }
}

impl StudioApp {
    pub(super) fn reload_egui_fonts(&self) {
        let mut extras: Vec<(&str, Vec<u8>)> = Vec::new();
        if let Some(path) = &self.thumbnail.title_custom_font {
            if let Ok(bytes) = std::fs::read(path) {
                extras.push(("title_custom", bytes));
            }
        }
        if let Some(path) = &self.thumbnail.subtitle_custom_font {
            if let Ok(bytes) = std::fs::read(path) {
                extras.push(("subtitle_custom", bytes));
            }
        }
        super::register_custom_fonts(&self.ctx, &extras);
    }

    pub(super) fn thumbnail_tab(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.horizontal_wrapped(|ui| {
                theme::heading_with_info(
                    ui,
                    "Thumbnail YouTube — Editor & Pratinjau Interaktif",
                    help::HEADING,
                );
                ui.add_space(8.0);
                if ui
                    .button("✨ Muat Contoh Seperti Gambar (Preset Worship)")
                    .on_hover_text("Terapkan pengaturan font, warna gold, susunan 2 kolom cermin, dan vignette seperti gambar referensi")
                    .clicked()
                {
                    self.thumbnail.reset_worship_preset();
                    self.reload_egui_fonts();
                    self.info("Preset Worship berhasil dimuat.");
                }
            });
            ui.weak(
                "Rancang thumbnail YouTube berkualitas 1920x1080. Pratinjau di sisi kanan berubah otomatis secara langsung saat formulir diubah.",
            );
        });
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);

        ui.columns(2, |columns| {
            self.thumbnail_fields(&mut columns[0]);
            self.thumbnail_output(&mut columns[1]);
        });
    }

    fn thumbnail_fields(&mut self, ui: &mut egui::Ui) {
        let frame = egui::Frame::new()
            .fill(theme::PANEL_BG)
            .stroke(egui::Stroke::new(1.0_f32, theme::FIELD_BORDER))
            .corner_radius(6)
            .inner_margin(egui::Margin::same(10));

        let mut trigger_font_reload = false;

        // Card 1: Teks & Tipografi
        frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.strong("1. Teks, Tipografi & Font");
            ui.add_space(4.0);
            let state = &mut self.thumbnail;

            theme::label_with_info(ui, "Judul Utama:", help::TITLE);
            ui.add(
                egui::TextEdit::singleline(&mut state.title)
                    .hint_text("Misal: Powerful Worship")
                    .desired_width(f32::INFINITY),
            );

            // Font Selector Judul Utama
            ui.horizontal(|ui| {
                ui.label("Font Judul:");
                let current_title_label = if let Some(path) = &state.title_custom_font {
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                } else {
                    TITLE_FONTS[state.title_font_preset].label.to_string()
                };

                egui::ComboBox::from_id_salt("title_font_select")
                    .selected_text(current_title_label)
                    .show_ui(ui, |ui| {
                        for (idx, font) in TITLE_FONTS.iter().enumerate() {
                            if ui
                                .selectable_label(
                                    state.title_custom_font.is_none()
                                        && state.title_font_preset == idx,
                                    font.label,
                                )
                                .clicked()
                            {
                                state.title_font_preset = idx;
                                state.title_custom_font = None;
                                trigger_font_reload = true;
                            }
                        }
                    });

                if ui
                    .button("📁 Unggah Font...")
                    .on_hover_text("Pilih berkas font (.ttf / .otf) dari komputer Anda")
                    .clicked()
                {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Font Files (*.ttf, *.otf)", &["ttf", "otf", "TTF", "OTF"])
                        .set_title("Pilih Berkas Font Judul Utama")
                        .pick_file()
                    {
                        state.title_custom_font = Some(path);
                        trigger_font_reload = true;
                    }
                }

                if state.title_custom_font.is_some()
                    && ui.small_button("✕ Reset ke Default").clicked()
                {
                    state.title_custom_font = None;
                    trigger_font_reload = true;
                }
            });

            ui.horizontal(|ui| {
                theme::label_with_info(ui, "Ukuran:", help::TITLE_SIZE);
                ui.add(egui::Slider::new(&mut state.title_size, 40..=240));
                ui.label("Posisi Y:");
                ui.add(egui::Slider::new(&mut state.title_y_pct, 0.01..=0.30).text("%"));
                theme::label_with_info(ui, "Warna:", help::COLORS);
                ui.color_edit_button_srgb(&mut state.title_color);
            });
            ui.add_space(8.0);

            theme::label_with_info(ui, "Sub Judul / Artis / Branding:", help::SUBTITLE);
            ui.add(
                egui::TextEdit::singleline(&mut state.subtitle)
                    .hint_text("Misal: Hillsong Worship")
                    .desired_width(f32::INFINITY),
            );

            // Font Selector Sub Judul
            ui.horizontal(|ui| {
                ui.label("Font Sub Judul:");
                let current_sub_label = if let Some(path) = &state.subtitle_custom_font {
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                } else {
                    SUBTITLE_FONTS[state.subtitle_font_preset].label.to_string()
                };

                egui::ComboBox::from_id_salt("subtitle_font_select")
                    .selected_text(current_sub_label)
                    .show_ui(ui, |ui| {
                        for (idx, font) in SUBTITLE_FONTS.iter().enumerate() {
                            if ui
                                .selectable_label(
                                    state.subtitle_custom_font.is_none()
                                        && state.subtitle_font_preset == idx,
                                    font.label,
                                )
                                .clicked()
                            {
                                state.subtitle_font_preset = idx;
                                state.subtitle_custom_font = None;
                                trigger_font_reload = true;
                            }
                        }
                    });

                if ui
                    .button("📁 Unggah Font...")
                    .on_hover_text("Pilih berkas font (.ttf / .otf) dari komputer Anda")
                    .clicked()
                {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Font Files (*.ttf, *.otf)", &["ttf", "otf", "TTF", "OTF"])
                        .set_title("Pilih Berkas Font Sub Judul")
                        .pick_file()
                    {
                        state.subtitle_custom_font = Some(path);
                        trigger_font_reload = true;
                    }
                }

                if state.subtitle_custom_font.is_some()
                    && ui.small_button("✕ Reset ke Default").clicked()
                {
                    state.subtitle_custom_font = None;
                    trigger_font_reload = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Ukuran:");
                ui.add(egui::Slider::new(&mut state.subtitle_size, 20..=160));
                ui.label("Posisi Y:");
                ui.add(egui::Slider::new(&mut state.subtitle_y_pct, 0.65..=0.95).text("%"));
                ui.label("Warna:");
                ui.color_edit_button_srgb(&mut state.subtitle_color);
                ui.small("(Disarankan warna emas/kuning)");
            });
        });
        ui.add_space(8.0);

        if trigger_font_reload {
            self.reload_egui_fonts();
        }

        // Card 2: Tata Letak Daftar Lagu (Tracklist)
        frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.strong("2. Tata Letak Daftar Lagu (Tracklist)");
            ui.add_space(4.0);
            let state = &mut self.thumbnail;
            ui.horizontal(|ui| {
                theme::label_with_info(ui, "Mode Kolom:", help::COLUMNS);
                egui::ComboBox::from_id_salt("thumbnail_columns_mode")
                    .selected_text(COLUMN_MODES[state.column_mode].1)
                    .show_ui(ui, |ui| {
                        for (index, (_, label)) in COLUMN_MODES.iter().enumerate() {
                            ui.selectable_value(&mut state.column_mode, index, *label);
                        }
                    });
            });
            ui.add_space(3.0);

            ui.horizontal(|ui| {
                theme::label_with_info(ui, "Nomor Kolom Kanan:", help::NUMBERING);
                ui.radio_value(&mut state.mirrored_numbers, true, "Cermin (Judul .01)");
                ui.radio_value(&mut state.mirrored_numbers, false, "Berurutan (11. Judul)");
            });
            ui.add_space(3.0);

            ui.horizontal(|ui| {
                theme::label_with_info(ui, "Ukuran Lagu:", help::TRACK_SIZE);
                ui.add(egui::Slider::new(&mut state.track_size, 16..=64));
                ui.label("Warna:");
                ui.color_edit_button_srgb(&mut state.track_color);
            });
            ui.add_space(4.0);

            if self.tracks.is_empty() {
                ui.colored_label(
                    theme::FOCUS,
                    "ℹ️ Belum ada lagu di playlist; menampilkan 19 lagu contoh (demo) seperti pada gambar referensi.",
                );
            } else {
                ui.colored_label(
                    theme::SUCCESS,
                    format!("✓ Menggunakan {} lagu dari playlist Anda saat ini.", self.tracks.len()),
                );
            }
        });
        ui.add_space(8.0);

        // Card 3: Latar Belakang & Vignette (Limit Sesuai Durasi Video)
        frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.strong("3. Latar Belakang & Efek Visual");
            ui.add_space(4.0);

            let bg_info = self.background.as_ref().map(|bg| {
                (
                    bg.image.as_ref().and_then(|p| p.file_name()).map(|f| f.to_string_lossy().to_string()),
                    bg.video.filename.clone(),
                    bg.video.duration,
                )
            });

            let mut trigger_extract = false;
            if let Some((img_name, vid_name, vid_duration)) = bg_info {
                if let Some(name) = img_name {
                    ui.label(format!("🖼️ Gambar latar: {name}"));
                } else {
                    ui.label(format!("🎬 Video latar: {vid_name}"));
                    let max_dur = vid_duration.max(0.1);
                    self.thumbnail.timestamp = self.thumbnail.timestamp.clamp(0.0, max_dur);

                    ui.horizontal(|ui| {
                        theme::label_with_info(ui, "Ambil frame detik:", help::FRAME);
                        ui.add(
                            egui::Slider::new(&mut self.thumbnail.timestamp, 0.0..=max_dur)
                                .custom_formatter(|val, _| format!("{val:.1}s / {max_dur:.1}s"))
                                .text("detik"),
                        );
                        let extracting = self.thumbnail.extracting_frame;
                        let btn = if extracting {
                            egui::Button::new("⏳ Mengambil...")
                        } else {
                            egui::Button::new("📸 Ambil Frame Video")
                        };
                        if ui.add_enabled(!extracting, btn).clicked() {
                            trigger_extract = true;
                        }
                    });
                    ui.small(format!("⏱️ Batas waktu frame sesuai durasi video: 0.0s – {:.1}s", max_dur));
                }
            } else {
                ui.weak("Belum ada video/gambar background dipilih di Main Menu.");
                ui.small("Menggunakan gradasi pemandangan gunung simulasi untuk pratinjau.");
            }

            if trigger_extract {
                self.extract_thumbnail_frame();
            }

            ui.add_space(4.0);

            ui.horizontal(|ui| {
                theme::label_with_info(ui, "Gelap di Tepi (Vignette):", help::VIGNETTE);
                ui.add(egui::Slider::new(&mut self.thumbnail.vignette, 0.0..=1.0));
            });
            ui.small("Membantu teks putih dan emas terlihat kontras dan terbaca jelas pada latar terang.");
        });
        ui.add_space(8.0);

        // Card 4: Ekspor Thumbnail
        frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.strong("4. Simpan & Ekspor Berkas");
            ui.add_space(4.0);

            let busy = self.thumbnail.busy;
            let btn = if busy {
                egui::Button::new("⏳ Sedang Merender Thumbnail...")
            } else {
                theme::primary_button("🚀 Buat Thumbnail Full HD (1920x1080)")
            };
            ui.horizontal(|ui| {
                if ui.add_enabled(!busy, btn).clicked() {
                    self.generate_thumbnail();
                }
                theme::info(ui, help::GENERATE);
            });
            ui.add_space(2.0);
            ui.small(
                "Disimpan di folder output dalam resolusi 1920x1080 Full HD (standar YouTube).",
            );
            if self.auto_generate_thumbnail {
                ui.colored_label(
                    theme::SUCCESS,
                    "✓ 'Buat Thumbnail Otomatis' juga aktif saat merender video dari Main Menu.",
                );
            }
        });
    }

    fn thumbnail_output(&mut self, ui: &mut egui::Ui) {
        let frame = egui::Frame::new()
            .fill(theme::FIELD_BG)
            .stroke(egui::Stroke::new(1.0_f32, theme::FIELD_BORDER))
            .corner_radius(6)
            .inner_margin(egui::Margin::same(12));

        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("Pratinjau Langsung (Canvas 16:9)");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new("1920x1080 Full HD")
                            .size(11.0)
                            .color(theme::FOCUS),
                    );
                });
            });
            ui.add_space(6.0);

            // Live Preview Canvas
            self.render_thumbnail_canvas(ui);

            ui.add_space(8.0);
            ui.small(
                "💡 Pratinjau interaktif real-time: teks, font kustom, ukuran font, tata letak cermin, dan vignette otomatis sinkron.",
            );

            if let Some(path) = self.thumbnail.result.clone() {
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);
                ui.colored_label(theme::SUCCESS, "✓ Berkas Thumbnail Berhasil Dibuat:");
                ui.label(path.display().to_string());
                ui.horizontal(|ui| {
                    if ui.button("🖼️ Buka Gambar").clicked() {
                        let _ = open_path(&path);
                    }
                    if ui.button("📁 Buka Folder Output").clicked() {
                        let _ = open_path(&self.paths.outputs);
                    }
                });
            }
        });
    }

    fn render_thumbnail_canvas(&mut self, ui: &mut egui::Ui) {
        let avail_w = ui.available_width().max(280.0);
        let canvas_w = avail_w;
        let canvas_h = canvas_w * (9.0 / 16.0);
        let scale = canvas_w / 1920.0;

        let (rect, _response) =
            ui.allocate_exact_size(Vec2::new(canvas_w, canvas_h), egui::Sense::hover());

        // Background source
        let bg_path = if let Some(frame) = &self.thumbnail.preview_frame {
            if frame.is_file() {
                Some(frame.clone())
            } else {
                None
            }
        } else if let Some(bg) = &self.background {
            if let Some(img) = &bg.image {
                if img.is_file() {
                    Some(img.clone())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some(path) = bg_path {
            ui.put(
                rect,
                egui::Image::new(format!("file://{}", path.display()))
                    .fit_to_exact_size(rect.size())
                    .corner_radius(4),
            );
        } else {
            // Draw scenic mountain sunset placeholder matching uploaded sample
            draw_scenic_placeholder(&ui.painter_at(rect), rect);
        }

        let painter = ui.painter_at(rect);

        // Vignette overlay
        draw_vignette(&painter, rect, self.thumbnail.vignette);

        // Gather track titles
        let track_titles: Vec<String> = if self.tracks.is_empty() {
            DEMO_TRACKS.iter().map(|&s| s.to_string()).collect()
        } else {
            self.tracks
                .iter()
                .map(|track| track.title.clone())
                .collect()
        };

        let state = &self.thumbnail;
        let title_font_path = resolve_font_path(
            &state.title_custom_font,
            &TITLE_FONTS[state.title_font_preset],
        );
        let subtitle_font_path = resolve_font_path(
            &state.subtitle_custom_font,
            &SUBTITLE_FONTS[state.subtitle_font_preset],
        );

        let spec = ThumbnailSpec {
            title: state.title.clone(),
            subtitle: state.subtitle.clone(),
            track_titles,
            column_mode: COLUMN_MODES[state.column_mode].0.to_owned(),
            number_style: if state.mirrored_numbers {
                "mirrored"
            } else {
                "sequential"
            }
            .to_owned(),
            title_font_size: state.title_size,
            track_font_size: state.track_size,
            subtitle_font_size: state.subtitle_size,
            vignette_opacity: state.vignette,
            title_color: hex(state.title_color),
            track_color: hex(state.track_color),
            subtitle_color: hex(state.subtitle_color),
            title_y_pct: state.title_y_pct,
            subtitle_y_pct: state.subtitle_y_pct,
            title_font_path,
            subtitle_font_path,
        };

        // Determine egui font families for live preview
        let title_font_family = if state.title_custom_font.is_some() {
            egui::FontFamily::Name("title_custom".into())
        } else {
            let choice_id = TITLE_FONTS[state.title_font_preset].id;
            egui::FontFamily::Name(choice_id.into())
        };

        let subtitle_font_family = if state.subtitle_custom_font.is_some() {
            egui::FontFamily::Name("subtitle_custom".into())
        } else {
            let choice_id = SUBTITLE_FONTS[state.subtitle_font_preset].id;
            egui::FontFamily::Name(choice_id.into())
        };

        // Draw Title (Top Centered)
        if !state.title.trim().is_empty() {
            let title_scale = (state.title_size as f32 * scale).max(14.0);
            let title_y = rect.min.y + (1080.0 * state.title_y_pct) * scale;
            let title_pos = Pos2::new(rect.center().x, title_y);
            let title_color = Color32::from_rgb(
                state.title_color[0],
                state.title_color[1],
                state.title_color[2],
            );
            let title_font = FontId::new(title_scale, title_font_family);
            draw_outlined_text(
                &painter,
                title_pos,
                egui::Align2::CENTER_TOP,
                state.title.trim(),
                title_font,
                title_color,
                (4.0 * scale).max(1.2),
            );
        }

        // Draw Track Rows (Left and Right columns)
        let rows = thumbnail::layout_tracks(&spec);
        let track_scale = (state.track_size as f32 * scale).max(8.0);
        let track_color = Color32::from_rgb(
            state.track_color[0],
            state.track_color[1],
            state.track_color[2],
        );
        let track_font = FontId::new(track_scale, egui::FontFamily::Name(theme::SEMIBOLD.into()));
        let outline_thickness = (2.4 * scale).max(1.0);

        for row in rows {
            let px = rect.min.x + (row.x as f32) * scale;
            let py = rect.min.y + (row.y as f32) * scale;
            let align2 = match row.align {
                Align::Left => egui::Align2::LEFT_TOP,
                Align::Right => egui::Align2::RIGHT_TOP,
                Align::Center => egui::Align2::CENTER_TOP,
            };
            draw_outlined_text(
                &painter,
                Pos2::new(px, py),
                align2,
                &row.text,
                track_font.clone(),
                track_color,
                outline_thickness,
            );
        }

        // Draw Subtitle (Bottom Centered)
        if !state.subtitle.trim().is_empty() {
            let sub_scale = (state.subtitle_size as f32 * scale).max(10.0);
            let sub_y = rect.min.y + (1080.0 * state.subtitle_y_pct) * scale;
            let sub_pos = Pos2::new(rect.center().x, sub_y);
            let sub_color = Color32::from_rgb(
                state.subtitle_color[0],
                state.subtitle_color[1],
                state.subtitle_color[2],
            );
            let sub_font = FontId::new(sub_scale, subtitle_font_family);
            draw_outlined_text(
                &painter,
                sub_pos,
                egui::Align2::CENTER_TOP,
                state.subtitle.trim(),
                sub_font,
                sub_color,
                (3.0 * scale).max(1.0),
            );
        }
    }

    pub(super) fn generate_thumbnail(&mut self) {
        let Some(background) = &self.background else {
            self.error("Pilih video atau gambar background terlebih dahulu.");
            return;
        };
        let is_video = background.image.is_none();
        let source = background
            .image
            .clone()
            .unwrap_or_else(|| background.video.path.clone().into());
        let state = &self.thumbnail;
        let track_titles: Vec<String> = if self.tracks.is_empty() {
            DEMO_TRACKS.iter().map(|&s| s.to_string()).collect()
        } else {
            self.tracks
                .iter()
                .map(|track| track.title.clone())
                .collect()
        };

        let title_font_path = resolve_font_path(
            &state.title_custom_font,
            &TITLE_FONTS[state.title_font_preset],
        );
        let subtitle_font_path = resolve_font_path(
            &state.subtitle_custom_font,
            &SUBTITLE_FONTS[state.subtitle_font_preset],
        );

        let spec = ThumbnailSpec {
            title: state.title.clone(),
            subtitle: state.subtitle.clone(),
            track_titles,
            column_mode: COLUMN_MODES[state.column_mode].0.to_owned(),
            number_style: if state.mirrored_numbers {
                "mirrored"
            } else {
                "sequential"
            }
            .to_owned(),
            title_font_size: state.title_size,
            track_font_size: state.track_size,
            subtitle_font_size: state.subtitle_size,
            vignette_opacity: state.vignette,
            title_color: hex(state.title_color),
            track_color: hex(state.track_color),
            subtitle_color: hex(state.subtitle_color),
            title_y_pct: state.title_y_pct,
            subtitle_y_pct: state.subtitle_y_pct,
            title_font_path,
            subtitle_font_path,
        };

        let timestamp = if is_video && background.video.duration > 0.0 {
            state.timestamp.clamp(0.0, background.video.duration)
        } else {
            0.0
        };
        let output = self
            .paths
            .outputs
            .join(format!("thumbnail_{}.jpg", random_id(8)));
        let (tools, temp) = (self.tools.clone(), self.paths.temp.clone());

        self.thumbnail.busy = true;
        self.log(
            LogLevel::Info,
            format!(
                "Membuat thumbnail YouTube Full HD pada detik {:.1}s...",
                timestamp
            ),
        );
        self.spawn(async move {
            let rendered =
                thumbnail::render(&tools, &temp, &source, timestamp, spec, output.clone()).await;
            Event::Thumbnail(rendered.map(|()| output))
        });
    }

    pub(super) fn extract_thumbnail_frame(&mut self) {
        let Some(background) = &self.background else {
            return;
        };
        if let Some(image_path) = &background.image {
            self.thumbnail.preview_frame = Some(image_path.clone());
            return;
        }
        let video_path = PathBuf::from(&background.video.path);
        let max_dur = background.video.duration;
        let timestamp = if max_dur > 0.0 {
            self.thumbnail.timestamp.clamp(0.0, max_dur)
        } else {
            self.thumbnail.timestamp.max(0.0)
        };
        let output = self
            .paths
            .temp
            .join(format!("thumb_frame_{}.jpg", random_id(8)));
        let tools = self.tools.clone();
        self.thumbnail.extracting_frame = true;
        self.log(
            LogLevel::Info,
            format!(
                "Mengambil frame video di detik {:.1}s untuk pratinjau thumbnail...",
                timestamp
            ),
        );
        self.spawn(async move {
            let res = thumbnail::extract_frame(&tools, &video_path, timestamp, &output).await;
            Event::ThumbnailFrame(res.map(|()| output))
        });
    }
}

fn draw_scenic_placeholder(painter: &egui::Painter, rect: Rect) {
    let w = rect.width();
    let h = rect.height();

    // Base background: night / dusk sky
    painter.rect_filled(rect, 4.0, Color32::from_rgb(16, 24, 38));

    // Sunset horizon glow (upper-mid section)
    let horizon_top = rect.min.y + h * 0.36;
    let horizon_bottom = rect.min.y + h * 0.68;
    let glow_rect = Rect::from_min_max(
        Pos2::new(rect.min.x, horizon_top),
        Pos2::new(rect.max.x, horizon_bottom),
    );
    painter.rect_filled(glow_rect, 0.0, Color32::from_rgb(120, 75, 45));

    // Mountain range silhouette (mid section)
    let mtn_top = rect.min.y + h * 0.52;
    let mtn_rect = Rect::from_min_max(
        Pos2::new(rect.min.x, mtn_top),
        Pos2::new(rect.max.x, rect.max.y),
    );
    painter.rect_filled(mtn_rect, 0.0, Color32::from_rgb(42, 34, 40));

    // Foreground rock / cliff (bottom section)
    let rock_top = rect.min.y + h * 0.72;
    let rock_rect = Rect::from_min_max(
        Pos2::new(rect.min.x, rock_top),
        Pos2::new(rect.max.x, rect.max.y),
    );
    painter.rect_filled(rock_rect, 0.0, Color32::from_rgb(20, 18, 22));

    // Small label in center
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "Latar Belakang Simulasi (Pilih video/gambar di Main Menu untuk latar nyata)",
        FontId::proportional(11.0 * (w / 640.0).clamp(0.6, 1.2)),
        Color32::from_white_alpha(70),
    );
}

fn draw_vignette(painter: &egui::Painter, rect: Rect, opacity: f32) {
    if opacity <= 0.01 {
        return;
    }
    let alpha = (255.0 * opacity.clamp(0.0, 1.0) * 0.95) as u8;
    let dark = Color32::from_black_alpha(alpha);
    let trans = Color32::TRANSPARENT;
    let w = rect.width();
    let h = rect.height();

    // Left edge
    let left_w = w * 0.42;
    let mut left_mesh = egui::Mesh::default();
    left_mesh.colored_vertex(Pos2::new(rect.min.x, rect.min.y), dark);
    left_mesh.colored_vertex(Pos2::new(rect.min.x, rect.max.y), dark);
    left_mesh.colored_vertex(Pos2::new(rect.min.x + left_w, rect.max.y), trans);
    left_mesh.colored_vertex(Pos2::new(rect.min.x + left_w, rect.min.y), trans);
    left_mesh.add_triangle(0, 1, 2);
    left_mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::Mesh(std::sync::Arc::new(left_mesh)));

    // Right edge
    let right_x = rect.max.x - w * 0.42;
    let mut right_mesh = egui::Mesh::default();
    right_mesh.colored_vertex(Pos2::new(right_x, rect.min.y), trans);
    right_mesh.colored_vertex(Pos2::new(right_x, rect.max.y), trans);
    right_mesh.colored_vertex(Pos2::new(rect.max.x, rect.max.y), dark);
    right_mesh.colored_vertex(Pos2::new(rect.max.x, rect.min.y), dark);
    right_mesh.add_triangle(0, 1, 2);
    right_mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::Mesh(std::sync::Arc::new(right_mesh)));

    // Top edge
    let top_h = h * 0.28;
    let mut top_mesh = egui::Mesh::default();
    top_mesh.colored_vertex(Pos2::new(rect.min.x, rect.min.y), dark);
    top_mesh.colored_vertex(Pos2::new(rect.min.x, rect.min.y + top_h), trans);
    top_mesh.colored_vertex(Pos2::new(rect.max.x, rect.min.y + top_h), trans);
    top_mesh.colored_vertex(Pos2::new(rect.max.x, rect.min.y), dark);
    top_mesh.add_triangle(0, 1, 2);
    top_mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::Mesh(std::sync::Arc::new(top_mesh)));

    // Bottom edge
    let bottom_y = rect.max.y - h * 0.25;
    let mut bottom_mesh = egui::Mesh::default();
    bottom_mesh.colored_vertex(Pos2::new(rect.min.x, bottom_y), trans);
    bottom_mesh.colored_vertex(Pos2::new(rect.min.x, rect.max.y), dark);
    bottom_mesh.colored_vertex(Pos2::new(rect.max.x, rect.max.y), dark);
    bottom_mesh.colored_vertex(Pos2::new(rect.max.x, bottom_y), trans);
    bottom_mesh.add_triangle(0, 1, 2);
    bottom_mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::Mesh(std::sync::Arc::new(bottom_mesh)));
}

fn draw_outlined_text(
    painter: &egui::Painter,
    pos: Pos2,
    align: egui::Align2,
    text: &str,
    font: FontId,
    color: Color32,
    outline_thickness: f32,
) {
    let black = Color32::BLACK;
    let offsets = [
        (-1.0, 0.0),
        (1.0, 0.0),
        (0.0, -1.0),
        (0.0, 1.0),
        (-0.7, -0.7),
        (0.7, -0.7),
        (-0.7, 0.7),
        (0.7, 0.7),
    ];
    for (dx, dy) in offsets {
        let p = Pos2::new(
            pos.x + dx * outline_thickness,
            pos.y + dy * outline_thickness,
        );
        painter.text(p, align, text, font.clone(), black);
    }
    // Drop shadow
    let shadow_p = Pos2::new(
        pos.x + outline_thickness * 1.4,
        pos.y + outline_thickness * 1.8,
    );
    painter.text(shadow_p, align, text, font.clone(), black);
    // Main text
    painter.text(pos, align, text, font, color);
}
