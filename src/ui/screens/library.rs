use crate::ui::components::dialog::child;
use crate::ui::*;

#[derive(Default)]
pub(in crate::ui) struct LibrarySelection {
    pub total: usize,
    pub saved: usize,
    /// Saved GIF MP4 files that can still be converted.
    pub gifs: usize,
}

impl Desktop {
    pub(in crate::ui) fn library_view(&mut self, ui: &mut Ui) {
        let bounds = ui.max_rect();
        ui.allocate_rect(bounds, egui::Sense::hover());
        // The file list stays at the reading width; account and media grids use the window.
        let data = self.app.get_library();
        let width = if data.view_ready && data.files_view {
            bounds.width().min(appearance::CARD_PAGE_WIDTH)
        } else {
            appearance::card_page_width(bounds.width())
        };
        let page =
            egui::Rect::from_center_size(bounds.center(), Vec2::new(width, bounds.height()))
                .shrink(12.0);
        let mut content = child(ui, "library-page", page);
        if self.app.get_library().view_ready {
            if self.app.get_library().workspace_pending {
                content.vertical_centered(|ui| {
                    ui.add_space(64.0);
                    ui.spinner();
                    ui.label(self.tr("Loading library…"));
                });
                return;
            }
            if self.app.get_library().files_view {
                self.download_details(&mut content);
            } else {
                self.library_account_view(&mut content);
            }
            return;
        }
        let app = self.app.clone();
        let data = app.get_library();
        let header = egui::Rect::from_min_size(page.min, Vec2::new(page.width(), 44.0));
        self.library_header(&mut content, header);
        let filters = egui::Rect::from_min_size(
            page.min + Vec2::new(0.0, 60.0),
            Vec2::new(page.width(), 42.0),
        );
        self.history_text(
            &content,
            filters,
            format!("{} ({})", self.tr("Accounts"), data.items.row_count()),
            14.0,
            true,
            self.colors(ui).secondary,
        );
        let search = crate::ui::components::account_search::search_rect(filters);
        let picker = egui::Rect::from_min_size(
            egui::pos2(search.left() - 162.0, search.top()),
            Vec2::new(146.0, appearance::CONTROL_HEIGHT),
        );
        let mut groups = vec![self.tr("All groups")];
        groups.extend(data.groups.iter().cloned());
        let index = data
            .groups
            .iter()
            .position(|group| group == &data.group_filter)
            .map_or(0, |index| index + 1);
        let next = self.choice_picker(
            &mut child(&mut content, "library-group-picker", picker),
            picker,
            &crate::ui::components::dropdown::SelectOptions::new("Filter group", &groups),
            index,
        );
        if next != index {
            app.update_library(|data| {
                data.group_filter = if next == 0 {
                    String::new()
                } else {
                    groups[next].clone()
                }
            });
        }
        let data = app.get_library();
        let mut query = data.query.clone();
        if self.account_search_input(
            &mut content,
            search,
            "library-query",
            "Search library",
            &mut query,
        ) {
            app.update_library(|data| data.query = query.clone());
        }
        let query = query.to_lowercase();
        let mut rows = Vec::new();
        data.items.visit(&mut |index, row| {
            if (data.group_filter.is_empty() || row.group_label == data.group_filter)
                && (query.is_empty()
                    || format!("{} {} {}", row.title, row.handle, row.account_labels)
                        .to_lowercase()
                        .contains(&query))
            {
                rows.push(index);
            }
            true
        });
        let mut list_top = 118.0;
        if !data.job.is_empty() && app.get_is_busy() {
            let mut progress = child(
                &mut content,
                "library-progress",
                egui::Rect::from_min_size(
                    page.min + Vec2::new(0.0, list_top),
                    Vec2::new(page.width(), 64.0),
                ),
            );
            if self.operation_status_row(&mut progress, &data.status, "Stop") {
                app.invoke_cancel_operation();
            }
            self.progress_bar(&mut progress, data.progress);
            list_top += 80.0;
        }
        if !data.error.is_empty() {
            let error = crate::ui::components::dialog::galley(
                ui.ctx(),
                data.error.clone(),
                page.width(),
                typography::BODY,
                false,
                self.colors(ui).danger,
            );
            let height = error.size().y + 16.0;
            let mut message = child(
                &mut content,
                "library-error",
                egui::Rect::from_min_size(
                    page.min + Vec2::new(0.0, list_top),
                    Vec2::new(page.width(), height),
                ),
            );
            self.error(&mut message, &data.error);
            list_top += height;
        }
        if data.select_mode && app.get_saved_auth_token().is_empty() {
            let notice = egui::Rect::from_min_size(
                page.min + Vec2::new(0.0, list_top),
                Vec2::new(page.width(), 24.0),
            );
            if self.auth_notice(
                &mut child(&mut content, "library-auth", notice),
                "Add auth_token in Settings to update accounts",
            ) {
                app.set_selected_tab(2);
                app.set_settings_category(1);
            }
            list_top += 34.0;
        }
        let list = egui::Rect::from_min_max(page.min + Vec2::new(0.0, list_top), page.max);
        let mut list_ui = child(&mut content, "library-accounts", list);
        if rows.is_empty() {
            list_ui.vertical_centered(|ui| {
                ui.add_space(64.0);
                if data.loading || app.get_history_busy() {
                    ui.spinner();
                }
                ui.label(self.tr(if data.loading || app.get_history_busy() {
                    "Loading library…"
                } else if data.items.row_count() == 0 {
                    "Downloaded accounts will appear here."
                } else {
                    "No accounts match this search."
                }));
            });
        }
        egui::ScrollArea::vertical()
            .id_salt("library-accounts")
            .auto_shrink([false, false])
            .show_themed(&mut list_ui, |ui| {
                let width = ui.available_width();
                let columns = appearance::account_card_columns(width);
                let card_width =
                    (width - (columns - 1) as f32 * appearance::ACCOUNT_CARD_GAP) / columns as f32;
                let count = rows.len().div_ceil(columns);
                let pitch = appearance::ACCOUNT_CARD_HEIGHT + appearance::ACCOUNT_CARD_ROW_GAP;
                let (grid, _) = ui.allocate_exact_size(
                    Vec2::new(
                        width,
                        (count as f32 * pitch - appearance::ACCOUNT_CARD_ROW_GAP).max(0.0),
                    ),
                    egui::Sense::hover(),
                );
                let first = (((ui.clip_rect().top() - grid.top()) / pitch)
                    .floor()
                    .max(0.0) as usize)
                    .saturating_sub(1)
                    .min(count);
                let last = ((((ui.clip_rect().bottom() - grid.top()) / pitch)
                    .ceil()
                    .max(0.0) as usize)
                    + 1)
                .min(count);
                for row in first..last {
                    let rect = egui::Rect::from_min_size(
                        grid.min + Vec2::new(0.0, row as f32 * pitch),
                        Vec2::new(width, appearance::ACCOUNT_CARD_HEIGHT),
                    );
                    let mut line = child(ui, ("library-row", row), rect);
                    line.spacing_mut().item_spacing.x = 12.0;
                    line.horizontal(|ui| {
                        for &index in &rows[row * columns..((row + 1) * columns).min(rows.len())] {
                            self.library_account_card(
                                ui,
                                &data.items.row_data(index).unwrap(),
                                card_width,
                            );
                        }
                    });
                }
            });
    }

