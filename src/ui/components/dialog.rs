use crate::ui::components::icon_button::IconStyle;
use crate::ui::*;

pub(in crate::ui) fn child(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    rect: egui::Rect,
) -> Ui {
    ui.new_child(
        egui::UiBuilder::new()
            .id_salt(id)
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    )
}

pub(in crate::ui) fn panel(
    ctx: &egui::Context,
    id: &str,
    label: &str,
    size: Vec2,
    colors: appearance::Palette,
    content: impl FnOnce(&mut Ui, egui::Rect),
) {
    egui::Modal::new(egui::Id::new(id))
        .backdrop_color(Color32::from_black_alpha(153))
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            let rect = egui::Rect::from_min_size(ui.next_widget_position(), size);
            let response = ui.allocate_rect(rect, egui::Sense::hover());
            response
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, label));
            ui.painter().rect_filled(rect, 12, colors.surface);
            ui.painter().rect_stroke(
                rect,
                12,
                egui::Stroke::new(1.0, colors.border),
                egui::StrokeKind::Inside,
            );
            content(ui, rect.shrink(20.0));
        });
}

pub(in crate::ui) fn galley(
    ctx: &egui::Context,
    text: String,
    width: f32,
    size: f32,
    bold: bool,
    color: Color32,
) -> std::sync::Arc<egui::Galley> {
    let font = egui::FontId::new(
        size,
        if bold {
            egui::FontFamily::Name("cera-bold".into())
        } else {
            egui::FontFamily::Proportional
        },
    );
    let mut job = egui::text::LayoutJob::simple(text, font, color, width);
    job.sections[0].format.line_height = Some(size + 6.0);
    ctx.fonts_mut(|fonts| fonts.layout_job(job))
}

impl Desktop {
    pub(in crate::ui) fn dialog_text(
        &self,
        ui: &mut Ui,
        text: &str,
        size: f32,
        bold: bool,
        color: Color32,
    ) {
        let galley = galley(
            ui.ctx(),
            self.tr(text),
            ui.available_width(),
            size,
            bold,
            color,
        );
        let (rect, _) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
        ui.painter().galley(rect.min, galley, color);
    }

    pub(in crate::ui) fn dialog_icon(
        &self,
        ui: &mut Ui,
        rect: egui::Rect,
        icon: Icon,
        label: &str,
    ) -> egui::Response {
        let colors = self.colors(ui);
        self.home_icon(
            &mut child(ui, label, rect),
            icon,
            label,
            32.0,
            IconStyle {
                idle: colors.control,
                hover: if matches!(icon, Icon::Reset) {
                    colors.danger.gamma_multiply(0.12)
                } else {
                    colors.muted_control
                },
                pressed: if matches!(icon, Icon::Reset) {
                    colors.danger.gamma_multiply(0.22)
                } else {
                    colors.muted_control
                },
                foreground: match icon {
                    Icon::Search => colors.heading,
                    Icon::Reset => colors.danger,
                    _ => colors.secondary,
                },
                border: egui::Stroke::NONE,
                radius: 16,
                icon_size: appearance::CONTROL_ICON_SIZE,
                visible: true,
            },
        )
    }

    pub(in crate::ui) fn dialog_heading(&self, ui: &Ui, rect: egui::Rect, title: &str) {
        ui.painter().text(
            rect.left_center(),
            egui::Align2::LEFT_CENTER,
            self.tr(title),
            egui::FontId::new(
                typography::HEADING,
                egui::FontFamily::Name("cera-bold".into()),
            ),
            self.colors(ui).heading,
        );
    }

    pub(in crate::ui) fn auth_notice(&self, ui: &mut Ui, text: &str) -> bool {
        let colors = self.colors(ui);
        let width = ui.available_width();
        let (row, _) = ui.allocate_exact_size(Vec2::new(width, 18.0), egui::Sense::hover());
        let link = self.tr("click here");
        let link_galley = galley(ui.ctx(), link.clone(), width, 12.0, true, colors.accent);
        let text_galley = galley(
            ui.ctx(),
            self.tr(text),
            (width - link_galley.size().x - 6.0).max(0.0),
            12.0,
            false,
            colors.secondary,
        );
        let link_rect = egui::Rect::from_min_size(
            row.min + Vec2::new(text_galley.size().x + 6.0, 0.0),
            link_galley.size(),
        );
        ui.painter().galley(row.min, text_galley, colors.secondary);
        let response = ui
            .interact(
                link_rect,
                ui.id().with("auth-settings"),
                egui::Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        self.text_link_feedback(ui, &response);
        ui.painter()
            .galley(link_rect.min, link_galley, colors.accent);
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Link, true, link.clone()));
        response.clicked()
    }

    pub(in crate::ui) fn dialog_field(
        &self,
        ui: &mut Ui,
        id: &str,
        label: &str,
        text: &mut String,
        hint: &str,
        size: Vec2,
        multiline: bool,
    ) -> egui::Response {
        let colors = self.colors(ui);
        let size = Vec2::new(
            size.x,
            if multiline {
                size.y
            } else {
                appearance::CONTROL_HEIGHT
            },
        );
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        ui.painter().rect_filled(rect, 6, colors.surface);
        let stroke = ui.painter().add(egui::Shape::Noop);
        let field_rect = rect.shrink2(Vec2::new(12.0, 6.0));
        let mut field = child(ui, id, field_rect);
        let edit = if multiline {
            egui::TextEdit::multiline(text)
        } else {
            egui::TextEdit::singleline(text)
        };
        let mut response = field.add(
            edit.id(egui::Id::new(id))
                .frame(egui::Frame::NONE)
                .margin(egui::Margin::ZERO)
                .hint_text(RichText::new(hint).color(colors.placeholder))
                .text_color(colors.heading)
                .min_size(field_rect.size())
                .desired_width(field_rect.width())
                .vertical_align(if multiline {
                    egui::Align::Min
                } else {
                    egui::Align::Center
                }),
        );
        if self.text_context_menu(&response, text, false) {
            response.mark_changed();
        }
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, ui.is_enabled(), self.tr(label))
        });
        ui.painter().set(
            stroke,
            egui::Shape::rect_stroke(
                rect,
                6,
                egui::Stroke::new(
                    1.0,
                    if response.has_focus() {
                        colors.accent
                    } else {
                        colors.input_border
                    },
                ),
                egui::StrokeKind::Inside,
            ),
        );
        response
    }
}
