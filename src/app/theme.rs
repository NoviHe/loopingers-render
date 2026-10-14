//! Design tokens and egui styling shared with the other Craft-style apps:
//! black window, `#1d1d1d` panels separated by thin gaps, square panels,
//! 4 px widgets and a single blue accent.

use eframe::egui::{
    self, epaint::Shadow, Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Visuals,
};

pub const APP_BG: Color32 = Color32::BLACK;
pub const HEADER_BG: Color32 = Color32::from_rgb(0x1d, 0x1d, 0x1d);
pub const PANEL_BG: Color32 = Color32::from_rgb(0x1d, 0x1d, 0x1d);
pub const FIELD_BG: Color32 = Color32::from_rgb(0x0e, 0x0e, 0x0e);
pub const FIELD_BORDER: Color32 = Color32::from_rgb(0x30, 0x30, 0x30);
pub const SEPARATOR: Color32 = Color32::from_rgb(0x30, 0x30, 0x30);
pub const ROW_ALT: Color32 = Color32::from_rgb(0x21, 0x21, 0x21);
pub const HOVER: Color32 = Color32::from_rgb(0x2c, 0x2c, 0x2c);
pub const PRESSED: Color32 = Color32::from_rgb(0x4b, 0x4b, 0x4b);
pub const TEXT: Color32 = Color32::from_rgb(0xd1, 0xd1, 0xd1);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0xb0, 0xb0, 0xb0);
pub const ACCENT: Color32 = Color32::from_rgb(0x2f, 0x6b, 0xdf);
pub const FOCUS: Color32 = Color32::from_rgb(0x57, 0x94, 0xec);
pub const SUCCESS: Color32 = Color32::from_rgb(0x2d, 0x9d, 0x78);
pub const DANGER: Color32 = Color32::from_rgb(0xe3, 0x48, 0x50);
pub const WARNING: Color32 = Color32::from_rgb(0xf0, 0xc0, 0x4f);

/// Font family names registered in `install_fonts`.
pub const SEMIBOLD: &str = "semibold";
pub const MEDIUM: &str = "medium";

/// Gap between panels, in points.
pub const GAP: i8 = 4;
pub const HEADER_HEIGHT: f32 = 38.0;
const WIDGET_RADIUS: u8 = 4;

pub fn apply(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();
    visuals.panel_fill = PANEL_BG;
    visuals.window_fill = PANEL_BG;
    visuals.extreme_bg_color = FIELD_BG;
    visuals.faint_bg_color = ROW_ALT;
    visuals.override_text_color = Some(TEXT);
    visuals.selection.bg_fill = ACCENT;
    visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.hyperlink_color = FOCUS;
    visuals.error_fg_color = DANGER;
    visuals.window_stroke = Stroke::new(1.0, FIELD_BORDER);
    visuals.window_corner_radius = CornerRadius::same(6);
    visuals.menu_corner_radius = CornerRadius::same(6);
    visuals.popup_shadow = Shadow {
        offset: [0, 4],
        blur: 16,
        spread: 0,
        color: Color32::from_black_alpha(140),
    };
    visuals.window_shadow = visuals.popup_shadow;

    let widgets = &mut visuals.widgets;
    for state in [
        &mut widgets.noninteractive,
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
        &mut widgets.open,
    ] {
        state.corner_radius = CornerRadius::same(WIDGET_RADIUS);
    }
    widgets.noninteractive.bg_fill = PANEL_BG;
    widgets.noninteractive.weak_bg_fill = PANEL_BG;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, SEPARATOR);
    widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    widgets.inactive.bg_fill = FIELD_BG;
    widgets.inactive.weak_bg_fill = FIELD_BG;
    widgets.inactive.bg_stroke = Stroke::new(1.0, FIELD_BORDER);
    widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    widgets.hovered.bg_fill = HOVER;
    widgets.hovered.weak_bg_fill = HOVER;
    widgets.hovered.bg_stroke = Stroke::new(1.0, FIELD_BORDER);
    widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::from_gray(0xe6));
    widgets.active.bg_fill = PRESSED;
    widgets.active.weak_bg_fill = PRESSED;
    widgets.active.fg_stroke = Stroke::new(1.0, Color32::from_gray(0xe6));
    widgets.open.bg_fill = HOVER;
    widgets.open.weak_bg_fill = HOVER;
    ctx.set_visuals(visuals);

    ctx.style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.button_padding = egui::vec2(8.0, 3.0);
        style.spacing.interact_size.y = 24.0;
        style.spacing.menu_margin = egui::Margin::same(4);
        style.animation_time = 0.12;
        for (text_style, size) in [
            (TextStyle::Body, 12.5),
            (TextStyle::Button, 12.5),
            (TextStyle::Small, 11.0),
            (TextStyle::Heading, 16.0),
            (TextStyle::Monospace, 12.0),
        ] {
            let family = match text_style {
                TextStyle::Monospace => FontFamily::Monospace,
                TextStyle::Heading => FontFamily::Name(SEMIBOLD.into()),
                _ => FontFamily::Proportional,
            };
            style
                .text_styles
                .insert(text_style, FontId::new(size, family));
        }
    });
}

