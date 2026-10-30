mod batch;
pub(in crate::ui) mod rows;
pub(in crate::ui) mod toolbar;
use crate::ui::components::icon_button::IconStyle;
use crate::ui::*;
impl Desktop {
    pub(in crate::ui) fn home_view(&mut self, ui: &mut Ui) {
        let app = self.app.clone();
        let extracting = app.get_is_extracting() && !app.get_library().feedback_active;
        let busy = app.get_is_busy() && !app.get_library().feedback_active;
        if app.get_batch_mode() && !app.get_batch_result_open() {
            self.batch_page(ui);
            self.home_mode_switch(ui);
            return;
        }
        let batch_result = app.get_batch_mode() && app.get_batch_result_open();
        let colors = self.colors(ui);
        egui::Frame::new().show(ui, |ui| {
            let toolbar = egui::Rect::from_min_max(
                ui.max_rect().right_top() + Vec2::new(-100.0, 12.0),
                ui.max_rect().right_top() + Vec2::new(-12.0, 52.0),
            );
            let mut toolbar_ui = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt("home-toolbar")
                    .max_rect(toolbar)
                    .layout(egui::Layout::right_to_left(egui::Align::Center)),
            );
            if !batch_result { toolbar_ui.scope(|ui| {
                if self
                    .icon_button(
                        ui,
                        Icon::Sliders,
                        "Fetch options",
                        app.get_batch_mode() || app.get_date_filter_enabled(),
                        40.0,
                    )
                    .clicked()
                {
                    app.set_fetch_options_open(true);
                }
                if self
                    .icon_button(ui, Icon::AccountSearch, "Search accounts", false, 40.0)
                    .clicked()
                {
                    app.set_account_search_open(true);
                }
            }); }

            let all = app.get_media_items();
            let hidden = app.get_home_results_hidden();
            if self.result_view.source_changed(&all) { self.limit = 30; }
            let empty = !batch_result && (hidden || (all.row_count() == 0 && !app.get_account_profile().available));
            let viewport_height = ui.available_height()-2.0*appearance::CONTENT_INSET;
            let page_id = if batch_result { format!("batch-result-page-{}", app.get_batch_result_index()) } else if hidden { "home-landing".into() } else { "home-page".into() };
            let scroll = egui::ScrollArea::vertical()
                .id_salt(page_id)
                .content_margin(appearance::CONTENT_INSET as i8)
                .scroll_bar_rect(ui.available_rect_before_wrap().shrink2(Vec2::new(0.0,16.0)))
                .auto_shrink([false, false])
                .show_themed(ui, |ui| {
                    if !hidden && std::mem::take(&mut self.results_jump_requested) {
                        ui.scroll_to_cursor_animation(Some(egui::Align::Min), egui::style::ScrollAnimation::duration(0.3));
                    }
                    ui.spacing_mut().item_spacing.y=14.0;
                    if empty {
                        ui.add_space(((viewport_height - 74.0) / 2.0).max(20.0));
                        ui.allocate_ui_with_layout(Vec2::new(ui.available_width(),20.0),egui::Layout::left_to_right(egui::Align::Center),|ui| {
                            ui.spacing_mut().interact_size.y=0.0;
                            ui.spacing_mut().item_spacing.x=6.0;
                            let auth = app.get_saved_auth_token().is_empty();
                            let prompt = if extracting{String::new()}else{self.tr(if auth {
                                "Add auth_token in Settings to search"
                            } else {
                                match app.get_fetch_source_index() {
                                    1 => "Fetch your Likes using the account signed in with auth_token.",
                                    2 => "Fetch your private Bookmarks using the account signed in with auth_token.",
                                    _ => "Drop any X (Twitter) username or link — grab all the media.",
                                }
                            })};

                            let text_width = ui
                                .painter()
                                .layout_no_wrap(prompt.clone(), egui::FontId::proportional(typography::BODY), colors.secondary)
                                .size()
                                .x;
                            let link_text=self.tr("click here");let link_font=egui::FontId::new(typography::BODY,egui::FontFamily::Name("cera-bold".into()));
                            let link_width=if auth&&!extracting{6.0+ui.painter().layout_no_wrap(link_text.clone(),link_font.clone(),colors.accent).size().x}else{0.0};
                            ui.add_space(((ui.available_width()-text_width-link_width)/2.0).max(0.0));
                            ui.label(RichText::new(prompt).color(colors.secondary));
                            if auth && !extracting && self.text_link(ui, RichText::new(link_text).font(link_font).color(colors.accent)).clicked() {
                                app.set_selected_tab(2);
                                app.set_settings_category(1);
                            }
                        });
                    }
                    if batch_result { self.batch_result_header(ui); } else { self.search_bar(ui, empty); }
                    if !batch_result && !hidden && app.get_date_filter_enabled() {
                        ui.vertical_centered(|ui| {
                            ui.small(format!(
                                "{}: {} → {}",
                                self.tr("Date range"),
                                app.get_fetch_start_date(),
                                app.get_fetch_end_date()
                            ));
                        });
                    }
                    if !extracting&&(busy || !app.get_last_error().is_empty()) {
                        appearance::card(ui).show(ui, |ui| {
                            if busy {
                                if self.operation_status_row(ui, &app.get_status(), "Cancel") {
                                    app.invoke_cancel_operation();
                                }
                            } else { ui.label(app.get_status()); }

                            if busy {
                                self.progress_bar(ui,app.get_progress());
                                self.setting_caption(ui,&app.get_progress_detail());
                            }
                            self.error(ui, &app.get_last_error());
                        });
                    }
                    if !batch_result && empty {
                        return;
                    }
                    self.profile_card(ui);
                    if extracting && app.get_account_profile().available {
                        self.setting_caption(ui, &app.get_status());
                    }
                    if all.row_count() == 0 && extracting { return; }
                    let results = app.global::<MediaResults>();
                    self.result_toolbar(ui, all.clone());
                    self.filter_toolbar(ui, all.clone());

                    let rows = self.result_view.get(all.clone(), &self.kind, self.limit.min(i32::MAX as usize) as i32, app.get_results_sort());
                    if rows.row_count() == 0 {
                        appearance::card(ui).show(ui, |ui| { ui.label(self.tr("Nothing in this filter.")); });
                    } else {
                        self.virtual_media_rows(ui, &rows);
                    }
                    let total = results.invoke_count(all, self.kind.clone()).max(0) as usize;
                    if rows.row_count() < total && ui.cursor().top() <= ui.clip_rect().bottom() + 200.0 {
                        self.limit = self.limit.saturating_add(30).min(total);
                        ui.ctx().request_repaint();
                    }
                    if ui.min_rect().height() > viewport_height + 240.0 {
                        ui.add_space(64.0);
                    }
                });
            if app.get_media_items().row_count() > 0
                && !hidden
                && scroll.state.offset.y > 240.0
                && app.get_selected_media_index() < 0
            {
                let bounds = egui::Rect::from_min_size(
                    scroll.inner_rect.right_bottom() - Vec2::splat(56.0),
                    Vec2::splat(40.0),
                );
                let mut jump = crate::ui::components::dialog::child(ui, "results-jump-top", bounds);
                if self.home_icon(&mut jump, Icon::ChevronUp, "Jump to top", 40.0, IconStyle {
                    idle: colors.surface,
                    hover: colors.control,
                    pressed: colors.muted_control,
                    foreground: colors.heading,
                    border: egui::Stroke::new(1.0, colors.control_border),
                    radius: 20,
                    icon_size: appearance::NAVIGATION_ICON_SIZE,
                    visible: true,
                }).clicked() {
                    self.scrolling.stop();
                    let mut state = egui::scroll_area::State::default();
                    state.offset = scroll.state.offset;
                    state.store(ui.ctx(), scroll.id);
                    self.results_jump_requested = true;
                    ui.ctx().request_repaint();
                }
            }
            self.home_navigation(ui);
            if empty && !batch_result {
                self.home_mode_switch(ui);
            }
        });
    }
    fn home_navigation(&mut self, ui: &mut Ui) {
        let app = self.app.clone();
        if (app.get_media_items().row_count() == 0 && !app.get_account_profile().available)
            || app.get_batch_mode()
        {
            return;
        }
        let hidden = app.get_home_results_hidden();
        for (forward, icon, label, offset, enabled) in [
            (false, Icon::Back, "Back to Home", 0.0, !hidden),
            (true, Icon::ArrowRight, "Forward to results", 48.0, hidden),
        ] {
            let rect = egui::Rect::from_min_size(
                ui.max_rect().left_top() + Vec2::new(appearance::CONTENT_INSET + offset, 12.0),
                Vec2::splat(40.0),
            );
            if self
                .history_icon(ui, rect, (icon, 18.0), label, enabled, false)
                .clicked()
            {
                app.set_home_results_hidden(!forward);
                app.set_selected_media_index(-1);
                self.results_jump_requested = false;
                self.scrolling.stop();
            }
        }
    }
    fn profile_card(&mut self, ui: &mut Ui) {
        let app = self.app.clone();
        let profile = app.get_account_profile();
        if !profile.available {
            if !app.get_is_busy() || app.get_is_extracting() {
                ui.label(
                    RichText::new(if app.get_selected_tab() == crate::library::TAB {
                        app.get_library().status
                    } else {
                        app.get_status()
                    })
                    .size(12.0)
                    .line_height(Some(18.0)),
                );
            }
            return;
        }
        let colors = self.colors(ui);
        let width = ui.available_width();
        let panel = egui::Frame::new()
            .fill(colors.surface)
            .stroke(egui::Stroke::new(1.0, colors.border))
            .corner_radius(16)
            .show(ui, |ui| {
                ui.set_min_width(width - 2.0);
                ui.spacing_mut().item_spacing.y = 0.0;
                if profile.has_banner || !profile.banner_url.is_empty() {
                    let (reserved, _) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), 56.0),
                        egui::Sense::hover(),
                    );
                    let banner =
                        egui::Rect::from_min_size(reserved.min, Vec2::new(reserved.width(), 96.0));
                    let mut image_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .id_salt("banner-image")
                            .max_rect(banner),
                    );
                    image_ui.multiply_opacity(0.9);
                    self.paint_media(&mut image_ui, &profile.banner_image, banner, 15);
                    if self
                        .banner_fade
                        .as_ref()
                        .is_none_or(|(color, _)| *color != colors.surface)
                    {
                        self.banner_fade = Some((
                            colors.surface,
                            ui.ctx().load_texture(
                                "profile-banner-fade",
                                egui::ColorImage::new(
                                    [1, 2],
                                    vec![Color32::from_black_alpha(38), colors.surface],
                                ),
                                egui::TextureOptions::LINEAR,
                            ),
                        ));
                    }
                    ui.painter().add(
                        egui::epaint::RectShape::filled(
                            banner,
                            egui::CornerRadius {
                                nw: 15,
                                ne: 15,
                                sw: 0,
                                se: 0,
                            },
                            Color32::WHITE,
                        )
                        .with_texture(
                            self.banner_fade.as_ref().unwrap().1.id(),
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.25), egui::pos2(1.0, 0.75)),
                        ),
                    );
                    let show = ui.rect_contains_pointer(banner);
                    let style = IconStyle {
                        idle: Color32::from_black_alpha(153),
                        hover: Color32::from_black_alpha(204),
                        pressed: colors.primary,
                        foreground: Color32::WHITE,
                        border: egui::Stroke::NONE,
                        radius: 14,
                        icon_size: appearance::CONTROL_ICON_SIZE,
                        visible: show,
                    };
                    for (id, icon, label, offset, enabled) in [
                        (
                            "banner-preview",
                            Icon::Eye,
                            "Preview banner",
                            -52.0 - appearance::COMPACT_CONTROL_SIZE,
                            !profile.banner_url.is_empty(),
                        ),
                        (
                            "banner-download",
                            Icon::Download,
                            "Download banner",
                            -44.0,
                            !self.busy(),
                        ),
                    ] {
                        let button = egui::Rect::from_min_size(
                            banner.right_top() + Vec2::new(offset, 8.0),
                            Vec2::splat(appearance::COMPACT_CONTROL_SIZE),
                        );
                        let mut child = ui.new_child(
                            egui::UiBuilder::new()
                                .id_salt(id)
                                .max_rect(button)
                                .layout(egui::Layout::top_down(egui::Align::Min)),
                        );
                        child.add_enabled_ui(enabled, |ui| {
                            if self
                                .home_icon(
                                    ui,
                                    icon,
                                    label,
                                    appearance::COMPACT_CONTROL_SIZE,
                                    style,
                                )
                                .clicked()
                            {
                                if id == "banner-preview" {
                                    self.banner_preview = Some(
                                        crate::download::engine::original_banner_url(
                                            &profile.banner_url,
                                        ),
                                    );
                                } else {
                                    app.invoke_download_profile_image("banner".into());
                                }
                            }
                        });
                    }
                }
                egui::Frame::new().inner_margin(15).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 10.0;
                    let (header, _) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), 64.0),
                        egui::Sense::hover(),
                    );
                    let avatar = egui::Rect::from_min_size(header.min, Vec2::splat(64.0));
                    ui.painter()
                        .circle_filled(avatar.center(), 32.0, colors.surface);
                    ui.painter()
                        .circle_filled(avatar.center(), 29.0, colors.control);
                    if profile.has_avatar {
                        self.paint_media(
                            ui,
                            &profile.avatar_image,
                            avatar.shrink(3.0),
                            if profile.avatar_square { 11 } else { 29 },
                        );
                        let mut child = ui.new_child(
                            egui::UiBuilder::new()
                                .id_salt("avatar-download")
                                .max_rect(avatar)
                                .layout(egui::Layout::top_down(egui::Align::Min)),
                        );
                        let style = IconStyle {
                            idle: Color32::from_black_alpha(128),
                            hover: Color32::from_black_alpha(170),
                            pressed: colors.primary,
                            foreground: Color32::WHITE,
                            border: egui::Stroke::NONE,
                            radius: if profile.avatar_square { 14 } else { 32 },
                            icon_size: appearance::NAVIGATION_ICON_SIZE,
                            visible: false,
                        };
                        child.add_enabled_ui(!self.busy(), |ui| {
                            if self
                                .home_icon(ui, Icon::Download, "Download avatar", 64.0, style)
                                .clicked()
                            {
                                app.invoke_download_profile_image("avatar".into());
                            }
                        });
                    } else {
                        Icon::User.image(colors.secondary, 28.0).paint_at(
                            ui,
                            egui::Rect::from_center_size(avatar.center(), Vec2::splat(28.0)),
                        );
                    }
                    let name_rect = egui::Rect::from_min_size(
                        header.min + Vec2::new(78.0, 10.0),
                        Vec2::new(header.width() - 78.0, 23.0),
                    );
                    let name_font = egui::FontId::new(
                        typography::HEADING,
                        egui::FontFamily::Name("cera-bold".into()),
                    );
                    let name_width = ui
                        .painter()
                        .layout_no_wrap(
                            profile.display_name.clone(),
                            name_font.clone(),
                            colors.heading,
                        )
                        .size()
                        .x
                        .min(name_rect.width() - 44.0);
                    let mut name = ui.new_child(
                        egui::UiBuilder::new()
                            .id_salt("profile-name")
                            .max_rect(egui::Rect::from_min_size(
                                name_rect.min,
                                Vec2::new(name_width, 23.0),
                            ))
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );
                    name.add(
                        egui::Label::new(
                            RichText::new(&profile.display_name)
                                .font(name_font)
                                .color(colors.heading),
                        )
                        .truncate(),
                    );
                    let mut badge_x = name_rect.left() + name_width + 6.0;
                    for (show, icon, tint, size) in [
                        (profile.verified, Icon::Verified, colors.accent, 15.0),
                        (profile.protected, Icon::Lock, colors.secondary, 16.0),
                    ] {
                        if show {
                            icon.image(tint, size).paint_at(
                                ui,
                                egui::Rect::from_center_size(
                                    egui::pos2(badge_x + size / 2.0, name_rect.center().y),
                                    Vec2::splat(size),
                                ),
                            );
                            badge_x += size + 6.0;
                        }
                    }
                    let mut handle = ui.new_child(
                        egui::UiBuilder::new()
                            .id_salt("profile-handle")
                            .max_rect(egui::Rect::from_min_size(
                                header.min + Vec2::new(78.0, 35.0),
                                Vec2::new(header.width() - 78.0, 20.0),
                            ))
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );
                    handle.add(
                        egui::Label::new(
                            RichText::new(format!("@{}", profile.username))
                                .size(14.0)
                                .color(colors.secondary),
                        )
                        .truncate(),
                    );
                    if !profile.bio.is_empty() {
                        self.linked_text(ui, &profile.bio, 14.0, colors.heading);
                    }
                    if !profile.location.is_empty()
                        || !profile.website_url.is_empty()
                        || !profile.joined.is_empty()
                    {
                        ui.allocate_ui_with_layout(
                            Vec2::new(ui.available_width(), 18.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.spacing_mut().interact_size.y = 0.0;
                                ui.spacing_mut().item_spacing.x = 5.0;
                                if !profile.location.is_empty() {
                                    ui.add(Icon::MapPin.image(colors.secondary, 12.0));
                                    ui.label(
                                        RichText::new(&profile.location)
                                            .size(12.0)
                                            .color(colors.secondary),
                                    );
                                    ui.add_space(9.0);
                                }
                                if !profile.website_url.is_empty() {
                                    ui.add(Icon::Globe.image(colors.secondary, 12.0));
                                    if self
                                        .text_link(
                                            ui,
                                            RichText::new(&profile.website_label)
                                                .size(12.0)
                                                .color(colors.accent),
                                        )
                                        .clicked()
                                    {
                                        app.invoke_open_link(profile.website_url.clone());
                                    }
                                    ui.add_space(9.0);
                                }
                                if !profile.joined.is_empty() {
                                    ui.add(Icon::Calendar.image(colors.secondary, 12.0));
                                    ui.label(
                                        RichText::new(format!(
                                            "{} {}",
                                            self.tr("Joined"),
                                            profile.joined
                                        ))
                                        .size(12.0)
                                        .color(colors.secondary),
                                    );
                                }
                            },
                        );
                    }
                    ui.allocate_ui_with_layout(
                        Vec2::new(ui.available_width(), 18.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.spacing_mut().interact_size.y = 0.0;
                            ui.spacing_mut().item_spacing.x = 3.0;
                            for (i, (value, label)) in [
                                (&profile.posts, "posts"),
                                (&profile.following, "following"),
                                (&profile.followers, "followers"),
                                (&profile.media, "media"),
                            ]
                            .into_iter()
                            .enumerate()
                            {
                                if i > 0 {
                                    ui.label(RichText::new("·").size(12.0).color(colors.secondary));
                                }
                                ui.label(
                                    RichText::new(value)
                                        .font(egui::FontId::new(
                                            12.0,
                                            egui::FontFamily::Name("cera-medium".into()),
                                        ))
                                        .color(colors.body),
                                );
                                ui.label(
                                    RichText::new(self.tr(label))
                                        .size(12.0)
                                        .color(colors.secondary),
                                );
                            }
                        },
                    );
                });
            });
        panel.response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Other, true, self.tr("Account profile"))
        });
    }
    fn search_bar(&mut self, ui: &mut Ui, empty: bool) {
        let app = self.app.clone();
        let colors = self.colors(ui);
        let (row, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), egui::Sense::hover());
        let width = appearance::SEARCH_WIDTH.min(row.width() - if empty { 0.0 } else { 200.0 });
        let rect =
            egui::Rect::from_center_size(row.center(), Vec2::new(width, appearance::SEARCH_HEIGHT));
        ui.painter().rect_filled(rect, 20.0, colors.surface);
        let border = ui.painter().add(egui::Shape::Noop);
        let input = egui::Rect::from_min_max(
            rect.min + Vec2::new(26.0, 5.0),
            rect.max - Vec2::new(82.0, 5.0),
        );
        let auth = !app.get_saved_auth_token().is_empty();
        let mut source = app.get_source_url();
        let mut input_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("source-field")
                .max_rect(input)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        let mut response = input_ui.add(
            egui::TextEdit::singleline(&mut source)
                .id(ui.id().with("source-url"))
                .frame(egui::Frame::NONE)
                .margin(egui::Margin::ZERO)
                .min_size(input.size())
                .desired_width(input.width())
                .vertical_align(egui::Align::Center)
                .hint_text(self.tr(if auth {
                    "X/Twitter URL or username"
                } else {
                    "Add auth_token in Settings to search"
                }))
                .interactive(!app.get_is_busy() && auth && !app.get_batch_mode()),
        );
        if self.text_context_menu(&response, &mut source, app.get_batch_mode()) {
            response.mark_changed();
        }
        ui.painter().set(
            border,
            egui::Shape::rect_stroke(
                rect,
                20.0,
                egui::Stroke::new(
                    1.0,
                    if response.has_focus() {
                        colors.accent
                    } else {
                        colors.input_border
                    },
                ),
                egui::StrokeKind::Inside,
            ),
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::TextEdit,
                !app.get_is_busy() && auth,
                self.tr("Search X/Twitter media URL"),
            )
        });
        if response.changed() {
            app.set_source_url(source);
        }
        let submit = response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
        let paste_rect = egui::Rect::from_min_size(
            egui::pos2(rect.right() - 76.0, rect.top() + 4.0),
            Vec2::splat(32.0),
        );
        let mut paste = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("paste")
                .max_rect(paste_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        let paste_style = IconStyle {
            radius: 16,
            ..IconStyle::ghost(colors, false)
        };
        paste.add_enabled_ui(!app.get_is_busy() && auth, |ui| {
            if self
                .search_icon(
                    ui,
                    Icon::Clipboard,
                    "Paste from clipboard",
                    32.0,
                    paste_style,
                )
                .clicked()
            {
                app.invoke_paste_from_clipboard();
            }
        });
        let extract_rect = egui::Rect::from_min_size(
            egui::pos2(rect.right() - 38.0, rect.top() + 4.0),
            Vec2::splat(32.0),
        );
        let mut extract = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("extract")
                .max_rect(extract_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        let extracting = app.get_is_extracting() && !app.get_library().feedback_active;
        let can_extract = extracting
            || (!self.busy()
                && auth
                && if app.get_batch_mode() {
                    app.get_batch_pending_count() > 0
                } else {
                    !app.get_source_url().trim().is_empty()
                });
        let label = if extracting {
            "Cancel operation"
        } else {
            match app.get_fetch_source_index() {
                1 => "Fetch my Likes",
                2 => "Fetch my Bookmarks",
                _ => "Search and extract",
            }
        };
        let style = IconStyle {
            idle: colors.control,
            hover: colors.muted_control,
            pressed: colors.muted_control,
            foreground: if extracting {
                Color32::TRANSPARENT
            } else {
                colors.heading
            },
            border: egui::Stroke::NONE,
            radius: 16,
            icon_size: 20.0,
            visible: true,
        };
        let clicked = extract
            .add_enabled_ui(can_extract, |ui| {
                self.search_icon(ui, Icon::ArrowRight, label, 32.0, style)
                    .clicked()
            })
            .inner;
        if extracting {
            egui::Spinner::new().color(colors.heading).paint_at(
                ui,
                egui::Rect::from_center_size(extract_rect.center(), Vec2::splat(20.0)),
            );
        }
        if (submit || clicked) && can_extract {
            if extracting {
                app.invoke_cancel_operation();
            } else if app.get_batch_mode() {
                app.invoke_start_batch();
            } else {
                app.invoke_start_extraction();
            }
        }
    }
    pub(in crate::ui) fn paint_media(
        &mut self,
        ui: &mut Ui,
        image: &Image,
        rect: egui::Rect,
        radius: u8,
    ) {
        if image.size().width == 0 {
            return;
        }
        let texture = self.media_texture(ui.ctx(), image);
        let source = image.size().width as f32 / image.size().height as f32;
        let target = rect.width() / rect.height();
        let uv = if source > target {
            let crop = (1.0 - target / source) / 2.0;
            egui::Rect::from_min_max(egui::pos2(crop, 0.0), egui::pos2(1.0 - crop, 1.0))
        } else {
            let crop = (1.0 - source / target) / 2.0;
            egui::Rect::from_min_max(egui::pos2(0.0, crop), egui::pos2(1.0, 1.0 - crop))
        };
        egui::Image::new(&texture)
            .uv(uv)
            .corner_radius(radius)
            .fit_to_exact_size(rect.size())
            .maintain_aspect_ratio(false)
            .paint_at(ui, rect);
    }
    pub(in crate::ui) fn media_texture(
        &mut self,
        ctx: &egui::Context,
        image: &Image,
    ) -> egui::TextureHandle {
        let entry = self.textures.entry(image.id).or_insert_with(|| {
            (
                ctx.load_texture(
                    format!("media-{}", image.id),
                    egui::ColorImage::from_rgba_unmultiplied(
                        [image.pixels.width as usize, image.pixels.height as usize],
                        &image.pixels.bytes,
                    ),
                    egui::TextureOptions::LINEAR,
                ),
                self.frame,
            )
        });

        entry.1 = self.frame;
        entry.0.clone()
    }
    fn media_tile(
        &mut self,
        ui: &mut Ui,
        rows: &ui_runtime::ModelRc<MediaItem>,
        item: &MediaItem,
        size: f32,
        position: usize,
    ) {
        let app = self.app.clone();
        let colors = self.colors(ui);
        self.visible_media.insert(item.media_index);
        if item.preview_image.size().width == 0 {
            app.invoke_load_thumbnail(item.media_index);
        }
        let selected = app
            .global::<MediaSelection>()
            .get_indices()
            .iter()
            .any(|i| i == item.media_index);
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::click());
        let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &item.title));
        ui.painter().rect_filled(rect, 12.0, colors.surface);
        self.paint_media(ui, &item.preview_image, rect, 12);
        let focus = crate::ui::components::navigation::focus_visible(ui, &response);
        ui.painter().rect_stroke(
            rect,
            12.0,
            egui::Stroke::new(
                if focus || selected { 2.0 } else { 1.0 },
                if selected || response.hovered() || focus {
                    colors.accent
                } else {
                    colors.border
                },
            ),
            egui::StrokeKind::Inside,
        );
        let (icon, color) = match item.media_type.as_str() {
            "photo" => (Icon::Photo, Color32::from_rgb(59, 130, 246)),
            "video" => (Icon::Video, Color32::from_rgb(239, 68, 68)),
            "animated_gif" => (Icon::Film, Color32::from_rgb(168, 85, 247)),
            _ => (Icon::Text, Color32::from_rgb(16, 185, 129)),
        };
        let badge = egui::Rect::from_min_size(rect.min + Vec2::splat(6.0), Vec2::new(24.0, 22.0));
        ui.painter().rect_filled(badge, 5.0, color);
        icon.image(Color32::WHITE, 13.0).paint_at(
            ui,
            egui::Rect::from_center_size(badge.center(), Vec2::splat(13.0)),
        );
        if selected {
            self.media_selected_badge(ui, badge.right_top() + Vec2::new(4.0, 0.0));
        }
        if item.preview_image.size().width == 0 && item.media_type != "text" {
            icon.image(colors.secondary, 28.0).paint_at(
                ui,
                egui::Rect::from_center_size(rect.center(), Vec2::splat(28.0)),
            );
        }
        if item.media_type == "text" {
            let text_rect = egui::Rect::from_min_max(
                rect.min + Vec2::new(12.0, 32.0),
                rect.max - Vec2::new(12.0, 12.0),
            );
            let galley = ui.painter().layout(
                item.tweet_text.to_string(),
                egui::FontId::proportional(12.0),
                colors.heading,
                text_rect.width(),
            );
            ui.painter()
                .with_clip_rect(text_rect.intersect(ui.clip_rect()))
                .galley(text_rect.min, galley, colors.heading);
        }
        self.media_number(
            ui,
            egui::Rect::from_min_max(
                rect.left_bottom() + Vec2::new(38.0, -30.0),
                rect.right_bottom() + Vec2::new(-38.0, -10.0),
            ),
            position,
        );
        for (id, icon, position, diameter) in [
            (
                "preview",
                Icon::Eye,
                rect.left_bottom() + Vec2::new(6.0, -34.0),
                28.0,
            ),
            (
                "download",
                if item.status == "Saved" {
                    Icon::Folder
                } else {
                    Icon::Download
                },
                rect.right_bottom() + Vec2::new(-34.0, -34.0),
                28.0,
            ),
        ] {
            if id == "download"
                && item.status == "Saved"
                && app.get_selected_tab() != crate::library::TAB
            {
                continue;
            }
            let button_rect = egui::Rect::from_min_size(position, Vec2::splat(diameter));
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt((item.media_index, id))
                    .max_rect(button_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            let style = IconStyle {
                idle: Color32::from_black_alpha(158),
                hover: colors.primary,
                pressed: colors.primary.gamma_multiply(0.82),
                foreground: Color32::WHITE,
                border: egui::Stroke::NONE,
                radius: 14,
                icon_size: 16.0,
                visible: true,
            };
            let caption = format!(
                "{} · {}",
                self.tr(if id == "preview" {
                    "Preview"
                } else if item.status == "Saved" {
                    "Open folder"
                } else if item.status == "Missing" {
                    "Download again"
                } else {
                    "Download file"
                }),
                item.title
            );
            let enabled = if id == "preview" {
                !item.payload_pending
            } else {
                !self.busy()
            };
            let button = child
                .add_enabled_ui(enabled, |ui| {
                    self.home_icon(ui, icon, &caption, diameter, style)
                })
                .inner;
            if button.clicked() && enabled {
                if id == "preview" {
                    self.media_preview(item);
                } else {
                    self.media_download_or_open(item);
                }
            }
        }
        if item.status == "Saved" {
            let badge = egui::Rect::from_min_size(
                rect.right_top() + Vec2::new(-28.0, 6.0),
                Vec2::splat(22.0),
            );
            ui.painter()
                .rect_filled(badge, 11.0, Color32::from_rgb(21, 128, 61));
            Icon::Check.image(Color32::WHITE, 14.0).paint_at(
                ui,
                egui::Rect::from_center_size(badge.center(), Vec2::splat(14.0)),
            );
        }
        if item.status == "Missing" {
            let badge = egui::Rect::from_min_size(
                rect.right_top() + Vec2::new(-28.0, 6.0),
                Vec2::splat(22.0),
            );
            ui.painter().rect_filled(badge, 11, appearance::DANGER_FILL);
            Icon::Alert
                .image(Color32::WHITE, 14.0)
                .paint_at(ui, badge.shrink(4.0));
            ui.interact(badge, ui.id().with("missing-file"), egui::Sense::hover())
                .on_hover_text(self.tr("File missing"));
        }
        if item.is_new {
            let caption = self.tr("New");
            let font = egui::FontId::new(
                typography::MICRO,
                egui::FontFamily::Name("cera-medium".into()),
            );
            let width = ui
                .painter()
                .layout_no_wrap(caption.clone(), font.clone(), Color32::WHITE)
                .size()
                .x
                + 10.0;
            let badge = egui::Rect::from_min_size(
                egui::pos2(
                    rect.center().x - width / 2.0,
                    rect.top() + if item.status == "Failed" { 34.0 } else { 6.0 },
                ),
                Vec2::new(width, 22.0),
            );
            ui.painter().rect_filled(badge, 5.0, colors.primary);
            ui.painter().text(
                badge.center(),
                egui::Align2::CENTER_CENTER,
                caption,
                font,
                Color32::WHITE,
            );
        }
        if item.status == "Failed" {
            let caption = self.tr("Failed");
            let font = egui::FontId::new(
                typography::MICRO,
                egui::FontFamily::Name("cera-medium".into()),
            );
            let width = (ui
                .painter()
                .layout_no_wrap(caption.clone(), font.clone(), Color32::WHITE)
                .size()
                .x
                + 12.0)
                .min(rect.width() - 44.0);
            let badge = egui::Rect::from_min_size(
                egui::pos2(rect.right() - width - 6.0, rect.top() + 6.0),
                Vec2::new(width, 22.0),
            );
            ui.painter()
                .rect_filled(badge, 5.0, Color32::from_rgb(183, 55, 49));
            ui.painter().text(
                badge.center(),
                egui::Align2::CENTER_CENTER,
                caption,
                font,
                Color32::WHITE,
            );
        }
        if response.clicked() {
            self.media_clicked(ui, rows, item, position);
        }
    }
}
