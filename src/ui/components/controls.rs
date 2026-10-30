use crate::ui::components::dialog::child;
use crate::ui::components::icon_button::interaction_fill;
use crate::ui::*;

#[derive(Clone, Copy, Default)]
pub(in crate::ui) enum ButtonTone {
    #[default]
    Neutral,
    Primary,
    Destructive,
    Selected,
}

pub(in crate::ui) struct ActionStyle {
    pub size: Vec2,
    pub icon: Option<(Icon, f32)>,
    pub tone: ButtonTone,
    pub radius: u8,
    pub exact_width: bool,
    pub truncate_label: bool,
    pub measure_bold: bool,
    pub measure_regular: bool,
    pub ghost: bool,
    pub align_left: bool,
}

impl ActionStyle {
    pub fn new(size: Vec2) -> Self {
        Self {
            size: Vec2::new(size.x, appearance::CONTROL_HEIGHT),
            icon: None,
            tone: ButtonTone::Neutral,
            radius: 6,
            exact_width: false,
            truncate_label: false,
            measure_bold: true,
            measure_regular: false,
            ghost: false,
            align_left: false,
        }
    }
}

fn ease_out(time: f32) -> f32 {
    let mut low = 0.0_f32;
    let mut high = 1.0_f32;
    for _ in 0..20 {
        let parameter = (low + high) / 2.0;
        let x = 3.0 * (1.0 - parameter) * parameter * parameter * 0.58 + parameter.powi(3);
        if x < time {
            low = parameter;
        } else {
            high = parameter;
        }
    }
    let parameter = (low + high) / 2.0;
    3.0 * (1.0 - parameter) * parameter * parameter + parameter.powi(3)
}

