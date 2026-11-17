use std::path::PathBuf;

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Vec2};
use loopkaraoke::{
    thumbnail::{self, Align, ThumbnailSpec},
    util::{open_path, random_id},
};

use super::{
    generate_tab::{
        card_header, generate_card, outline_icon_button, primary_render_button, GenerateIcon,
    },
    help::thumbnail as help,
    hex,
    theme::{self, UiText},
    Event, LogLevel, StudioApp, Tab,
};

const EXAMPLE_TITLE: &str = "Lorem Ipsum";
const EXAMPLE_SUBTITLE: &str = "Dolor Sit Amet";

const NUMBER_STYLES: [(&str, &str); 3] = [
    ("mirrored", "Cermin"),
    ("sequential", "Berurutan"),
    ("mirrored_sequential", "Cermin Berurutan"),
];

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
    "Lorem Ipsum Dolor",
    "Sit Amet Consectetur",
    "Adipiscing Elit",
    "Sed Do Eiusmod",
    "Tempor Incididunt",
    "Ut Labore Et Dolore",
    "Magna Aliqua",
    "Ut Enim Ad Minim",
    "Veniam Quis Nostrud",
    "Exercitation Ullamco",
    "Laboris Nisi Ut",
    "Aliquip Ex Ea",
    "Commodo Consequat",
    "Duis Aute Irure",
    "Dolor In Reprehenderit",
    "Voluptate Velit Esse",
    "Cillum Dolore Eu",
    "Fugiat Nulla Pariatur",
    "Excepteur Sint Occaecat",
];

pub struct ThumbnailState {
    pub title: String,
    pub subtitle: String,
    pub timestamp: f64,
    pub manual_image: Option<PathBuf>,
    pub column_mode: usize,
    pub number_style: usize,
    pub use_first_title: bool,
    pub subtitle_visible: bool,
    pub track_font_preset: usize,
    pub track_custom_font: Option<PathBuf>,
    pub title_size: u32,
    pub track_size: u32,
    pub subtitle_size: u32,
    pub vignette: f32,
    pub title_on_video: bool,
    pub subtitle_on_video: bool,
    pub tracklist_on_video: bool,
    pub vignette_on_video: bool,
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
    pub preview_source: Option<PathBuf>,
    pub extracting_frame: bool,
    pub busy: bool,
    pub result: Option<PathBuf>,
}

impl Default for ThumbnailState {
    fn default() -> Self {
        Self {
            title: String::new(),
            subtitle: String::new(),
            timestamp: 0.0,
            manual_image: None,
            column_mode: 0,
            number_style: 0,
            use_first_title: true,
            subtitle_visible: true,
            track_font_preset: 7,
            track_custom_font: None,
            title_size: 120,
            track_size: 36,
            subtitle_size: 80,
            vignette: 0.55,
            title_on_video: true,
            subtitle_on_video: false,
            tracklist_on_video: true,
            vignette_on_video: false,
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
            preview_source: None,
            extracting_frame: false,
            busy: false,
            result: None,
        }
    }
}

impl ThumbnailState {
    fn set_manual_image(&mut self, path: PathBuf) -> Result<(u32, u32), String> {
        let dimensions = image::ImageReader::open(&path)
            .and_then(|reader| reader.with_guessed_format())
            .map_err(|error| error.to_string())?
            .into_dimensions()
            .map_err(|error| error.to_string())?;
        if dimensions.0 == 0 || dimensions.1 == 0 {
            return Err("Ukuran gambar tidak valid.".into());
        }
        self.manual_image = Some(path);
        self.result = None;
        Ok(dimensions)
    }

    fn manual_source(&self) -> Option<(PathBuf, bool, f64)> {
        self.manual_image
            .as_ref()
            .filter(|path| path.is_file())
            .map(|path| (path.clone(), false, 0.0))
    }

    fn page_source(
        &self,
        generate_video: Option<(PathBuf, bool, f64)>,
    ) -> Option<(PathBuf, bool, f64)> {
        self.manual_source().or(generate_video)
    }

