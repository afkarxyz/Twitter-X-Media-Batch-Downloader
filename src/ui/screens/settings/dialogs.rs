use crate::ui::components::dialog::child;
use crate::ui::components::dropdown::SelectOptions;
use crate::ui::*;

pub(in crate::ui) fn first_open(ctx: &egui::Context, id: &str, open: bool) -> bool {
    ctx.data_mut(|data| {
        let key = egui::Id::new(("dialog-seen", id));
        let seen = data.get_temp::<bool>(key).unwrap_or(false);
        data.insert_temp(key, open);
        open && !seen
    })
}

impl Desktop {
    pub(in crate::ui) fn preference_modal(
        &self,
        ctx: &egui::Context,
        id: &str,
        width: f32,
        content: impl FnOnce(&mut Ui) -> bool,
    ) -> bool {
        let colors = appearance::Palette::new(
            &self.app.global::<AppTheme>().get_preset(),
            ctx.theme() == egui::Theme::Dark,
        );
        egui::Modal::new(egui::Id::new(id))
            .backdrop_color(Color32::from_black_alpha(153))
            .frame(
                egui::Frame::new()
                    .fill(colors.surface)
                    .stroke(egui::Stroke::new(1.0, colors.border))
                    .corner_radius(12)
                    .inner_margin(23),
            )
            .show(ctx, |ui| {
                ui.set_width(width.min(ctx.content_rect().width() - 48.0) - 48.0);
                ui.spacing_mut().item_spacing.y = 16.0;
                content(ui)
            })
            .inner
    }

