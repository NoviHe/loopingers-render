//! Modern UI theme and helper system ported from `modern_egui` (`v0.2.3`) for `egui 0.32.3`,
//! paired with LoopKaraoke Studio's sidebar, info tooltips, and custom Inter/JetBrains Mono fonts.

#![allow(dead_code)]

use std::hash::Hash;

use eframe::egui::{
    self, epaint::Shadow, Atom, Button, Color32, Context, CornerRadius, FontFamily, FontId, Id,
    InnerResponse, IntoAtoms, Response, Stroke, Style, Theme, ThemePreference, Ui, Widget,
};

pub const PALETTE_DARK: Palette = Palette::dark();

pub const APP_BG: Color32 = PALETTE_DARK.bg;
pub const PANEL_BG: Color32 = PALETTE_DARK.surface;
pub const FIELD_BG: Color32 = PALETTE_DARK.bg;
pub const FIELD_BORDER: Color32 = PALETTE_DARK.border;
pub const SEPARATOR: Color32 = PALETTE_DARK.border;
pub const ROW_ALT: Color32 = PALETTE_DARK.surface_alt;
pub const HOVER: Color32 = Color32::from_rgb(0x28, 0x2c, 0x37);
pub const PRESSED: Color32 = Color32::from_rgb(0x34, 0x39, 0x47);
pub const TEXT: Color32 = PALETTE_DARK.text;
pub const TEXT_DIM: Color32 = PALETTE_DARK.text_muted;
pub const ACCENT: Color32 = PALETTE_DARK.accent;
pub const FOCUS: Color32 = PALETTE_DARK.accent_hover;
pub const SUCCESS: Color32 = PALETTE_DARK.success;
pub const DANGER: Color32 = PALETTE_DARK.danger;
pub const WARNING: Color32 = PALETTE_DARK.warning;

/// Font family names registered in `install_fonts`.
pub const SEMIBOLD: &str = "semibold";
pub const MEDIUM: &str = "medium";

/// Gap between panels, in points.
pub const GAP: i8 = 6;
pub const SIDEBAR_WIDTH: f32 = 132.0;
pub const SIDEBAR_COMPACT_WIDTH: f32 = 56.0;

pub mod metrics {
    //! Design tokens: typography, spacing, control sizes, corner radii and layout.

    use eframe::egui::{self, FontFamily, FontId, Style, TextStyle};

    use super::SEMIBOLD;

    /// Base unit used to derive the theme's typography and layout metrics.
    pub const EM: f32 = 14.0;

    // Typography
    pub const FONT_XS: f32 = EM * 0.80;
    pub const FONT_SM: f32 = EM * 0.90;
    pub const FONT_MD: f32 = EM;
    pub const FONT_CT: f32 = EM * 1.14;
    pub const FONT_LG: f32 = EM * 1.28;
    pub const FONT_XL: f32 = EM * 1.47;
    pub const FONT_2XL: f32 = EM * 1.73;

    // Spacing
    pub const SPACE_1: f32 = EM * 0.27;
    pub const SPACE_2: f32 = EM * 0.53;
    pub const SPACE_3: f32 = EM * 0.80;
    pub const SPACE_4: f32 = EM * 1.07;
    pub const SPACE_5: f32 = EM * 1.33;
    pub const SPACE_6: f32 = EM * 1.60;

    // Controls
    pub const CONTROL_HEIGHT: f32 = EM * 2.1;
    pub const CONTROL_HEIGHT_LG: f32 = EM * 2.6;
    pub const BUTTON_PAD_X: f32 = EM * 0.67;
    pub const BUTTON_PAD_Y: f32 = EM * 0.40;
    pub const INPUT_HEIGHT: f32 = EM * 2.0;
    pub const INPUT_WIDTH: f32 = EM * 18.0;

    // Radius
    pub const RADIUS_SM: f32 = EM * 0.27;
    pub const RADIUS_MD: f32 = EM * 0.53;
    pub const RADIUS_LG: f32 = EM * 0.80;

    // Gap
    pub const GAP_INLINE: f32 = SPACE_2;
    pub const GAP_GROUP: f32 = SPACE_3;
    pub const GAP_SECTION: f32 = SPACE_4;
    pub const GAP_PAGE: f32 = SPACE_5;

    // Layouts
    pub const PANEL_PADDING_VALUE: f32 = EM * 1.07;
    pub const PANEL_PADDING: egui::Margin = egui::Margin::same(PANEL_PADDING_VALUE as i8);
    pub const SIDEBAR_WIDTH: f32 = EM * 14.0;
    pub const SIDEBAR_MIN_WIDTH: f32 = EM * 11.0;
    pub const SIDEBAR_MAX_WIDTH: f32 = EM * 21.0;

    /// Applies typography, spacing, and control metrics to an egui style.
    pub fn apply(style: &mut Style) {
        style.text_styles = [
            (
                TextStyle::Heading,
                FontId::new(FONT_XL, FontFamily::Name(SEMIBOLD.into())),
            ),
            (
                TextStyle::Body,
                FontId::new(FONT_MD, FontFamily::Proportional),
            ),
            (
                TextStyle::Button,
                FontId::new(FONT_MD, FontFamily::Proportional),
            ),
            (
                TextStyle::Small,
                FontId::new(FONT_XS, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(FONT_SM, FontFamily::Monospace),
            ),
        ]
        .into();

        style.spacing.item_spacing = egui::vec2(SPACE_2, SPACE_2);
        style.spacing.button_padding = egui::vec2(BUTTON_PAD_X, BUTTON_PAD_Y);
        style.spacing.interact_size.y = CONTROL_HEIGHT;
        style.spacing.menu_margin = egui::Margin::same(6);
        style.animation_time = 0.12;
        // Prevent egui from drawing bright orange debug lines under sub-pixel rects.
        style.debug.show_unaligned = false;
    }
}

/// Semantic color palette matching `modern_egui::theme::Palette`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub surface_alt: Color32,
    pub elevated: Color32,

