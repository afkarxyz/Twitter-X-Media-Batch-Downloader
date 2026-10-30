use crate::ui::*;

#[derive(Clone, Copy)]
pub(in crate::ui) struct IconStyle {
    pub idle: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    pub foreground: Color32,
    pub border: egui::Stroke,
    pub radius: u8,
    pub icon_size: f32,
    pub visible: bool,
}

impl IconStyle {
    pub fn ghost(colors: appearance::Palette, selected: bool) -> Self {
        Self {
            idle: if selected {
                colors.accent_soft
            } else {
                Color32::TRANSPARENT
            },
            hover: if selected {
                colors.accent_soft
            } else {
                colors.control
            },
            pressed: if selected {
                colors.accent_soft
            } else {
                colors.muted_control
            },
            foreground: if selected {
                colors.accent
            } else {
                colors.secondary
            },
            border: egui::Stroke::NONE,
            radius: 8,
            icon_size: appearance::NAVIGATION_ICON_SIZE,
            visible: true,
        }
    }

    pub fn destructive(colors: appearance::Palette) -> Self {
        Self {
            foreground: colors.danger,
            hover: colors.danger.gamma_multiply(0.12),
            pressed: colors.danger.gamma_multiply(0.22),
            ..Self::ghost(colors, false)
        }
    }
}

fn blend(a: Color32, b: Color32, amount: f32) -> Color32 {
    let a = a.to_array();
    let b = b.to_array();
    let channel = |i: usize| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * amount).round() as u8;
    Color32::from_rgba_premultiplied(channel(0), channel(1), channel(2), channel(3))
}

pub(in crate::ui) fn interaction_fill(
    ui: &Ui,
    response: &egui::Response,
    idle: Color32,
    hover: Color32,
    pressed: Color32,
) -> Color32 {
    let amount =
        ui.ctx()
            .animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.12);
    if response.is_pointer_button_down_on() {
        pressed
    } else {
        blend(idle, hover, amount)
    }
}

impl Desktop {
    pub(in crate::ui) fn home_icon(
        &self,
        ui: &mut Ui,
        icon: Icon,
        label: &str,
        size: f32,
        style: IconStyle,
    ) -> egui::Response {
        let icon_size = style.icon_size.min(if size > appearance::CONTROL_HEIGHT {
            appearance::NAVIGATION_ICON_SIZE
        } else {
            appearance::CONTROL_ICON_SIZE
        });
        self.paint_icon_button(ui, icon, label, size, style, icon_size)
    }

    pub(in crate::ui) fn search_icon(
        &self,
        ui: &mut Ui,
        icon: Icon,
        label: &str,
        size: f32,
        style: IconStyle,
    ) -> egui::Response {
        self.paint_icon_button(ui, icon, label, size, style, style.icon_size)
    }

    fn paint_icon_button(
        &self,
        ui: &mut Ui,
        icon: Icon,
        label: &str,
        size: f32,
        style: IconStyle,
        icon_size: f32,
    ) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::click());
        let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        let opacity = ui.painter().opacity();
        if !ui.is_enabled() {
            ui.set_opacity(opacity / ui.visuals().disabled_alpha() * 0.42);
        }
        if style.visible || response.hovered() || response.has_focus() {
            let fill = interaction_fill(ui, &response, style.idle, style.hover, style.pressed);
            ui.painter().rect_filled(rect, style.radius, fill);
            let stroke = if crate::ui::components::navigation::focus_visible(ui, &response) {
                egui::Stroke::new(2.0, self.colors(ui).accent)
            } else {
                style.border
            };
            if stroke.width > 0.0 {
                ui.painter()
                    .rect_stroke(rect, style.radius, stroke, egui::StrokeKind::Inside);
            }
            let center =
                rect.center() + Vec2::new(0.0, f32::from(response.is_pointer_button_down_on()));
            icon.image(style.foreground, icon_size).paint_at(
                ui,
                egui::Rect::from_center_size(center, Vec2::splat(icon_size)),
            );
        }
        ui.set_opacity(opacity);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), self.tr(label))
        });
        self.control_tooltip(ui, &response, label);
        if response.clicked() {
            response.request_focus();
        }
        response
    }

    pub(in crate::ui) fn control_tooltip(&self, ui: &Ui, response: &egui::Response, label: &str) {
        let rect = response.rect;
        let mut tooltip = egui::Tooltip::for_enabled(response).gap(8.0).width(324.0);
        tooltip.popup = tooltip
            .popup
            .align(if rect.center().y > ui.ctx().viewport_rect().center().y {
                egui::emath::RectAlign::TOP
            } else {
                egui::emath::RectAlign::BOTTOM
            })
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_gray(41))
                    .corner_radius(5)
                    .inner_margin(egui::Margin::symmetric(8, 6)),
            );
        tooltip.show(|ui| {
            ui.set_min_height(16.0);
            ui.label(
                RichText::new(self.tr(label))
                    .size(13.0)
                    .color(Color32::WHITE),
            );
        });
    }

    pub(in crate::ui) fn home_action(
        &self,
        ui: &mut Ui,
        text: &str,
        icon: Option<(Icon, f32)>,
        primary: bool,
        min_size: Vec2,
        radius: u8,
    ) -> egui::Response {
        self.action_button(
            ui,
            text,
            ActionStyle {
                icon,
                tone: if primary {
                    ButtonTone::Primary
                } else {
                    ButtonTone::Neutral
                },
                radius,
                exact_width: !primary && min_size.x > 0.0,
                measure_bold: primary || radius == (appearance::CONTROL_HEIGHT / 2.0) as u8,
                ..ActionStyle::new(min_size)
            },
        )
    }
}
