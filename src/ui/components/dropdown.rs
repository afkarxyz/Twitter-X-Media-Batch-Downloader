use crate::ui::*;
pub(in crate::ui) type ChoiceFlag = (&'static str, &'static [u8]);
pub(in crate::ui) struct SelectOptions<'a> {
    pub label: &'a str,
    pub choices: &'a [String],
    pub markers: &'a [Color32],
    pub selected_marker: Option<Color32>,
    pub flags: &'a [Option<ChoiceFlag>],
}
impl<'a> SelectOptions<'a> {
    pub fn new(label: &'a str, choices: &'a [String]) -> Self {
        Self {
            label,
            choices,
            flags: &[],
            markers: &[],
            selected_marker: None,
        }
    }
}
impl Desktop {
    pub(in crate::ui) fn choice_picker(
        &self,
        ui: &mut Ui,
        rect: egui::Rect,
        options: &SelectOptions<'_>,
        selected: usize,
    ) -> usize {
        let rect = egui::Rect::from_center_size(
            rect.center(),
            Vec2::new(rect.width(), appearance::CONTROL_HEIGHT),
        );
        let choices = options.choices;
        let label = options.label;
        if choices.is_empty() {
            return 0;
        }
        let colors = self.colors(ui);
        let id = ui.id().with(("select", label));
        let popup_id = id.with("choices");
        let mut selected = selected.min(choices.len() - 1);
        let mut open = egui::Popup::is_id_open(ui.ctx(), popup_id);
        let mut highlighted = ui
            .ctx()
            .data_mut(|data| data.get_temp::<usize>(id.with("highlighted")))
            .unwrap_or(selected)
            .min(choices.len() - 1);
        let mut reveal = false;
        let response = ui
            .interact(rect, id, egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let focus =
            ui.is_enabled() && crate::ui::components::navigation::focus_visible(ui, &response);
        let mut activate = false;
        let mut close = false;
        if response.has_focus() {
            ui.memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    id,
                    egui::EventFilter {
                        vertical_arrows: true,
                        tab: open,
                        escape: open,
                        ..Default::default()
                    },
                )
            });
            activate = ui.input_mut(|input| {
                input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                    || input.consume_key(egui::Modifiers::NONE, egui::Key::Space)
            });
            for (key, offset) in [(egui::Key::ArrowDown, 1), (egui::Key::ArrowUp, -1)] {
                if ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, key)) {
                    reveal = true;
                    if open {
                        highlighted = (highlighted as i32 + offset)
                            .clamp(0, choices.len() as i32 - 1)
                            as usize;
                    } else {
                        open = true;
                        highlighted = selected;
                    }
                }
            }
            close = open
                && ui.input_mut(|input| {
                    input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                        || input.consume_key(egui::Modifiers::NONE, egui::Key::Tab)
                        || input.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab)
                });
        }
        if response.clicked() && !activate {
            reveal = true;
            open = !open;
            highlighted = selected;
            response.request_focus();
        }
        if activate {
            if open {
                selected = highlighted;
                close = true;
            } else {
                open = true;
                highlighted = selected;
            }
        }
        if !ui.is_enabled() {
            open = false;
        }
        if close {
            open = false;
            response.request_focus();
        }
        if open {
            egui::Popup::open_id(ui.ctx(), popup_id);
        } else {
            egui::Popup::close_id(ui.ctx(), popup_id);
        }
        ui.painter().rect_filled(rect, 7, colors.surface);
        ui.painter().rect_stroke(
            rect,
            7,
            egui::Stroke::new(
                if open || focus { 2.0 } else { 1.0 },
                if open || focus {
                    colors.accent
                } else {
                    colors.input_border
                },
            ),
            egui::StrokeKind::Inside,
        );
        let mut job = egui::text::LayoutJob::simple(
            choices[selected].clone(),
            egui::FontId::proportional(typography::BODY),
            colors.heading,
            rect.width()
                - if options.flags.get(selected).is_some_and(Option::is_some)
                    || options.markers.get(selected).is_some()
                {
                    68.0
                } else {
                    45.0
                },
        );
        job.wrap.max_rows = 1;
        let text = ui.painter().layout_job(job);
        ui.painter().galley(
            egui::pos2(
                rect.left()
                    + if options.flags.get(selected).is_some_and(Option::is_some)
                        || options.markers.get(selected).is_some()
                    {
                        34.0
                    } else {
                        11.0
                    },
                rect.center().y - text.size().y / 2.0,
            ),
            text,
            colors.heading,
        );
        if let Some(marker) = options
            .selected_marker
            .or_else(|| options.markers.get(selected).copied())
        {
            ui.painter()
                .circle_filled(rect.left_center() + Vec2::new(18.0, 0.0), 7.0, marker);
        }
        if let Some(Some((uri, bytes))) = options.flags.get(selected) {
            egui::Image::from_bytes(*uri, *bytes).paint_at(
                ui,
                egui::Rect::from_min_size(rect.min + Vec2::new(10.0, 10.0), Vec2::new(18.0, 14.0)),
            );
        }
        (if open {
            Icon::ChevronUp
        } else {
            Icon::ChevronDown
        })
        .image(colors.secondary, 16.0)
        .paint_at(
            ui,
            egui::Rect::from_center_size(
                egui::pos2(rect.right() - 18.0, rect.center().y),
                Vec2::splat(16.0),
            ),
        );
        response.widget_info(|| {
            let mut info = egui::WidgetInfo::labeled(
                egui::WidgetType::ComboBox,
                ui.is_enabled(),
                self.tr(label),
            );
            info.current_text_value = Some(choices[selected].clone());
            info
        });
        let mut chosen = None;
        egui::Popup::from_response(&response)
            .id(popup_id)
            .open_memory(None)
            .gap(4.0)
            .align(egui::emath::RectAlign::BOTTOM_START)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .frame(egui::Frame::NONE)
            .show(|ui| {
                let bounds = egui::Rect::from_min_size(
                    ui.next_widget_position(),
                    Vec2::new(
                        rect.width(),
                        (choices.len() as f32 * (appearance::CONTROL_HEIGHT + 2.0) + 6.0)
                            .min(280.0),
                    ),
                );
                ui.allocate_rect(bounds, egui::Sense::hover());
                ui.painter().add(
                    egui::epaint::Shadow {
                        offset: [0, 0],
                        blur: 12,
                        spread: 0,
                        color: Color32::from_black_alpha(36),
                    }
                    .as_shape(bounds, 8),
                );
                ui.painter().rect_filled(bounds, 8, colors.surface);
                ui.painter().rect_stroke(
                    bounds,
                    8,
                    egui::Stroke::new(1.0, colors.input_border),
                    egui::StrokeKind::Inside,
                );
                let mut list =
                    crate::ui::components::dialog::child(ui, "select-options", bounds.shrink(4.0));
                egui::ScrollArea::vertical()
                    .id_salt(id.with("options-scroll"))
                    .max_height(bounds.height() - 8.0)
                    .auto_shrink([false, false])
                    .show_themed(&mut list, |ui| {
                        let (grid, _) = ui.allocate_exact_size(
                            Vec2::new(
                                bounds.width() - 8.0,
                                choices.len() as f32 * (appearance::CONTROL_HEIGHT + 2.0) - 2.0,
                            ),
                            egui::Sense::hover(),
                        );
                        for (index, choice) in choices.iter().enumerate() {
                            let row = egui::Rect::from_min_size(
                                grid.min
                                    + Vec2::new(
                                        0.0,
                                        index as f32 * (appearance::CONTROL_HEIGHT + 2.0),
                                    ),
                                Vec2::new(bounds.width() - 8.0, appearance::CONTROL_HEIGHT),
                            );
                            if reveal && index == highlighted {
                                ui.scroll_to_rect(row, None);
                            }
                            let response = ui
                                .interact(row, id.with(("choice", index)), egui::Sense::CLICK)
                                .on_hover_cursor(egui::CursorIcon::PointingHand);
                            if index == highlighted || response.hovered() {
                                ui.painter().rect_filled(row, 5, colors.control);
                            }
                            let inset = if options.flags.get(index).is_some_and(Option::is_some)
                                || options.markers.get(index).is_some()
                            {
                                34.0
                            } else {
                                9.0
                            };
                            self.history_text(
                                ui,
                                egui::Rect::from_min_max(
                                    row.min + Vec2::new(inset, 0.0),
                                    row.max - Vec2::new(28.0, 0.0),
                                ),
                                choice.clone(),
                                14.0,
                                false,
                                colors.heading,
                            );
                            if let Some(marker) = options.markers.get(index) {
                                ui.painter().circle_filled(
                                    row.left_center() + Vec2::new(16.0, 0.0),
                                    7.0,
                                    *marker,
                                );
                            }
                            if let Some(Some((uri, bytes))) = options.flags.get(index) {
                                egui::Image::from_bytes(*uri, *bytes).paint_at(
                                    ui,
                                    egui::Rect::from_min_size(
                                        row.min + Vec2::new(8.0, 8.0),
                                        Vec2::new(18.0, 14.0),
                                    ),
                                );
                            }
                            if index == selected {
                                Icon::Check.image(colors.accent, 16.0).paint_at(
                                    ui,
                                    egui::Rect::from_min_size(
                                        row.right_top() + Vec2::new(-24.0, 8.0),
                                        Vec2::splat(16.0),
                                    ),
                                );
                            }
                            response.widget_info(|| {
                                egui::WidgetInfo::selected(
                                    egui::WidgetType::Button,
                                    true,
                                    index == selected,
                                    choice.as_str(),
                                )
                            });
                            if response.clicked() {
                                chosen = Some(index);
                                ui.close();
                            }
                        }
                    });
            });
        if let Some(index) = chosen {
            selected = index;
            highlighted = index;
            egui::Popup::close_id(ui.ctx(), popup_id);
            response.request_focus();
        }
        ui.ctx()
            .data_mut(|data| data.insert_temp(id.with("highlighted"), highlighted));
        selected
    }
}