    pub border: Color32,
    pub border_strong: Color32,

    pub text: Color32,
    pub text_strong: Color32,
    pub text_muted: Color32,

    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_active: Color32,
    pub on_accent: Color32,

    pub danger: Color32,
    pub danger_hover: Color32,
    pub danger_active: Color32,
    pub on_danger: Color32,

    pub success: Color32,
    pub warning: Color32,
}

impl Palette {
    pub const fn dark() -> Self {
        Self {
            bg: Color32::from_rgb(17, 18, 23),
            surface: Color32::from_rgb(25, 27, 34),
            surface_alt: Color32::from_rgb(34, 37, 46),
            elevated: Color32::from_rgb(31, 33, 41),

            border: Color32::from_rgb(45, 48, 59),
            border_strong: Color32::from_rgb(66, 70, 84),

            text: Color32::from_rgb(232, 234, 240),
            text_strong: Color32::from_rgb(250, 251, 254),
            text_muted: Color32::from_rgb(143, 149, 166),

            accent: Color32::from_rgb(59, 130, 246),
            accent_hover: Color32::from_rgb(96, 165, 250),
            accent_active: Color32::from_rgb(37, 99, 235),
            on_accent: Color32::from_rgb(255, 255, 255),

            danger: Color32::from_rgb(226, 78, 84),
            danger_hover: Color32::from_rgb(240, 97, 103),
            danger_active: Color32::from_rgb(198, 62, 68),
            on_danger: Color32::from_rgb(255, 255, 255),

            success: Color32::from_rgb(64, 190, 130),
            warning: Color32::from_rgb(230, 170, 70),
        }
    }

    pub const fn light() -> Self {
        Self {
            bg: Color32::from_rgb(244, 245, 248),
            surface: Color32::from_rgb(255, 255, 255),
            surface_alt: Color32::from_rgb(238, 240, 245),
            elevated: Color32::from_rgb(255, 255, 255),

            border: Color32::from_rgb(222, 225, 232),
            border_strong: Color32::from_rgb(196, 201, 212),

            text: Color32::from_rgb(28, 31, 40),
            text_strong: Color32::from_rgb(9, 11, 17),
            text_muted: Color32::from_rgb(106, 112, 128),

            accent: Color32::from_rgb(37, 99, 235),
            accent_hover: Color32::from_rgb(29, 78, 216),
            accent_active: Color32::from_rgb(30, 64, 175),
            on_accent: Color32::from_rgb(255, 255, 255),

            danger: Color32::from_rgb(211, 57, 63),
            danger_hover: Color32::from_rgb(226, 74, 80),
            danger_active: Color32::from_rgb(184, 46, 52),
            on_danger: Color32::from_rgb(255, 255, 255),

            success: Color32::from_rgb(34, 154, 96),
            warning: Color32::from_rgb(190, 130, 30),
        }
    }

    pub fn store(self, ctx: &Context, theme: Theme) {
        ctx.data_mut(|d| d.insert_temp(Self::id(theme), self));
    }

    fn id(theme: Theme) -> Id {
        Id::new(("modern_egui_palette", theme))
    }

    pub fn of(ctx: &Context) -> Self {
        let theme = ctx.theme();
        ctx.data(|d| d.get_temp::<Self>(Self::id(theme)))
            .unwrap_or_else(|| Self::for_theme(theme))
    }

    pub fn for_theme(theme: Theme) -> Self {
        match theme {
            Theme::Dark => Self::dark(),
            Theme::Light => Self::light(),
        }
    }
}

pub const fn rgb_hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

/// Applies the `modern_egui` dark/light palettes, widget visuals, and spacing metrics.
pub fn apply(ctx: &Context) {
    ctx.set_theme(ThemePreference::Dark);
    apply_with(ctx, Palette::dark(), Palette::light());
}

pub fn apply_with(ctx: &Context, dark: Palette, light: Palette) {
    dark.store(ctx, Theme::Dark);
    light.store(ctx, Theme::Light);

    ctx.style_mut_of(Theme::Dark, |s| {
        s.visuals = egui::Visuals::dark();
        colors(s, &dark);
    });

    ctx.style_mut_of(Theme::Light, |s| {
        s.visuals = egui::Visuals::light();
        colors(s, &light);
    });

    ctx.all_styles_mut(metrics::apply);
}

