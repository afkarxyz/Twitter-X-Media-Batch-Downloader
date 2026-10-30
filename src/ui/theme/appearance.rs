//! The visual tokens and geometry of the previous desktop interface.
use crate::ui::*;

pub const RAIL_WIDTH: f32 = 56.0;
pub const SETTINGS_WIDTH: f32 = 230.0;
pub const CONTENT_INSET: f32 = 12.0;
pub const SEARCH_WIDTH: f32 = 650.0;
pub const SEARCH_HEIGHT: f32 = 40.0;
pub const SETTINGS_CONTENT_WIDTH: f32 = 820.0;
pub const SUPPORT_WIDTH: f32 = 544.0;
pub const CONTROL_HEIGHT: f32 = 32.0;
pub const CONTROL_ICON_SIZE: f32 = 16.0;
pub const NAVIGATION_SIZE: f32 = 40.0;
pub const NAVIGATION_ICON_SIZE: f32 = 18.0;
pub const COMPACT_CONTROL_SIZE: f32 = 28.0;
pub const ACCOUNT_CARD_HEIGHT: f32 = 94.0;
pub const ACCOUNT_CARD_ROW_GAP: f32 = 10.0;
pub const DANGER_FILL: Color32 = Color32::from_rgb(183, 55, 49);
pub const DANGER_HOVER: Color32 = Color32::from_rgb(200, 68, 61);
pub const DANGER_PRESSED: Color32 = Color32::from_rgb(146, 45, 40);

pub(in crate::ui) trait ThemedScrollArea {
    fn show_themed<R>(
        self,
        ui: &mut Ui,
        content: impl FnOnce(&mut Ui) -> R,
    ) -> egui::scroll_area::ScrollAreaOutput<R>;
}

