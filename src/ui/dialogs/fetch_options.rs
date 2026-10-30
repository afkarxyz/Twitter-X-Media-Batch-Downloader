use crate::ui::components::dialog::{child, galley, panel};
use crate::ui::*;

const SOURCE_PUBLIC: &str = "Enter a profile, media, or search URL in Home. Protected posts require access from the account signed in with your auth_token.";
const SOURCE_LIKES: &str = "Fetch Likes from the account signed in with your auth_token. Its username is detected automatically.";
const SOURCE_BOOKMARKS: &str =
    "Bookmarks are private to the account signed in with your auth_token. No username is needed.";
const SOURCE_HELP: &str = "Set auth_token in Settings before fetching. Private lists cannot use batch accounts or date filters.";
const BATCH_HELP: &str =
    "Fetch profiles from the batch queue. These options apply to every account in the queue.";
const DATE_HELP: &str = "Search posts from an account within a date range.";
const QUICK_HELP: &str = "Stop after reaching saved data. Turn off for a full fetch. Reposts and private lists always use a full fetch.";
const QUICK_PRIVATE: &str = "Private lists use a full fetch, so Quick refresh is unavailable.";
const QUICK_BATCH: &str = "Stop each account after reaching saved data. Turn off for a full fetch. Reposts always use a full fetch.";

impl Desktop {
    pub(in crate::ui) fn fetch_switch(
        &self,
        ui: &mut Ui,
        value: &mut bool,
        label: &str,
        description: &str,
        enabled: bool,
    ) -> egui::Response {
        let colors = self.colors(ui);
        let width = ui.available_width();
        let description = galley(
            ui.ctx(),
            self.tr(description),
            width - 50.0,
            12.0,
            false,
            colors.secondary,
        );
        let height = 24.0 + description.size().y;
        let (row, _) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
        ui.painter().galley(
            row.min,
            galley(
                ui.ctx(),
                self.tr(label),
                width - 50.0,
                14.0,
                true,
                colors.heading,
            ),
            colors.heading,
        );
        ui.painter().galley(
            row.min + Vec2::new(0.0, 24.0),
            description,
            colors.secondary,
        );
        let switch = egui::Rect::from_center_size(
            egui::pos2(row.right() - 19.0, row.center().y),
            Vec2::new(38.0, 22.0),
        );
        self.switch_control(ui, switch, value, label, enabled)
    }