    pub fn reset_example_preset(&mut self) {
        self.title.clear();
        self.subtitle.clear();
        self.column_mode = 0;
        self.number_style = 0;
        self.use_first_title = true;
        self.subtitle_visible = true;
        self.track_font_preset = 7;
        self.track_custom_font = None;
        self.title_size = 120;
        self.track_size = 36;
        self.subtitle_size = 80;
        self.vignette = 0.55;
        self.title_on_video = true;
        self.subtitle_on_video = false;
        self.tracklist_on_video = true;
        self.vignette_on_video = false;
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

pub fn extract_font_family_name(path: &std::path::Path) -> Option<String> {
    let data = std::fs::read(path).ok()?;
    if data.len() < 12 {
        return None;
    }
    let num_tables = u16::from_be_bytes([data[4], data[5]]) as usize;
    let mut name_offset = None;
    for i in 0..num_tables {
        let off = 12 + i * 16;
        if off + 16 > data.len() {
            break;
        }
        if &data[off..off + 4] == b"name" {
            name_offset = Some(u32::from_be_bytes([
                data[off + 8],
                data[off + 9],
                data[off + 10],
                data[off + 11],
            ]) as usize);
            break;
        }
    }
    let name_off = name_offset?;
    if name_off + 6 > data.len() {
        return None;
    }
    let count = u16::from_be_bytes([data[name_off + 2], data[name_off + 3]]) as usize;
    let str_off = u16::from_be_bytes([data[name_off + 4], data[name_off + 5]]) as usize;
    let storage = name_off + str_off;

    for i in 0..count {
        let rec = name_off + 6 + i * 12;
        if rec + 12 > data.len() {
            break;
        }
        let plat = u16::from_be_bytes([data[rec], data[rec + 1]]);
        let name_id = u16::from_be_bytes([data[rec + 6], data[rec + 7]]);
        let len = u16::from_be_bytes([data[rec + 8], data[rec + 9]]) as usize;
        let off = u16::from_be_bytes([data[rec + 10], data[rec + 11]]) as usize;

        if name_id == 1 {
            let start = storage + off;
            if start + len <= data.len() {
                let slice = &data[start..start + len];
                if plat == 3 {
                    // Windows Unicode (UTF-16 BE)
                    let u16s: Vec<u16> = slice
                        .chunks_exact(2)
                        .map(|c| u16::from_be_bytes([c[0], c[1]]))
                        .collect();
                    if let Ok(s) = String::from_utf16(&u16s) {
                        let trimmed = s.trim().to_string();
                        if !trimmed.is_empty() {
                            return Some(trimmed);
                        }
                    }
                } else if plat == 1 {
                    // Mac Roman / ASCII / Latin1
                    let s: String = slice.iter().map(|&b| b as char).collect();
                    let trimmed = s.trim().to_string();
                    if !trimmed.is_empty() {
                        return Some(trimmed);
                    }
                }
            }
        }
    }
    None
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
        if let Some(path) = &self.thumbnail.track_custom_font {
            if let Ok(bytes) = std::fs::read(path) {
                extras.push(("track_custom", bytes));
            }
        }
        if let Some(path) = &self.style.custom_font {
            if let Ok(bytes) = std::fs::read(path) {
                extras.push(("karaoke_custom", bytes));
            }
        }
        super::register_custom_fonts(&self.ctx, &extras);
    }

    pub(super) fn thumbnail_tab(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 8.0;
        ui.page_title("Thumbnail");
        ui.muted_label(
            "Buat thumbnail mandiri dari gambar Anda atau gunakan video dan playlist Generate. Ekspor JPG Full HD.",
        )
        .on_hover_text(help::HEADING);
        ui.add_space(4.0);
        generate_card(ui, |ui| {
            card_header(
                ui,
                GenerateIcon::Sliders,
                "Editor Thumbnail",
                "Teks, daftar lagu, dan latar dalam satu preview.",
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if outline_icon_button(ui, GenerateIcon::Folder, "Unggah Gambar").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Gambar", &loopkaraoke::media::IMAGE_EXTENSIONS)
                        .pick_file()
                    {
                        self.set_thumbnail_image(path);
                    }
                }
                if self.thumbnail.manual_source().is_none()
                    && self.generate.video.is_some()
                    && outline_icon_button(ui, GenerateIcon::Reset, "Pilih Acak").clicked()
                {
                    let duration = self
                        .generate
                        .video
                        .as_ref()
                        .map_or(0.1, |video| video.duration.max(0.1));
                    use rand::Rng;
                    self.thumbnail.timestamp = rand::thread_rng().gen_range(0.0..duration);
                    self.extract_thumbnail_frame();
                }
                if outline_icon_button(ui, GenerateIcon::Reset, "Contoh Playlist").clicked() {
                    self.thumbnail.reset_example_preset();
                    self.reload_egui_fonts();
                    self.info("Contoh Playlist berhasil dimuat.");
                }
                if outline_icon_button(ui, GenerateIcon::Play, "Buka Generate").clicked() {
                    self.tab = Tab::Generate;
                }
            });
            if let Some(path) = self.thumbnail.manual_image.clone() {
                ui.horizontal_wrapped(|ui| {
                    ui.muted_label(path.file_name().unwrap_or_default().to_string_lossy())
                        .on_hover_text(path.display().to_string());
                    if outline_icon_button(ui, GenerateIcon::Trash, "Hapus Pilihan").clicked() {
                        self.thumbnail.manual_image = None;
                        self.thumbnail.result = None;
                    }
                });
            }
            ui.muted_label("Gambar unggahan dipakai terlebih dahulu. Jika tidak ada, latar otomatis memakai video Generate. Daftar lagu mengikuti Playlist.");
            ui.muted_label("Opsi “Tampilkan di Video” hanya berlaku untuk Render Old; sumber thumbnail tetap gambar unggahan atau video Generate.");
        });
        if ui.available_width() >= 1080.0 {
            ui.columns(2, |columns| {
                self.thumbnail_fields(&mut columns[0]);
                self.thumbnail_output(&mut columns[1]);
            });
        } else {
            self.thumbnail_output(ui);
            ui.add_space(8.0);
            self.thumbnail_fields(ui);
        }
    }

    fn thumbnail_fields(&mut self, ui: &mut egui::Ui) {
        let p = theme::Palette::of(ui.ctx());
        let frame = egui::Frame::new()
            .fill(p.surface)
            .stroke(egui::Stroke::new(1.0_f32, p.border))
            .corner_radius(12)
            .inner_margin(egui::Margin::symmetric(16, 12));

        let mut trigger_font_reload = false;

        // Card 1: Teks & Tipografi
        frame.show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            let max_w = ui.available_width();
            ui.set_width(max_w);
            ui.set_max_width(max_w);
            card_header(ui, GenerateIcon::Sliders, "Teks & Tipografi", "Judul besar, sub judul, font, dan warna.");
            ui.add_space(4.0);
            let first_title = self.generate.tracks.first()
                .map(|track| loopkaraoke::media::clean_proper_title(&track.title));
            let state = &mut self.thumbnail;

            ui.horizontal_wrapped(|ui| {
                theme::label_with_info(ui, "Judul Utama:", help::TITLE);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(&mut state.title_on_video, "Tampilkan di Video")
                        .on_hover_text("Jika dicentang, teks Judul Utama juga ditampilkan pada video; jika tidak, hanya tampil di Thumbnail.");
                    if first_title.is_some() {
                        ui.checkbox(&mut state.use_first_title, "Judul Pertama")
                            .on_hover_text("Gunakan judul lagu pertama dari Playlist. Otomatis mengikuti perubahan urutan.");
                    }
                });
            });
            let automatic_title = state.use_first_title && first_title.is_some();
            let mut first_title_text = first_title.unwrap_or_default();
            let title_input = if automatic_title { &mut first_title_text } else { &mut state.title };
            ui.add_enabled_ui(!automatic_title, |ui| {
                theme::text_field(ui, title_input, EXAMPLE_TITLE, theme::InputWidth::Fill);
            });

            // Font Selector Judul Utama
            ui.horizontal_wrapped(|ui| {
                ui.label("Font Judul Utama:");
                let current_title_label = if let Some(path) = &state.title_custom_font {
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                } else {
                    TITLE_FONTS[state.title_font_preset].label.to_string()
                };

                egui::ComboBox::from_id_salt("title_font_select")
                    .width(ui.available_width().min(280.0))
                    .truncate()
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

                if outline_icon_button(ui, GenerateIcon::Folder, "Pilih File Font")
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
                    && outline_icon_button(ui, GenerateIcon::Reset, "Reset Font").clicked()
                {
                    state.title_custom_font = None;
                    trigger_font_reload = true;
                }
            });

            thumbnail_slider(ui, "Ukuran Judul", &mut state.title_size, 40..=240, " px", 0, None, true);
            thumbnail_percentage(ui, "Posisi Y", &mut state.title_y_pct, 1.0..=30.0);
            ui.horizontal(|ui| {
                theme::label_with_info(ui, "Warna Judul:", help::COLORS);
                ui.color_edit_button_srgb(&mut state.title_color);
            });
            ui.add_space(8.0);

            ui.horizontal_wrapped(|ui| {
                theme::label_with_info(ui, "Sub Judul / Artis / Branding:", help::SUBTITLE);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(&mut state.subtitle_on_video, "Tampilkan di Video")
                        .on_hover_text("Jika tidak dicentang, teks Sub Judul hanya tampil di Thumbnail dan tidak tampil di video.");
                    ui.checkbox(&mut state.subtitle_visible, "Tampilkan Sub Judul")
                        .on_hover_text("Sembunyikan Sub Judul pada preview, thumbnail, dan overlay video tanpa menghapus teksnya.");
                });
            });
            theme::text_field(ui, &mut state.subtitle, EXAMPLE_SUBTITLE, theme::InputWidth::Fill);

            // Font Selector Sub Judul
            ui.horizontal_wrapped(|ui| {
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
                    .width(ui.available_width().min(280.0))
                    .truncate()
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

                if outline_icon_button(ui, GenerateIcon::Folder, "Pilih File Font")
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
                    && outline_icon_button(ui, GenerateIcon::Reset, "Reset Font").clicked()
                {
                    state.subtitle_custom_font = None;
                    trigger_font_reload = true;
                }
            });

            thumbnail_slider(ui, "Ukuran Sub Judul", &mut state.subtitle_size, 20..=160, " px", 0, None, true);
            thumbnail_percentage(ui, "Posisi Y", &mut state.subtitle_y_pct, 65.0..=95.0);
            ui.horizontal(|ui| {
                ui.muted_label("Warna Sub Judul:");
                ui.color_edit_button_srgb(&mut state.subtitle_color);
            });
            ui.add_space(12.0);
            ui.separator();
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                ui.strong("Tata Letak Daftar Lagu");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(&mut state.tracklist_on_video, "Tampilkan di Video")
                        .on_hover_text("Jika dicentang, daftar judul lagu (tracklist) juga ditampilkan pada video; jika tidak, hanya tampil di Thumbnail.");
                });
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                theme::label_with_info(ui, "Mode Kolom:", help::COLUMNS);
                egui::ComboBox::from_id_salt("thumbnail_columns_mode")
                    .width(ui.available_width().min(280.0))
                    .truncate()
                    .selected_text(COLUMN_MODES[state.column_mode].1)
                    .show_ui(ui, |ui| {
                        for (index, (_, label)) in COLUMN_MODES.iter().enumerate() {
                            ui.selectable_value(&mut state.column_mode, index, *label);
                        }
                    });
            });
            ui.add_space(3.0);

            ui.horizontal_wrapped(|ui| {
                theme::label_with_info(ui, "Penomoran:", help::NUMBERING);
                egui::ComboBox::from_id_salt("thumbnail_number_style")
                    .width(ui.available_width().min(280.0)).truncate()
                    .selected_text(NUMBER_STYLES[state.number_style].1)
                    .show_ui(ui, |ui| {
                        for (index, (_, label)) in NUMBER_STYLES.iter().enumerate() {
                            ui.selectable_value(&mut state.number_style, index, *label);
                        }
                    });
            });
            ui.add_space(6.0);
            if thumbnail_track_font_picker(ui, &mut state.track_font_preset, &mut state.track_custom_font) {
                trigger_font_reload = true;
            }
            ui.add_space(6.0);

            thumbnail_slider(ui, "Ukuran Lagu", &mut state.track_size, 16..=64, " px", 0, None, true);
            ui.horizontal(|ui| {
                ui.muted_label("Warna Daftar Lagu:");
                ui.color_edit_button_srgb(&mut state.track_color);
            });
            ui.add_space(4.0);

            if self.generate.tracks.is_empty() {
                ui.muted_label("Belum ada lagu Generate. Preview memakai daftar contoh.");
            } else {
                ui.colored_label(p.success, format!("Menggunakan {} lagu dari Generate.", self.generate.tracks.len()));
            }
        });
        ui.add_space(8.0);

        // Card 3: Latar Belakang & Vignette (Limit Sesuai Durasi Video)
        frame.show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            let max_w = ui.available_width();
            ui.set_width(max_w);
            ui.set_max_width(max_w);
            card_header(ui, GenerateIcon::Folder, "Latar & Efek", "Pilih gambar sendiri atau frame video Generate, lalu atur efek latar.");
            ui.add_space(4.0);

            if let Some((image, _, _)) = self.thumbnail.manual_source() {
                ui.muted_label(format!("Latar dari gambar manual: {}", image.file_name().unwrap_or_default().to_string_lossy()));
            } else {
            let bg_info = self.generate.video.as_ref().map(|video| (video.filename.clone(), video.duration));
            if let Some((name, duration)) = bg_info {
                ui.muted_label(format!("Video Generate: {name}"));
                let max_dur = duration.max(0.1);
                self.thumbnail.timestamp = self.thumbnail.timestamp.clamp(0.0, max_dur);
                let extracting = self.thumbnail.extracting_frame;
                let trigger_extract = thumbnail_slider(
                    ui,
                    "Posisi Frame",
                    &mut self.thumbnail.timestamp,
                    0.0..=max_dur,
                    " s",
                    1,
                    Some(if extracting { "Mengambil frame..." } else { "Ambil Frame" }),
                    !extracting,
                );
                if trigger_extract {
                    self.extract_thumbnail_frame();
                }
                ui.muted_label(format!("Rentang frame: 0–{max_dur:.1} detik."));
            } else {
                ui.muted_label("Pilih video di Generate untuk mengambil frame dan mengekspor thumbnail.");
            }

            }

            ui.add_space(4.0);

            thumbnail_percentage(ui, "Gelap di Tepi (Vignette)", &mut self.thumbnail.vignette, 0.0..=100.0);
            ui.checkbox(&mut self.thumbnail.vignette_on_video, "Tampilkan Vignette di Video")
                .on_hover_text("Aktifkan vignette saat render melalui Render Old.");
            ui.small("Membantu teks putih dan emas terlihat kontras dan terbaca jelas pada latar terang.");
        });
        ui.add_space(8.0);

        // Card 4: Ekspor Thumbnail
        frame.show(ui, |ui| {
            let max_w = ui.available_width();
            ui.set_width(max_w);
            ui.set_max_width(max_w);
            card_header(
                ui,
                GenerateIcon::Upload,
                "Ekspor Thumbnail",
                "Gambar JPG 1920×1080 untuk sampul album.",
            );
            ui.add_space(4.0);

            let busy = self.thumbnail.busy;
            let label = if busy {
                "Sedang Merender Thumbnail..."
            } else {
                "Buat Thumbnail Full HD (1920x1080)"
            };
            if primary_render_button(ui, label, !busy && self.thumbnail_source().is_some())
                .clicked()
            {
                self.generate_thumbnail();
            }
            ui.add_space(2.0);
            ui.small(
                "Disimpan di folder output dalam resolusi 1920x1080 Full HD (standar YouTube).",
            );
        });
        if trigger_font_reload {
            self.reload_egui_fonts();
        }
    }

    fn thumbnail_output(&mut self, ui: &mut egui::Ui) {
        let p = theme::Palette::of(ui.ctx());
        let frame = egui::Frame::new()
            .fill(p.surface)
            .stroke(egui::Stroke::new(1.0_f32, p.border))
            .corner_radius(12)
            .inner_margin(egui::Margin::symmetric(16, 12));

        frame.show(ui, |ui| {
            let max_w = ui.available_width();
            ui.set_width(max_w);
            ui.set_max_width(max_w);
            ui.horizontal_wrapped(|ui| {
                ui.strong("Preview Thumbnail");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new("1920x1080 Full HD")
                            .size(11.0)
                            .color(p.accent),
                    );
                });
            });
            ui.add_space(6.0);

            // Live Preview Canvas
            self.render_thumbnail_canvas(ui);

            ui.add_space(8.0);
            ui.small(
                "💡 Preview interaktif real-time: teks, font kustom, ukuran font, tata letak cermin, dan vignette otomatis sinkron.",
            );

            if let Some(path) = self.thumbnail.result.clone() {
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);
                ui.colored_label(p.success, "✓ Berkas Thumbnail Berhasil Dibuat:");
                ui.label(path.display().to_string());
                ui.horizontal_wrapped(|ui| {
                    if ui.add(theme::StyledButton::secondary("Buka Gambar").min_size(egui::vec2(120.0, 36.0))).clicked() {
                        let _ = open_path(&path);
                    }
                    if ui.add(theme::StyledButton::secondary("Buka Folder").min_size(egui::vec2(120.0, 36.0))).clicked() {
                        let _ = open_path(path.parent().unwrap_or(&self.paths.outputs));
                    }
                });
            }
        });
    }

    fn render_thumbnail_canvas(&mut self, ui: &mut egui::Ui) {
        let avail_w = ui.available_width().max(180.0);
        let canvas_w = avail_w;
        let canvas_h = canvas_w * (9.0 / 16.0);

        let (rect, _response) =
            ui.allocate_exact_size(Vec2::new(canvas_w, canvas_h), egui::Sense::hover());

        // Preview and export resolve the same media source.
        let bg_path = self.thumbnail_preview_path();

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
        self.draw_thumbnail_overlay(&painter, rect, true);
    }

    pub(super) fn build_thumbnail_spec(&self, for_video: bool) -> ThumbnailSpec {
        let state = &self.thumbnail;
        let titles = if self.thumbnail_uses_generate() {
            self.generate
                .tracks
                .iter()
                .map(|track| track.title.clone())
                .collect()
        } else {
            self.tracks
                .iter()
                .map(|track| track.title.clone())
                .collect()
        };
        let track_titles = thumbnail_track_titles(titles, for_video, state.tracklist_on_video);

        let title_font_path = resolve_font_path(
            &state.title_custom_font,
            &TITLE_FONTS[state.title_font_preset],
        );
        let subtitle_font_path = resolve_font_path(
            &state.subtitle_custom_font,
            &SUBTITLE_FONTS[state.subtitle_font_preset],
        );

        ThumbnailSpec {
            title: if !for_video || state.title_on_video {
                thumbnail_title(
                    &state.title,
                    state.use_first_title,
                    if self.thumbnail_uses_generate() {
                        self.generate
                            .tracks
                            .first()
                            .map(|track| track.title.as_str())
                    } else {
                        self.tracks.first().map(|track| track.title.as_str())
                    },
                )
            } else {
                String::new()
            },
            subtitle: thumbnail_subtitle(
                &state.subtitle,
                state.subtitle_visible,
                for_video,
                state.subtitle_on_video,
            ),
            track_titles,
            column_mode: COLUMN_MODES[state.column_mode].0.to_owned(),
            number_style: NUMBER_STYLES[state.number_style].0.to_owned(),
            title_font_size: state.title_size,
            track_font_size: state.track_size,
            subtitle_font_size: state.subtitle_size,
            vignette_opacity: if !for_video || state.vignette_on_video {
                state.vignette
            } else {
                0.0
            },
            title_color: hex(state.title_color),
            track_color: hex(state.track_color),
            subtitle_color: hex(state.subtitle_color),
            title_y_pct: state.title_y_pct,
            subtitle_y_pct: state.subtitle_y_pct,
            title_font_path,
            subtitle_font_path,
            track_font_path: resolve_font_path(
                &state.track_custom_font,
                &TITLE_FONTS[state.track_font_preset],
            ),
        }
    }

    /// Menggambar overlay judul/daftar lagu/sub judul dan mengembalikan area teks yang digambar
    /// (dipakai preview Karaoke untuk mendeteksi tumpukan).
    pub(super) fn draw_thumbnail_overlay(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        use_demo_if_empty: bool,
    ) -> Vec<Rect> {
        let mut drawn: Vec<Rect> = Vec::new();
        let painter = painter.with_clip_rect(rect);
        let painter = &painter;
        let scale_x = rect.width() / 1920.0;
        let scale_y = rect.height() / 1080.0;
        let scale = scale_x.min(scale_y);
        // Match ab_glyph's pt_to_px_scale factor (~0.72) so preview proportions match 1920x1080 output
        const PT_TO_PX: f32 = 0.72;

        let state = &self.thumbnail;
        let spec = self.build_thumbnail_spec(!use_demo_if_empty);

        // Vignette overlay: selalu tampil di Thumbnail, di Video hanya jika dicentang
        if spec.vignette_opacity > 0.0 {
            draw_vignette(painter, rect, spec.vignette_opacity);
        }

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
        if !spec.title.trim().is_empty() {
            let title_scale = (state.title_size as f32 * PT_TO_PX * scale).max(6.0);
            let title_y = rect.min.y + (1080.0 * state.title_y_pct) * scale_y;
            let title_pos = Pos2::new(rect.center().x, title_y);
            let title_color = Color32::from_rgb(
                state.title_color[0],
                state.title_color[1],
                state.title_color[2],
            );
            let title_font = FontId::new(title_scale, title_font_family);
            drawn.push(draw_outlined_text(
                painter,
                title_pos,
                egui::Align2::CENTER_TOP,
                spec.title.trim(),
                title_font,
                title_color,
                (3.6 * scale).max(0.8),
            ));
        }

        // Draw Track Rows (Left and Right columns)
        let rows = thumbnail::layout_tracks(&spec);
        let track_scale = (state.track_size as f32 * PT_TO_PX * scale).max(4.5);
        let track_color = Color32::from_rgb(
            state.track_color[0],
            state.track_color[1],
            state.track_color[2],
        );
        let track_family = if state.track_custom_font.is_some() {
            "track_custom"
        } else {
            TITLE_FONTS[state.track_font_preset].id
        };
        let track_font = FontId::new(track_scale, egui::FontFamily::Name(track_family.into()));
        let outline_thickness = (2.2 * scale).max(0.6);

        for row in rows {
            let px = rect.min.x + (row.x as f32) * scale_x;
            let py = rect.min.y + (row.y as f32) * scale_y;
            let align2 = match row.align {
                Align::Left => egui::Align2::LEFT_TOP,
                Align::Right => egui::Align2::RIGHT_TOP,
                Align::Center => egui::Align2::CENTER_TOP,
            };
            drawn.push(draw_outlined_text(
                painter,
                Pos2::new(px, py),
                align2,
                &row.text,
                track_font.clone(),
                track_color,
                outline_thickness,
            ));
        }

        // Draw Subtitle (Bottom Centered)
        if !spec.subtitle.trim().is_empty() {
            let sub_scale = (state.subtitle_size as f32 * PT_TO_PX * scale).max(5.5);
            let sub_y = rect.min.y + (1080.0 * state.subtitle_y_pct) * scale_y;
            let sub_pos = Pos2::new(rect.center().x, sub_y);
            let sub_color = Color32::from_rgb(
                state.subtitle_color[0],
                state.subtitle_color[1],
                state.subtitle_color[2],
            );
            let sub_font = FontId::new(sub_scale, subtitle_font_family);
            drawn.push(draw_outlined_text(
                painter,
                sub_pos,
                egui::Align2::CENTER_TOP,
                spec.subtitle.trim(),
                sub_font,
                sub_color,
                (2.6 * scale).max(0.7),
            ));
        }
        drawn
    }

    pub(super) fn set_thumbnail_image(&mut self, path: PathBuf) {
        match self.thumbnail.set_manual_image(path) {
            Ok((width, height)) => self.info(format!("Gambar thumbnail siap: {width}x{height}.")),
            Err(error) => self.error(format!("Gambar tidak dapat dibaca: {error}")),
        }
    }

    pub(super) fn thumbnail_uses_generate(&self) -> bool {
        matches!(self.tab, Tab::Thumbnail | Tab::Karaoke)
    }

    fn thumbnail_source(&self) -> Option<(PathBuf, bool, f64)> {
        if self.tab == Tab::Thumbnail {
            return self.thumbnail.page_source(
                self.generate
                    .video
                    .as_ref()
                    .map(|video| (video.path.clone().into(), true, video.duration)),
            );
        }
        if self.thumbnail_uses_generate() {
            self.generate
                .video
                .as_ref()
                .map(|video| (video.path.clone().into(), true, video.duration))
        } else {
            self.background.as_ref().map(|background| {
                (
                    background
                        .image
                        .clone()
                        .unwrap_or_else(|| background.video.path.clone().into()),
                    background.image.is_none(),
                    background.video.duration,
                )
            })
        }
    }

    pub(super) fn thumbnail_preview_path(&self) -> Option<PathBuf> {
        let (source, is_video, _) = self.thumbnail_source()?;
        if !is_video {
            return source.is_file().then_some(source);
        }
        if self.thumbnail.preview_source.as_ref() == Some(&source) {
            self.thumbnail
                .preview_frame
                .as_ref()
                .filter(|path| path.is_file())
                .cloned()
        } else {
            None
        }
    }

    pub(super) fn generate_thumbnail(&mut self) {
        let Some((source, is_video, duration)) = self.thumbnail_source() else {
            self.error("Pilih gambar manual atau video Generate terlebih dahulu. Untuk Render Old, pilih media di halaman tersebut.");
            return;
        };
        let spec = self.build_thumbnail_spec(false);
        let timestamp = if is_video && duration > 0.0 {
            self.thumbnail.timestamp.clamp(0.0, duration)
        } else {
            0.0
        };
        let output_dir = if self.thumbnail_uses_generate() {
            self.generate
                .output_dir
                .as_ref()
                .unwrap_or(&self.paths.outputs)
        } else {
            &self.paths.outputs
        };
        if let Err(error) = std::fs::create_dir_all(output_dir) {
            self.error(format!("Gagal membuat folder output: {error}"));
            return;
        }
        let output = output_dir.join(format!("thumbnail_{}.jpg", random_id(8)));
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
        let Some((video_path, is_video, max_dur)) = self.thumbnail_source() else {
            return;
        };
        if !is_video {
            self.thumbnail.preview_frame = Some(video_path.clone());
            self.thumbnail.preview_source = Some(video_path);
            return;
        }
        if self.thumbnail.extracting_frame {
            return;
        }
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
                "Mengambil frame video di detik {:.1}s untuk preview thumbnail...",
                timestamp
            ),
        );
        self.spawn(async move {
            let res = thumbnail::extract_frame(&tools, &video_path, timestamp, &output).await;
            Event::ThumbnailFrame {
                source: video_path,
                result: res.map(|()| output),
            }
        });
    }
}