    fn library_header(&mut self, ui: &mut Ui, rect: egui::Rect) {
        let app = self.app.clone();
        let data = app.get_library();
        let enabled = !self.busy() && !app.get_history_busy();
        let mut right = rect.right();
        let mut slot = |width: f32| {
            right -= width;
            let area = egui::Rect::from_min_size(
                egui::pos2(right, rect.center().y - 16.0),
                Vec2::new(width, 32.0),
            );
            right -= 8.0;
            area
        };
        if self
            .history_icon(
                ui,
                slot(32.0),
                (Icon::Reset, 16.0),
                "Refresh library",
                enabled,
                false,
            )
            .clicked()
        {
            app.invoke_library_refresh();
        }
        if data.select_mode && !data.selection.is_empty() {
            let label = self
                .tr("Update selected ({})")
                .replace("{}", &data.selection.len().to_string());
            let width = ui
                .painter()
                .layout_no_wrap(
                    label.clone(),
                    egui::FontId::proportional(typography::BODY),
                    Color32::WHITE,
                )
                .size()
                .x
                + 24.0;
            if self
                .history_button(
                    ui,
                    slot(width),
                    &label,
                    None,
                    true,
                    enabled && !app.get_saved_auth_token().is_empty(),
                )
                .clicked()
            {
                app.invoke_library_refetch_selected();
            }
            if self
                .history_icon(
                    ui,
                    slot(32.0),
                    (Icon::Tags, 16.0),
                    "Assign group and tags",
                    enabled,
                    false,
                )
                .clicked()
            {
                app.set_account_labels_error(String::new());
                app.set_account_labels_open(true);
            }
        }
        if data.select_mode {
            let label = self.tr("Select visible");
            let width = ui
                .painter()
                .layout_no_wrap(
                    label.clone(),
                    egui::FontId::proportional(typography::BODY),
                    Color32::WHITE,
                )
                .size()
                .x
                + 24.0;
            if self
                .history_button(ui, slot(width), &label, None, false, enabled)
                .clicked()
            {
                let query = data.query.to_lowercase();
                app.update_library(|data| {
                    for row in data.items.iter() {
                        if (data.group_filter.is_empty() || data.group_filter == row.group_label)
                            && (query.is_empty()
                                || format!("{} {} {}", row.title, row.handle, row.account_labels)
                                    .to_lowercase()
                                    .contains(&query))
                            && !data.selection.contains(&row.id)
                        {
                            data.selection.push(row.id);
                        }
                    }
                });
            }
        }
        let label = self.tr(if data.select_mode {
            "Cancel selection"
        } else {
            "Select accounts"
        });
        let width = ui
            .painter()
            .layout_no_wrap(
                label.clone(),
                egui::FontId::proportional(typography::BODY),
                Color32::WHITE,
            )
            .size()
            .x
            + 24.0;
        if self
            .history_button(
                ui,
                slot(width),
                &label,
                None,
                false,
                enabled && data.items.row_count() > 0,
            )
            .clicked()
        {
            app.update_library(|data| {
                data.select_mode = !data.select_mode;
                data.selection.clear();
            });
        }
        self.history_title(
            ui,
            egui::Rect::from_min_max(rect.min, egui::pos2(right, rect.bottom())),
            self.tr("Library"),
        );
    }

