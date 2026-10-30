use crate::ui::components::dialog::{child, galley, panel};
use crate::ui::components::icon_button::IconStyle;
use crate::ui::*;

impl Desktop {
    pub(in crate::ui) fn account_search_dialog(&mut self, ctx: &egui::Context) {
        let app = self.app.clone();
        if !app.get_account_search_open() {
            self.account_dialog_opened = false;
            self.account_page_requested = None;
            return;
        }
        let first = !self.account_dialog_opened;
        self.account_dialog_opened = true;
        if first {
            self.account_result_index = 0;
            self.account_scroll_reset = true;
            self.account_page_requested = None;
        }
        let colors = appearance::Palette::new(
            &app.global::<AppTheme>().get_preset(),
            ctx.theme() == egui::Theme::Dark,
        );
        let missing = app.get_saved_auth_token().is_empty();
        let busy = app.get_account_search_busy();
        let items = app.get_account_search_items();
        let mut query = app.get_account_search_query();
        let more = !app.get_account_search_cursor().is_empty()
            && query == app.get_account_search_result_query();
        let error = app.get_account_search_error();
        let width = (appearance::SEARCH_WIDTH + 40.0).min(ctx.content_rect().width() - 48.0);
        let search_width = appearance::SEARCH_WIDTH.min(width - 40.0);
        let error_layout = (!error.is_empty()).then(|| {
            let mut job = egui::text::LayoutJob::simple(
                error.clone(),
                egui::FontId::proportional(12.0),
                ctx.style_of(ctx.theme()).visuals.error_fg_color,
                search_width,
            );
            job.sections[0].format.line_height = Some(18.0);
            job.wrap.max_rows = 4;
            ctx.fonts_mut(|fonts| fonts.layout_job(job))
        });
        let retry = !error.is_empty() && more && items.row_count() > 0 && !missing;
        let extra = error_layout
            .as_ref()
            .map_or(0.0, |text| text.size().y + 12.0)
            + if retry { 44.0 } else { 0.0 };
        let hint = if app.get_account_search_result_query().is_empty() {
            self.tr("Search for an account by name or username.")
        } else {
            self.tr("No accounts match “{}”.")
                .replace("{}", &app.get_account_search_result_query())
        };
        let hint = galley(ctx, hint, search_width, 14.0, false, colors.secondary);
        let columns = if width - 40.0 >= 612.0 { 2 } else { 1 };
        let height = if items.row_count() == 0 {
            180.0 + hint.size().y
        } else {
            154.0 + items.row_count().div_ceil(columns) as f32 * 96.0
        };
        let size = Vec2::new(
            width,
            (height + extra).min(680.0_f32.min(ctx.content_rect().height() - 48.0)),
        );
        let field_id = egui::Id::new("account-search-query");
        let results_id = egui::Id::new("account-search-results");
        let focused = ctx.memory(|m| m.focused());
        let tab = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Tab));
        let backtab = ctx.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab));
        let mut close = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        let mut submit = false;
        let mut load_more = false;
        let mut chosen = None;
        let mut reveal = false;
        if focused == Some(results_id) && items.row_count() > 0 {
            for (key, offset) in [
                (egui::Key::ArrowDown, columns as isize),
                (egui::Key::ArrowUp, -(columns as isize)),
                (egui::Key::ArrowRight, 1),
                (egui::Key::ArrowLeft, -1),
            ] {
                if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key)) {
                    let next = self.account_result_index as isize + offset;
                    if next >= 0 && next < items.row_count() as isize {
                        self.account_result_index = next as usize;
                        reveal = true;
                    }
                }
            }
            if ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                    || i.consume_key(egui::Modifiers::NONE, egui::Key::Space)
            }) {
                chosen = items
                    .row_data(self.account_result_index)
                    .map(|item| item.handle);
            }
        }
        panel(
            ctx,
            "home-account-search",
            "Account search dialog",
            size,
            colors,
            |ui, inner| {
                let keyboard = ui.interact(
                    inner,
                    egui::Id::new("account-dialog-keyboard"),
                    egui::Sense::focusable_noninteractive(),
                );
                keyboard.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Other,
                        true,
                        "Account search keyboard",
                    )
                });
                let header = egui::Rect::from_min_size(inner.min, Vec2::new(inner.width(), 32.0));
                self.dialog_heading(ui, header, "Search accounts");
                let close_response = self.dialog_icon(
                    ui,
                    egui::Rect::from_min_size(
                        header.right_top() - Vec2::new(32.0, 0.0),
                        Vec2::splat(32.0),
                    ),
                    Icon::Close,
                    "Close account search",
                );
                close |= close_response.clicked();
                let input_rect = egui::Rect::from_center_size(
                    egui::pos2(inner.center().x, inner.top() + 64.0),
                    Vec2::new(search_width, appearance::SEARCH_HEIGHT),
                );
                ui.painter().rect_filled(input_rect, 20, colors.surface);
                let border = ui.painter().add(egui::Shape::Noop);
                let field_rect = egui::Rect::from_min_max(
                    input_rect.min + Vec2::new(26.0, 5.0),
                    input_rect.max - Vec2::new(40.0, 5.0),
                );
                let mut field = child(ui, "search-field", field_rect);
                if missing || busy {
                    field.disable();
                }
                let mut response = field.add(
                    egui::TextEdit::singleline(&mut query)
                        .id(field_id)
                        .frame(egui::Frame::NONE)
                        .margin(egui::Margin::ZERO)
                        .min_size(field_rect.size())
                        .desired_width(field_rect.width())
                        .vertical_align(egui::Align::Center)
                        .hint_text(self.tr("Name or username")),
                );
                if self.text_context_menu(&response, &mut query, false) {
                    response.mark_changed();
                }
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::TextEdit,
                        !missing && !busy,
                        self.tr("Search accounts"),
                    )
                });
                if response.changed() {
                    app.set_account_search_query(query.clone());
                    self.account_page_requested = None;
                }
                if first {
                    ctx.memory_mut(|m| {
                        m.request_focus(if missing {
                            egui::Id::new("account-dialog-keyboard")
                        } else {
                            field_id
                        })
                    });
                }
                submit |= !missing
                    && !busy
                    && response.lost_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.painter().set(
                    border,
                    egui::Shape::rect_stroke(
                        input_rect,
                        20,
                        egui::Stroke::new(
                            1.0,
                            if response.has_focus() {
                                colors.accent
                            } else if missing || busy {
                                colors.input_border.gamma_multiply(0.5)
                            } else {
                                colors.input_border
                            },
                        ),
                        egui::StrokeKind::Inside,
                    ),
                );
                let search_rect = egui::Rect::from_min_size(
                    input_rect.right_top() + Vec2::new(-38.0, 4.0),
                    Vec2::splat(32.0),
                );
                let mut search_ui = child(ui, "search-button", search_rect);
                if missing || busy || query.is_empty() {
                    search_ui.disable();
                }
                let search_response = self.search_icon(
                    &mut search_ui,
                    Icon::Search,
                    if busy {
                        "Searching…"
                    } else {
                        "Search accounts"
                    },
                    32.0,
                    IconStyle {
                        idle: colors.control,
                        hover: colors.muted_control,
                        pressed: colors.muted_control,
                        foreground: colors.body,
                        border: egui::Stroke::NONE,
                        radius: 16,
                        icon_size: 20.0,
                        visible: true,
                    },
                );
                submit |= search_response.clicked();
                if busy {
                    ui.painter()
                        .circle_filled(search_rect.center(), 16.0, colors.control);
                    ui.put(
                        search_rect.shrink(7.0),
                        egui::Spinner::new().size(18.0).color(colors.heading),
                    );
                }
                let mut notices = child(
                    ui,
                    "account-search-notices",
                    egui::Rect::from_min_max(
                        input_rect.left_top() + Vec2::new(0.0, 52.0),
                        egui::pos2(input_rect.right(), inner.bottom()),
                    ),
                );
                notices.spacing_mut().item_spacing.y = 12.0;
                if missing {
                    if self.auth_notice(&mut notices, "Add auth_token in Settings to search") {
                        app.set_selected_tab(2);
                        app.set_settings_category(1);
                        close = true;
                    }
                } else {
                    self.dialog_text(
                        &mut notices,
                        "Click an account to fill its username in Home.",
                        12.0,
                        false,
                        colors.secondary,
                    );
                }
                let mut retry_response = None;
                if let Some(text) = &error_layout {
                    let color = ui.visuals().error_fg_color;
                    let (rect, _) = notices.allocate_exact_size(text.size(), egui::Sense::hover());
                    notices.painter().galley(rect.min, text.clone(), color);
                    if retry {
                        notices.add_enabled_ui(!busy && !self.busy(), |ui| {
                            let response = self.home_action(
                                ui,
                                &self.tr("Try again"),
                                None,
                                false,
                                Vec2::new(100.0, 32.0),
                                6,
                            );
                            load_more |= response.clicked();
                            retry_response = Some(response.id);
                        });
                    }
                }
                let result_top = notices.min_rect().bottom() + 12.0;
                let results_rect = egui::Rect::from_min_max(
                    egui::pos2(inner.left(), result_top),
                    egui::pos2(
                        inner.right(),
                        (inner.bottom()
                            - if busy && items.row_count() > 0 {
                                28.0
                            } else {
                                0.0
                            })
                        .max(result_top),
                    ),
                );
                let group = ui.interact(
                    results_rect,
                    results_id,
                    egui::Sense::focusable_noninteractive(),
                );
                group.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Account results")
                });
                let mut results_ui = child(ui, "account-result-grid", results_rect);
                let scroll_down = ui.input(|input| {
                    input
                        .pointer
                        .hover_pos()
                        .is_some_and(|pos| results_rect.contains(pos))
                        && input.events.iter().any(|event| {
                            matches!(event,
                                egui::Event::MouseWheel { delta, modifiers, .. }
                                    if delta.y < 0.0 && !modifiers.ctrl && !modifiers.command
                            )
                        })
                });
                let scroll = egui::ScrollArea::vertical()
                    .id_salt("account-result-scroll")
                    .scroll_bar_rect(results_rect.shrink2(Vec2::new(0.0, 16.0)))
                    .max_height(results_rect.height())
                    .auto_shrink([false, false]);
                let scroll = if self.account_scroll_reset {
                    scroll.vertical_scroll_offset(0.0)
                } else {
                    scroll
                };
                self.account_scroll_reset = false;
                let scroll = scroll.show_themed(&mut results_ui, |ui| {
                    let width = ui.available_width();
                    let columns = if width >= 612.0 { 2 } else { 1 };
                    let card_width = (width - 12.0 * (columns - 1) as f32) / columns as f32;
                    let rows = items.row_count().div_ceil(columns);
                    let (grid, _) = ui.allocate_exact_size(
                        Vec2::new(width, (rows as f32 * 96.0 - 12.0).max(0.0)),
                        egui::Sense::hover(),
                    );
                    let card_rect = |index: usize| {
                        egui::Rect::from_min_size(
                            grid.min
                                + Vec2::new(
                                    (index % columns) as f32 * (card_width + 12.0),
                                    (index / columns) as f32 * 96.0,
                                ),
                            Vec2::new(card_width, 84.0),
                        )
                    };
                    if reveal && self.account_result_index < items.row_count() {
                        ui.scroll_to_rect(card_rect(self.account_result_index), None);
                    }
                    let first = (((ui.clip_rect().top() - grid.top()) / 96.0)
                        .floor()
                        .max(0.0) as usize)
                        .saturating_sub(1)
                        .min(rows);
                    let last = (((ui.clip_rect().bottom() - grid.top()) / 96.0)
                        .ceil()
                        .max(0.0) as usize
                        + 1)
                    .min(rows);
                    for index in first * columns..(last * columns).min(items.row_count()) {
                        let item = items.row_data(index).unwrap();
                        let rect = card_rect(index);
                        let mut card = child(ui, (&item.handle, index), rect);
                        let response = card
                            .interact(rect, card.id().with("choose"), egui::Sense::click())
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        response.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                true,
                                format!("{} · @{}", item.name, item.handle),
                            )
                        });
                        let selected = group.has_focus() && index == self.account_result_index;
                        let highlight = selected || response.hovered() || response.has_focus();
                        card.painter().rect_filled(
                            rect,
                            12,
                            if response.hovered() {
                                colors.accent_soft
                            } else {
                                colors.surface
                            },
                        );
                        card.painter().rect_stroke(
                            rect,
                            12,
                            egui::Stroke::new(
                                1.0,
                                if highlight {
                                    colors.accent
                                } else {
                                    colors.border
                                },
                            ),
                            egui::StrokeKind::Inside,
                        );
                        let avatar = egui::Rect::from_min_size(
                            rect.min + Vec2::new(12.0, 18.0),
                            Vec2::splat(48.0),
                        );
                        if item.avatar.size().width > 0 {
                            self.paint_media(&mut card, &item.avatar, avatar, 24);
                        } else {
                            Icon::User.image(colors.secondary, 20.0).paint_at(
                                &card,
                                egui::Rect::from_center_size(avatar.center(), Vec2::splat(20.0)),
                            );
                        }
                        let left = rect.left() + 72.0;
                        let text_width = rect.width() - 84.0;
                        let top = rect.center().y - if item.bio.is_empty() { 20.5 } else { 31.0 };
                        let text = |text: String,
                                    y: f32,
                                    size: f32,
                                    bold: bool,
                                    color: Color32,
                                    width: f32| {
                            let font = egui::FontId::new(
                                size,
                                if bold {
                                    egui::FontFamily::Name("cera-bold".into())
                                } else {
                                    egui::FontFamily::Proportional
                                },
                            );
                            let mut job = egui::text::LayoutJob::simple(text, font, color, width);
                            job.wrap.max_rows = 1;
                            job.wrap.break_anywhere = true;
                            let g = card.ctx().fonts_mut(|f| f.layout_job(job));
                            let result = g.size().x;
                            card.painter().galley(egui::pos2(left, y), g, color);
                            result
                        };
                        let name_width = text(
                            item.name.clone(),
                            top,
                            14.0,
                            true,
                            colors.heading,
                            text_width - if item.verified { 19.0 } else { 0.0 },
                        );
                        if item.verified {
                            Icon::Verified.image(colors.accent, 14.0).paint_at(
                                &card,
                                egui::Rect::from_min_size(
                                    egui::pos2(left + name_width + 5.0, top + 2.0),
                                    Vec2::splat(14.0),
                                ),
                            );
                        }
                        let mut metadata = format!("@{}", item.handle);
                        if !item.followers.is_empty() {
                            metadata.push_str(&format!(
                                " · {}",
                                self.tr("{} followers").replace("{}", &item.followers)
                            ));
                        }
                        if item.protected {
                            metadata.push_str(&format!(" · {}", self.tr("Protected")));
                        }
                        text(
                            metadata,
                            top + 23.0,
                            12.0,
                            false,
                            colors.secondary,
                            text_width,
                        );
                        if !item.bio.is_empty() {
                            text(
                                item.bio.clone(),
                                top + 44.0,
                                12.0,
                                false,
                                colors.secondary,
                                text_width,
                            );
                        }
                        if response.clicked() {
                            self.account_result_index = index;
                            group.request_focus();
                            chosen = Some(item.handle);
                        }
                    }
                });
                if items.row_count() == 0 && !busy && !missing && error.is_empty() {
                    ui.painter().galley(
                        results_rect.center() - hint.size() / 2.0,
                        hint.clone(),
                        colors.secondary,
                    );
                }
                if items.row_count() > 0 && busy {
                    ui.put(
                        egui::Rect::from_center_size(
                            egui::pos2(inner.center().x, inner.bottom() - 12.0),
                            Vec2::splat(20.0),
                        ),
                        egui::Spinner::new().size(18.0).color(colors.secondary),
                    );
                }
                let page = (query.clone(), app.get_account_search_cursor());
                if items.row_count() > 0
                    && more
                    && !busy
                    && !missing
                    && !self.busy()
                    && !app.get_auth_check_busy()
                    && error.is_empty()
                    && (scroll.state.offset.y > 1.0 || scroll_down)
                    && scroll.state.offset.y + scroll.inner_rect.height() + 120.0
                        >= scroll.content_size.y
                    && self.account_page_requested.as_ref() != Some(&page)
                {
                    self.account_page_requested = Some(page);
                    load_more = true;
                }
                if tab || backtab {
                    let mut order = Vec::new();
                    if !missing {
                        if !busy {
                            order.push(field_id);
                            if !query.is_empty() {
                                order.push(search_response.id);
                            }
                        }
                        if items.row_count() > 0 {
                            order.push(results_id);
                        }
                        if let Some(retry) = retry_response {
                            order.push(retry);
                        }
                    }
                    order.push(close_response.id);
                    let index =
                        focused.and_then(|id| order.iter().position(|candidate| *candidate == id));
                    let next = match index {
                        Some(i) if backtab => (i + order.len() - 1) % order.len(),
                        Some(i) => (i + 1) % order.len(),
                        None => 0,
                    };
                    ctx.memory_mut(|m| m.request_focus(order[next]));
                }
                ctx.memory_mut(|memory| {
                    if let Some(id) = memory.focused() {
                        memory.set_focus_lock_filter(
                            id,
                            egui::EventFilter {
                                tab: true,
                                horizontal_arrows: id == field_id || id == results_id,
                                vertical_arrows: id == field_id || id == results_id,
                                escape: true,
                            },
                        );
                    }
                });
            },
        );
        if submit && !missing && !busy && !close && chosen.is_none() {
            self.account_result_index = 0;
            self.account_scroll_reset = true;
            self.account_page_requested = None;
            app.invoke_search_accounts(query, false);
        } else if load_more && !missing && !busy && !close && chosen.is_none() {
            self.account_page_requested = Some((query.clone(), app.get_account_search_cursor()));
            app.invoke_search_accounts(query, true);
        }
        if let Some(handle) = chosen {
            app.set_source_url(handle);
            app.set_selected_tab(0);
            app.set_batch_mode(false);
            close = true;
        }
        if close {
            app.invoke_close_account_search();
            self.account_dialog_opened = false;
        }
    }
}