fn colors(s: &mut Style, p: &Palette) {
    let sm_radius = CornerRadius::same(metrics::RADIUS_SM as u8);
    let md_radius = CornerRadius::same(metrics::RADIUS_MD as u8);

    let w = &mut s.visuals.widgets;
    for state in [
        &mut w.noninteractive,
        &mut w.inactive,
        &mut w.hovered,
        &mut w.active,
        &mut w.open,
    ] {
        state.corner_radius = sm_radius;
    }

    // Separators, frame outlines, non-interactive text.
    w.noninteractive.bg_fill = p.surface;
    w.noninteractive.weak_bg_fill = p.surface;
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.text);

    // Buttons, combo boxes and checkboxes at rest.
    w.inactive.weak_bg_fill = p.surface_alt;
    w.inactive.bg_fill = p.surface_alt;
    w.inactive.bg_stroke = Stroke::new(1.0_f32, p.border);
    w.inactive.fg_stroke = Stroke::new(1.0_f32, p.text);

    w.hovered.weak_bg_fill = p.surface_alt.lerp_to_gamma(p.text, 0.08);
    w.hovered.bg_fill = p.surface_alt.lerp_to_gamma(p.text, 0.08);
    w.hovered.bg_stroke = Stroke::new(1.0_f32, p.border_strong);
    w.hovered.fg_stroke = Stroke::new(1.0_f32, p.text_strong);

    // Pressed state.
    w.active.weak_bg_fill = p.surface_alt.lerp_to_gamma(p.text, 0.16);
    w.active.bg_fill = p.surface_alt.lerp_to_gamma(p.text, 0.16);
    w.active.bg_stroke = Stroke::new(1.0_f32, p.accent);
    w.active.fg_stroke = Stroke::new(1.0_f32, p.text_strong);

    w.open.weak_bg_fill = p.surface_alt;
    w.open.bg_fill = p.surface_alt;
    w.open.bg_stroke = Stroke::new(1.0_f32, p.border_strong);
    w.open.fg_stroke = Stroke::new(1.0_f32, p.text);

    // Surfaces & floating popups.
    s.visuals.panel_fill = p.bg;
    s.visuals.window_fill = p.elevated;
    s.visuals.window_stroke = Stroke::new(1.0_f32, p.border);
    s.visuals.window_corner_radius = md_radius;
    s.visuals.menu_corner_radius = md_radius;
    s.visuals.popup_shadow = Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(150),
    };
    s.visuals.window_shadow = s.visuals.popup_shadow;
    s.visuals.extreme_bg_color = p.bg;
    s.visuals.faint_bg_color = p.surface_alt.gamma_multiply(0.5);
    s.visuals.code_bg_color = p.surface_alt;

    // Selection, links, semantic colours.
    s.visuals.override_text_color = Some(p.text);
    s.visuals.selection.bg_fill = p.accent;
    s.visuals.selection.stroke = Stroke::new(1.0_f32, p.on_accent);
    s.visuals.hyperlink_color = p.accent;
    s.visuals.error_fg_color = p.danger;
    s.visuals.warn_fg_color = p.warning;
    s.visuals.text_cursor.stroke = Stroke::new(2.0_f32, p.accent);
}

/// Semantic text sizes matching `modern_egui`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSize {
    Xs,
    Sm,
    Md,
    CardTitle,
    Lg,
    Xl,
    Xxl,
}

impl TextSize {
    pub const fn value(self) -> f32 {
        match self {
            Self::Xs => metrics::FONT_XS,
            Self::Sm => metrics::FONT_SM,
            Self::Md => metrics::FONT_MD,
            Self::CardTitle => metrics::FONT_CT,
            Self::Lg => metrics::FONT_LG,
            Self::Xl => metrics::FONT_XL,
            Self::Xxl => metrics::FONT_2XL,
        }
    }
}

/// Semantic color roles available to [`StyledText`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TextColor {
    #[default]
    Text,
    Strong,
    Muted,
    Accent,
    Success,
    Warning,
    Danger,
    Custom(Color32),
}

/// A themed text widget with configurable size, color, and emphasis.
pub struct StyledText {
    text: String,
    size: TextSize,
    color: TextColor,
}

impl StyledText {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            size: TextSize::Md,
            color: TextColor::default(),
        }
    }

    #[must_use]
    pub fn size(mut self, size: TextSize) -> Self {
        self.size = size;
        self
    }

    #[must_use]
    pub fn color(mut self, color: TextColor) -> Self {
        self.color = color;
        self
    }
}

impl Widget for StyledText {
    fn ui(self, ui: &mut Ui) -> Response {
        let p = Palette::of(ui.ctx());
        let color = match self.color {
            TextColor::Text => p.text,
            TextColor::Strong => p.text_strong,
            TextColor::Muted => p.text_muted,
            TextColor::Accent => p.accent_hover,
            TextColor::Success => p.success,
            TextColor::Warning => p.warning,
            TextColor::Danger => p.danger,
            TextColor::Custom(color) => color,
        };
        let mut rich = egui::RichText::new(self.text)
            .size(self.size.value())
            .color(color);
        if matches!(
            self.size,
            TextSize::CardTitle | TextSize::Lg | TextSize::Xl | TextSize::Xxl
        ) {
            rich = rich.family(FontFamily::Name(SEMIBOLD.into()));
        }
        ui.label(rich)
    }
}

/// Visual variant of a [`StyledButton`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Primary,
    Secondary,
    Ghost,
    Danger,
}

/// A themed wrapper around [`egui::Button`].
pub struct StyledButton<'a> {
    inner: Button<'a>,
    variant: Variant,
}

impl<'a> StyledButton<'a> {
    pub fn new(atoms: impl IntoAtoms<'a>, variant: Variant) -> Self {
        let inner = Button::new(atoms)
            .corner_radius(CornerRadius::same(metrics::RADIUS_SM as u8))
            .wrap_mode(egui::TextWrapMode::Extend);
        Self { variant, inner }
    }

    pub fn primary(atoms: impl IntoAtoms<'a>) -> Self {
        Self::new(atoms, Variant::Primary)
    }