    pub(in crate::ui) fn settings_dialogs(&mut self, ctx: &egui::Context) {
        let app = self.app.clone();
        let first = first_open(ctx, "integrity", app.get_integrity_open());
        if app.get_integrity_open() {
            let busy = app.get_integrity_busy();
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
                if busy {
                    app.invoke_cancel_operation();
                } else {
                    app.set_integrity_open(false);
                }
            }
            let colors = appearance::Palette::new(
                &app.global::<AppTheme>().get_preset(),
                ctx.theme() == egui::Theme::Dark,
            );
            let size = Vec2::new(
                900.0_f32.min(ctx.content_rect().width() - 48.0),
                620.0_f32.min(ctx.content_rect().height() - 48.0),
            );
            crate::ui::components::dialog::panel(
                ctx,
                "integrity",
                "File integrity dialog",
                size,
                colors,
                |ui, inner| {
                    let mut body = child(ui, "integrity-body", inner);
                    body.spacing_mut().item_spacing.y = 12.0;
                    self.setting_text(
                        &mut body,
                        "Check downloaded files",
                        17.0,
                        true,
                        colors.heading,
                    );
                    self.setting_caption(&mut body,"Quick checks presence and recorded size. Deep also compares SHA-256. Older files without a checksum remain unverified.");
                    self.setting_text(
                        &mut body,
                        &app.get_integrity_status(),
                        14.0,
                        false,
                        colors.heading,
                    );
                    let top = body.next_widget_position().y;
                    let report = egui::Rect::from_min_max(
                        egui::pos2(inner.left(), top),
                        egui::pos2(inner.right(), inner.bottom() - 44.0),
                    );
                    let mut contents = child(ui, "integrity-report", report.shrink(8.0));
                    egui::ScrollArea::vertical()
                        .max_height(report.height() - 16.0)
                        .auto_shrink([false, false])
                        .show_themed(&mut contents, |ui| {
                            self.setting_text(
                                ui,
                                &app.get_integrity_report(),
                                12.0,
                                false,
                                colors.body,
                            );
                        });
                    let footer = egui::Rect::from_min_size(
                        egui::pos2(inner.left(), inner.bottom() - 32.0),
                        Vec2::new(inner.width(), 32.0),
                    );
                    let mut actions = child(ui, "integrity-actions", footer);
                    actions.horizontal(|ui| {
                        if self
                            .setting_action(
                                ui,
                                "Quick check",
                                Some(Icon::Check),
                                0,
                                Vec2::new(140.0, 32.0),
                                !busy,
                            )
                            .clicked()
                        {
                            app.invoke_check_downloads(false);
                        }
                        if self
                            .setting_action(
                                ui,
                                "Deep check",
                                None,
                                0,
                                Vec2::new(140.0, 32.0),
                                !busy,
                            )
                            .clicked()
                        {
                            app.invoke_check_downloads(true);
                        }
                    });
                    let mut close = child(
                        ui,
                        "integrity-close",
                        egui::Rect::from_min_size(
                            footer.right_top() - Vec2::new(110.0, 0.0),
                            Vec2::new(110.0, 32.0),
                        ),
                    );
                    let response = self.setting_action(
                        &mut close,
                        if busy { "Stop" } else { "Close" },
                        None,
                        0,
                        Vec2::new(110.0, 32.0),
                        true,
                    );
                    if first {
                        response.request_focus();
                    }
                    if response.clicked() {
                        if busy {
                            app.invoke_cancel_operation();
                        } else {
                            app.set_integrity_open(false);
                        }
                    }
                },
            );
        }
        let first = first_open(ctx, "backup", app.get_backup_restore_open());
        if app.get_backup_restore_open() {
            let mut close =
                ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            close|=self.preference_modal(ctx,"restore-backup",520.0,|ui|{
                self.setting_text(ui,"Restore database backup?",17.0,true,self.colors(ui).heading);
                self.setting_text(ui,&app.get_backup_summary(),12.0,false,self.colors(ui).body);
                self.setting_text(ui,"This replaces current history, account labels and download records. Media files and authentication settings stay as they are. A recovery backup is created first.",14.0,false,self.colors(ui).body);
                self.setting_error(ui,&app.get_backup_error());
                let mut close=false;let width=(ui.available_width()-10.0)/2.0;ui.horizontal(|ui|{ui.spacing_mut().item_spacing.x=10.0;
                    let cancel=self.setting_action(ui,"Cancel",Some(Icon::Close),0,Vec2::new(width, appearance::CONTROL_HEIGHT),true);if first{cancel.request_focus();}close|=cancel.clicked();
                    if self.setting_action(ui,"Restore",Some(Icon::Upload),2,Vec2::new(width, appearance::CONTROL_HEIGHT),!self.busy()&&!app.get_backup_busy()).clicked(){app.invoke_restore_database();close=true;}
                });close
            });
            if close {
                app.set_backup_restore_open(false);
            }
        }
        let renderer = app.global::<RendererSettings>();
        let first = first_open(ctx, "renderer", renderer.get_prompt_open());
        if renderer.get_prompt_open() {
            let mut close = !renderer.get_restarting()
                && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            close |= self.preference_modal(ctx, "renderer-restart", 440.0, |ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), 26.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.add(Icon::Restart.image(self.colors(ui).accent, 22.0));
                        self.setting_text(
                            ui,
                            "Restart to apply renderer",
                            17.0,
                            true,
                            self.colors(ui).heading,
                        );
                    },
                );
                self.setting_text(
                    ui,
                    "Your renderer preference has been saved. Restart the app to apply it.",
                    14.0,
                    false,
                    self.colors(ui).body,
                );
                if self.busy() {
                    self.setting_caption(
                        ui,
                        "Restart will be available when the current operation finishes.",
                    );
                }
                self.setting_error(ui, &renderer.get_error());
                let mut close = false;
                let width = (ui.available_width() - 10.0) / 2.0;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    let later = self.setting_action(
                        ui,
                        "Restart later",
                        None,
                        0,
                        Vec2::new(width, appearance::CONTROL_HEIGHT),
                        !renderer.get_restarting(),
                    );
                    if first {
                        later.request_focus();
                    }
                    close |= later.clicked();
                    if self
                        .setting_action(
                            ui,
                            "Restart now",
                            Some(Icon::Restart),
                            1,
                            Vec2::new(width, appearance::CONTROL_HEIGHT),
                            !self.busy() && !renderer.get_restarting(),
                        )
                        .clicked()
                    {
                        renderer.invoke_restart_now();
                    }
                });
                close
            });
            if close {
                renderer.set_prompt_open(false);
            }
        }
        let first = first_open(ctx, "logs", app.get_log_viewer_open());
        if app.get_log_viewer_open() {
            self.settings_log_viewer(ctx, first);
        }
    }

    fn settings_log_viewer(&self, ctx: &egui::Context, first: bool) {
        let app = self.app.clone();
        let colors = appearance::Palette::new(
            &app.global::<AppTheme>().get_preset(),
            ctx.theme() == egui::Theme::Dark,
        );
        let popup_open = egui::Popup::is_any_open(ctx);
        if !popup_open && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            app.set_log_viewer_open(false);
        }
        let size = Vec2::new(
            1000.0_f32.min(ctx.content_rect().width() - 48.0),
            720.0_f32.min(ctx.content_rect().height() - 48.0),
        );
        crate::ui::components::dialog::panel(
            ctx,
            "logs",
            "Session logs dialog",
            size,
            colors,
            |ui, inner| {
                let mut body = child(ui, "log-body", inner);
                body.spacing_mut().item_spacing.y = 12.0;
                let (heading, _) =
                    body.allocate_exact_size(Vec2::new(inner.width(), 32.0), egui::Sense::hover());
                self.history_text(
                    &body,
                    heading,
                    self.tr("Session logs"),
                    17.0,
                    true,
                    colors.heading,
                );
                let mut close = child(
                    ui,
                    "log-close",
                    egui::Rect::from_min_size(
                        inner.right_top() - Vec2::new(32.0, 0.0),
                        Vec2::splat(32.0),
                    ),
                );
                let close_response =
                    self.setting_icon(&mut close, Icon::Close, "Close session logs", 32.0);
                if first && (app.get_log_sessions().row_count() == 0 || app.get_logs_busy()) {
                    close_response.request_focus();
                }
                if close_response.clicked() {
                    app.set_log_viewer_open(false);
                }
                let sessions = app.get_log_sessions().iter().collect::<Vec<_>>();
                let selected = app.get_log_session_index().max(0) as usize;
                let (picker_row, _) = body.allocate_exact_size(
                    Vec2::new(inner.width(), appearance::CONTROL_HEIGHT),
                    egui::Sense::hover(),
                );
                let picker = egui::Rect::from_min_size(
                    picker_row.min,
                    Vec2::new(picker_row.width() - 122.0, appearance::CONTROL_HEIGHT),
                );
                let mut selector = child(&mut body, "log-session-picker", picker);
                if app.get_logs_busy() || sessions.is_empty() {
                    selector.disable();
                }
                let choices = if sessions.is_empty() {
                    vec![self.tr("No session logs yet.")]
                } else {
                    sessions
                };
                let next = self.choice_picker(
                    &mut selector,
                    picker,
                    &SelectOptions::new("Session", &choices),
                    selected,
                );
                if first && selector.is_enabled() {
                    ctx.memory_mut(|memory| {
                        memory.request_focus(selector.id().with(("select", "Session")))
                    });
                }
                if next != selected {
                    app.invoke_load_log(next as i32);
                }
                let mut refresh = child(
                    &mut body,
                    "log-refresh",
                    egui::Rect::from_min_size(
                        picker_row.right_top() - Vec2::new(110.0, -2.0),
                        Vec2::new(110.0, 32.0),
                    ),
                );
                if self
                    .setting_action(
                        &mut refresh,
                        "Refresh",
                        Some(Icon::Restart),
                        0,
                        Vec2::new(110.0, 32.0),
                        !app.get_logs_busy(),
                    )
                    .clicked()
                {
                    app.invoke_refresh_logs();
                }
                body.horizontal(|ui| {
                    if self
                        .setting_action(
                            ui,
                            if app.get_logs_live() {
                                "Live: On"
                            } else {
                                "Live: Off"
                            },
                            None,
                            0,
                            Vec2::new(140.0, 32.0),
                            true,
                        )
                        .clicked()
                    {
                        app.set_logs_live(!app.get_logs_live());
                    }
                    if self
                        .setting_action(
                            ui,
                            if app.get_logs_follow() {
                                "Auto-scroll: On"
                            } else {
                                "Auto-scroll: Off"
                            },
                            None,
                            0,
                            Vec2::new(160.0, 32.0),
                            true,
                        )
                        .clicked()
                    {
                        app.set_logs_follow(!app.get_logs_follow());
                    }
                });
                self.setting_caption(
                    &mut body,
                    if app.get_logs_busy() {
                        "Loading…"
                    } else if app.get_log_truncated() {
                        "Showing the latest 256 KiB. Export saves the full session log."
                    } else {
                        "Live refreshes every second. Copy uses the displayed text."
                    },
                );
                let error = app.get_logs_error();
                let error_height = if error.is_empty() { 0.0 } else { 29.0 };
                let report = egui::Rect::from_min_max(
                    body.next_widget_position(),
                    egui::pos2(inner.right(), inner.bottom() - 44.0 - error_height),
                );
                body.painter().rect_filled(report, 6, colors.surface);
                body.painter().rect_stroke(
                    report,
                    6,
                    egui::Stroke::new(1.0, colors.input_border),
                    egui::StrokeKind::Inside,
                );
                let mut contents = child(&mut body, "log-contents", report.shrink(8.0));
                let text = app.get_log_text();
                let signature = egui::Id::new((&text, app.get_logs_follow()));
                let changed = ctx.data_mut(|data| {
                    let key = egui::Id::new("log-scroll-signature");
                    let changed = data.get_temp::<egui::Id>(key) != Some(signature);
                    data.insert_temp(key, signature);
                    changed
                });
                egui::ScrollArea::vertical()
                    .id_salt("session-log-scroll")
                    .max_height(report.height() - 16.0)
                    .auto_shrink([false, false])
                    .show_themed(&mut contents, |ui| {
                        let mut content = text.as_str();
                        let response = ui.add(
                            egui::TextEdit::multiline(&mut content)
                                .font(egui::FontId::proportional(12.0))
                                .text_color(colors.heading)
                                .frame(egui::Frame::NONE)
                                .margin(egui::Margin::ZERO)
                                .desired_width(ui.available_width())
                                .desired_rows(1)
                                .min_size(Vec2::new(0.0, report.height() - 16.0)),
                        );
                        let mut readonly_text = text.clone();
                        self.text_context_menu(&response, &mut readonly_text, true);
                        response.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::TextEdit,
                                true,
                                self.tr("Session log contents"),
                            )
                        });
                        if changed && app.get_logs_follow() {
                            response.scroll_to_me(Some(egui::Align::BOTTOM));
                        }
                    });
                if text.is_empty() && !app.get_logs_busy() {
                    let caption = self.tr(if app.get_log_sessions().row_count() == 0 {
                        "No session logs yet."
                    } else {
                        "This session log is empty."
                    });
                    body.painter().text(
                        report.center(),
                        egui::Align2::CENTER_CENTER,
                        caption,
                        egui::FontId::proportional(typography::BODY),
                        colors.secondary,
                    );
                }
                let mut footer = child(
                    &mut body,
                    "logs-footer",
                    egui::Rect::from_min_max(
                        egui::pos2(inner.left(), inner.bottom() - 32.0),
                        inner.max,
                    ),
                );
                footer.horizontal(|ui| {
                    for (label, icon, width, enabled, index) in [
                        (
                            "Copy",
                            Icon::Copy,
                            110.0,
                            !app.get_logs_busy() && !text.is_empty(),
                            0,
                        ),
                        (
                            "Export log",
                            Icon::Download,
                            140.0,
                            !app.get_logs_busy() && app.get_log_session_index() >= 0,
                            1,
                        ),
                        (
                            "Export errors",
                            Icon::Download,
                            140.0,
                            !app.get_logs_busy() && app.get_log_session_index() >= 0,
                            2,
                        ),
                        ("Open folder", Icon::Folder, 140.0, true, 3),
                    ] {
                        if self
                            .setting_action(
                                ui,
                                label,
                                Some(icon),
                                0,
                                Vec2::new(width, 32.0),
                                enabled,
                            )
                            .clicked()
                        {
                            match index {
                                0 => app.invoke_copy_log(),
                                1 => app.invoke_export_log(),
                                2 => app.invoke_export_log_errors(),
                                _ => app.invoke_open_logs(),
                            }
                        }
                    }
                });
                if !error.is_empty() {
                    let mut error_ui = child(
                        &mut body,
                        "log-error",
                        egui::Rect::from_min_size(
                            egui::pos2(inner.left(), report.bottom() + 12.0),
                            Vec2::new(inner.width(), 17.0),
                        ),
                    );
                    self.setting_error(&mut error_ui, &error);
                }
            },
        );
    }
}
