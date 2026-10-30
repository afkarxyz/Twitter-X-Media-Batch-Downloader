use crate::ui::*;

impl Desktop {
    pub(in crate::ui) fn xtractor_setup_dialog(&self, ctx: &egui::Context) {
        let app = self.app.clone();
        let busy = app.get_xtractor_busy();
        ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        self.preference_modal(ctx, "xtractor-setup", 480.0, |ui| {
            let colors = self.colors(ui);
            self.setting_text(ui, "Download xtractor", 17.0, true, colors.heading);
            self.setting_text(
                ui,
                "Xtractor is required to use this app. Download it to continue.",
                14.0,
                false,
                colors.body,
            );
            let status = app.get_xtractor_status();
            let detail = app.get_xtractor_detail();
            if busy {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().size(16.0).color(colors.accent));
                    self.setting_text(
                        ui,
                        if status.is_empty() {
                            "Downloading xtractor…"
                        } else {
                            &status
                        },
                        14.0,
                        false,
                        colors.heading,
                    );
                });
                self.progress_bar(ui, app.get_xtractor_progress());
                if !detail.is_empty() {
                    self.setting_caption(ui, &detail);
                }
            } else if !status.is_empty() {
                egui::ScrollArea::vertical()
                    .max_height(100.0)
                    .show_themed(ui, |ui| {
                        self.setting_text(ui, &status, 12.0, false, ui.visuals().error_fg_color);
                        if !detail.is_empty() && detail != status {
                            self.setting_text(ui, &detail, 12.0, false, colors.secondary);
                        }
                    });
            }
            let label = if busy {
                "Downloading…"
            } else if status.is_empty() {
                "Download"
            } else {
                "Try again"
            };
            let button = ui
                .allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), appearance::CONTROL_HEIGHT),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        ui.add_enabled_ui(!busy, |ui| {
                            self.action_button(
                                ui,
                                &self.tr(label),
                                ActionStyle {
                                    icon: Some((Icon::Download, 16.0)),
                                    tone: ButtonTone::Primary,
                                    exact_width: true,
                                    truncate_label: true,
                                    ..ActionStyle::new(Vec2::new(160.0, appearance::CONTROL_HEIGHT))
                                },
                            )
                        })
                        .inner
                    },
                )
                .inner;
            if button.clicked() {
                app.invoke_update_xtractor();
            }
            ui.response().widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Xtractor setup dialog")
            });
            false
        });
    }
}
