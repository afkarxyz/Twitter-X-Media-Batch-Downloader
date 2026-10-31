pub(in crate::ui) mod cards;
pub(in crate::ui) mod details;
pub(in crate::ui) mod labels;
pub(in crate::ui) mod toolbar;
use crate::ui::components::dialog::{child, galley};
use crate::ui::*;

pub(in crate::ui) struct HistoryConfirmation {
    pub id: Option<String>,
    pub title: String,
    pub group: bool,
    pub saved: bool,
}

impl Desktop {
    pub(in crate::ui) fn history(&mut self, ui: &mut Ui) {
        if self.app.get_history_show_saved() {
            self.app.set_history_show_saved(false);
            self.app
                .invoke_search_history(self.app.get_history_search_query(), false);
        }
        let bounds = ui.max_rect();
        ui.allocate_rect(bounds, egui::Sense::hover());
        // Session details stay at the reading width; the account grid gains columns.
        let details = !self.app.get_fetch_sessions_key().is_empty();
        let width = if details {
            bounds.width().min(appearance::CARD_PAGE_WIDTH)
        } else {
            appearance::card_page_width(bounds.width())
        };
        let page = egui::Rect::from_center_size(bounds.center(), Vec2::new(width, bounds.height()))
            .shrink(12.0);
        let mut content = child(ui, "history-page", page);
        if details {
            self.fetch_session_details(&mut content);
            return;
        }
        let header = egui::Rect::from_min_size(page.min, Vec2::new(page.width(), 44.0));
        self.history_header(&mut content, header);
        let filters = egui::Rect::from_min_size(
            egui::pos2(page.left(), header.bottom() + 16.0),
            Vec2::new(page.width(), 42.0),
        );
        self.history_filters(&mut content, filters);
        let y = filters.bottom() + 16.0;
        let list = egui::Rect::from_min_max(egui::pos2(page.left(), y), page.max);
        let rows = self.app.get_history_items();
        if rows.row_count() == 0 {
            self.history_empty(&mut content, list);
            return;
        }
        let key = format!(
            "{}:{}:{}",
            self.app.get_history_show_saved(),
            self.app.get_history_source_filter(),
            self.app.get_history_search_query()
        );
        let reset = key != self.history_scroll_key;
        self.history_scroll_key = key;
        let mut list_ui = child(&mut content, "history-grid", list);
        let area = egui::ScrollArea::vertical()
            .id_salt("history-items")
            .scroll_bar_rect(list.shrink2(Vec2::new(0.0, 16.0)))
            .max_height(list.height())
            .auto_shrink([false, false]);
        let area = if reset {
            area.vertical_scroll_offset(0.0)
        } else {
            area
        };
        area.show_themed(&mut list_ui, |ui| {
            let width = ui.available_width();
            let columns = appearance::account_card_columns(width);
            let gap = appearance::ACCOUNT_CARD_GAP;
            let card_width = (width - (columns - 1) as f32 * gap) / columns as f32;
            let pitch = appearance::ACCOUNT_CARD_HEIGHT + appearance::ACCOUNT_CARD_ROW_GAP;
            let height = (rows.row_count().div_ceil(columns) as f32 * pitch
                - appearance::ACCOUNT_CARD_ROW_GAP)
                .max(0.0);
            let (grid, _) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
            for (index, row) in rows.iter().enumerate() {
                let rect = egui::Rect::from_min_size(
                    grid.min
                        + Vec2::new(
                            (index % columns) as f32 * (card_width + gap),
                            (index / columns) as f32 * pitch,
                        ),
                    Vec2::new(card_width, appearance::ACCOUNT_CARD_HEIGHT),
                );
                self.history_card(ui, rect, &row, false);
            }
        });
    }