    fn library_account_card(&mut self, ui: &mut Ui, row: &crate::HistoryItem, width: f32) {
        let app = self.app.clone();
        let colors = self.colors(ui);
        let data = app.get_library();
        let selected = data.select_mode && data.selection.contains(&row.id);
        let (rect, _) = ui.allocate_exact_size(
            Vec2::new(width, appearance::ACCOUNT_CARD_HEIGHT),
            egui::Sense::hover(),
        );
        let mut card = child(ui, &row.id, rect);
        let response = card
            .interact(
                rect,
                card.id().with("open"),
                if self.busy() {
                    egui::Sense::hover()
                } else {
                    egui::Sense::click()
                },
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::Button,
                !self.busy(),
                selected,
                format!("{} · {}", self.tr("Open library account"), row.title),
            )
        });
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
                if selected || response.hovered() {
                    colors.accent
                } else {
                    colors.border
                },
            ),
            egui::StrokeKind::Inside,
        );
        if crate::ui::components::navigation::focus_visible(&card, &response) {
            card.painter().rect_stroke(
                rect,
                12,
                egui::Stroke::new(2.0, colors.accent),
                egui::StrokeKind::Inside,
            );
        }
        let avatar = egui::Rect::from_center_size(
            egui::pos2(rect.left() + 36.0, rect.center().y),
            Vec2::splat(48.0),
        );
        if row.has_avatar {
            self.paint_media(&mut card, &row.avatar_image, avatar, 24);
        } else {
            Icon::User.image(colors.secondary, 20.0).paint_at(
                &card,
                egui::Rect::from_center_size(avatar.center(), Vec2::splat(20.0)),
            );
        }
        if data.select_mode {
            let square = egui::Rect::from_center_size(avatar.center(), Vec2::splat(20.0));
            card.painter()
                .rect_filled(square.expand(2.0), 5, colors.surface);
            card.painter().rect_filled(
                square,
                4,
                if selected {
                    colors.accent
                } else {
                    colors.surface
                },
            );
            if selected {
                Icon::Check
                    .image(colors.page, 14.0)
                    .paint_at(&card, square.shrink(3.0));
            } else {
                card.painter().rect_stroke(
                    square,
                    4,
                    egui::Stroke::new(1.0, colors.secondary),
                    egui::StrokeKind::Inside,
                );
            }
        }
        let job = data.queue.iter().find(|job| job.key == row.id);
        let detail = if let Some(job) = job {
            format!(
                "{} · {}",
                self.tr(&job.status),
                self.tr("{} media")
                    .replace("{}", &job.media_count.to_string())
            )
        } else {
            format!(
                "{} · {} · {}",
                self.tr("{} available")
                    .replace("{}", &row.saved_count.to_string()),
                self.tr("{} media")
                    .replace("{}", &row.media_count.to_string()),
                row.size_text
            )
        };
        let mut lines = vec![row.title.clone(), format!("@{}", row.handle), detail];
        if !row.account_labels.is_empty() {
            lines.push(row.account_labels.clone());
        }
        let total = 20.0 + (lines.len() - 1) as f32 * 20.0;
        let mut y = rect.center().y - total / 2.0;
        for (index, text) in lines.into_iter().enumerate() {
            let height = if index == 0 { 20.0 } else { 17.0 };
            self.history_text(
                &card,
                egui::Rect::from_min_size(
                    egui::pos2(rect.left() + 72.0, y),
                    Vec2::new(rect.width() - 166.0, height),
                ),
                text,
                if index == 0 { 14.0 } else { 12.0 },
                index == 0,
                if index == 0 {
                    colors.heading
                } else if index == 3 && row.account_color.a() > 0 {
                    appearance::mix(
                        row.account_color,
                        if ui.visuals().dark_mode {
                            Color32::WHITE
                        } else {
                            Color32::BLACK
                        },
                        0.45,
                    )
                } else if job.is_some_and(|job| job.status == "Failed") && index == 2 {
                    colors.danger
                } else {
                    colors.secondary
                },
            );
            y += height + 3.0;
        }
        if let Some(job) = job
            && !job.error.is_empty()
        {
            response.clone().on_hover_text(&job.error);
        }
        let update = egui::Rect::from_center_size(
            egui::pos2(rect.right() - 62.0, rect.center().y),
            Vec2::splat(32.0),
        );
        let folder = egui::Rect::from_center_size(
            egui::pos2(rect.right() - 24.0, rect.center().y),
            Vec2::splat(32.0),
        );
        let update_response = self.history_icon(
            &mut card,
            update,
            (Icon::Restart, 16.0),
            &format!("{} · {}", self.tr("Refetch"), row.title),
            !self.busy(),
            false,
        );
        let folder_response = self.history_icon(
            &mut card,
            folder,
            (Icon::Folder, 16.0),
            &format!("{} · {}", self.tr("Open folder"), row.title),
            !row.output_dir.is_empty(),
            false,
        );
        if update_response.clicked() {
            app.invoke_library_refetch(row.id.clone());
        }
        if folder_response.clicked() {
            app.invoke_open_download_path(row.output_dir.clone(), true);
        }
        if response.clicked() && !update_response.clicked() && !folder_response.clicked() {
            if data.select_mode {
                app.update_library(|data| {
                    if data.selection.contains(&row.id) {
                        data.selection.retain(|key| key != &row.id);
                    } else {
                        data.selection.push(row.id.clone());
                    }
                });
            } else {
                app.invoke_library_open(row.id.clone());
            }
        }
    }

    fn library_account_view(&mut self, ui: &mut Ui) {
        let app = self.app.clone();
        let data = app.get_library();
        if self.library_account_seen != data.account {
            self.library_account_seen.clone_from(&data.account);
            self.kind = "all".into();
            self.limit = 30;
        }
        let bounds = ui.max_rect();
        let header = egui::Rect::from_min_size(bounds.min, Vec2::new(bounds.width(), 44.0));
        if self
            .history_icon(
                ui,
                egui::Rect::from_min_size(header.min, Vec2::splat(40.0)),
                (Icon::Back, 18.0),
                "Back to library",
                true,
                false,
            )
            .clicked()
        {
            app.set_selected_media_index(-1);
            app.update_library(|data| {
                data.account.clear();
                data.view_ready = false;
            });
            return;
        }
        self.history_title(
            ui,
            egui::Rect::from_min_max(header.min + Vec2::new(52.0, 0.0), header.max),
            format!("{} · @{}", data.title, data.account),
        );
        let body = egui::Rect::from_min_max(bounds.min + Vec2::new(0.0, 60.0), bounds.max);
        let mut content = child(ui, "library-media", body);
        egui::ScrollArea::vertical()
            .id_salt(("library-media", &data.account, &self.kind))
            .auto_shrink([false, false])
            .show_themed(&mut content, |ui| {
                ui.spacing_mut().item_spacing.y = 14.0;
                ui.horizontal_wrapped(|ui| {
                    if self
                        .setting_action(
                            ui,
                            "Refetch",
                            Some(Icon::Restart),
                            0,
                            Vec2::new(116.0, 32.0),
                            !self.busy(),
                        )
                        .clicked()
                    {
                        app.invoke_library_refetch(data.account.clone());
                    }
                    let selected = crate::media::results::downloadable_selection(&app);
                    let download = if selected > 0 {
                        self.tr("Download selected ({})")
                            .replace("{}", &selected.to_string())
                    } else {
                        "Download pending".into()
                    };
                    if self
                        .setting_action(
                            ui,
                            &download,
                            Some(Icon::Download),
                            1,
                            Vec2::new(if selected > 0 { 200.0 } else { 160.0 }, 32.0),
                            !self.busy() && (selected > 0 || data.pending + data.missing > 0),
                        )
                        .clicked()
                    {
                        app.invoke_library_download(if selected > 0 {
                            crate::library::SELECTED
                        } else {
                            -1
                        });
                    }
                    if self
                        .setting_action(
                            ui,
                            "Open folder",
                            Some(Icon::Folder),
                            0,
                            Vec2::new(116.0, 32.0),
                            !data.folder.is_empty(),
                        )
                        .clicked()
                    {
                        app.invoke_open_download_path(data.folder.clone(), true);
                    }
                    if self
                        .setting_action(
                            ui,
                            "Check files",
                            Some(Icon::Verified),
                            0,
                            Vec2::new(116.0, 32.0),
                            !self.busy(),
                        )
                        .clicked()
                    {
                        app.invoke_library_refresh();
                    }
                    let chosen = self.library_selection();
                    let convert = if chosen.gifs > 0 {
                        self.tr("Convert selected GIFs ({})")
                            .replace("{}", &chosen.gifs.to_string())
                    } else {
                        "Convert GIFs".into()
                    };
                    if app.get_archive_gif_count() > 0
                        && self
                            .setting_action(
                                ui,
                                &convert,
                                Some(Icon::Film),
                                0,
                                Vec2::new(if chosen.gifs > 0 { 210.0 } else { 132.0 }, 32.0),
                                !self.busy(),
                            )
                            .clicked()
                    {
                        app.set_archive_conversion_open(true);
                    }
                    if self
                        .setting_action(
                            ui,
                            "File list",
                            Some(Icon::List),
                            0,
                            Vec2::new(100.0, 32.0),
                            true,
                        )
                        .clicked()
                    {
                        app.update_library(|data| data.files_view = true);
                    }
                });
                let chosen = self.library_selection();
                if chosen.total > 0 {
                    ui.horizontal_wrapped(|ui| {
                        let colors = self.colors(ui);
                        ui.label(
                            RichText::new(
                                self.tr("{} selected").replace("{}", &chosen.total.to_string()),
                            )
                            .font(egui::FontId::new(
                                typography::BODY,
                                egui::FontFamily::Name("cera-bold".into()),
                            ))
                            .color(colors.secondary),
                        );
                        let idle = !self.busy();
                        for (label, icon, tone, width, enabled) in [
                            ("Download again", Icon::Restart, 0, 150.0, idle && chosen.saved > 0),
                            ("Copy to folder…", Icon::Copy, 0, 150.0, idle && chosen.saved > 0),
                            ("Delete files", Icon::Trash, 2, 130.0, idle && chosen.saved > 0),
                            ("Clear selection", Icon::Close, 0, 150.0, true),
                        ] {
                            if self
                                .setting_action(
                                    ui,
                                    label,
                                    Some(icon),
                                    tone,
                                    Vec2::new(width, 32.0),
                                    enabled,
                                )
                                .clicked()
                            {
                                match label {
                                    "Download again" => {
                                        app.invoke_library_download(crate::library::REDOWNLOAD)
                                    }
                                    "Copy to folder…" => app.invoke_library_copy_selected(),
                                    "Delete files" => self.library_delete_confirmation = true,
                                    _ => app.global::<MediaSelection>().invoke_clear(),
                                }
                            }
                        }
                    });
                }
                if !data.job.is_empty() && app.get_is_busy() {
                    if self.operation_status_row(ui, &data.status, "Stop") {
                        app.invoke_cancel_operation();
                    }
                    self.progress_bar(ui, data.progress);
                    self.setting_caption(ui, &data.progress_detail);
                }
                self.error(ui, &data.error);
                ui.horizontal_wrapped(|ui| {
                    for (key, label, count) in [
                        ("all", "All", app.get_media_items().row_count() as i32),
                        ("library-saved", "Available", data.available),
                        ("library-missing", "File missing", data.missing),
                        (
                            "library-pending",
                            "Not downloaded",
                            data.missing + data.pending,
                        ),
                    ] {
                        if self
                            .home_action(
                                ui,
                                &format!("{}  {count}", self.tr(label)),
                                None,
                                self.kind == key,
                                Vec2::new(0.0, 32.0),
                                16,
                            )
                            .clicked()
                        {
                            self.kind = key.into();
                            self.limit = 30;
                        }
                    }
                    let selection = app.global::<MediaSelection>();
                    let items = app.get_media_items();
                    // Saved files cannot be selected, so only unsaved results count.
                    let selectable = self.kind != "library-saved"
                        && app.global::<MediaResults>().invoke_count(
                            items.clone(),
                            if self.kind == "library-missing" {
                                "library-missing"
                            } else {
                                "library-pending"
                            }
                            .into(),
                        ) > 0;
                    let label = self.tr(
                        if selection.invoke_all_selected(
                            selection.get_indices(),
                            items,
                            self.kind.clone(),
                        ) {
                            "Deselect filter"
                        } else {
                            "Select filter"
                        },
                    );
                    ui.add_enabled_ui(!self.busy() && selectable, |ui| {
                        if self
                            .home_action(ui, &label, None, false, Vec2::new(0.0, 32.0), 16)
                            .clicked()
                        {
                            selection.invoke_toggle_filter(self.kind.clone());
                        }
                    });
                });
                let all = app.get_media_items();
                let rows = self.result_view.get(
                    all.clone(),
                    &self.kind,
                    self.limit as i32,
                    app.get_results_sort(),
                );
                if rows.row_count() == 0 {
                    ui.label(self.tr("Nothing in this filter."));
                } else {
                    self.virtual_media_rows(ui, &rows);
                }
                let total = app
                    .global::<MediaResults>()
                    .invoke_count(all, self.kind.clone())
                    .max(0) as usize;
                if rows.row_count() < total && ui.cursor().top() <= ui.clip_rect().bottom() + 200.0
                {
                    self.limit = (self.limit + 30).min(total);
                    ui.ctx().request_repaint();
                }
            });
    }

    /// Counts what the selection in an open library account can act on.
    pub(in crate::ui) fn library_selection(&self) -> LibrarySelection {
        let app = &self.app;
        let mut summary = LibrarySelection::default();
        if app.get_selected_tab() != crate::library::TAB || !app.get_library().view_ready {
            return summary;
        }
        let selected = app
            .global::<MediaSelection>()
            .get_indices()
            .iter()
            .collect::<HashSet<_>>();
        if selected.is_empty() {
            return summary;
        }
        app.get_media_items().visit(&mut |_, item| {
            if selected.contains(&item.media_index) {
                summary.total += 1;
                if item.status == "Saved" {
                    summary.saved += 1;
                    summary.gifs += usize::from(
                        item.media_type == "animated_gif"
                            && std::path::Path::new(&item.saved_path)
                                .extension()
                                .is_some_and(|extension| extension.eq_ignore_ascii_case("mp4")),
                    );
                }
            }
            true
        });
        summary
    }

    pub(in crate::ui) fn library_delete_confirm(&mut self, ctx: &egui::Context) {
        if !self.library_delete_confirmation {
            return;
        }
        let count = self.library_selection().saved;
        if count == 0 {
            self.library_delete_confirmation = false;
            return;
        }
        let (close, confirmed) = self.destructive_confirm(
            ctx,
            "Delete confirmation",
            self.tr("Delete {} files from disk?").replace("{}", &count.to_string()),
            self.tr("The selected files are permanently deleted from your folder. They stay in the library as missing files and can be downloaded again."),
            "Delete files",
        );
        if confirmed {
            self.app.invoke_library_delete_selected();
        }
        if close || confirmed {
            self.library_delete_confirmation = false;
        }
    }

    pub(in crate::ui) fn media_download_or_open(&self, item: &MediaItem) {
        if self.app.get_selected_tab() == crate::library::TAB && item.status == "Saved" {
            if let Some(folder) = std::path::Path::new(&item.saved_path).parent() {
                self.app
                    .invoke_open_download_path(folder.to_string_lossy().into_owned(), true);
            }
        } else {
            self.app.invoke_download_selected_media(item.media_index);
        }
    }
}