    pub(in crate::ui) fn fetch_options_dialog(&mut self, ctx: &egui::Context) {
        let app = self.app.clone();
        if !app.get_fetch_options_open() {
            self.fetch_dialog_opened = false;
            return;
        }
        let first = !self.fetch_dialog_opened;
        self.fetch_dialog_opened = true;
        let colors = appearance::Palette::new(
            &app.global::<AppTheme>().get_preset(),
            ctx.theme() == egui::Theme::Dark,
        );
        let width = 520.0_f32.min(ctx.content_rect().width() - 48.0);
        let inner_width = width - 40.0;
        let mut source = app.get_fetch_source_index().clamp(0, 2) as usize;
        let source_help = [SOURCE_PUBLIC, SOURCE_LIKES, SOURCE_BOOKMARKS][source];
        let mut batch = app.get_batch_mode();
        let quick_help = if batch {
            QUICK_BATCH
        } else if source == 0 {
            QUICK_HELP
        } else {
            QUICK_PRIVATE
        };
        let auth_missing = app.get_saved_auth_token().is_empty();
        let mut dates = app.get_date_filter_enabled();
        let mut start = app.get_fetch_start_date();
        let mut end = app.get_fetch_end_date();
        let mut sources = app.get_batch_sources_text();
        let height_of = |text: &str, width: f32| {
            galley(ctx, self.tr(text), width, 12.0, false, colors.secondary)
                .size()
                .y
        };
        let source_height = if batch {
            20.0 + 8.0 + height_of(BATCH_HELP, inner_width)
        } else {
            20.0 + 8.0
                + appearance::CONTROL_HEIGHT
                + 8.0
                + height_of(source_help, inner_width)
                + 8.0
                + height_of(SOURCE_HELP, inner_width)
        } + if auth_missing { 26.0 } else { 0.0 };
        let mut content_height = source_height
            + 33.0
            + 24.0
            + height_of(DATE_HELP, inner_width - 50.0)
            + 16.0
            + 24.0
            + height_of(quick_help, inner_width - 50.0);
        if dates {
            content_height += 70.0;
        }
        if !app.get_fetch_options_error().is_empty() {
            content_height += 16.0 + height_of(&app.get_fetch_options_error(), inner_width);
        }
        let height = (content_height + 88.0).min(ctx.content_rect().height() - 48.0);
        let popup_was_open = egui::Popup::is_any_open(ctx);
        let mut close = !popup_was_open
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        panel(
            ctx,
            "home-fetch-options",
            "Fetch options dialog",
            Vec2::new(width, height),
            colors,
            |ui, inner| {
                let header = egui::Rect::from_min_size(inner.min, Vec2::new(inner.width(), 32.0));
                self.dialog_heading(ui, header, "Fetch options");
                if self
                    .dialog_icon(
                        ui,
                        egui::Rect::from_min_size(
                            header.right_top() - Vec2::new(76.0, 0.0),
                            Vec2::splat(32.0),
                        ),
                        Icon::Reset,
                        "Reset fetch options",
                    )
                    .clicked()
                    && app.invoke_reset_settings(5)
                {
                    batch = app.get_batch_mode();
                    dates = app.get_date_filter_enabled();
                    start = app.get_fetch_start_date();
                    end = app.get_fetch_end_date();
                    sources = app.get_batch_sources_text();
                }
                close |= self
                    .dialog_icon(
                        ui,
                        egui::Rect::from_min_size(
                            header.right_top() - Vec2::new(32.0, 0.0),
                            Vec2::splat(32.0),
                        ),
                        Icon::Close,
                        "Close fetch options",
                    )
                    .clicked();
                let body = egui::Rect::from_min_max(inner.min + Vec2::new(0.0, 48.0), inner.max);
                let mut content = child(ui, "fetch-options-content", body);
                content.spacing_mut().item_spacing.y = 16.0;
                egui::ScrollArea::vertical()
                    .id_salt("fetch-options-scroll")
                    .max_height(body.height())
                    .scroll_bar_rect(body.shrink2(Vec2::new(0.0, 16.0)))
                    .auto_shrink([false, false])
                    .show_themed(&mut content, |ui| {
                        ui.set_width(inner_width);
                        ui.spacing_mut().item_spacing.y = 16.0;
                        ui.vertical(|ui| {
                            ui.set_width(inner_width);
                            ui.spacing_mut().item_spacing.y = 8.0;
                            if batch {
                                self.dialog_text(ui, "Batch accounts", 14.0, true, colors.heading);
                                self.dialog_text(ui, BATCH_HELP, 12.0, false, colors.secondary);
                            } else {
                                self.dialog_text(ui, "Fetch source", 14.0, true, colors.heading);
                                ui.spacing_mut().interact_size.y = appearance::CONTROL_HEIGHT;
                                ui.spacing_mut().button_padding = Vec2::new(10.0, 6.0);
                                let old_source = source;
                                let choices = [
                                    self.tr("Public · Profile / URL"),
                                    self.tr("Private · My Likes"),
                                    self.tr("Private · My Bookmarks"),
                                ];
                                let (picker, _) = ui.allocate_exact_size(
                                    Vec2::new(inner_width, appearance::CONTROL_HEIGHT),
                                    egui::Sense::hover(),
                                );
                                let mut picker_ui = child(ui, "fetch-source-picker", picker);
                                if self.busy() {
                                    picker_ui.disable();
                                }
                                source = self.choice_picker(
                                    &mut picker_ui,
                                    picker,
                                    &crate::ui::components::dropdown::SelectOptions::new(
                                        "Fetch source",
                                        &choices,
                                    ),
                                    source,
                                );
                                if first && source > 0 {
                                    ui.memory_mut(|m| {
                                        m.request_focus(
                                            picker_ui.id().with(("select", "Fetch source")),
                                        )
                                    });
                                }
                                if old_source != source {
                                    app.invoke_choose_fetch_source(source as i32);
                                    batch = app.get_batch_mode();
                                    dates = app.get_date_filter_enabled();
                                    start = app.get_fetch_start_date();
                                    end = app.get_fetch_end_date();
                                    sources = app.get_batch_sources_text();
                                }
                                self.dialog_text(
                                    ui,
                                    [SOURCE_PUBLIC, SOURCE_LIKES, SOURCE_BOOKMARKS][source],
                                    12.0,
                                    false,
                                    colors.secondary,
                                );
                                self.dialog_text(ui, SOURCE_HELP, 12.0, false, colors.secondary);
                            }
                            if auth_missing
                                && self.auth_notice(ui, "Add auth_token in Settings to fetch")
                            {
                                app.set_selected_tab(2);
                                app.set_settings_category(1);
                                close = true;
                            }
                        });
                        let divider = |ui: &mut Ui| {
                            let (rect, _) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width(), 1.0),
                                egui::Sense::hover(),
                            );
                            ui.painter().rect_filled(rect, 0, colors.border);
                        };
                        divider(ui);
                        let mut changed = false;
                        ui.vertical(|ui| {
                            ui.set_width(inner_width);
                            ui.spacing_mut().item_spacing.y = 12.0;
                            changed |= self
                                .fetch_switch(
                                    ui,
                                    &mut dates,
                                    "Date range",
                                    DATE_HELP,
                                    batch || source == 0,
                                )
                                .changed();
                            if dates {
                                let field_width = (inner_width - 12.0) / 2.0;
                                ui.horizontal_top(|ui| {
                                    ui.spacing_mut().item_spacing.x = 12.0;
                                    for (id, label, value) in [
                                        ("fetch-start", "Start date", &mut start),
                                        ("fetch-end", "End date", &mut end),
                                    ] {
                                        ui.vertical(|ui| {
                                            ui.set_width(field_width);
                                            ui.spacing_mut().item_spacing.y = 6.0;
                                            self.dialog_text(
                                                ui,
                                                label,
                                                12.0,
                                                false,
                                                colors.secondary,
                                            );
                                            changed |= self
                                                .dialog_field(
                                                    ui,
                                                    id,
                                                    label,
                                                    value,
                                                    "YYYY-MM-DD",
                                                    Vec2::new(
                                                        field_width,
                                                        appearance::CONTROL_HEIGHT,
                                                    ),
                                                    false,
                                                )
                                                .changed();
                                        });
                                    }
                                });
                            }
                        });
                        let mut quick = (batch || source == 0) && app.get_incremental_fetch();
                        if self
                            .fetch_switch(
                                ui,
                                &mut quick,
                                "Quick refresh",
                                if batch {
                                    QUICK_BATCH
                                } else if source == 0 {
                                    QUICK_HELP
                                } else {
                                    QUICK_PRIVATE
                                },
                                batch || source == 0,
                            )
                            .changed()
                        {
                            app.set_incremental_fetch(quick);
                            app.invoke_settings_changed();
                        }
                        if changed {
                            app.invoke_apply_fetch_options(
                                batch,
                                dates,
                                start.clone(),
                                end.clone(),
                                sources.clone(),
                            );
                        }
                        if !app.get_fetch_options_error().is_empty() {
                            self.dialog_text(
                                ui,
                                &app.get_fetch_options_error(),
                                12.0,
                                false,
                                ui.visuals().error_fg_color,
                            );
                        }
                    });
            },
        );
        if close {
            app.set_fetch_options_open(false);
            self.fetch_dialog_opened = false;
        }
    }
}