/// A header tab: 14 pt label, brighter when active, with a 2 pt underline.
pub fn header_tab(ui: &mut egui::Ui, title: &str, active: bool, hint: &str) -> bool {
    let galley =
        ui.painter()
            .layout_no_wrap(title.to_string(), FontId::proportional(14.0), TEXT_DIM);
    let size = egui::vec2(galley.size().x + 20.0, HEADER_HEIGHT - 12.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let color = if active || response.hovered() {
        Color32::from_gray(0xe6)
    } else {
        TEXT_DIM
    };
    let text_pos = egui::pos2(rect.min.x + 10.0, rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(text_pos, galley, color);
    if active {
        let y = rect.bottom() + 3.0;
        ui.painter().line_segment(
            [
                egui::pos2(rect.min.x + 10.0, y),
                egui::pos2(rect.max.x - 10.0, y),
            ],
            Stroke::new(2.0, Color32::from_gray(0xe6)),
        );
    }
    let clicked = response.clicked();
    response.on_hover_text(hint);
    clicked
}

/// Small ⓘ icon; hovering it shows `text`. Drawn with the painter so it does
/// not depend on a font having the glyph.
pub fn info(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(16.0), egui::Sense::hover());
    let color = if response.hovered() { FOCUS } else { TEXT_DIM };
    let painter = ui.painter();
    painter.circle_stroke(rect.center(), 6.5, Stroke::new(1.0, color));
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "i",
        FontId::proportional(10.5),
        color,
    );
    response.on_hover_ui(|ui| {
        ui.set_max_width(340.0);
        ui.label(text);
    })
}

/// Label followed by an ⓘ icon, for form rows.
pub fn label_with_info(ui: &mut egui::Ui, label: &str, text: &str) {
    ui.label(label);
    info(ui, text);
}

/// Bold label followed by an ⓘ icon, for control groups.
pub fn strong_with_info(ui: &mut egui::Ui, label: &str, text: &str) {
    ui.horizontal(|ui| {
        ui.strong(label);
        info(ui, text);
    });
}

/// Heading followed by an ⓘ icon that explains what the section is for.
pub fn heading_with_info(ui: &mut egui::Ui, title: &str, text: &str) {
    ui.horizontal(|ui| {
        ui.heading(title);
        info(ui, text);
    });
}

/// Frame for a content panel: panel background inside a gap on the black window.
pub fn panel_frame(inner: i8) -> egui::Frame {
    egui::Frame::new()
        .fill(PANEL_BG)
        .inner_margin(egui::Margin::same(inner))
        .outer_margin(egui::Margin::same(GAP))
}