    pub fn secondary(atoms: impl IntoAtoms<'a>) -> Self {
        Self::new(atoms, Variant::Secondary)
    }

    pub fn ghost(atoms: impl IntoAtoms<'a>) -> Self {
        Self::new(atoms, Variant::Ghost)
    }

    pub fn danger(atoms: impl IntoAtoms<'a>) -> Self {
        Self::new(atoms, Variant::Danger)
    }

    pub fn min_size(mut self, size: egui::Vec2) -> Self {
        self.inner = self.inner.min_size(size);
        self
    }

    pub fn small(mut self) -> Self {
        self.inner = self.inner.small();
        self
    }

    pub fn corner_radius(mut self, radius: impl Into<CornerRadius>) -> Self {
        self.inner = self.inner.corner_radius(radius);
        self
    }

    pub fn sense(mut self, sense: egui::Sense) -> Self {
        self.inner = self.inner.sense(sense);
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.inner = self.inner.selected(selected);
        self
    }

    pub fn shortcut_text(mut self, text: impl Into<Atom<'a>>) -> Self {
        self.inner = self.inner.shortcut_text(text);
        self
    }

    pub fn wrap(mut self) -> Self {
        self.inner = self.inner.wrap();
        self
    }

    pub fn truncate(mut self) -> Self {
        self.inner = self.inner.truncate();
        self
    }
}

impl Widget for StyledButton<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.scope(|ui| {
            self.variant.apply(ui);
            self.inner.ui(ui)
        })
        .inner
    }
}

impl Variant {
    fn apply(&self, ui: &mut Ui) {
        let p = Palette::of(ui.ctx());
        let widgets = &mut ui.style_mut().visuals.widgets;

        match self {
            Variant::Primary => {
                widgets.inactive.bg_fill = p.accent;
                widgets.inactive.weak_bg_fill = p.accent;
                widgets.inactive.bg_stroke = Stroke::new(1.0_f32, p.accent);
                widgets.inactive.fg_stroke.color = p.on_accent;

                widgets.hovered.bg_fill = p.accent_hover;
                widgets.hovered.weak_bg_fill = p.accent_hover;
                widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.accent_hover);
                widgets.hovered.fg_stroke.color = p.on_accent;

                widgets.active.bg_fill = p.accent_active;
                widgets.active.weak_bg_fill = p.accent_active;
                widgets.active.bg_stroke = Stroke::new(1.0_f32, p.accent_active);
                widgets.active.fg_stroke.color = p.on_accent;
            }
            Variant::Secondary => {}
            Variant::Ghost => {
                widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::TRANSPARENT);
                widgets.inactive.fg_stroke.color = p.text;

                widgets.hovered.weak_bg_fill = p.surface_alt;
                widgets.hovered.bg_fill = p.surface_alt;
                widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.border);
                widgets.hovered.fg_stroke.color = p.text_strong;

                widgets.active.weak_bg_fill = p.elevated;
                widgets.active.bg_fill = p.elevated;
                widgets.active.bg_stroke = Stroke::new(1.0_f32, p.border_strong);
                widgets.active.fg_stroke.color = p.text_strong;
            }
            Variant::Danger => {
                widgets.inactive.bg_fill = p.danger;
                widgets.inactive.weak_bg_fill = p.danger;
                widgets.inactive.bg_stroke = Stroke::new(1.0_f32, p.danger);
                widgets.inactive.fg_stroke.color = p.on_danger;

                widgets.hovered.bg_fill = p.danger_hover;
                widgets.hovered.weak_bg_fill = p.danger_hover;
                widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.danger_hover);
                widgets.hovered.fg_stroke.color = p.on_danger;

                widgets.active.bg_fill = p.danger_active;
                widgets.active.weak_bg_fill = p.danger_active;
                widgets.active.bg_stroke = Stroke::new(1.0_f32, p.danger_active);
                widgets.active.fg_stroke.color = p.on_danger;
            }
        }
    }
}

/// Spacing helpers for [`egui::Ui`] based on the theme's metric scale.
pub trait UiMetrics {
    fn space1(&mut self);
    fn space2(&mut self);
    fn space3(&mut self);
    fn space4(&mut self);
    fn space5(&mut self);
    fn space6(&mut self);
    fn space_inline(&mut self);
    fn space_group(&mut self);
    fn space_section(&mut self);
    fn space_page(&mut self);
}

impl UiMetrics for Ui {
    fn space1(&mut self) {
        self.add_space(metrics::SPACE_1);
    }
    fn space2(&mut self) {
        self.add_space(metrics::SPACE_2);
    }
    fn space3(&mut self) {
        self.add_space(metrics::SPACE_3);
    }
    fn space4(&mut self) {
        self.add_space(metrics::SPACE_4);
    }
    fn space5(&mut self) {
        self.add_space(metrics::SPACE_5);
    }
    fn space6(&mut self) {
        self.add_space(metrics::SPACE_6);
    }
    fn space_inline(&mut self) {
        self.add_space(metrics::GAP_INLINE);
    }
    fn space_group(&mut self) {
        self.add_space(metrics::GAP_GROUP);
    }
    fn space_section(&mut self) {
        self.add_space(metrics::GAP_SECTION);
    }
    fn space_page(&mut self) {
        self.add_space(metrics::GAP_PAGE);
    }
}