impl ThemedScrollArea for egui::ScrollArea {
    fn show_themed<R>(
        self,
        ui: &mut Ui,
        content: impl FnOnce(&mut Ui) -> R,
    ) -> egui::scroll_area::ScrollAreaOutput<R> {
        let content_style = ui.style().clone();
        let dark = ui.visuals().dark_mode;
        let visuals = ui.visuals_mut();
        visuals.extreme_bg_color = if dark {
            Color32::from_gray(44)
        } else {
            Color32::from_gray(240)
        };
        for state in [
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
        ] {
            state.bg_fill = if dark {
                Color32::from_white_alpha(20)
            } else {
                Color32::from_black_alpha(115)
            };
            state.corner_radius = egui::CornerRadius::same(3);
        }
        let output = self.show(ui, |ui| {
            // Only scrollbar painting inherits these colors; controls keep their own states.
            ui.set_style(content_style.clone());
            content(ui)
        });
        ui.set_style(content_style);
        output
    }
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub page: Color32,
    pub surface: Color32,
    pub sidebar: Color32,
    pub control: Color32,
    pub muted_control: Color32,
    pub heading: Color32,
    pub title: Color32,
    pub placeholder: Color32,
    pub body: Color32,
    pub secondary: Color32,
    pub muted: Color32,
    pub border: Color32,
    pub control_border: Color32,
    pub primary: Color32,
    pub accent: Color32,
    pub on_accent: Color32,
    pub accent_soft: Color32,
    pub input_border: Color32,
    pub danger: Color32,
}
pub(in crate::ui) fn mix(a: Color32, b: Color32, weight: f32) -> Color32 {
    // Slint rounds exact half-channel values down when quantizing its palette.
    let channel = |a: u8, b: u8| {
        let value = a as f32 * weight + b as f32 * (1.0 - weight);
        if value.fract() > 0.5 {
            (value + 0.5) as u8
        } else {
            value as u8
        }
    };
    Color32::from_rgb(
        channel(a.r(), b.r()),
        channel(a.g(), b.g()),
        channel(a.b(), b.b()),
    )
}
pub(in crate::ui) fn luminance(color: Color32) -> f32 {
    let linear = |value: u8| {
        let value = value as f32 / 255.0;
        if value <= 0.03928 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
}
pub(in crate::ui) fn contrast(a: Color32, b: Color32) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
impl Palette {
    pub fn new(preset: &str, dark: bool) -> Self {
        let scheme = schemes::find(preset);
        let colors = if dark { scheme.dark } else { scheme.light };
        let background = schemes::rgb(colors.background);
        let background_alt = schemes::rgb(colors.background_alt);
        let text = schemes::rgb(colors.text);
        // Cards sit above the page in both modes, so the lighter background is the surface.
        let (page, surface) = match luminance(background).total_cmp(&luminance(background_alt)) {
            std::cmp::Ordering::Less => (background, background_alt),
            std::cmp::Ordering::Greater => (background_alt, background),
            std::cmp::Ordering::Equal if dark => (background, mix(text, background, 0.05)),
            std::cmp::Ordering::Equal => (mix(text, background, 0.04), background),
        };
        let secondary = schemes::rgb(colors.text_muted);
        let border = schemes::rgb(colors.border);
        let accent = schemes::rgb(colors.primary);
        let on_accent = if contrast(accent, Color32::WHITE) >= contrast(accent, page) {
            Color32::WHITE
        } else {
            page
        };
        Self {
            page,
            surface,
            sidebar: mix(page, surface, 0.72),
            control: mix(surface, text, if dark { 0.9 } else { 0.94 }),
            muted_control: mix(surface, text, if dark { 0.86 } else { 0.90 }),
            heading: mix(text, page, 0.94),
            title: text,
            placeholder: mix(text, surface, 0.68),
            body: mix(text, page, 0.88),
            secondary,
            muted: mix(secondary, page, 0.85),
            border,
            control_border: mix(border, text, if dark { 0.76 } else { 0.88 }),
            input_border: mix(border, text, if dark { 0.80 } else { 0.86 }),
            // Media overlays always carry white icons, so they use the light-mode primary.
            primary: schemes::rgb(scheme.light.primary),
            accent,
            on_accent,
            accent_soft: mix(accent, surface, if dark { 0.20 } else { 0.14 }),
            danger: if dark {
                Color32::from_rgb(248, 113, 113)
            } else {
                DANGER_FILL
            },
        }
    }
    pub fn apply(self, visuals: &mut egui::Visuals) {
        visuals.panel_fill = self.page;
        visuals.window_fill = self.surface;
        visuals.extreme_bg_color = self.surface;
        visuals.window_stroke = egui::Stroke::new(1.0, self.border);
        visuals.window_corner_radius = egui::CornerRadius::same(12);
        visuals.menu_corner_radius = egui::CornerRadius::same(8);
        visuals.override_text_color = Some(self.body);
        visuals.weak_text_color = Some(self.secondary);
        visuals.hyperlink_color = self.accent;
        visuals.selection.bg_fill = self.accent;
        visuals.selection.stroke = egui::Stroke::new(1.0, Color32::WHITE);
        for widget in [
            &mut visuals.widgets.noninteractive,
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
            &mut visuals.widgets.open,
        ] {
            widget.corner_radius = egui::CornerRadius::same(6);
            widget.bg_fill = self.surface;
            widget.weak_bg_fill = self.surface;
            widget.bg_stroke = egui::Stroke::new(1.0, self.control_border);
            widget.fg_stroke = egui::Stroke::new(1.0, self.heading);
            widget.expansion = 0.0;
        }
        visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, self.border);
        visuals.widgets.hovered.bg_fill = self.control;
        visuals.widgets.hovered.weak_bg_fill = self.control;
        visuals.widgets.active.bg_fill = self.muted_control;
        visuals.widgets.active.weak_bg_fill = self.muted_control;
        visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, self.accent);
        visuals.widgets.open.bg_fill = self.control;
        visuals.widgets.open.weak_bg_fill = self.control;
    }
}
pub struct Card {
    frame: egui::Frame,
    width: f32,
}
impl Card {
    pub fn show<R>(
        self,
        ui: &mut Ui,
        content: impl FnOnce(&mut Ui) -> R,
    ) -> egui::InnerResponse<R> {
        self.frame.show(ui, |ui| {
            let width = (self.width - 34.0).max(0.0);
            ui.set_min_width(width);
            ui.set_max_width(width);
            ui.spacing_mut().item_spacing.y = 10.0;
            content(ui)
        })
    }
}
pub fn card(ui: &mut Ui) -> Card {
    Card {
        width: ui.available_width(),
        frame: egui::Frame::new()
            .fill(ui.visuals().window_fill)
            .stroke(ui.visuals().window_stroke)
            .corner_radius(8)
            .inner_margin(16),
    }
}
impl Desktop {
    pub(in crate::ui) fn colors(&self, ui: &Ui) -> Palette {
        Palette::new(
            &self.app.global::<AppTheme>().get_preset(),
            ui.visuals().dark_mode,
        )
    }
    pub(in crate::ui) fn icon_button(
        &self,
        ui: &mut Ui,
        icon: Icon,
        label: &str,
        selected: bool,
        size: f32,
    ) -> egui::Response {
        let colors = self.colors(ui);
        self.home_icon(
            ui,
            icon,
            label,
            size,
            crate::ui::components::icon_button::IconStyle::ghost(colors, selected),
        )
    }
}