fn thumbnail_track_font_picker(
    ui: &mut egui::Ui,
    preset: &mut usize,
    custom: &mut Option<PathBuf>,
) -> bool {
    let mut changed = false;
    ui.muted_label("Font Daftar Lagu");
    ui.horizontal_wrapped(|ui| {
        let label = custom
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| TITLE_FONTS[*preset].label.to_owned());
        egui::ComboBox::from_id_salt("track_font_select")
            .width(ui.available_width().min(280.0))
            .truncate()
            .selected_text(label)
            .show_ui(ui, |ui| {
                for (index, font) in TITLE_FONTS.iter().enumerate() {
                    if ui
                        .selectable_label(custom.is_none() && *preset == index, font.label)
                        .clicked()
                    {
                        *preset = index;
                        *custom = None;
                        changed = true;
                    }
                }
            });
        if outline_icon_button(ui, GenerateIcon::Folder, "Pilih File Font").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Font", &["ttf", "otf"])
                .pick_file()
            {
                *custom = Some(path);
                changed = true;
            }
        }
        if custom.is_some() && outline_icon_button(ui, GenerateIcon::Reset, "Reset Font").clicked()
        {
            *custom = None;
            *preset = 7;
            changed = true;
        }
    });
    changed
}

fn thumbnail_title(manual: &str, use_first: bool, first: Option<&str>) -> String {
    if use_first {
        if let Some(title) = first {
            return loopkaraoke::media::clean_proper_title(title);
        }
    }
    thumbnail_text(manual, EXAMPLE_TITLE)
}

