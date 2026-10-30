use crate::ui::*;
impl Desktop {
    pub(in crate::ui) fn theme_editor(&mut self, ui: &mut Ui) {
        let app = self.app.clone();
        let colors = self.colors(ui);
        let rows = schemes::SCHEMES.len().div_ceil(4);
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), 153.0 + rows as f32 * 54.0),
            egui::Sense::hover(),
        );
        ui.painter().rect_filled(rect, 8, colors.surface);
        ui.painter().rect_stroke(
            rect,
            8,
            egui::Stroke::new(1.0, appearance::mix(colors.border, colors.surface, 0.58)),
            egui::StrokeKind::Inside,
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Theme settings card")
        });
        self.history_text(
            ui,
            egui::Rect::from_min_size(
                rect.min + Vec2::new(15.0, 13.0),
                Vec2::new(rect.width() - 30.0, 22.0),
            ),
            self.tr("Theme"),
            16.0,
            true,
            colors.heading,
        );
        let width = (rect.width() - 28.0).min(540.0);
        let mode_width = (width - 16.0) / 3.0;
        for (index, (mode, label)) in [("system", "System"), ("dark", "Dark"), ("light", "Light")]
            .into_iter()
            .enumerate()
        {
            let option = egui::Rect::from_min_size(
                rect.min + Vec2::new(14.0 + index as f32 * (mode_width + 8.0), 42.0),
                Vec2::new(mode_width, 60.0),
            );
            let dark = match mode {
                "dark" => true,
                "light" => false,
                _ => ui.visuals().dark_mode,
            };
            let response = self.theme_option(
                ui,
                option,
                &app.global::<AppTheme>().get_preset(),
                &self.tr(label),
                (app.get_theme_mode() == mode, dark, true),
            );
            if response.clicked() {
                app.set_theme_mode(mode.into());
                app.invoke_save_theme_mode(mode.into());
            }
        }
        ui.painter().hline(
            (rect.left() + 14.0)..=(rect.right() - 14.0),
            rect.top() + 110.0,
            egui::Stroke::new(1.0, appearance::mix(colors.border, colors.surface, 0.58)),
        );
        self.history_text(
            ui,
            egui::Rect::from_min_size(
                rect.min + Vec2::new(15.0, 118.0),
                Vec2::new(rect.width() - 30.0, 18.0),
            ),
            self.tr("Palette"),
            12.0,
            true,
            colors.secondary,
        );
        let width = (rect.width() - 28.0 - 18.0) / 4.0;
        for (index, scheme) in schemes::SCHEMES.iter().enumerate() {
            let option = egui::Rect::from_min_size(
                rect.min
                    + Vec2::new(
                        14.0 + (index % 4) as f32 * (width + 6.0),
                        145.0 + (index / 4) as f32 * 54.0,
                    ),
                Vec2::new(width, 48.0),
            );
            if self
                .theme_option(
                    ui,
                    option,
                    scheme.id,
                    scheme.label,
                    (
                        app.global::<AppTheme>().get_preset() == scheme.id,
                        ui.visuals().dark_mode,
                        false,
                    ),
                )
                .clicked()
            {
                app.global::<AppTheme>().set_preset(scheme.id.into());
                app.invoke_save_theme_preset(scheme.id.into());
            }
        }
    }

    fn theme_option(
        &self,
        ui: &Ui,
        rect: egui::Rect,
        preset: &str,
        label: &str,
        state: (bool, bool, bool),
    ) -> egui::Response {
        let (selected, dark, mode) = state;
        let colors = self.colors(ui);
        let preview = appearance::Palette::new(preset, dark);
        let response = ui
            .interact(
                rect,
                ui.id().with(("theme-choice", label)),
                egui::Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let focus = crate::ui::components::navigation::focus_visible(ui, &response);
        ui.painter().rect_filled(
            rect,
            if mode { 10 } else { 8 },
            if response.hovered() {
                colors.control
            } else {
                colors.surface
            },
        );
        ui.painter().rect_stroke(
            rect,
            if mode { 10 } else { 8 },
            egui::Stroke::new(
                if focus { 2.0 } else { 1.0 },
                if focus { colors.accent } else { colors.border },
            ),
            egui::StrokeKind::Inside,
        );
        let sample = egui::Rect::from_center_size(
            egui::pos2(
                rect.left() + if mode { 38.0 } else { 34.0 },
                rect.center().y,
            ),
            Vec2::splat(36.0),
        );
        ui.painter().rect_filled(sample, 9, preview.page);
        ui.painter().rect_stroke(
            sample,
            9,
            egui::Stroke::new(1.0, preview.border),
            egui::StrokeKind::Inside,
        );
        ui.painter()
            .rect_filled(sample.shrink(4.0), 5, preview.surface);
        for (y, width, alpha) in [(13.0, 10.0, 1.0), (19.0, 8.0, 0.55)] {
            ui.painter().rect_filled(
                egui::Rect::from_min_size(sample.min + Vec2::new(8.0, y), Vec2::new(width, 3.0)),
                2,
                preview.title.gamma_multiply(alpha),
            );
        }
        ui.painter()
            .circle_filled(sample.min + Vec2::new(24.5, 16.5), 3.5, preview.primary);
        self.history_text(
            ui,
            egui::Rect::from_min_max(
                egui::pos2(sample.right() + 8.0, rect.top()),
                egui::pos2(rect.right() - 30.0, rect.bottom()),
            ),
            label.to_owned(),
            if mode { 14.0 } else { 12.0 },
            true,
            if selected { colors.accent } else { colors.body },
        );
        if selected {
            Icon::Check.image(colors.accent, 14.0).paint_at(
                ui,
                egui::Rect::from_center_size(
                    egui::pos2(
                        rect.right() - if mode { 28.0 } else { 23.0 },
                        rect.center().y,
                    ),
                    Vec2::splat(14.0),
                ),
            );
        }
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, selected, label)
        });
        if response.clicked() {
            response.request_focus();
        }
        response
    }
}