/// Typography helpers for applying common semantic text roles to [`egui::Ui`].
pub trait UiText {
    fn app_title(&mut self, text: impl Into<String>) -> Response;
    fn page_title(&mut self, text: impl Into<String>) -> Response;
    fn section_title(&mut self, text: impl Into<String>) -> Response;
    fn card_title(&mut self, text: impl Into<String>) -> Response;
    fn muted_label(&mut self, text: impl Into<String>) -> Response;
    fn metadata_label(&mut self, text: impl Into<String>) -> Response;
    fn strong_label(&mut self, text: impl Into<String>) -> Response;
    fn text(&mut self, text: impl Into<String>, size: TextSize) -> Response;
    fn text_colored(
        &mut self,
        text: impl Into<String>,
        size: TextSize,
        color: TextColor,
    ) -> Response;
}

impl UiText for Ui {
    fn app_title(&mut self, text: impl Into<String>) -> Response {
        self.add(
            StyledText::new(text)
                .size(TextSize::Xxl)
                .color(TextColor::Strong),
        )
    }

    fn page_title(&mut self, text: impl Into<String>) -> Response {
        self.add(
            StyledText::new(text)
                .size(TextSize::Xl)
                .color(TextColor::Strong),
        )
    }

    fn section_title(&mut self, text: impl Into<String>) -> Response {
        self.add(
            StyledText::new(text)
                .size(TextSize::Lg)
                .color(TextColor::Strong),
        )
    }

    fn card_title(&mut self, text: impl Into<String>) -> Response {
        self.add(
            StyledText::new(text)
                .size(TextSize::CardTitle)
                .color(TextColor::Strong),
        )
    }

    fn muted_label(&mut self, text: impl Into<String>) -> Response {
        self.add(
            StyledText::new(text)
                .size(TextSize::Sm)
                .color(TextColor::Muted),
        )
    }

    fn metadata_label(&mut self, text: impl Into<String>) -> Response {
        self.add(
            StyledText::new(text)
                .size(TextSize::Xs)
                .color(TextColor::Muted),
        )
    }

    fn strong_label(&mut self, text: impl Into<String>) -> Response {
        self.add(
            StyledText::new(text)
                .size(TextSize::Md)
                .color(TextColor::Strong),
        )
    }

    fn text(&mut self, text: impl Into<String>, size: TextSize) -> Response {
        self.add(StyledText::new(text).size(size))
    }

    fn text_colored(
        &mut self,
        text: impl Into<String>,
        size: TextSize,
        color: TextColor,
    ) -> Response {
        self.add(StyledText::new(text).size(size).color(color))
    }
}

/// Width of a themed single-line text input.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputWidth {
    Standard,
    Fixed(f32),
    Fill,
}

/// Convenience helpers for themed single-line text inputs.
pub trait UiInputs {
    fn text_input(&mut self, text: &mut String) -> Response;
    fn text_input_with_width(&mut self, text: &mut String, width: InputWidth) -> Response;
    fn text_input_hint(&mut self, text: &mut String, hint: impl Into<egui::WidgetText>)
        -> Response;
    fn text_input_hint_with_width(
        &mut self,
        text: &mut String,
        hint: impl Into<egui::WidgetText>,
        width: InputWidth,
    ) -> Response;
}

fn add_text_input(
    ui: &mut Ui,
    text: &mut String,
    hint: Option<egui::WidgetText>,
    width: InputWidth,
) -> Response {
    let width = match width {
        InputWidth::Standard => metrics::INPUT_WIDTH,
        InputWidth::Fill => ui.available_width(),
        InputWidth::Fixed(w) => w,
    };

    let mut input = egui::TextEdit::singleline(text).vertical_align(egui::Align::Center);
    if let Some(hint) = hint {
        input = input.hint_text(hint);
    }
    ui.add_sized([width, metrics::INPUT_HEIGHT], input)
}

impl UiInputs for Ui {
    fn text_input(&mut self, text: &mut String) -> Response {
        add_text_input(self, text, None, InputWidth::Standard)
    }

    fn text_input_hint(
        &mut self,
        text: &mut String,
        hint: impl Into<egui::WidgetText>,
    ) -> Response {
        add_text_input(self, text, Some(hint.into()), InputWidth::Standard)
    }

    fn text_input_with_width(&mut self, text: &mut String, width: InputWidth) -> Response {
        add_text_input(self, text, None, width)
    }

    fn text_input_hint_with_width(
        &mut self,
        text: &mut String,
        hint: impl Into<egui::WidgetText>,
        width: InputWidth,
    ) -> Response {
        add_text_input(self, text, Some(hint.into()), width)
    }
}

/// Convenience helpers for semantic button variants.
pub trait UiButtons {
    fn primary_button<'a>(&mut self, text: impl IntoAtoms<'a>) -> Response;
    fn secondary_button<'a>(&mut self, text: impl IntoAtoms<'a>) -> Response;
    fn danger_button<'a>(&mut self, text: impl IntoAtoms<'a>) -> Response;
    fn ghost_button<'a>(&mut self, text: impl IntoAtoms<'a>) -> Response;
}

impl UiButtons for Ui {
    fn primary_button<'a>(&mut self, text: impl IntoAtoms<'a>) -> Response {
        self.add(StyledButton::primary(text))
    }

    fn secondary_button<'a>(&mut self, text: impl IntoAtoms<'a>) -> Response {
        self.add(StyledButton::secondary(text))
    }

    fn danger_button<'a>(&mut self, text: impl IntoAtoms<'a>) -> Response {
        self.add(StyledButton::danger(text))
    }

    fn ghost_button<'a>(&mut self, text: impl IntoAtoms<'a>) -> Response {
        self.add(StyledButton::ghost(text))
    }
}