fn thumbnail_subtitle(text: &str, visible: bool, for_video: bool, on_video: bool) -> String {
    if visible && (!for_video || on_video) {
        thumbnail_text(text, EXAMPLE_SUBTITLE)
    } else {
        String::new()
    }
}

#[allow(clippy::too_many_arguments)]
fn thumbnail_slider<T: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut T,
    range: std::ops::RangeInclusive<T>,
    suffix: &str,
    decimals: usize,
    action_label: Option<&str>,
    action_enabled: bool,
) -> bool {
    let tooltip = match label {
        "Ukuran Judul" => help::TITLE_SIZE,
        "Ukuran Lagu" => help::TRACK_SIZE,
        "Posisi Frame" => help::FRAME,
        "Gelap di Tepi (Vignette)" => help::VIGNETTE,
        _ => "Geser slider, seret nilai angka, atau klik angkanya untuk mengetik nilai.",
    };
    let response = ui.scope(|ui| {
        theme::control_slider(
            ui,
            label,
            value,
            range,
            suffix,
            decimals,
            action_label,
            action_enabled,
        )
    });
    response.response.on_hover_text(tooltip);
    response.inner
}

fn thumbnail_percentage(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
) {
    let mut percent = *value * 100.0;
    let previous = percent;
    thumbnail_slider(ui, label, &mut percent, range, " %", 1, None, true);
    if percent != previous {
        *value = percent / 100.0;
    }
}

