use crate::ui::components::icon_button::IconStyle;
use crate::ui::*;

impl Desktop {
    pub(in crate::ui) fn result_toolbar(
        &mut self,
        ui: &mut Ui,
        items: ui_runtime::ModelRc<MediaItem>,
    ) {
        let app = self.app.clone();
        let results = app.global::<MediaResults>();
        let selected = crate::media::results::downloadable_selection(&app);
        let colors = self.colors(ui);
        let new = results.invoke_new_count(items.clone());
        let count = format!(
            "{} {}{}",
            items.row_count(),
            self.tr("media found"),
            if new > 0 {
                format!(" · {} {}", new, self.tr("new"))
            } else {
                String::new()
            }
        );
        let (row, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), egui::Sense::hover());
        let counted = ui.painter().text(
            row.left_center(),
            egui::Align2::LEFT_CENTER,
            &count,
            egui::FontId::new(typography::BODY, egui::FontFamily::Name("cera-bold".into())),
            colors.secondary,
        );
        self.post_type_picker(
            ui,
            &items,
            egui::pos2(
                counted.right() + 12.0,
                row.center().y - appearance::CONTROL_HEIGHT / 2.0,
            ),
        );
        let mut actions = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("result-actions")
                .max_rect(row)
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
        );
        actions.spacing_mut().item_spacing.x = 8.0;
        actions.add_enabled_ui(app.get_has_output(), |ui| {
            if self
                .home_action(
                    ui,
                    &self.tr("Open folder"),
                    Some((Icon::Folder, 20.0)),
                    false,
                    Vec2::new(126.0, 32.0),
                    6,
                )
                .clicked()
            {
                app.invoke_open_output_folder();
            }
            if self
                .home_action(
                    ui,
                    &self.tr("Export JSON"),
                    Some((Icon::FileJson, 20.0)),
                    false,
                    Vec2::new(140.0, 32.0),
                    6,
                )
                .clicked()
            {
                app.invoke_export_json();
            }
        });
        let caption = if selected > 0 {
            self.tr("Download selected ({})")
                .replace("{}", &selected.to_string())
        } else {
            self.tr(match crate::media::results::split_filter(&self.kind).0 {
                "photo" => "Download images",
                "video" => "Download videos",
                "animated_gif" => "Download GIFs",
                "text" => "Download text",
                _ => "Download all",
            })
        };
        actions.add_enabled_ui(
            !self.busy() && (selected > 0 || results.invoke_count(items, self.kind.clone()) > 0),
            |ui| {
                if self
                    .home_action(
                        ui,
                        &caption,
                        Some((Icon::Download, 20.0)),
                        true,
                        Vec2::new(148.0, 32.0),
                        6,
                    )
                    .clicked()
                {
                    app.invoke_start_downloads(if selected > 0 {
                        "selected".into()
                    } else {
                        self.kind.clone()
                    });
                }
            },
        );
        if app.get_has_more() && !app.get_batch_result_open() {
            actions.add_enabled_ui(
                !self.busy() && !app.get_saved_auth_token().is_empty(),
                |ui| {
                    if self
                        .home_action(
                            ui,
                            &self.tr("Continue"),
                            Some((Icon::ArrowRight, 20.0)),
                            false,
                            Vec2::new(100.0, 32.0),
                            6,
                        )
                        .clicked()
                    {
                        app.invoke_continue_extraction();
                    }
                },
            );
        }
    }

    pub(in crate::ui) fn filter_toolbar(
        &mut self,
        ui: &mut Ui,
        items: ui_runtime::ModelRc<MediaItem>,
    ) {
        let app = self.app.clone();
        let results = app.global::<MediaResults>();
        let selection = app.global::<MediaSelection>();
        let colors = self.colors(ui);
        let (row, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), appearance::CONTROL_HEIGHT),
            egui::Sense::hover(),
        );
        let mut filters = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("media-filters")
                .max_rect(row)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        filters.spacing_mut().item_spacing.x = 6.0;
        let (types, post) = crate::media::results::split_filter(&self.kind);
        let types = types.to_owned();
        for (key, label, icon) in [
            ("all", "All", None),
            ("photo", "Image", Some(Icon::Photo)),
            ("video", "Video", Some(Icon::Video)),
            ("animated_gif", "GIF", Some(Icon::Film)),
            ("text", "Text", Some(Icon::Text)),
        ] {
            let active = types == key
                || (types != "all"
                    && key != "all"
                    && crate::media::results::matches_kind(key, &types));
            let count = results.invoke_count(
                items.clone(),
                crate::media::results::with_post_type(key, post),
            );
            let caption = format!("{}  {count}", self.tr(label));
            filters.add_enabled_ui(key == "all" || count > 0, |ui| {
                if self
                    .home_action(
                        ui,
                        &caption,
                        icon.map(|icon| (icon, 16.0)),
                        active,
                        Vec2::new(0.0, appearance::CONTROL_HEIGHT),
                        16,
                    )
                    .clicked()
                {
                    self.kind = crate::media::results::toggle_kind(&self.kind, key);
                    self.limit = 30;
                }
            });
        }
        let sort_left = row.right() - 204.0;
        let sort_rect = egui::Rect::from_min_size(
            egui::pos2(sort_left, row.top()),
            Vec2::new(126.0, appearance::CONTROL_HEIGHT),
        );
        let mut sorting = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("sort")
                .max_rect(sort_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        sorting.spacing_mut().interact_size.y = appearance::CONTROL_HEIGHT;
        sorting.spacing_mut().button_padding = Vec2::new(8.0, 4.0);
        let mut sort = app.get_results_sort();
        let choices = ["Fetch order", "Newest first", "Oldest first"].map(|label| self.tr(label));
        let next = self.choice_picker(
            &mut sorting,
            sort_rect,
            &crate::ui::components::dropdown::SelectOptions::new("Result order", &choices),
            sort.clamp(0, 2) as usize,
        );
        if next != sort as usize {
            sort = next as i32;
            app.set_results_sort(sort);
            app.invoke_settings_changed();
        }
        for (view, label, icon, left) in [
            ("grid", "Grid view", Icon::Grid, row.right() - 70.0),
            (
                "list",
                "List view",
                Icon::List,
                row.right() - appearance::CONTROL_HEIGHT,
            ),
        ] {
            let rect = egui::Rect::from_min_size(
                egui::pos2(left, row.top()),
                Vec2::splat(appearance::CONTROL_HEIGHT),
            );
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt(view)
                    .max_rect(rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            let style = IconStyle {
                foreground: colors.secondary,
                radius: 6,
                icon_size: appearance::CONTROL_ICON_SIZE,
                ..IconStyle::ghost(colors, app.get_results_view() == view)
            };
            if self
                .home_icon(&mut child, icon, label, appearance::CONTROL_HEIGHT, style)
                .clicked()
            {
                app.set_results_view(view.into());
                app.invoke_settings_changed();
            }
        }
        let all_selected = selection.invoke_all_selected(
            selection.get_indices(),
            items.clone(),
            self.kind.clone(),
        );
        let label = self.tr(if all_selected {
            "Deselect filter"
        } else {
            "Select filter"
        });
        let text_width = ui
            .painter()
            .layout_no_wrap(
                label.clone(),
                egui::FontId::new(
                    typography::BODY,
                    egui::FontFamily::Name("cera-medium".into()),
                ),
                colors.heading,
            )
            .size()
            .x
            + 24.0;
        let left = sort_left - 6.0 - text_width;
        let mut selecting = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("filter-selection")
                .max_rect(egui::Rect::from_min_size(
                    egui::pos2(left, row.top()),
                    Vec2::new(text_width, appearance::CONTROL_HEIGHT),
                ))
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        selecting.add_enabled_ui(
            !self.busy() && results.invoke_count(items, self.kind.clone()) > 0,
            |ui| {
                if self
                    .home_action(
                        ui,
                        &label,
                        None,
                        false,
                        Vec2::new(text_width, appearance::CONTROL_HEIGHT),
                        6,
                    )
                    .clicked()
                {
                    selection.invoke_toggle_filter(self.kind.clone());
                }
            },
        );
    }

    /// Ctrl/Cmd+A selects every downloadable item in the current filter and
    /// Esc clears the selection. Runs after dialogs so their keys win.
    pub(in crate::ui) fn selection_shortcuts(&self, ctx: &egui::Context) {
        let app = &self.app;
        if self.busy()
            || app.get_selected_media_index() >= 0
            || ctx.memory(|memory| memory.top_modal_layer().is_some())
            || egui::Popup::is_any_open(ctx)
            || ctx.text_edit_focused()
        {
            return;
        }
        let selection = app.global::<MediaSelection>();
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::A)) {
            let items = app.get_media_items();
            if !selection.invoke_all_selected(selection.get_indices(), items, self.kind.clone()) {
                selection.invoke_toggle_filter(self.kind.clone());
            }
        } else if selection.get_indices().row_count() > 0
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            selection.invoke_clear();
        }
    }

    /// Narrows results to original posts, replies, quotes or reposts, starting
    /// at `left_top`. It lists the post types present and only appears when
    /// the results mix at least two of them.
    fn post_type_picker(
        &mut self,
        ui: &mut Ui,
        items: &ui_runtime::ModelRc<MediaItem>,
        left_top: egui::Pos2,
    ) {
        use crate::media::results::{POST_TYPES, split_filter, with_post_type};
        let results = self.app.global::<MediaResults>();
        let current = split_filter(&self.kind).1;
        let present = POST_TYPES[1..]
            .iter()
            .copied()
            .filter(|post| results.invoke_count(items.clone(), format!("all@{post}")) > 0)
            .collect::<Vec<_>>();
        if present.len() < 2 {
            if current != "all" {
                self.kind = with_post_type(&self.kind, "all");
            }
            return;
        }
        let options = POST_TYPES
            .iter()
            .copied()
            .filter(|post| *post == "all" || *post == current || present.contains(post))
            .collect::<Vec<_>>();
        let rect = egui::Rect::from_min_size(
            left_top,
            Vec2::new(POST_TYPE_PICKER_WIDTH, appearance::CONTROL_HEIGHT),
        );
        let mut picker = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("post-type")
                .max_rect(rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        picker.spacing_mut().interact_size.y = appearance::CONTROL_HEIGHT;
        picker.spacing_mut().button_padding = Vec2::new(8.0, 4.0);
        // Counts stay on the type buttons, which follow the chosen post type.
        let choices = options
            .iter()
            .map(|post| {
                self.tr(match *post {
                    "original" => "Originals",
                    "reply" => "Replies",
                    "quote" => "Quotes",
                    "repost" => "Reposts",
                    _ => "All posts",
                })
            })
            .collect::<Vec<_>>();
        let selected = options
            .iter()
            .position(|post| *post == current)
            .unwrap_or_default();
        let next = self.choice_picker(
            &mut picker,
            rect,
            &crate::ui::components::dropdown::SelectOptions::new("Post type", &choices),
            selected,
        );
        if next != selected {
            self.kind = with_post_type(&self.kind, options[next]);
            self.limit = 30;
        }
    }
}

const POST_TYPE_PICKER_WIDTH: f32 = 120.0;