/// Background level of a card, mapped to the surface colors of the [`Palette`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelVariant {
    Surface,
    SurfaceAlt,
    Elevated,
}

impl PanelVariant {
    fn into_color(self, palette: Palette) -> Color32 {
        match self {
            PanelVariant::Surface => palette.surface,
            PanelVariant::SurfaceAlt => palette.surface_alt,
            PanelVariant::Elevated => palette.elevated,
        }
    }

    fn into_border_color(self, palette: Palette) -> Color32 {
        match self {
            PanelVariant::Surface | PanelVariant::SurfaceAlt => palette.border,
            PanelVariant::Elevated => palette.border_strong,
        }
    }
}

/// Themed containers for grouping related content.
pub trait UiPanels {
    fn card<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>;
    fn interactive_card<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>;
    fn card_with_variant<R>(
        &mut self,
        variant: PanelVariant,
        add_contents: impl FnOnce(&mut Ui) -> R,
    ) -> InnerResponse<R>;
}

impl UiPanels for Ui {
    fn card_with_variant<R>(
        &mut self,
        variant: PanelVariant,
        add_contents: impl FnOnce(&mut Ui) -> R,
    ) -> InnerResponse<R> {
        let p = Palette::of(self.ctx());

        egui::Frame::new()
            .fill(variant.into_color(p))
            .stroke(Stroke::new(1.0_f32, variant.into_border_color(p)))
            .corner_radius(CornerRadius::same(metrics::RADIUS_MD as u8))
            .inner_margin(metrics::PANEL_PADDING)
            .show(self, add_contents)
    }

    fn card<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
        self.card_with_variant(PanelVariant::Surface, add_contents)
    }

    fn interactive_card<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
        let p = Palette::of(self.ctx());
        let mut frame = egui::Frame::new()
            .fill(PanelVariant::Surface.into_color(p))
            .stroke(Stroke::new(
                1.0_f32,
                PanelVariant::Surface.into_border_color(p),
            ))
            .corner_radius(CornerRadius::same(metrics::RADIUS_MD as u8))
            .inner_margin(metrics::PANEL_PADDING)
            .begin(self);

        let inner = add_contents(&mut frame.content_ui);
        let response = frame.allocate_space(self);

        if response.hovered() {
            frame.frame.fill = PanelVariant::Elevated.into_color(p);
            frame.frame.stroke = Stroke::new(1.0_f32, PanelVariant::Elevated.into_border_color(p));
        }
        frame.paint(self);

        InnerResponse::new(inner, response)
    }
}

/// Layout helpers for arranging groups of widgets on an [`egui::Ui`].
pub trait UiLayouts {
    fn center_row<R>(
        &mut self,
        id_salt: impl Hash,
        add_contents: impl FnOnce(&mut Ui) -> R,
    ) -> InnerResponse<R>;
}

impl UiLayouts for Ui {
    fn center_row<R>(
        &mut self,
        id_salt: impl Hash,
        add_contents: impl FnOnce(&mut Ui) -> R,
    ) -> InnerResponse<R> {
        let id = self.id().with(id_salt);
        let last_width: Option<f32> = self.data(|d| d.get_temp(id));

        let response = self.horizontal(|ui| {
            let available = ui.available_width();

            let margin = match last_width {
                Some(w) if available.is_finite() => ((available - w) / 2.0).max(0.0),
                _ => 0.0,
            };
            ui.add_space(margin);

            let inner = ui.scope(|ui| add_contents(ui));

            let width = inner.response.rect.width();
            ui.data_mut(|d| d.insert_temp(id, width));

            if last_width.is_none_or(|w| (w - width).abs() > 0.1) {
                ui.ctx()
                    .request_discard("center_row: measured width changed")
            }

            inner
        });

        response.inner
    }
}

/// The main action of a screen: accent-filled `StyledButton::primary`.
pub fn primary_button(text: impl Into<String>) -> StyledButton<'static> {
    StyledButton::primary(text.into())
}

#[derive(Clone, Copy)]
pub enum SidebarIcon {
    Dashboard,
    Generate,
    Background,
    Playlist,
    Lyrics,
    Karaoke,
    Visualizer,
    Render,
    Thumbnail,
    Settings,
}