fn thumbnail_text(value: &str, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_owned()
    } else {
        value.to_owned()
    }
}

fn thumbnail_track_titles(
    titles: Vec<String>,
    for_video: bool,
    tracklist_on_video: bool,
) -> Vec<String> {
    if for_video && !tracklist_on_video {
        Vec::new()
    } else if titles.is_empty() && !for_video {
        DEMO_TRACKS
            .iter()
            .map(|title| (*title).to_owned())
            .collect()
    } else {
        titles
    }
}

pub(super) fn draw_scenic_placeholder(painter: &egui::Painter, rect: Rect) {
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
) -> Rect {
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
    painter.text(pos, align, text, font, color)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_image_source_works_without_generate_and_rejects_invalid_replacements() {
        let dir = std::env::temp_dir().join(format!("thumbnail-image-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let image_path = dir.join("background.png");
        image::RgbImage::new(12, 8).save(&image_path).unwrap();
        let mut state = ThumbnailState::default();
        let generate_video = Some((dir.join("generate.mp4"), true, 30.0));
        assert_eq!(state.page_source(generate_video.clone()), generate_video);
        assert!(state.page_source(None).is_none());
        assert_eq!(state.set_manual_image(image_path.clone()).unwrap(), (12, 8));
        assert_eq!(
            state.manual_source(),
            Some((image_path.clone(), false, 0.0))
        );
        assert_eq!(
            state.page_source(generate_video.clone()),
            state.manual_source()
        );
        let invalid = dir.join("invalid.png");
        std::fs::write(&invalid, b"not an image").unwrap();
        assert!(state.set_manual_image(invalid.clone()).is_err());
        assert_eq!(state.manual_image, Some(image_path.clone()));
        std::fs::remove_file(&image_path).unwrap();
        assert!(state.manual_source().is_none());
        assert_eq!(state.page_source(generate_video.clone()), generate_video);
        std::fs::remove_file(invalid).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn first_song_title_overrides_manual_text_only_when_enabled_and_available() {
        assert!(ThumbnailState::default().use_first_title);
        assert_eq!(
            thumbnail_title("Manual", true, Some("Lagu Pertama.mp3")),
            "Lagu Pertama"
        );
        assert_eq!(
            thumbnail_title("Manual", true, Some("Lagu Kedua.mp3")),
            "Lagu Kedua"
        );
        assert_eq!(
            thumbnail_title("Manual", false, Some("Lagu Pertama.mp3")),
            "Manual"
        );
        assert_eq!(thumbnail_title("Manual", true, None), "Manual");
        assert_eq!(thumbnail_title("", true, None), EXAMPLE_TITLE);
    }

    #[test]
    fn subtitle_visibility_controls_both_thumbnail_and_video_without_losing_text() {
        assert_eq!(thumbnail_subtitle("Brand", false, false, true), "");
        assert_eq!(thumbnail_subtitle("Brand", false, true, true), "");
        assert_eq!(thumbnail_subtitle("Brand", true, false, false), "Brand");
        assert_eq!(thumbnail_subtitle("Brand", true, true, false), "");
        assert_eq!(thumbnail_subtitle("Brand", true, true, true), "Brand");
        assert_eq!(thumbnail_subtitle("", true, false, false), EXAMPLE_SUBTITLE);
    }

    #[test]
    fn empty_thumbnail_inputs_use_example_text_without_filling_the_fields() {
        let state = ThumbnailState::default();
        assert!(state.title.is_empty() && state.subtitle.is_empty());
        assert_eq!(thumbnail_text(&state.title, EXAMPLE_TITLE), "Lorem Ipsum");
        assert_eq!(thumbnail_text("  ", EXAMPLE_SUBTITLE), "Dolor Sit Amet");
        assert_eq!(thumbnail_text("Album Saya", EXAMPLE_TITLE), "Album Saya");
    }

    #[test]
    fn thumbnail_uses_selected_playlist_and_demo_only_for_empty_cover_preview() {
        let titles = vec!["Lagu Generate B".to_owned(), "Lagu Generate A".to_owned()];
        assert_eq!(thumbnail_track_titles(titles.clone(), false, false), titles);
        assert!(thumbnail_track_titles(titles.clone(), true, false).is_empty());
        assert_eq!(thumbnail_track_titles(titles.clone(), true, true), titles);
        assert_eq!(
            thumbnail_track_titles(Vec::new(), false, true).len(),
            DEMO_TRACKS.len()
        );
        assert!(thumbnail_track_titles(Vec::new(), true, true).is_empty());
    }

    #[test]
    fn parses_font_family_name_if_available() {
        let windows_root = std::env::var_os("SystemRoot").map_or_else(
            || std::path::PathBuf::from(r"C:\Windows"),
            std::path::PathBuf::from,
        );
        let impact = windows_root.join("Fonts").join("impact.ttf");
        if impact.is_file() {
            assert_eq!(
                extract_font_family_name(&impact),
                Some("Impact".to_string())
            );
        }
    }
}
