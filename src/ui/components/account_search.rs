use crate::ui::components::{dialog::child, icon_button::IconStyle};
use crate::ui::*;

pub(in crate::ui) fn search_rect(row: egui::Rect) -> egui::Rect {
    let width = 380.0_f32.min((row.width() - 370.0).max(180.0));
    egui::Rect::from_center_size(
        egui::pos2(row.right() - width / 2.0, row.center().y),
        Vec2::new(width, appearance::CONTROL_HEIGHT),
    )
}

impl Desktop {
    pub(in crate::ui) fn account_search_input(
        &self,
        ui: &mut Ui,
        rect: egui::Rect,
        id: &str,
        label: &str,
        query: &mut String,
    ) -> bool {
        let colors = self.colors(ui);
        ui.painter().rect_filled(rect, 6, colors.surface);
        let border = ui.painter().add(egui::Shape::Noop);
        Icon::Search
            .image(colors.secondary, appearance::CONTROL_ICON_SIZE)
            .paint_at(
                ui,
                egui::Rect::from_center_size(
                    egui::pos2(rect.left() + 20.0, rect.center().y),
                    Vec2::splat(appearance::CONTROL_ICON_SIZE),
                ),
            );
        let field = egui::Rect::from_min_max(
            rect.min + Vec2::new(36.0, 4.0),
            rect.max - Vec2::new(40.0, 4.0),
        );
        let mut field_ui = child(ui, id, field);
        let mut response = field_ui.add(
            egui::TextEdit::singleline(query)
                .id(egui::Id::new(id))
                .frame(egui::Frame::NONE)
                .margin(egui::Margin::ZERO)
                .min_size(field.size())
                .desired_width(field.width())
                .vertical_align(egui::Align::Center)
                .text_color(colors.heading)
                .hint_text(RichText::new(self.tr("Search accounts…")).color(colors.placeholder)),
        );
        if self.text_context_menu(&response, query, false) {
            response.mark_changed();
        }
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, ui.is_enabled(), self.tr(label))
        });
        ui.painter().set(
            border,
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
        let mut changed = response.changed();
        if !query.is_empty() {
            let clear = egui::Rect::from_center_size(
                egui::pos2(rect.right() - 18.0, rect.center().y),
                Vec2::splat(28.0),
            );
            let mut action = child(ui, (id, "clear"), clear);
            if self
                .home_icon(
                    &mut action,
                    Icon::Close,
                    "Clear search",
                    28.0,
                    IconStyle {
                        radius: 14,
                        icon_size: 14.0,
                        ..IconStyle::ghost(colors, false)
                    },
                )
                .clicked()
            {
                query.clear();
                changed = true;
            }
        }
        changed
    }
}