/// Collapse button (`◧`) in the expanded sidebar header.
pub fn sidebar_toggle_button(ui: &mut Ui) -> bool {
    let p = Palette::of(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(egui::vec2(26.0, 26.0), egui::Sense::click());
    let painter = ui.painter();
    if response.hovered() {
        painter.rect_filled(
            rect,
            CornerRadius::same(metrics::RADIUS_SM as u8),
            p.surface_alt,
        );
    }
    let color = if response.hovered() {
        p.text_strong
    } else {
        p.text_muted
    };
    let panel = egui::Rect::from_center_size(rect.center(), egui::vec2(15.0, 12.0));
    painter.rect_stroke(
        panel,
        CornerRadius::same(2),
        Stroke::new(1.3_f32, color),
        egui::StrokeKind::Middle,
    );
    let split_x = panel.min.x + 5.0;
    painter.line_segment(
        [
            egui::pos2(split_x, panel.min.y),
            egui::pos2(split_x, panel.max.y),
        ],
        Stroke::new(1.3_f32, color),
    );
    let clicked = response.clicked();
    response.on_hover_text("Ciutkan sidebar");
    clicked
}

fn draw_sidebar_icon(painter: &egui::Painter, c: egui::Pos2, icon: SidebarIcon, color: Color32) {
    let stroke = Stroke::new(1.3_f32, color);
    match icon {
        SidebarIcon::Dashboard => {
            for (dx, dy) in [(-3.5_f32, -3.5_f32), (3.5, -3.5), (-3.5, 3.5), (3.5, 3.5)] {
                let cell = egui::Rect::from_center_size(
                    egui::pos2(c.x + dx, c.y + dy),
                    egui::vec2(5.0, 5.0),
                );
                painter.rect_stroke(
                    cell,
                    CornerRadius::same(1),
                    stroke,
                    egui::StrokeKind::Middle,
                );
            }
        }
        SidebarIcon::Generate => {
            let pts = [
                egui::pos2(c.x, c.y - 6.5),
                egui::pos2(c.x + 1.8, c.y - 1.8),
                egui::pos2(c.x + 6.5, c.y),
                egui::pos2(c.x + 1.8, c.y + 1.8),
                egui::pos2(c.x, c.y + 6.5),
                egui::pos2(c.x - 1.8, c.y + 1.8),
                egui::pos2(c.x - 6.5, c.y),
                egui::pos2(c.x - 1.8, c.y - 1.8),
            ];
            for seg in pts.windows(2) {
                painter.line_segment([seg[0], seg[1]], stroke);
            }
            painter.line_segment([pts[7], pts[0]], stroke);
            painter.circle_filled(egui::pos2(c.x + 4.8, c.y - 4.8), 1.3, color);
        }
        SidebarIcon::Background => {
            let frame = egui::Rect::from_center_size(c, egui::vec2(14.0, 10.0));
            painter.rect_stroke(
                frame,
                CornerRadius::same(2),
                stroke,
                egui::StrokeKind::Middle,
            );
            painter.circle_filled(egui::pos2(c.x - 3.5, c.y - 2.0), 1.3, color);
            painter.line_segment(
                [egui::pos2(c.x - 5.0, c.y + 3.5), egui::pos2(c.x - 1.5, c.y)],
                Stroke::new(1.2_f32, color),
            );
            painter.line_segment(
                [egui::pos2(c.x - 1.5, c.y), egui::pos2(c.x + 1.0, c.y + 2.2)],
                Stroke::new(1.2_f32, color),
            );
            painter.line_segment(
                [
                    egui::pos2(c.x + 1.0, c.y + 2.2),
                    egui::pos2(c.x + 3.5, c.y - 0.5),
                ],
                Stroke::new(1.2_f32, color),
            );
            painter.line_segment(
                [
                    egui::pos2(c.x + 3.5, c.y - 0.5),
                    egui::pos2(c.x + 5.5, c.y + 3.5),
                ],
                Stroke::new(1.2_f32, color),
            );
        }
        SidebarIcon::Playlist => {
            for dy in [-4.0_f32, 0.0, 4.0] {
                painter.circle_filled(egui::pos2(c.x - 5.2, c.y + dy), 1.2, color);
                painter.line_segment(
                    [
                        egui::pos2(c.x - 2.2, c.y + dy),
                        egui::pos2(c.x + 6.2, c.y + dy),
                    ],
                    Stroke::new(1.4_f32, color),
                );
            }
        }
        SidebarIcon::Lyrics => {
            let doc = egui::Rect::from_center_size(c, egui::vec2(11.0, 14.0));
            painter.rect_stroke(doc, CornerRadius::same(2), stroke, egui::StrokeKind::Middle);
            for (dy, end_x) in [(-3.0_f32, 3.0_f32), (0.0, 3.0), (3.0, 1.2)] {
                painter.line_segment(
                    [
                        egui::pos2(c.x - 3.0, c.y + dy),
                        egui::pos2(c.x + end_x, c.y + dy),
                    ],
                    Stroke::new(1.2_f32, color),
                );
            }
        }
        SidebarIcon::Karaoke => {
            let mic =
                egui::Rect::from_center_size(egui::pos2(c.x, c.y - 2.2), egui::vec2(4.4, 7.2));
            painter.rect_stroke(mic, CornerRadius::same(2), stroke, egui::StrokeKind::Middle);
            let cradle = [
                egui::pos2(c.x - 4.2, c.y - 1.5),
                egui::pos2(c.x - 4.2, c.y + 1.2),
                egui::pos2(c.x - 2.2, c.y + 3.2),
                egui::pos2(c.x + 2.2, c.y + 3.2),
                egui::pos2(c.x + 4.2, c.y + 1.2),
                egui::pos2(c.x + 4.2, c.y - 1.5),
            ];
            for seg in cradle.windows(2) {
                painter.line_segment([seg[0], seg[1]], Stroke::new(1.2_f32, color));
            }
            painter.line_segment(
                [egui::pos2(c.x, c.y + 3.2), egui::pos2(c.x, c.y + 6.2)],
                Stroke::new(1.2_f32, color),
            );
            painter.line_segment(
                [
                    egui::pos2(c.x - 3.0, c.y + 6.2),
                    egui::pos2(c.x + 3.0, c.y + 6.2),
                ],
                Stroke::new(1.2_f32, color),
            );
        }
        SidebarIcon::Visualizer => {
            for (dx, h) in [
                (-6.0_f32, 5.0_f32),
                (-3.0, 11.0),
                (0.0, 7.0),
                (3.0, 13.0),
                (6.0, 6.0),
            ] {
                painter.line_segment(
                    [
                        egui::pos2(c.x + dx, c.y - h / 2.0),
                        egui::pos2(c.x + dx, c.y + h / 2.0),
                    ],
                    Stroke::new(1.8_f32, color),
                );
            }
        }
        SidebarIcon::Render => {
            let box_rect = egui::Rect::from_center_size(c, egui::vec2(13.0, 11.0));
            painter.rect_stroke(
                box_rect,
                CornerRadius::same(2),
                stroke,
                egui::StrokeKind::Middle,
            );
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(c.x - 2.0, c.y - 2.8),
                    egui::pos2(c.x - 2.0, c.y + 2.8),
                    egui::pos2(c.x + 3.0, c.y),
                ],
                color,
                Stroke::NONE,
            ));
        }
        SidebarIcon::Thumbnail => {
            let frame = egui::Rect::from_center_size(c, egui::vec2(14.0, 10.0));
            painter.rect_stroke(
                frame,
                CornerRadius::same(2),
                stroke,
                egui::StrokeKind::Middle,
            );
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(c.x - 1.5, c.y - 2.5),
                    egui::pos2(c.x - 1.5, c.y + 2.5),
                    egui::pos2(c.x + 3.0, c.y),
                ],
                color,
                Stroke::NONE,
            ));
        }
        SidebarIcon::Settings => {
            for (dy, knob_dx) in [(-4.0_f32, -2.5_f32), (0.0, 2.5), (4.0, -1.5)] {
                painter.line_segment(
                    [
                        egui::pos2(c.x - 6.5, c.y + dy),
                        egui::pos2(c.x + 6.5, c.y + dy),
                    ],
                    Stroke::new(1.3_f32, color),
                );
                painter.circle_filled(egui::pos2(c.x + knob_dx, c.y + dy), 2.0, color);
            }
        }
    }
}

