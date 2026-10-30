use crate::ui::components::dialog::{child, galley};
use crate::ui::*;

impl Desktop {
    pub(in crate::ui) fn history_labels_dialog(&mut self, ctx: &egui::Context) {
        let app = self.app.clone();
        if !app.get_account_labels_open() {
            self.labels_dialog_opened = false;
            return;
        }
        let first = !self.labels_dialog_opened;
        self.labels_dialog_opened = true;
        if first {
            self.labels_group.clear();
            self.labels_tags.clear();
            self.labels_family = 0;
            self.labels_shade = 4;
        }
        let colors = appearance::Palette::new(
            &app.global::<AppTheme>().get_preset(),
            ctx.theme() == egui::Theme::Dark,
        );
        let mut group = self.labels_group.clone();
        let mut tags = self.labels_tags.clone();
        let (mut family, mut shade) = (self.labels_family, self.labels_shade);
        let description =
            self.tr("Replace labels for selected accounts. Empty fields clear existing labels.");
        let width = 460.0_f32.min(ctx.content_rect().width() - 48.0);
        let inner_width = width - 48.0;
        let description = galley(ctx, description, inner_width, 12.0, false, colors.secondary);
        let error = app.get_account_labels_error();
        let error_height = if error.is_empty() {
            0.0
        } else {
            galley(ctx, self.tr(&error), inner_width, 12.0, false, Color32::RED)
                .size()
                .y
                + 12.0
        };
        let height =
            260.0 + description.size().y + if family > 0 { 92.0 } else { 0.0 } + error_height;
        let popup_open = egui::Popup::is_any_open(ctx);
        let mut close = !popup_open
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        egui::Modal::new(egui::Id::new("history-labels"))
            .backdrop_color(Color32::from_black_alpha(153))
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let panel =
                    egui::Rect::from_min_size(ui.next_widget_position(), Vec2::new(width, height));
                let response = ui.allocate_rect(panel, egui::Sense::hover());
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Other,
                        true,
                        "Account labels dialog",
                    )
                });
                ui.painter().rect_filled(panel, 12, colors.surface);
                ui.painter().rect_stroke(
                    panel,
                    12,
                    egui::Stroke::new(1.0, colors.border),
                    egui::StrokeKind::Inside,
                );
                let inner = panel.shrink(24.0);
                let mut y = inner.top();
                self.history_text(
                    ui,
                    egui::Rect::from_min_size(
                        egui::pos2(inner.left(), y),
                        Vec2::new(inner_width, 24.0),
                    ),
                    self.tr("Assign group and tags"),
                    17.0,
                    true,
                    colors.heading,
                );
                y += 36.0;
                ui.painter().galley(
                    egui::pos2(inner.left(), y),
                    description.clone(),
                    colors.secondary,
                );
                y += description.size().y + 12.0;
                for (id, label, value) in [
                    ("history-label-group", "Group", &mut group),
                    ("history-label-tags", "Tags, separated by commas", &mut tags),
                ] {
                    let rect = egui::Rect::from_min_size(
                        egui::pos2(inner.left(), y),
                        Vec2::new(inner_width, appearance::CONTROL_HEIGHT),
                    );
                    let response = self.dialog_field(
                        &mut child(ui, id, rect),
                        id,
                        label,
                        value,
                        &self.tr(label),
                        rect.size(),
                        false,
                    );
                    if first && id == "history-label-group" {
                        response.request_focus();
                    }
                    y += appearance::CONTROL_HEIGHT + 12.0;
                }
                let picker = egui::Rect::from_min_size(
                    egui::pos2(inner.left(), y),
                    Vec2::new(inner_width, appearance::CONTROL_HEIGHT),
                );
                let mut picker_ui = child(ui, "label-color-picker", picker);
                picker_ui.spacing_mut().interact_size.y = appearance::CONTROL_HEIGHT;
                picker_ui.spacing_mut().button_padding = Vec2::new(10.0, 6.0);
                let choices = std::iter::once(self.tr("Default color"))
                    .chain(swatches::FAMILIES.iter().map(|(name, _)| {
                        self.tr(&format!("{}{}", name[..1].to_uppercase(), &name[1..]))
                    }))
                    .collect::<Vec<_>>();
                let markers = std::iter::once(colors.accent)
                    .chain(
                        swatches::FAMILIES
                            .iter()
                            .map(|(name, _)| swatches::color(name, 4)),
                    )
                    .collect::<Vec<_>>();
                let current = if family == 0 {
                    colors.accent
                } else {
                    swatches::color(swatches::FAMILIES[family - 1].0, shade)
                };
                family = self.choice_picker(
                    &mut picker_ui,
                    picker,
                    &crate::ui::components::dropdown::SelectOptions {
                        markers: &markers,
                        selected_marker: Some(current),
                        ..crate::ui::components::dropdown::SelectOptions::new(
                            "Group color",
                            &choices,
                        )
                    },
                    family,
                );
                y += appearance::CONTROL_HEIGHT + 12.0;
                if family > 0 {
                    let color = swatches::color(swatches::FAMILIES[family - 1].0, shade);
                    self.history_text(
                        ui,
                        egui::Rect::from_min_size(
                            egui::pos2(inner.left(), y),
                            Vec2::new(inner_width, 18.0),
                        ),
                        self.tr("Shade"),
                        12.0,
                        false,
                        colors.secondary,
                    );
                    let caption = format!(
                        "{} · #{:02x}{:02x}{:02x}",
                        swatches::LEVELS[shade],
                        color.r(),
                        color.g(),
                        color.b()
                    );
                    let text = ui.painter().layout_no_wrap(
                        caption,
                        egui::FontId::proportional(12.0),
                        colors.secondary,
                    );
                    ui.painter().galley(
                        egui::pos2(inner.right() - text.size().x, y),
                        text,
                        colors.secondary,
                    );
                    let shade_id = egui::Id::new("history-label-shades");
                    let shades = egui::Rect::from_min_size(
                        egui::pos2(inner.left(), y + 26.0),
                        Vec2::new(inner_width, 54.0),
                    );
                    let focus =
                        ui.interact(shades, shade_id, egui::Sense::focusable_noninteractive());
                    focus.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Other, true, self.tr("Shade"))
                    });
                    if focus.has_focus() {
                        ui.memory_mut(|m| {
                            m.set_focus_lock_filter(
                                shade_id,
                                egui::EventFilter {
                                    horizontal_arrows: true,
                                    ..Default::default()
                                },
                            )
                        });
                        for (key, next) in [
                            (egui::Key::ArrowLeft, shade.saturating_sub(1)),
                            (egui::Key::ArrowRight, (shade + 1).min(10)),
                            (egui::Key::Home, 0),
                            (egui::Key::End, 10),
                        ] {
                            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key)) {
                                shade = next;
                            }
                        }
                    }
                    let item_width = (inner_width - 40.0) / 11.0;
                    for (index, level) in swatches::LEVELS.iter().enumerate() {
                        let rect = egui::Rect::from_min_size(
                            shades.min + Vec2::new(index as f32 * (item_width + 4.0), 0.0),
                            Vec2::new(item_width, 54.0),
                        );
                        let color = swatches::color(swatches::FAMILIES[family - 1].0, index);
                        let label = format!(
                            "{} {level} · #{:02x}{:02x}{:02x}",
                            choices[family].clone(),
                            color.r(),
                            color.g(),
                            color.b()
                        );
                        let response = ui
                            .interact(rect, ui.id().with(("shade", index)), egui::Sense::CLICK)
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        let tile = egui::Rect::from_min_size(rect.min, Vec2::new(item_width, 34.0));
                        if shade == index {
                            ui.painter().rect_stroke(
                                tile,
                                7,
                                egui::Stroke::new(
                                    2.0,
                                    if focus.has_focus() {
                                        colors.heading
                                    } else {
                                        colors.accent
                                    },
                                ),
                                egui::StrokeKind::Inside,
                            );
                        }
                        ui.painter().rect_filled(tile.shrink(3.0), 4, color);
                        ui.painter().rect_stroke(
                            tile.shrink(3.0),
                            4,
                            egui::Stroke::new(1.0, colors.input_border),
                            egui::StrokeKind::Inside,
                        );
                        ui.painter().text(
                            egui::pos2(rect.center().x, rect.top() + 46.0),
                            egui::Align2::CENTER_CENTER,
                            level.to_string(),
                            egui::FontId::proportional(typography::MICRO),
                            if shade == index {
                                colors.accent
                            } else {
                                colors.secondary
                            },
                        );
                        response.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::WidgetType::RadioButton,
                                true,
                                shade == index,
                                label.clone(),
                            )
                        });
                        if response.clicked() {
                            shade = index;
                            focus.request_focus();
                        }
                        self.control_tooltip(ui, &response, &label);
                    }
                    y += 92.0;
                }
                if !error.is_empty() {
                    let text = galley(
                        ctx,
                        self.tr(&error),
                        inner_width,
                        12.0,
                        false,
                        Color32::from_rgb(183, 55, 49),
                    );
                    ui.painter().galley(
                        egui::pos2(inner.left(), y),
                        text.clone(),
                        Color32::from_rgb(183, 55, 49),
                    );
                    y += text.size().y + 12.0;
                }
                let buttons = egui::Rect::from_min_size(
                    egui::pos2(inner.left(), y),
                    Vec2::new((inner_width - 10.0) / 2.0, appearance::CONTROL_HEIGHT),
                );
                close |= self
                    .history_button(ui, buttons, &self.tr("Cancel"), None, false, true)
                    .clicked();
                if self
                    .history_button(
                        ui,
                        buttons.translate(Vec2::new(buttons.width() + 10.0, 0.0)),
                        &self.tr("Save"),
                        None,
                        true,
                        !self.busy(),
                    )
                    .clicked()
                {
                    let color = if family == 0 {
                        String::new()
                    } else {
                        let color = swatches::color(swatches::FAMILIES[family - 1].0, shade);
                        format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
                    };
                    close |= app.invoke_apply_account_labels(group.clone(), tags.clone(), color);
                }
            });
        self.labels_group = group;
        self.labels_tags = tags;
        self.labels_family = family;
        self.labels_shade = shade;
        if close {
            app.set_account_labels_open(false);
            self.labels_dialog_opened = false;
        }
    }
}
