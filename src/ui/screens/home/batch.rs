use crate::ui::components::dialog::child;
use crate::ui::*;

impl Desktop {
    pub(in crate::ui) fn home_mode_switch(&self, ui: &mut Ui) {
        let app = self.app.clone();
        let rect = egui::Rect::from_center_size(
            egui::pos2(ui.max_rect().center().x, ui.max_rect().top() + 32.0),
            Vec2::new(208.0, appearance::CONTROL_HEIGHT + 8.0),
        );
        let mut controls = child(ui, "home-mode", rect);
        if self.busy() {
            controls.disable();
            controls.set_opacity(ui.painter().opacity() * 0.45);
        }
        let colors = self.colors(&controls);
        controls.painter().rect_filled(rect, 8, colors.surface);
        controls.painter().rect_stroke(
            rect,
            8,
            egui::Stroke::new(1.0, colors.border),
            egui::StrokeKind::Inside,
        );
        let amount = ui.ctx().animate_bool_with_time_and_easing(
            egui::Id::new("home-mode-position"),
            app.get_batch_mode(),
            0.18,
            egui::emath::easing::quadratic_out,
        );
        let thumb = egui::Rect::from_min_size(
            rect.min + Vec2::new(4.0 + 100.0 * amount, 4.0),
            Vec2::new(100.0, appearance::CONTROL_HEIGHT),
        );
        controls.painter().rect_filled(thumb, 6, colors.accent_soft);
        let mut change = None;
        for (index, (batch, label, icon)) in
            [(false, "Single", Icon::User), (true, "Batch", Icon::List)]
                .into_iter()
                .enumerate()
        {
            let segment = egui::Rect::from_min_size(
                rect.min + Vec2::new(4.0 + index as f32 * 100.0, 4.0),
                Vec2::new(100.0, appearance::CONTROL_HEIGHT),
            );
            let selected = app.get_batch_mode() == batch;
            let response = controls
                .interact(
                    segment,
                    egui::Id::new(("home-mode-option", batch)),
                    egui::Sense::click(),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if crate::ui::components::navigation::focus_visible(&controls, &response) {
                controls.painter().rect_stroke(
                    segment,
                    6,
                    egui::Stroke::new(2.0, colors.accent),
                    egui::StrokeKind::Inside,
                );
            }
            let foreground = if selected {
                colors.accent
            } else if controls.is_enabled() && response.hovered() {
                if controls.visuals().dark_mode {
                    Color32::WHITE
                } else {
                    colors.heading
                }
            } else {
                colors.secondary
            };
            icon.image(foreground, appearance::CONTROL_ICON_SIZE)
                .paint_at(
                    &controls,
                    egui::Rect::from_center_size(
                        egui::pos2(segment.left() + 20.0, segment.center().y),
                        Vec2::splat(appearance::CONTROL_ICON_SIZE),
                    ),
                );
            self.history_text(
                &controls,
                egui::Rect::from_min_max(
                    segment.min + Vec2::new(35.0, 0.0),
                    segment.max - Vec2::new(8.0, 0.0),
                ),
                self.tr(label),
                14.0,
                selected,
                foreground,
            );
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::Button,
                    controls.is_enabled(),
                    selected,
                    self.tr(label),
                )
            });
            if response.clicked() && !selected {
                change = Some(batch);
            }
            if controls.is_enabled() && response.has_focus() {
                controls.memory_mut(|memory| {
                    memory.set_focus_lock_filter(
                        response.id,
                        egui::EventFilter {
                            horizontal_arrows: true,
                            ..Default::default()
                        },
                    )
                });
                let target = controls.input_mut(|input| {
                    if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft) {
                        Some(false)
                    } else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight) {
                        Some(true)
                    } else {
                        None
                    }
                });
                if let Some(target) = target.filter(|target| *target != app.get_batch_mode()) {
                    change = Some(target);
                }
            }
        }
        if let Some(target) = change {
            app.invoke_switch_home_mode(target);
            controls.memory_mut(|memory| {
                if app.get_batch_mode()
                    || app.get_home_results_hidden()
                    || app.get_media_items().row_count() == 0
                {
                    memory.request_focus(egui::Id::new(("home-mode-option", target)));
                } else {
                    for option in [false, true] {
                        memory.surrender_focus(egui::Id::new(("home-mode-option", option)));
                    }
                }
            });
        }
    }

    pub(in crate::ui) fn batch_page(&mut self, ui: &mut Ui) {
        let app = self.app.clone();
        let bounds = ui.available_rect_before_wrap();
        let width = bounds.width().min(844.0) - 24.0;
        let column = egui::Rect::from_min_size(
            egui::pos2(bounds.center().x - width / 2.0, bounds.top() + 12.0),
            Vec2::new(width, bounds.height() - 24.0),
        );
        let mut page = child(ui, "batch-page", column);
        let header = egui::Rect::from_min_size(column.min, Vec2::new(width, 40.0));
        self.history_title(&page, header, self.tr("Batch"));
        let mut options = child(
            &mut page,
            "batch-options",
            egui::Rect::from_min_size(
                header.right_top() - Vec2::new(32.0, -4.0),
                Vec2::splat(32.0),
            ),
        );
        options.add_enabled_ui(!self.busy() && !app.get_batch_history_profiles(), |ui| {
            if self
                .icon_button(ui, Icon::Sliders, "Fetch options", false, 32.0)
                .clicked()
            {
                app.set_fetch_options_open(true);
            }
        });
        let body = egui::Rect::from_min_max(column.min + Vec2::new(0.0, 56.0), column.max);
        let mut content = child(&mut page, "batch-page-content", body);
        egui::ScrollArea::vertical().id_salt("batch-page-scroll").auto_shrink([false, false])
            .show_themed(&mut content, |ui| {
                ui.spacing_mut().item_spacing.y = 16.0;
                if !app.get_batch_history_profiles() && (app.get_batch_fetch_items().row_count() == 0 || self.batch_list_editing) && !app.get_batch_running() {
                self.setting_card(ui, "Batch account list", 10.0, |this, ui| {
                    this.setting_heading(ui, "Batch accounts");
                    this.setting_caption(ui, "One handle or profile URL per line. Up to 100 accounts; duplicates are removed.");
                    let mut sources = this.batch_sources_draft.clone().unwrap_or_else(|| app.get_batch_sources_text());
                    let colors = this.colors(ui);
                    let (frame, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 104.0), egui::Sense::hover());
                    ui.painter().rect_filled(frame, 6, colors.surface);
                    let border = ui.painter().add(egui::Shape::Noop);
                    let mut editor = child(ui, "batch-source-editor", frame.shrink2(Vec2::new(12.0, 6.0)));
                    let response = editor.add_enabled_ui(!this.busy(), |ui| {
                        egui::ScrollArea::vertical().id_salt("batch-source-scroll").max_height(92.0).auto_shrink([false, false])
                            .show_themed(ui, |ui| {
                                let mut response = ui.add(egui::TextEdit::multiline(&mut sources).frame(egui::Frame::NONE).margin(egui::Margin::ZERO).desired_width(ui.available_width()).desired_rows(4).hint_text("@account\nhttps://x.com/another_account/media"));
                                if this.text_context_menu(&response, &mut sources, false) { response.mark_changed(); }
                                response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, ui.is_enabled(), this.tr("Batch account list")));
                                response
                            }).inner
                    }).inner;
                    if response.changed() { this.batch_sources_draft = Some(sources.clone()); }
                    ui.painter().set(border, egui::Shape::rect_stroke(frame, 6, egui::Stroke::new(1.0, if response.has_focus() { colors.accent } else { colors.input_border }), egui::StrokeKind::Inside));
                    ui.horizontal(|ui| {
                        if this.setting_action(ui, "Update queue", Some(Icon::Check), 0, Vec2::new(120.0, 32.0), !this.busy() && !sources.trim().is_empty()).clicked()
                            && app.invoke_apply_fetch_options(true, app.get_date_filter_enabled(), app.get_fetch_start_date(), app.get_fetch_end_date(), sources.clone()) {
                            this.batch_list_editing = false;
                            this.batch_sources_draft = None;
                        }
                        if app.get_batch_fetch_items().row_count() > 0 && this.setting_action(ui, "Cancel", Some(Icon::Close), 0, Vec2::new(100.0, 32.0), !this.busy()).clicked() {
                            app.set_batch_sources_text(app.get_batch_fetch_items().iter().map(|row| row.source).collect::<Vec<_>>().join("\n"));
                            app.set_fetch_options_error(String::new());
                            this.batch_sources_draft = None;
                            this.batch_list_editing = false;
                        }
                    });
                    this.setting_error(ui, &app.get_fetch_options_error());
                });
                } else if !app.get_batch_history_profiles() && self.setting_action(ui, "Edit accounts", Some(Icon::List), 0, Vec2::new(120.0, 32.0), !self.busy()).clicked() {
                    self.batch_list_editing = true;
                }
                if app.get_batch_fetch_items().row_count() > 0 {
                    let rows = app.get_batch_fetch_items();
                    let processed = rows.iter().filter(|row| !matches!(row.status.as_str(), "Queued" | "Fetching")).count();
                    let (status_row, _) = ui.allocate_exact_size(Vec2::new(width, 24.0), egui::Sense::hover());
                    let status_text = if app.get_batch_running() {
                        let source = rows.iter().find(|row| row.status == "Fetching").map(|row| row.source).unwrap_or_default();
                        egui::Spinner::new().size(16.0).paint_at(ui, egui::Rect::from_center_size(status_row.left_center() + Vec2::new(8.0, 0.0), Vec2::splat(16.0)));
                        format!("{source} · {}", app.get_status())
                    } else {
                        self.tr("Open a result to inspect it. Back returns to this queue.")
                    };
                    let text_rect = egui::Rect::from_min_max(status_row.min + Vec2::new(if app.get_batch_running() { 28.0 } else { 0.0 }, 0.0), status_row.right_bottom() - Vec2::new(72.0, 0.0));
                    self.history_text(ui, text_rect, status_text.clone(), 12.0, false, self.colors(ui).secondary);
                    ui.interact(text_rect, ui.id().with("batch-progress-status"), egui::Sense::hover()).widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &status_text));
                    self.history_text(ui, egui::Rect::from_min_size(status_row.right_top() - Vec2::new(60.0, 0.0), Vec2::new(60.0, 24.0)), format!("{processed} / {}", rows.row_count()), 12.0, true, self.colors(ui).secondary);
                    self.progress_bar(ui, processed as f32 / rows.row_count() as f32 * 100.0);
                    self.batch_queue(ui);
                }
            });
    }

    pub(in crate::ui) fn batch_result_header(&self, ui: &mut Ui) {
        let app = self.app.clone();
        let (row, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), egui::Sense::hover());
        let mut back = child(
            ui,
            "batch-result-back",
            egui::Rect::from_min_size(row.min, Vec2::splat(40.0)),
        );
        if self
            .home_icon(
                &mut back,
                Icon::Back,
                "Back to batch",
                40.0,
                crate::ui::components::icon_button::IconStyle {
                    idle: self.colors(ui).surface,
                    border: egui::Stroke::new(1.0, self.colors(ui).control_border),
                    radius: 20,
                    ..crate::ui::components::icon_button::IconStyle::ghost(self.colors(ui), false)
                },
            )
            .clicked()
        {
            app.set_batch_result_open(false);
            app.set_selected_media_index(-1);
        }
        self.history_title(
            ui,
            egui::Rect::from_min_max(row.min + Vec2::new(48.0, 0.0), row.max),
            self.tr("Batch results"),
        );
    }

    pub(in crate::ui) fn batch_queue(&mut self, ui: &mut Ui) {
        let app = self.app.clone();
        let colors = self.colors(ui);
        let items = app.get_batch_fetch_items();
        let notice = app.get_saved_auth_token().is_empty() && !app.get_batch_running();
        let profiles = app.get_batch_history_profiles();
        let rows_height = (items.row_count() as f32 * 68.0).min(300.0);
        let width = ui.available_width().min(820.0);
        let height = 24.0
            + 32.0
            + 10.0
            + rows_height
            + if notice { 27.0 } else { 0.0 }
            + if profiles { 27.0 } else { 0.0 };
        let (space, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), height),
            egui::Sense::hover(),
        );
        let panel = egui::Rect::from_center_size(space.center(), Vec2::new(width, height));
        ui.painter().rect_filled(panel, 8, colors.surface);
        ui.painter().rect_stroke(
            panel,
            8,
            egui::Stroke::new(1.0, colors.border),
            egui::StrokeKind::Inside,
        );
        let mut contents = child(ui, "batch-queue", panel.shrink(12.0));
        contents.spacing_mut().item_spacing.y = 10.0;
        if profiles {
            self.setting_caption(&mut contents, "Each account keeps its saved fetch filters.");
        }
        if notice && self.auth_notice(&mut contents, "Add auth_token in Settings to fetch") {
            app.set_selected_tab(2);
            app.set_settings_category(1);
        }
        let (header, _) = contents.allocate_exact_size(
            Vec2::new(contents.available_width(), 32.0),
            egui::Sense::hover(),
        );
        self.history_text(
            &contents,
            header,
            format!("{} ({})", self.tr("Batch accounts"), items.row_count()),
            14.04,
            true,
            colors.heading,
        );
        let mut actions = child(&mut contents, "batch-actions", header);
        actions.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            for (label, enabled, primary) in [
                ("Clear queue", !self.busy(), false),
                ("Reset queue", !self.busy(), false),
                (
                    if app.get_batch_running() {
                        "Stop batch"
                    } else {
                        "Fetch pending"
                    },
                    app.get_batch_running()
                        || (!self.busy()
                            && !app.get_saved_auth_token().is_empty()
                            && !app.get_auth_check_busy()
                            && !self.batch_list_editing
                            && app.get_fetch_options_error().is_empty()
                            && app.get_batch_pending_count() > 0),
                    !app.get_batch_running(),
                ),
            ] {
                let response = ui
                    .add_enabled_ui(enabled, |ui| {
                        self.action_button(
                            ui,
                            &self.tr(label),
                            ActionStyle {
                                icon: Some((
                                    match label {
                                        "Clear queue" => Icon::Trash,
                                        "Reset queue" => Icon::Reset,
                                        "Stop batch" => Icon::Close,
                                        _ => Icon::Play,
                                    },
                                    appearance::CONTROL_ICON_SIZE,
                                )),
                                tone: if matches!(
                                    label,
                                    "Clear queue" | "Reset queue" | "Stop batch"
                                ) {
                                    ButtonTone::Destructive
                                } else if primary {
                                    ButtonTone::Primary
                                } else {
                                    ButtonTone::Neutral
                                },
                                measure_regular: !primary,
                                ..ActionStyle::new(Vec2::new(0.0, 32.0))
                            },
                        )
                    })
                    .inner;
                if response.clicked() {
                    match label {
                        "Clear queue" => {
                            app.invoke_clear_batch();
                            self.batch_sources_draft = None;
                            self.batch_list_editing = false;
                        }
                        "Reset queue" => app.invoke_reset_batch(),
                        "Stop batch" => app.invoke_cancel_operation(),
                        _ => app.invoke_start_batch(),
                    }
                }
            }
        });
        egui::ScrollArea::vertical()
            .id_salt("batch-accounts")
            .max_height(rows_height)
            .auto_shrink([false, true])
            .show_themed(&mut contents, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for (index, item) in items.iter().enumerate() {
                    let (row, _) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), 68.0),
                        egui::Sense::hover(),
                    );
                    if app.get_batch_result_index() == item.index {
                        ui.painter().rect_filled(
                            row.shrink2(Vec2::new(0.0, 2.0)),
                            6,
                            colors.accent_soft,
                        );
                    }
                    if index > 0 {
                        ui.painter().hline(
                            row.x_range(),
                            row.top(),
                            egui::Stroke::new(1.0, colors.border),
                        );
                    }
                    let caption = self.tr("View results");
                    let label_width = ui
                        .painter()
                        .layout_no_wrap(
                            caption.clone(),
                            egui::FontId::proportional(14.04),
                            colors.heading,
                        )
                        .size()
                        .x;
                    let action_width = (label_width + 24.0).max(96.0);
                    let inset = 12.0;
                    let avatar = egui::Rect::from_center_size(
                        egui::pos2(row.left() + inset + 20.0, row.center().y),
                        Vec2::splat(40.0),
                    );
                    ui.painter()
                        .circle_filled(avatar.center(), 20.0, colors.control);
                    if item.has_avatar {
                        self.paint_media(
                            ui,
                            &item.avatar_image,
                            avatar,
                            if item.avatar_square { 8 } else { 20 },
                        );
                    } else {
                        Icon::User.image(colors.secondary, 18.0).paint_at(
                            ui,
                            egui::Rect::from_center_size(avatar.center(), Vec2::splat(18.0)),
                        );
                    }
                    let text_left = avatar.right() + 12.0 - row.left();
                    let text_width =
                        (row.width() - action_width - 12.0 - inset - text_left).max(1.0);
                    self.history_text(
                        ui,
                        egui::Rect::from_min_size(
                            row.min + Vec2::new(text_left, 8.0),
                            Vec2::new(text_width, 24.0),
                        ),
                        item.source.clone(),
                        14.04,
                        false,
                        colors.heading,
                    );
                    let status = format!(
                        "{}{}{}",
                        self.tr(&item.status),
                        if item.history_id.is_empty() && item.media_count == 0 {
                            String::new()
                        } else {
                            format!(
                                " · {}",
                                self.tr("{} media")
                                    .replace("{}", &item.media_count.to_string())
                            )
                        },
                        if item.error.is_empty() {
                            String::new()
                        } else {
                            format!(" · {}", item.error)
                        }
                    );
                    self.history_text(
                        ui,
                        egui::Rect::from_min_size(
                            row.min + Vec2::new(text_left, 36.0),
                            Vec2::new(text_width, 20.0),
                        ),
                        status,
                        12.0,
                        false,
                        if item.status == "Failed" {
                            Color32::from_rgb(183, 55, 49)
                        } else {
                            colors.secondary
                        },
                    );
                    let mut result = child(
                        ui,
                        ("batch-result", item.index),
                        egui::Rect::from_min_size(
                            row.right_top() + Vec2::new(-action_width - inset, 18.0),
                            Vec2::new(action_width, 32.0),
                        ),
                    );
                    let response = result
                        .add_enabled_ui(
                            !app.get_history_busy()
                                && (!self.busy() || app.get_batch_running())
                                && !item.history_id.is_empty(),
                            |ui| {
                                self.action_button(
                                    ui,
                                    &caption,
                                    ActionStyle {
                                        exact_width: true,
                                        ..ActionStyle::new(Vec2::new(action_width, 32.0))
                                    },
                                )
                            },
                        )
                        .inner;
                    if response.clicked() {
                        app.invoke_open_batch_result(item.index);
                    }
                }
            });
        let response = ui.interact(
            panel,
            ui.id().with("batch-queue-panel"),
            egui::Sense::hover(),
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Batch queue")
        });
    }
}