    pub(in crate::ui) fn history_text(
        &self,
        ui: &Ui,
        rect: egui::Rect,
        text: String,
        size: f32,
        bold: bool,
        color: Color32,
    ) {
        let font = egui::FontId::new(
            typography::size(size),
            if bold {
                egui::FontFamily::Name("cera-bold".into())
            } else {
                egui::FontFamily::Proportional
            },
        );
        let mut job = egui::text::LayoutJob::simple(text, font, color, rect.width().max(0.0));
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        let galley = ui.painter().layout_job(job);
        ui.painter().galley(
            egui::pos2(rect.left(), rect.center().y - galley.size().y / 2.0),
            galley,
            color,
        );
    }

    pub(in crate::ui) fn history_title(&self, ui: &Ui, rect: egui::Rect, title: String) {
        self.history_text(
            ui,
            rect.shrink2(Vec2::new(6.0, 0.0)),
            title,
            20.0,
            true,
            self.colors(ui).title,
        );
    }

    fn history_empty(&self, ui: &mut Ui, bounds: egui::Rect) {
        let colors = self.colors(ui);
        let saved = self.app.get_history_show_saved();
        let circle = egui::Rect::from_center_size(
            egui::pos2(bounds.center().x, bounds.top() + 64.0),
            Vec2::splat(48.0),
        );
        ui.painter().rect_filled(circle, 24, colors.surface);
        ui.painter().rect_stroke(
            circle,
            24,
            egui::Stroke::new(1.0, colors.border),
            egui::StrokeKind::Inside,
        );
        (if saved {
            Icon::HardDrive
        } else {
            Icon::History
        })
        .image(colors.secondary, 20.0)
        .paint_at(
            ui,
            egui::Rect::from_center_size(circle.center(), Vec2::splat(20.0)),
        );
        let query = self.app.get_history_search_query();
        let text = if self.app.get_history_busy() {
            self.tr("Loading history…")
        } else if !query.is_empty() {
            self.tr("No accounts match “{}”.").replace("{}", &query)
        } else if self.app.get_history_source_filter() > 0 {
            self.tr("No fetches match this filter.")
        } else {
            self.tr(if saved {
                "No downloads yet."
            } else {
                "No accounts fetched yet."
            })
        };
        let text = galley(
            ui.ctx(),
            text,
            bounds.width() - 40.0,
            14.0,
            false,
            colors.secondary,
        );
        ui.painter().galley(
            egui::pos2(
                bounds.center().x - text.size().x / 2.0,
                bounds.top() + 100.0,
            ),
            text,
            colors.secondary,
        );
    }

    pub(in crate::ui) fn history_confirm(&mut self, ctx: &egui::Context) {
        if self.app.get_selected_tab() != 1 {
            self.history_confirmation = None;
            return;
        }
        let Some(confirmation) = self.history_confirmation.as_ref() else {
            return;
        };
        let title = if confirmation.id.is_some() {
            self.tr("Remove {} from history?")
                .replace("{}", &confirmation.title)
        } else {
            self.tr(if confirmation.saved {
                "Clear download history?"
            } else {
                "Clear fetch history?"
            })
        };
        let message=self.tr(if confirmation.group{"All fetch sessions in this group will be removed. Downloaded files will remain in your folder."}else{"The history records will be removed. Downloaded files will remain in your folder."});
        let action = if confirmation.id.is_some() {
            "Remove"
        } else {
            "Clear history"
        };
        let (close, confirmed) =
            self.destructive_confirm(ctx, "History confirmation", title, message, action);
        if confirmed {
            let confirmation = self.history_confirmation.take().unwrap();
            if let Some(id) = confirmation.id {
                self.app.invoke_remove_history(id);
            } else {
                self.app.invoke_clear_history();
            }
        } else if close {
            self.history_confirmation = None;
        }
    }