impl Desktop {
    pub(in crate::ui) fn progress_bar(&self, ui: &mut Ui, progress: f32) {
        let colors = self.colors(ui);
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 4.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 2, colors.control);
        ui.painter().rect_filled(
            egui::Rect::from_min_size(
                rect.min,
                Vec2::new(rect.width() * (progress / 100.0).clamp(0.0, 1.0), 4.0),
            ),
            2,
            colors.accent,
        );
    }

    pub(in crate::ui) fn operation_status_row(
        &self,
        ui: &mut Ui,
        status: &str,
        cancel_label: &str,
    ) -> bool {
        let colors = self.colors(ui);
        let (row, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), appearance::CONTROL_HEIGHT),
            egui::Sense::hover(),
        );
        egui::Spinner::new()
            .size(16.0)
            .color(colors.secondary)
            .paint_at(
                ui,
                egui::Rect::from_center_size(
                    egui::pos2(row.left() + 8.0, row.center().y),
                    Vec2::splat(16.0),
                ),
            );
        let cancel = egui::Rect::from_min_size(
            row.right_top() - Vec2::new(100.0, 0.0),
            Vec2::new(100.0, appearance::CONTROL_HEIGHT),
        );
        let text = egui::Rect::from_min_max(
            row.min + Vec2::new(28.0, 0.0),
            egui::pos2(cancel.left() - 12.0, row.bottom()),
        );
        let mut job = egui::text::LayoutJob::simple(
            status.to_owned(),
            egui::FontId::proportional(typography::BODY),
            colors.heading,
            text.width().max(1.0),
        );
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        let galley = ui.painter().layout_job(job);
        ui.painter().galley(
            egui::pos2(text.left(), text.center().y - galley.size().y / 2.0),
            galley,
            colors.heading,
        );
        ui.interact(
            text,
            ui.id().with(("operation-status", cancel_label)),
            egui::Sense::hover(),
        )
        .widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), status)
        });
        let mut action = child(ui, ("operation-cancel", cancel_label), cancel);
        self.action_button(
            &mut action,
            &self.tr(cancel_label),
            ActionStyle {
                tone: ButtonTone::Destructive,
                icon: Some((Icon::Close, appearance::CONTROL_ICON_SIZE)),
                exact_width: true,
                truncate_label: true,
                ..ActionStyle::new(cancel.size())
            },
        )
        .clicked()
    }

    pub(in crate::ui) fn action_button(
        &self,
        ui: &mut Ui,
        text: &str,
        mut style: ActionStyle,
    ) -> egui::Response {
        if let Some((_, size)) = &mut style.icon {
            *size = appearance::CONTROL_ICON_SIZE;
        }
        let colors = self.colors(ui);
        let filled = matches!(style.tone, ButtonTone::Primary | ButtonTone::Destructive);
        let selected = matches!(style.tone, ButtonTone::Selected);
        let foreground = if matches!(style.tone, ButtonTone::Primary) {
            colors.on_accent
        } else if filled {
            Color32::WHITE
        } else if selected {
            colors.accent
        } else {
            colors.heading
        };
        let font = egui::FontId::new(
            typography::BODY,
            egui::FontFamily::Name(
                if filled || selected {
                    "cera-bold"
                } else {
                    "cera-medium"
                }
                .into(),
            ),
        );
        let galley = if style.truncate_label && style.exact_width && style.size.x > 0.0 {
            let extra = style.icon.map_or(0.0, |(_, size)| size + 7.0);
            let mut job = egui::text::LayoutJob::simple(
                text.to_owned(),
                font,
                foreground,
                (style.size.x - extra - 24.0).max(1.0),
            );
            job.wrap.max_rows = 1;
            job.wrap.break_anywhere = true;
            ui.painter().layout_job(job)
        } else {
            ui.painter()
                .layout_no_wrap(text.to_owned(), font, foreground)
        };
        let truncated = galley.elided;
        let measure = if style.measure_regular {
            ui.painter()
                .layout_no_wrap(
                    text.to_owned(),
                    egui::FontId::proportional(14.04),
                    foreground,
                )
                .size()
                .x
        } else if style.measure_bold {
            ui.painter()
                .layout_no_wrap(
                    text.to_owned(),
                    egui::FontId::new(typography::BODY, egui::FontFamily::Name("cera-bold".into())),
                    foreground,
                )
                .size()
                .x
        } else {
            galley.size().x
        };
        let extra = style.icon.map_or(0.0, |(_, size)| size + 7.0);
        let width = if style.exact_width && style.size.x > 0.0 {
            style.size.x
        } else {
            style.size.x.max(measure + extra + 24.0)
        };
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(width, style.size.y), egui::Sense::hover());
        let button = child(ui, (text, "action"), rect);
        let response = button
            .interact(rect, button.id().with("activate"), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let (idle, hover, pressed) = if !button.is_enabled() {
            let fill = if style.ghost {
                Color32::TRANSPARENT
            } else {
                colors.control
            };
            (fill, fill, fill)
        } else {
            match style.tone {
                ButtonTone::Primary => (
                    colors.accent,
                    colors.accent.gamma_multiply(0.9),
                    colors.accent.gamma_multiply(0.84),
                ),
                ButtonTone::Destructive => (
                    appearance::DANGER_FILL,
                    appearance::DANGER_HOVER,
                    appearance::DANGER_PRESSED,
                ),
                ButtonTone::Selected => {
                    (colors.accent_soft, colors.accent_soft, colors.accent_soft)
                }
                ButtonTone::Neutral => (
                    if style.ghost {
                        Color32::TRANSPARENT
                    } else {
                        colors.surface
                    },
                    colors
                        .control
                        .gamma_multiply(if ui.visuals().dark_mode { 1.08 } else { 1.02 }),
                    colors.muted_control,
                ),
            }
        };
        button.painter().rect_filled(
            rect,
            style.radius,
            interaction_fill(&button, &response, idle, hover, pressed),
        );
        let focus = crate::ui::components::navigation::focus_visible(&button, &response);
        if (!filled && !style.ghost) || focus {
            button.painter().rect_stroke(
                rect,
                style.radius,
                egui::Stroke::new(
                    if focus { 2.0 } else { 1.0 },
                    if focus || selected {
                        colors.accent
                    } else {
                        colors.control_border
                    },
                ),
                egui::StrokeKind::Inside,
            );
        }
        let shift = Vec2::new(0.0, f32::from(response.is_pointer_button_down_on()));
        let left = if style.align_left {
            rect.left() + 12.0
        } else {
            rect.center().x - (galley.size().x + extra) / 2.0
        };
        if let Some((icon, size)) = style.icon {
            icon.image(foreground, size).paint_at(
                &button,
                egui::Rect::from_center_size(
                    egui::pos2(left + size / 2.0, rect.center().y) + shift,
                    Vec2::splat(size),
                ),
            );
        }
        button.painter().galley(
            egui::pos2(left + extra, rect.center().y - galley.size().y / 2.0) + shift,
            galley,
            foreground,
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, button.is_enabled(), text)
        });
        if truncated {
            self.control_tooltip(ui, &response, text);
        }
        if response.clicked() {
            response.request_focus();
        }
        response
    }

    pub(in crate::ui) fn switch_control(
        &self,
        ui: &mut Ui,
        rect: egui::Rect,
        value: &mut bool,
        label: &str,
        enabled: bool,
    ) -> egui::Response {
        let mut control = child(ui, (label, "switch"), rect);
        let parent_opacity = ui.painter().opacity();
        if !enabled && control.is_enabled() {
            control.disable();
        }
        if !control.is_enabled() {
            control.set_opacity(parent_opacity * 0.45);
        }
        let colors = self.colors(ui);
        let mut response = control
            .interact(rect, control.id().with("toggle"), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        if response.clicked() {
            *value = !*value;
            response.mark_changed();
            response.request_focus();
        }
        let amount = ui.ctx().animate_bool_with_time_and_easing(
            response.id.with("checked"),
            *value,
            0.18,
            egui::emath::easing::quadratic_out,
        );
        let tint = ui.ctx().animate_bool_with_time_and_easing(
            response.id.with("tint"),
            *value,
            0.14,
            ease_out,
        );
        control.painter().rect_filled(
            rect,
            11,
            appearance::mix(colors.accent, colors.control_border, tint),
        );
        control.painter().circle_filled(
            egui::pos2(rect.left() + 11.0 + 16.0 * amount, rect.center().y),
            8.0,
            Color32::WHITE,
        );
        if crate::ui::components::navigation::focus_visible(&control, &response) {
            control.painter().rect_stroke(
                rect,
                11,
                egui::Stroke::new(2.0, colors.heading),
                egui::StrokeKind::Inside,
            );
        }
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::Checkbox,
                control.is_enabled(),
                *value,
                self.tr(label),
            )
        });
        response
    }

    pub(in crate::ui) fn control_slider(
        &self,
        ui: &mut Ui,
        rect: egui::Rect,
        label: &str,
        value: &mut f32,
        maximum: f32,
        step: f32,
        enabled: bool,
    ) -> egui::Response {
        let mut control = child(ui, (label, "slider"), rect);
        if !enabled {
            control.disable();
        }
        control.set_opacity(ui.painter().opacity());
        let track_rect = rect.shrink2(Vec2::new(10.0_f32.min(rect.width() / 2.0), 0.0));
        let mut response = control.interact(
            rect,
            control.id().with("value"),
            egui::Sense::click_and_drag(),
        );
        if response.drag_started() {
            response.request_focus();
        }
        let range = maximum.max(0.001);
        if control.is_enabled() {
            ui.input(|input| {
                use egui::accesskit::{Action, ActionData};
                for request in input.accesskit_action_requests(response.id, Action::SetValue) {
                    if let Some(ActionData::NumericValue(next)) = request.data {
                        let next = (next as f32).clamp(0.0, range);
                        if next.is_finite() && next != *value {
                            *value = next;
                            response.mark_changed();
                        }
                    }
                }
            });
        }
        if let Some(point) = response
            .interact_pointer_pos()
            .filter(|_| control.is_enabled())
        {
            let next = ((point.x - track_rect.left()) / track_rect.width().max(1.0))
                .clamp(0.0, 1.0)
                * range;
            if *value != next {
                *value = next;
                response.mark_changed();
            }
        }
        if control.is_enabled() && response.has_focus() {
            control.memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    response.id,
                    egui::EventFilter {
                        horizontal_arrows: true,
                        ..Default::default()
                    },
                )
            });
            for (key, direction) in [(egui::Key::ArrowLeft, -1.0), (egui::Key::ArrowRight, 1.0)] {
                if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key)) {
                    *value = (*value + step * direction).clamp(0.0, range);
                    response.mark_changed();
                }
            }
            for (key, next) in [(egui::Key::Home, 0.0), (egui::Key::End, range)] {
                if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key)) {
                    *value = next;
                    response.mark_changed();
                }
            }
        }
        if response.clicked() {
            response.request_focus();
        }
        let colors = self.colors(ui);
        let center = rect.center().y;
        let track = egui::Rect::from_min_size(
            egui::pos2(track_rect.left(), center - 2.0),
            Vec2::new(track_rect.width(), 4.0),
        );
        let disabled = if ui.visuals().dark_mode {
            Color32::from_white_alpha(41)
        } else {
            Color32::from_black_alpha(56)
        };
        let rail = if !control.is_enabled() {
            disabled
        } else if ui.visuals().dark_mode {
            Color32::from_white_alpha(20)
        } else {
            Color32::from_black_alpha(115)
        };
        let accent = if control.is_enabled() {
            colors.accent
        } else {
            disabled
        };
        control.painter().rect_filled(track, 2, rail);
        let knob = egui::pos2(
            track.left() + track.width() * (*value / range).clamp(0.0, 1.0),
            center,
        );
        control.painter().rect_filled(
            egui::Rect::from_min_max(track.min, egui::pos2(knob.x, track.bottom())),
            2,
            accent,
        );
        control.painter().circle_filled(
            knob,
            10.0,
            if ui.visuals().dark_mode {
                Color32::from_gray(69)
            } else {
                Color32::WHITE
            },
        );
        control.painter().circle_stroke(
            knob,
            10.5,
            egui::Stroke::new(
                1.0,
                if ui.visuals().dark_mode {
                    Color32::from_white_alpha(23)
                } else {
                    Color32::from_black_alpha(25)
                },
            ),
        );
        let radius = if !control.is_enabled() {
            6.0
        } else if response.is_pointer_button_down_on() || response.has_focus() {
            5.0
        } else if response.hovered() {
            7.0
        } else {
            6.0
        };
        control.painter().circle_filled(knob, radius, accent);
        if crate::ui::components::navigation::focus_visible(&control, &response) {
            control.painter().rect_stroke(
                rect,
                5,
                egui::Stroke::new(1.0, colors.accent),
                egui::StrokeKind::Inside,
            );
        }
        response.widget_info(|| {
            let mut info = egui::WidgetInfo::labeled(
                egui::WidgetType::Slider,
                control.is_enabled(),
                self.tr(label),
            );
            info.value = Some(*value as f64);
            info
        });
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_min_numeric_value(0.0);
            node.set_max_numeric_value(range as f64);
            if control.is_enabled() {
                node.add_action(egui::accesskit::Action::SetValue);
            }
        });
        response
    }

    pub(in crate::ui) fn text_context_menu(
        &self,
        response: &egui::Response,
        value: &mut String,
        read_only: bool,
    ) -> bool {
        let key = response.id.with("context-selection");
        let current = egui::text_edit::TextEditState::load(&response.ctx, response.id)
            .and_then(|state| state.cursor.char_range());
        // TextEdit moves its caret on a secondary click; menu actions use the existing selection.
        let preserve = response.secondary_clicked() || response.context_menu_opened();
        let selected = if preserve {
            response
                .ctx
                .data(|data| data.get_temp::<egui::text::CCursorRange>(key))
                .or(current)
        } else {
            current
        };
        let editing = response.ctx.input(|i| {
            i.events.iter().any(|event| {
                matches!(
                    event,
                    egui::Event::Text(_)
                        | egui::Event::Paste(_)
                        | egui::Event::Cut
                        | egui::Event::Key { pressed: true, .. }
                )
            }) || (response.contains_pointer() && i.pointer.primary_pressed())
        });
        if !preserve
            && (editing
                || current.is_some_and(|range| !range.is_empty())
                || response
                    .ctx
                    .data(|data| data.get_temp::<egui::text::CCursorRange>(key))
                    .is_none())
            && let Some(range) = current
        {
            response.ctx.data_mut(|data| data.insert_temp(key, range));
        }
        let mut request = None;
        response.context_menu(|ui| {
            ui.set_min_width(164.0);
            ui.spacing_mut().item_spacing.y = 2.0;
            let selection = selected.filter(|range| !range.is_empty() && !value.is_empty());
            for (label, icon, enabled, action) in [
                ("Cut", Icon::Cut, !read_only && selection.is_some(), 0),
                ("Copy", Icon::Copy, selection.is_some(), 1),
                ("Paste", Icon::Clipboard, !read_only, 2),
                ("Select all", Icon::SelectAll, !value.is_empty(), 3),
            ] {
                if read_only && matches!(action, 0 | 2) {
                    continue;
                }
                let response = ui
                    .add_enabled_ui(enabled, |ui| {
                        self.action_button(
                            ui,
                            &self.tr(label),
                            ActionStyle {
                                icon: Some((icon, 16.0)),
                                ghost: true,
                                align_left: true,
                                exact_width: true,
                                ..ActionStyle::new(Vec2::new(164.0, 32.0))
                            },
                        )
                    })
                    .inner;
                if response.clicked() {
                    request = Some((action, selected));
                    ui.close();
                }
            }
        });
        let Some((action, selection)) = request else {
            if preserve && let Some(range) = selected {
                let mut state = egui::text_edit::TextEditState::load(&response.ctx, response.id)
                    .unwrap_or_default();
                state.cursor.set_char_range(Some(range));
                state.store(&response.ctx, response.id);
            }
            return false;
        };
        let mut state =
            egui::text_edit::TextEditState::load(&response.ctx, response.id).unwrap_or_default();
        let mut undoer = state.undoer();
        if action == 0 {
            undoer.add_undo(&(state.cursor.char_range().unwrap_or_default(), value.clone()));
        }
        if action == 3 {
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(value.chars().count()),
                )));
        }
        if let Some(range) = selection {
            let selected = range.as_sorted_char_range();
            let start: usize = selected.start.into();
            let end: usize = selected.end.into();
            let count = value.chars().count();
            let selected = start.min(count)..end.min(count);
            if matches!(action, 0 | 1) {
                response.ctx.copy_text(
                    value
                        .chars()
                        .skip(selected.start)
                        .take(selected.len())
                        .collect(),
                );
            }
            if action == 0 {
                let mut chars = value.chars().collect::<Vec<_>>();
                chars.drain(selected.clone());
                *value = chars.into_iter().collect();
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::one(
                        egui::text::CCursor::new(selected.start),
                    )));
            }
        }
        if action == 2 {
            response
                .ctx
                .send_viewport_cmd(egui::ViewportCommand::RequestPaste);
        }
        if action == 0 {
            undoer.add_undo(&(state.cursor.char_range().unwrap_or_default(), value.clone()));
            state.set_undoer(undoer);
        }
        if let Some(range) = state.cursor.char_range() {
            response.ctx.data_mut(|data| data.insert_temp(key, range));
        }
        state.store(&response.ctx, response.id);
        response.request_focus();
        response.ctx.request_repaint();
        action == 0
    }
}