/// A vertical sidebar navigation item styled with `modern_egui` palette tokens.
pub fn sidebar_tab(
    ui: &mut Ui,
    icon: SidebarIcon,
    title: &str,
    active: bool,
    done: bool,
    compact: bool,
    hint: &str,
) -> bool {
    let p = Palette::of(ui.ctx());
    let width = ui.available_width();
    let size = egui::vec2(width, 32.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let painter = ui.painter();
    let sm_radius = CornerRadius::same(metrics::RADIUS_SM as u8);

    if active {
        painter.rect_filled(rect, sm_radius, p.surface_alt);
        let bar = egui::Rect::from_min_max(
            egui::pos2(rect.min.x, rect.min.y + 5.0),
            egui::pos2(rect.min.x + 3.0, rect.max.y - 5.0),
        );
        painter.rect_filled(bar, CornerRadius::same(2), p.accent);
    } else if response.hovered() {
        painter.rect_filled(rect, sm_radius, p.elevated);
    }

    let color = if active || response.hovered() {
        p.text_strong
    } else {
        p.text_muted
    };

    if compact {
        draw_sidebar_icon(painter, rect.center(), icon, color);
        if done {
            painter.circle_filled(
                egui::pos2(rect.max.x - 7.0, rect.min.y + 7.0),
                3.0,
                p.success,
            );
        }
        let clicked = response.clicked();
        response.on_hover_text(format!("{title} — {hint}"));
        clicked
    } else {
        draw_sidebar_icon(
            painter,
            egui::pos2(rect.min.x + 18.0, rect.center().y),
            icon,
            color,
        );
        let galley = painter.layout_no_wrap(
            title.to_string(),
            FontId::proportional(metrics::FONT_MD),
            color,
        );
        let text_pos = egui::pos2(rect.min.x + 34.0, rect.center().y - galley.size().y / 2.0);
        painter.galley(text_pos, galley, color);

        if done {
            painter.circle_filled(
                egui::pos2(rect.max.x - 10.0, rect.center().y),
                3.5,
                p.success,
            );
        }

        let clicked = response.clicked();
        response.on_hover_text(hint);
        clicked
    }
}

/// Small ⓘ icon; hovering it shows `text`.
pub fn info(ui: &mut Ui, text: &str) -> Response {
    let p = Palette::of(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(16.0), egui::Sense::hover());
    let color = if response.hovered() {
        p.accent_hover
    } else {
        p.text_muted
    };
    let painter = ui.painter();
    painter.circle_stroke(rect.center(), 6.5, Stroke::new(1.0_f32, color));
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

pub fn label_with_info(ui: &mut Ui, label: &str, text: &str) {
    ui.label(label);
    info(ui, text);
}

pub fn strong_with_info(ui: &mut Ui, label: &str, text: &str) {
    ui.horizontal(|ui| {
        ui.strong_label(label);
        info(ui, text);
    });
}

pub fn heading_with_info(ui: &mut Ui, title: &str, text: &str) {
    ui.horizontal(|ui| {
        ui.page_title(title);
        info(ui, text);
    });
}

/// Frame for a floating surface panel inside a gap on the current palette background.
pub fn panel_frame(ctx: &Context, inner: i8) -> egui::Frame {
    let p = Palette::of(ctx);
    egui::Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0_f32, p.border))
        .corner_radius(CornerRadius::same(metrics::RADIUS_MD as u8))
        .inner_margin(egui::Margin::same(inner))
        .outer_margin(egui::Margin {
            left: GAP,
            right: GAP,
            top: GAP,
            bottom: 0,
        })
}