    /// A centered confirmation for an irreversible action; returns (closed, confirmed).
    /// `label` names the dialog for accessibility and keeps its state apart.
    pub(in crate::ui) fn destructive_confirm(
        &self,
        ctx: &egui::Context,
        label: &str,
        title: String,
        message: String,
        action: &str,
    ) -> (bool, bool) {
        let colors = appearance::Palette::new(
            &self.app.global::<AppTheme>().get_preset(),
            ctx.theme() == egui::Theme::Dark,
        );
        let centered = |text: String, size: f32, bold: bool, line: f32, color: Color32| {
            let font = egui::FontId::new(
                size,
                if bold {
                    egui::FontFamily::Name("cera-bold".into())
                } else {
                    egui::FontFamily::Proportional
                },
            );
            let mut job = egui::text::LayoutJob::simple(text, font, color, 332.0);
            job.halign = egui::Align::Center;
            job.sections[0].format.line_height = Some(line);
            ctx.fonts_mut(|fonts| fonts.layout_job(job))
        };
        let title_galley = centered(title, 17.0, true, 24.0, colors.heading);
        let message_galley = centered(message, 14.0, false, 21.0, colors.secondary);
        let height = 174.0 + title_galley.size().y + message_galley.size().y;
        let mut close = false;
        let mut confirmed = false;
        let id = egui::Id::new(("confirmation", label));
        let response = egui::Modal::new(id)
            .area(egui::Modal::default_area(id).anchor(egui::Align2::CENTER_CENTER, [28.0, 0.0]))
            .backdrop_color(Color32::TRANSPARENT)
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let viewport = ctx.content_rect();
                let backdrop = egui::Rect::from_min_max(
                    viewport.min + Vec2::new(appearance::RAIL_WIDTH, 0.0),
                    viewport.max,
                );
                ui.painter()
                    .rect_filled(backdrop, 0, Color32::from_black_alpha(199));
                let panel =
                    egui::Rect::from_min_size(ui.next_widget_position(), Vec2::new(380.0, height));
                let response = ui.allocate_rect(panel, egui::Sense::hover());
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Other, true, label)
                });
                ui.painter().rect_filled(panel, 18, colors.surface);
                ui.painter().rect_stroke(
                    panel,
                    18,
                    egui::Stroke::new(1.0, colors.border),
                    egui::StrokeKind::Inside,
                );
                let mut y = panel.top() + 24.0;
                let warning = egui::pos2(panel.center().x, y + 24.0);
                let destructive = Color32::from_rgb(183, 55, 49);
                ui.painter()
                    .circle_filled(warning, 24.0, destructive.gamma_multiply(0.12));
                Icon::Alert
                    .image(destructive, 22.0)
                    .paint_at(ui, egui::Rect::from_center_size(warning, Vec2::splat(22.0)));
                y += 62.0;
                ui.painter().galley(
                    egui::pos2(panel.center().x, y),
                    title_galley.clone(),
                    colors.heading,
                );
                y += title_galley.size().y + 14.0;
                ui.painter().galley(
                    egui::pos2(panel.center().x, y),
                    message_galley.clone(),
                    colors.secondary,
                );
                let buttons = egui::Rect::from_min_size(
                    egui::pos2(panel.left() + 24.0, panel.bottom() - 56.0),
                    Vec2::new(161.0, appearance::CONTROL_HEIGHT),
                );
                close |= self
                    .home_action(
                        &mut child(ui, "history-cancel", buttons),
                        &self.tr("Cancel"),
                        None,
                        false,
                        buttons.size(),
                        10,
                    )
                    .clicked();
                let remove = buttons.translate(Vec2::new(171.0, 0.0));
                let mut action_ui = child(ui, "history-confirm-remove", remove);
                if self.busy() {
                    action_ui.disable();
                }
                confirmed = self
                    .action_button(
                        &mut action_ui,
                        &self.tr(action),
                        ActionStyle {
                            tone: ButtonTone::Destructive,
                            icon: Some((Icon::Trash, appearance::CONTROL_ICON_SIZE)),
                            radius: 10,
                            exact_width: true,
                            ..ActionStyle::new(remove.size())
                        },
                    )
                    .clicked();
            });
        close |= response.should_close();
        (close, confirmed)
    }
}
