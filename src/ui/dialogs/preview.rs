use crate::ui::components::dialog::child;
use crate::ui::components::icon_button::IconStyle;
use crate::ui::*;

impl Desktop {
    pub(in crate::ui) fn preview_overlay(&mut self, ctx: &egui::Context) {
        let app = self.app.clone();
        let index = app.get_selected_media_index();
        if index < 0 {
            self.preview_texture = Default::default();
            self.preview_index = None;
            // The profile image viewer shares the canvas; only reset it once both are closed.
            if self.profile_preview.is_none() {
                self.preview_canvas.info_hidden = false;
                self.preview_canvas.leave_fullscreen(ctx);
                self.preview_canvas.reset();
            }
            return;
        }
        let items = self.preview_view.get(
            app.get_media_items(),
            &self.kind,
            i32::MAX,
            app.get_results_sort(),
        );
        let position = app
            .global::<MediaResults>()
            .invoke_position(items.clone(), index);
        if position < 0 {
            self.preview_texture = Default::default();
            self.preview_canvas.leave_fullscreen(ctx);
            app.set_selected_media_index(-1);
            return;
        }
        let position = position as usize;
        let selected = items.row_data(position).unwrap();
        let item = &selected;
        let reset_scroll = self.preview_index != Some(index);
        self.preview_index = Some(index);
        if reset_scroll {
            self.preview_texture = Default::default();
            self.preview_canvas.reset();
        }
        // Rebuilt result models and failed loads fall back to the thumbnail; asking
        // again restores the original without reselecting. Pending and recently
        // failed requests are skipped by request().
        if item.media_type == "photo" && item.detail_image.size().width == 0 {
            crate::media::images::request(
                &app,
                crate::media::images::Kind::Preview,
                item.preview_url.to_string(),
            );
        }
        let fullscreen = self.preview_canvas.fullscreen;
        let viewport = ctx.viewport_rect();
        let width = if fullscreen {
            viewport.width()
        } else {
            1600.0_f32.min(viewport.width() - 88.0)
        };
        let height = if fullscreen {
            viewport.height()
        } else {
            viewport.height() - 48.0
        };
        let text_post = item.media_type == "text";
        let video = matches!(item.media_type.as_str(), "video" | "animated_gif");
        let show_info = !fullscreen && !self.preview_canvas.info_hidden;
        let info_width = if show_info { 320.0 } else { 0.0 };
        let mut close = false;
        let playback = app.global::<PreviewPlayback>();
        let frame = playback.get_frame();
        if !video || frame.is_none() {
            self.preview_texture = Default::default();
        }
        let has_image = !text_post
            && ((video && frame.is_some())
                || item.detail_image.size().width > 0
                || item.preview_image.size().width > 0);
        let id = egui::Id::new("media-preview");
        let modal = egui::Modal::new(id)
            .area(
                egui::Area::new(id)
                    .kind(egui::UiKind::Modal)
                    .sense(egui::Sense::hover())
                    .order(egui::Order::Foreground)
                    .fixed_pos(if fullscreen {
                        viewport.min
                    } else {
                        viewport.center() + Vec2::new(28.0 - width / 2.0, -height / 2.0)
                    })
                    .constrain(false),
            )
            .backdrop_color(Color32::TRANSPARENT)
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let colors = self.colors(ui);
                ui.painter().rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(appearance::RAIL_WIDTH, viewport.top()),
                        viewport.max,
                    ),
                    0,
                    Color32::from_black_alpha(209),
                );
                let (panel, response) =
                    ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Other,
                        true,
                        self.tr("Media preview"),
                    )
                });
                ui.painter()
                    .rect_filled(panel, if fullscreen { 0 } else { 18 }, colors.surface);
                let stage =
                    egui::Rect::from_min_size(panel.min, Vec2::new(width - info_width, height));
                ui.painter().rect_filled(
                    stage,
                    egui::CornerRadius {
                        nw: if fullscreen { 0 } else { 18 },
                        ne: if fullscreen || show_info { 0 } else { 18 },
                        sw: if fullscreen { 0 } else { 18 },
                        se: if fullscreen || show_info { 0 } else { 18 },
                    },
                    Color32::BLACK,
                );
                let image = if item.detail_image.size().width > 0 {
                    &item.detail_image
                } else {
                    &item.preview_image
                };
                if text_post {
                    let mut text = child(
                        ui,
                        "text-post-stage",
                        egui::Rect::from_min_max(
                            stage.min + Vec2::new(54.0, 48.0),
                            stage.max - Vec2::new(54.0, 16.0),
                        ),
                    );
                    text.visuals_mut().hyperlink_color =
                        appearance::Palette::new(&app.global::<AppTheme>().get_preset(), true)
                            .accent;
                    egui::ScrollArea::vertical()
                        .id_salt("preview-text-stage")
                        .auto_shrink([false, false])
                        .show_themed(&mut text, |ui| {
                            egui::Frame::new().inner_margin(12.0).show(ui, |ui| {
                                self.linked_text(ui, &item.tweet_text, 15.0, Color32::WHITE);
                            });
                        });
                } else if video && let Some(frame) = &frame {
                    let texture = self.preview_texture.update(ui.ctx(), frame);
                    self.preview_media_canvas(ui, &texture, stage);
                } else if image.size().width > 0 {
                    let texture = self.media_texture(ui.ctx(), image);
                    self.preview_media_canvas(ui, &texture, stage);
                }
                let loading = if video {
                    frame.is_none() && playback.get_loading() && playback.get_error().is_empty()
                } else {
                    item.media_type == "photo"
                        && item.detail_image.size().width == 0
                        && crate::media::images::pending(
                            crate::media::images::Kind::Preview,
                            &item.preview_url,
                        )
                };
                let shown = (video && frame.is_some()) || image.size().width > 0;
                if loading {
                    // Keep the indicator above the playback controls.
                    let area = if video {
                        egui::Rect::from_min_max(stage.min, stage.max - Vec2::new(0.0, 56.0))
                    } else {
                        stage
                    };
                    self.preview_loading(ui, area, shown);
                } else if !text_post && !shown {
                    ui.painter().text(
                        stage.center(),
                        egui::Align2::CENTER_CENTER,
                        self.tr("Preview unavailable"),
                        egui::FontId::proportional(typography::BODY),
                        Color32::from_gray(163),
                    );
                }
                if video {
                    self.preview_playback_controls(
                        ui,
                        egui::Rect::from_min_size(
                            stage.left_bottom() + Vec2::new(12.0, -52.0),
                            Vec2::new(stage.width() - 24.0, 42.0),
                        ),
                        item.media_type == "animated_gif",
                    );
                    if !playback.get_error().is_empty() {
                        let error = egui::Rect::from_center_size(
                            stage.center(),
                            Vec2::new(stage.width() - 128.0, 110.0),
                        );
                        ui.painter()
                            .rect_filled(error, 10, Color32::from_black_alpha(221));
                        let mut content = child(ui, "preview-playback-error", error.shrink(12.0));
                        self.setting_text(
                            &mut content,
                            &format!(
                                "{}\n{}",
                                self.tr("Video playback failed."),
                                playback.get_error()
                            ),
                            12.0,
                            false,
                            Color32::WHITE,
                        );
                        if self
                            .setting_action(
                                &mut content,
                                "Try again",
                                Some(Icon::Play),
                                1,
                                Vec2::new(130.0, 32.0),
                                true,
                            )
                            .clicked()
                        {
                            playback.invoke_retry();
                        }
                    }
                }
                for (label, icon, next, point) in [
                    (
                        "Previous",
                        Icon::ChevronLeft,
                        position.checked_sub(1),
                        stage.left_center() + Vec2::new(10.0, -21.0),
                    ),
                    (
                        "Next",
                        Icon::ChevronRight,
                        (position + 1 < items.row_count()).then_some(position + 1),
                        stage.right_center() + Vec2::new(-52.0, -21.0),
                    ),
                ] {
                    if let Some(next) = next {
                        let mut control = child(
                            ui,
                            label,
                            egui::Rect::from_min_size(
                                point,
                                Vec2::splat(appearance::NAVIGATION_SIZE),
                            ),
                        );
                        if self
                            .home_icon(
                                &mut control,
                                icon,
                                label,
                                appearance::NAVIGATION_SIZE,
                                IconStyle {
                                    idle: Color32::from_black_alpha(128),
                                    hover: Color32::from_black_alpha(199),
                                    ..self.preview_icon_style(26.0)
                                },
                            )
                            .clicked()
                        {
                            let index = items.row_data(next).unwrap().media_index;
                            app.set_selected_media_index(index);
                            app.invoke_load_media_preview(index);
                        }
                    }
                }
                if items.row_count() > 1 {
                    let text = format!("{} / {}", position + 1, items.row_count());
                    let galley = ui.painter().layout_no_wrap(
                        text,
                        egui::FontId::new(12.0, egui::FontFamily::Name("cera-bold".into())),
                        Color32::WHITE,
                    );
                    let counter = egui::Rect::from_min_size(
                        panel.min + Vec2::splat(14.0),
                        Vec2::new(galley.size().x + 20.0, 24.0),
                    );
                    ui.painter()
                        .rect_filled(counter, 12, Color32::from_black_alpha(140));
                    ui.painter().galley(
                        counter.center() - galley.size() / 2.0,
                        galley,
                        Color32::WHITE,
                    );
                }
                self.preview_view_controls(ui, stage, has_image, text_post);
                let mut closing = child(
                    ui,
                    "preview-close",
                    egui::Rect::from_min_size(
                        panel.right_top() + Vec2::new(-44.0, 10.0),
                        Vec2::splat(appearance::CONTROL_HEIGHT),
                    ),
                );
                close |= self
                    .home_icon(
                        &mut closing,
                        Icon::Close,
                        "Close preview",
                        appearance::CONTROL_HEIGHT,
                        if show_info {
                            IconStyle {
                                icon_size: 18.0,
                                radius: 17,
                                ..IconStyle::ghost(colors, false)
                            }
                        } else {
                            self.preview_icon_style(18.0)
                        },
                    )
                    .clicked();
                if show_info {
                    let details = egui::Rect::from_min_max(
                        egui::pos2(stage.right(), panel.top() + 54.0),
                        egui::pos2(
                            panel.right(),
                            panel.bottom() - (2.0 * appearance::CONTROL_HEIGHT + 40.0),
                        ),
                    );
                    let mut contents = child(ui, "preview-info", details);
                    let mut scroll = egui::ScrollArea::vertical()
                        .id_salt("preview-details")
                        .max_height(details.height())
                        .auto_shrink([false, false]);
                    if reset_scroll {
                        scroll = scroll.vertical_scroll_offset(0.0);
                    }
                    scroll.show_themed(&mut contents, |ui| {
                        egui::Frame::new()
                            .inner_margin(egui::Margin {
                                left: 16,
                                right: 16,
                                top: 0,
                                bottom: 16,
                            })
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing.y = 10.0;
                                self.preview_information(ui, item, text_post);
                            });
                    });
                    ui.painter().line_segment(
                        [
                            egui::pos2(stage.right(), panel.top()),
                            egui::pos2(stage.right(), panel.bottom()),
                        ],
                        egui::Stroke::new(1.0, colors.border),
                    );
                    ui.painter().line_segment(
                        [
                            egui::pos2(
                                stage.right() + 16.0,
                                panel.bottom() - (2.0 * appearance::CONTROL_HEIGHT + 40.0),
                            ),
                            egui::pos2(
                                panel.right() - 16.0,
                                panel.bottom() - (2.0 * appearance::CONTROL_HEIGHT + 40.0),
                            ),
                        ],
                        egui::Stroke::new(1.0, colors.border),
                    );
                    let footer = egui::Rect::from_min_max(
                        egui::pos2(
                            stage.right() + 16.0,
                            panel.bottom() - (2.0 * appearance::CONTROL_HEIGHT + 24.0),
                        ),
                        panel.right_bottom() - Vec2::splat(16.0),
                    );
                    let mut download = child(
                        ui,
                        "preview-download",
                        egui::Rect::from_min_size(
                            footer.min,
                            Vec2::new(footer.width(), appearance::CONTROL_HEIGHT),
                        ),
                    );
                    let library = app.get_selected_tab() == crate::library::TAB;
                    let label = if library && item.status == "Saved" {
                        self.tr("Open folder")
                    } else if library && item.status == "Missing" {
                        self.tr("Download again")
                    } else if item.status == "Saved" {
                        self.tr("Saved")
                    } else {
                        self.tr("Download {}").replace(
                            "{}",
                            if app.get_convert_gif() && item.media_type == "animated_gif" {
                                "GIF"
                            } else {
                                &item.file_extension
                            },
                        )
                    };
                    download.add_enabled_ui(
                        !self.busy() && (library || item.status != "Saved"),
                        |ui| {
                            if self
                                .action_button(
                                    ui,
                                    &label,
                                    ActionStyle {
                                        tone: ButtonTone::Primary,
                                        icon: Some((
                                            if library && item.status == "Saved" {
                                                Icon::Folder
                                            } else {
                                                Icon::Download
                                            },
                                            16.0,
                                        )),
                                        radius: 10,
                                        exact_width: true,
                                        ..ActionStyle::new(Vec2::new(
                                            footer.width(),
                                            appearance::CONTROL_HEIGHT,
                                        ))
                                    },
                                )
                                .clicked()
                            {
                                self.media_download_or_open(item);
                            }
                        },
                    );
                    let action_width = (footer.width() - 8.0) / 2.0;
                    for (label, icon, button_width, offset, enabled) in [
                        (
                            "View",
                            Icon::Eye,
                            action_width,
                            0.0,
                            !text_post && !item.url.is_empty(),
                        ),
                        (
                            "Open post",
                            Icon::ArrowRight,
                            action_width,
                            action_width + 8.0,
                            !item.tweet_url.is_empty(),
                        ),
                    ] {
                        let mut control = child(
                            ui,
                            label,
                            egui::Rect::from_min_size(
                                footer.left_top()
                                    + Vec2::new(offset, appearance::CONTROL_HEIGHT + 8.0),
                                Vec2::new(button_width, appearance::CONTROL_HEIGHT),
                            ),
                        );
                        control.add_enabled_ui(enabled, |ui| {
                            if self
                                .action_button(
                                    ui,
                                    &self.tr(label),
                                    ActionStyle {
                                        icon: Some((icon, 15.0)),
                                        radius: 10,
                                        exact_width: true,
                                        truncate_label: true,
                                        ..ActionStyle::new(Vec2::new(
                                            button_width,
                                            appearance::CONTROL_HEIGHT,
                                        ))
                                    },
                                )
                                .clicked()
                            {
                                if label == "View" {
                                    app.invoke_open_link(item.url.clone());
                                } else {
                                    app.invoke_open_post(item.tweet_url.clone());
                                }
                            }
                        });
                    }
                }
                if !fullscreen {
                    let caption = if show_info {
                        "Hide details"
                    } else {
                        "Show details"
                    };
                    let mut toggle = child(
                        ui,
                        "preview-details-toggle",
                        egui::Rect::from_min_size(
                            panel.right_top()
                                + Vec2::new(if show_info { -84.0 } else { -124.0 }, 10.0),
                            Vec2::splat(appearance::CONTROL_HEIGHT),
                        ),
                    );
                    if self
                        .home_icon(
                            &mut toggle,
                            if show_info {
                                Icon::ChevronRight
                            } else {
                                Icon::Info
                            },
                            caption,
                            appearance::CONTROL_HEIGHT,
                            if show_info {
                                IconStyle {
                                    icon_size: 18.0,
                                    radius: 17,
                                    ..IconStyle::ghost(colors, false)
                                }
                            } else {
                                self.preview_icon_style(18.0)
                            },
                        )
                        .clicked()
                    {
                        self.preview_canvas.info_hidden = !self.preview_canvas.info_hidden;
                        self.preview_canvas.reset();
                        ctx.request_repaint();
                    }
                    if show_info {
                        let title = egui::Rect::from_min_size(
                            egui::pos2(stage.right() + 16.0, panel.top() + 12.0),
                            Vec2::new(info_width - 110.0, 30.0),
                        );
                        self.history_text(
                            ui,
                            title,
                            self.tr("Post details"),
                            15.0,
                            true,
                            colors.heading,
                        );
                    }
                }
                if !fullscreen {
                    ui.painter().rect_stroke(
                        panel,
                        18,
                        egui::Stroke::new(1.0, colors.border),
                        egui::StrokeKind::Inside,
                    );
                }
            });
        if modal.is_top_modal {
            self.preview_view_shortcuts(ctx, has_image);
        }
        close |= !fullscreen && modal.backdrop_response.clicked();
        if modal.is_top_modal
            && !egui::Popup::is_any_open(ctx)
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            if self.preview_canvas.fullscreen {
                self.preview_canvas.leave_fullscreen(ctx);
            } else {
                close = true;
            }
        }
        if close {
            self.preview_texture = Default::default();
            self.preview_canvas.leave_fullscreen(ctx);
            self.preview_canvas.reset();
            self.preview_canvas.info_hidden = false;
            self.preview_index = None;
            app.set_selected_media_index(-1);
            return;
        }
        if modal.is_top_modal && !ctx.text_edit_focused() && !egui::Popup::is_any_open(ctx) {
            for (key, next) in [
                (egui::Key::ArrowLeft, position.checked_sub(1)),
                (
                    egui::Key::ArrowRight,
                    (position + 1 < items.row_count()).then_some(position + 1),
                ),
            ] {
                if let Some(next) = next {
                    if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, key)) {
                        let index = items.row_data(next).unwrap().media_index;
                        app.set_selected_media_index(index);
                        app.invoke_load_media_preview(index);
                    }
                }
            }
            if !ctx.egui_wants_keyboard_input()
                && video
                && app.global::<PreviewPlayback>().get_ready()
                && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Space))
            {
                app.global::<PreviewPlayback>().invoke_toggle();
            }
        }
    }

    pub(in crate::ui) fn preview_icon_style(&self, size: f32) -> IconStyle {
        IconStyle {
            idle: Color32::from_black_alpha(140),
            hover: Color32::from_black_alpha(204),
            pressed: Color32::from_black_alpha(230),
            foreground: Color32::WHITE,
            border: egui::Stroke::NONE,
            radius: (appearance::NAVIGATION_SIZE / 2.0) as u8,
            icon_size: size.min(appearance::NAVIGATION_ICON_SIZE),
            visible: true,
        }
    }

    fn preview_playback_controls(&self, ui: &mut Ui, rect: egui::Rect, looping: bool) {
        let playback = self.app.global::<PreviewPlayback>();
        ui.painter()
            .rect_filled(rect, 8, Color32::from_black_alpha(199));
        let mut play = child(
            ui,
            "preview-play",
            egui::Rect::from_min_size(rect.min + Vec2::new(6.0, 5.0), Vec2::splat(32.0)),
        );
        let style = IconStyle {
            idle: Color32::TRANSPARENT,
            hover: Color32::from_white_alpha(32),
            radius: 6,
            ..self.preview_icon_style(appearance::CONTROL_ICON_SIZE)
        };
        play.add_enabled_ui(
            playback.get_ready() && playback.get_error().is_empty(),
            |ui| {
                if self
                    .home_icon(
                        ui,
                        if playback.get_playing() {
                            Icon::Pause
                        } else {
                            Icon::Play
                        },
                        if playback.get_playing() {
                            "Pause"
                        } else {
                            "Play"
                        },
                        32.0,
                        style,
                    )
                    .clicked()
                {
                    playback.invoke_toggle();
                }
            },
        );
        let position = playback.get_position_text();
        let duration = playback.get_duration_text();
        let measure = |text: &str| {
            ui.painter()
                .layout_no_wrap(
                    text.to_owned(),
                    egui::FontId::proportional(12.0),
                    Color32::WHITE,
                )
                .size()
                .x
        };
        let position_width = measure(&position);
        let duration_width = measure(&duration);
        let position_rect = egui::Rect::from_min_size(
            rect.min + Vec2::new(46.0, 0.0),
            Vec2::new(position_width, 42.0),
        );
        self.history_text(ui, position_rect, position, 12.0, false, Color32::WHITE);
        let trailing_width = if looping { 24.0 } else { 32.0 + 8.0 + 76.0 };
        let duration_rect = egui::Rect::from_min_size(
            rect.right_top() - Vec2::new(10.0 + trailing_width + 8.0 + duration_width, 0.0),
            Vec2::new(duration_width, 42.0),
        );
        let seek = egui::Rect::from_min_max(
            position_rect.right_top() + Vec2::new(8.0, 5.0),
            duration_rect.left_bottom() - Vec2::new(8.0, 5.0),
        );
        let mut value = playback.get_position();
        if self
            .control_slider(
                ui,
                seek,
                "Playback position",
                &mut value,
                playback.get_duration().max(1.0),
                0.1,
                playback.get_ready() && playback.get_can_seek(),
            )
            .changed()
        {
            playback.invoke_seek(value);
        }
        self.history_text(
            ui,
            duration_rect,
            duration,
            12.0,
            false,
            Color32::from_gray(212),
        );
        if looping {
            let icon = egui::Rect::from_center_size(
                rect.right_center() - Vec2::new(26.0, 0.0),
                Vec2::splat(16.0),
            );
            Icon::Repeat
                .image(Color32::from_gray(212), 16.0)
                .paint_at(ui, icon);
            ui.interact(icon, ui.id().with("looping-gif"), egui::Sense::hover())
                .widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Image, true, self.tr("Looping GIF"))
                });
        } else {
            let mut sound = child(
                ui,
                "preview-sound",
                egui::Rect::from_center_size(
                    rect.right_center() - Vec2::new(110.0, 0.0),
                    Vec2::splat(32.0),
                ),
            );
            sound.add_enabled_ui(playback.get_ready(), |ui| {
                if self
                    .home_icon(
                        ui,
                        if playback.get_muted() || playback.get_volume() == 0.0 {
                            Icon::Muted
                        } else {
                            Icon::Volume
                        },
                        if playback.get_muted() {
                            "Unmute"
                        } else {
                            "Mute"
                        },
                        32.0,
                        IconStyle {
                            icon_size: appearance::CONTROL_ICON_SIZE,
                            ..style
                        },
                    )
                    .clicked()
                {
                    playback.invoke_mute(!playback.get_muted());
                }
            });
            let volume = egui::Rect::from_center_size(
                rect.right_center() - Vec2::new(48.0, 0.0),
                Vec2::new(76.0, 32.0),
            );
            let mut value = playback.get_volume();
            if self
                .control_slider(
                    ui,
                    volume,
                    "Volume",
                    &mut value,
                    1.0,
                    0.05,
                    playback.get_ready(),
                )
                .changed()
            {
                playback.invoke_change_volume(value);
            }
        }
    }

    fn preview_information(&mut self, ui: &mut Ui, item: &MediaItem, text_post: bool) {
        let colors = self.colors(ui);
        let (header, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), egui::Sense::hover());
        let avatar = egui::Rect::from_min_size(header.min, Vec2::splat(40.0));
        ui.painter()
            .circle_filled(avatar.center(), 20.0, colors.surface);
        let profile = self.app.get_account_profile();
        if item.author_avatar_image.size().width > 0 {
            self.paint_media(
                ui,
                &item.author_avatar_image,
                avatar,
                if item.author_avatar_square { 6 } else { 20 },
            );
        } else if profile.has_avatar && profile.username.eq_ignore_ascii_case(&item.author_handle) {
            self.paint_media(ui, &profile.avatar_image, avatar, 20);
        } else {
            Icon::User.image(colors.secondary, 18.0).paint_at(
                ui,
                egui::Rect::from_center_size(avatar.center(), Vec2::splat(18.0)),
            );
        }
        let mut name_job = egui::text::LayoutJob::simple(
            item.author_name.clone(),
            egui::FontId::new(14.04, egui::FontFamily::Name("cera-bold".into())),
            colors.heading,
            (header.width() - 144.0).max(1.0),
        );
        name_job.wrap.max_rows = 1;
        name_job.wrap.break_anywhere = true;
        let name = ui.painter().layout_job(name_job);
        ui.painter().galley(
            header.min + Vec2::new(50.0, (20.0 - name.size().y) / 2.0),
            name.clone(),
            colors.heading,
        );
        if item.author_verified || (profile.verified && profile.username == item.author_handle) {
            Icon::Verified.image(colors.accent, 13.0).paint_at(
                ui,
                egui::Rect::from_min_size(
                    header.min + Vec2::new(55.0 + name.size().x, 3.5),
                    Vec2::splat(13.0),
                ),
            );
        }
        self.history_text(
            ui,
            egui::Rect::from_min_size(
                header.min + Vec2::new(50.0, 22.0),
                Vec2::new(header.width() - 100.0, 17.0),
            ),
            format!("@{}", item.author_handle),
            12.0,
            false,
            colors.secondary,
        );
        let (kind, tint) = match item.media_type.as_str() {
            "photo" => ("Image", Color32::from_rgb(59, 130, 246)),
            "animated_gif" => ("GIF", Color32::from_rgb(168, 85, 247)),
            "video" => ("Video", Color32::from_rgb(239, 68, 68)),
            _ => ("Text", Color32::from_rgb(16, 185, 129)),
        };
        let kind = ui.painter().layout_no_wrap(
            self.tr(kind),
            egui::FontId::new(9.96, egui::FontFamily::Name("cera-bold".into())),
            Color32::WHITE,
        );
        let badge = egui::Rect::from_min_size(
            header.right_top() + Vec2::new(-kind.size().x - 16.0, 0.0),
            Vec2::new(kind.size().x + 16.0, 22.0),
        );
        ui.painter().rect_filled(badge, 11, tint);
        ui.painter()
            .galley(badge.center() - kind.size() / 2.0, kind, Color32::WHITE);
        if !text_post && !item.tweet_text.is_empty() {
            self.linked_text(ui, &item.tweet_text, 14.04, colors.heading);
        }
        if item.is_repost {
            self.preview_metadata(
                ui,
                Icon::Repost,
                &format!("{} @{}", self.tr("Reposted by"), item.reposted_by),
            );
        }
        if !item.alt_text.is_empty() {
            ui.scope(|ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                self.setting_text(ui, "Alt text", 12.0, true, colors.secondary);
                self.linked_text(ui, &item.alt_text, 14.04, colors.heading);
            });
        }
        self.setting_divider(ui);
        ui.scope(|ui| {
            ui.spacing_mut().interact_size.y = 20.0;
            ui.spacing_mut().item_spacing = Vec2::new(16.0, 8.0);
            let metric_width = ((ui.available_width() - 32.0) / 3.0).floor();
            ui.horizontal_wrapped(|ui| {
                for (icon, label, value) in [
                    (Icon::Reply, "Replies", &item.replies),
                    (Icon::Repost, "Reposts", &item.reposts),
                    (Icon::Heart, "Likes", &item.likes),
                    (Icon::Eye, "Views", &item.views),
                    (Icon::Quote, "Quotes", &item.quotes),
                    (Icon::Bookmark, "Bookmarks", &item.bookmarks),
                ] {
                    let width = ui
                        .painter()
                        .layout_no_wrap(
                            value.clone(),
                            egui::FontId::proportional(12.0),
                            colors.secondary,
                        )
                        .size()
                        .x;
                    let (rect, response) = ui.allocate_exact_size(
                        Vec2::new(metric_width.max(19.0 + width), 20.0),
                        egui::Sense::hover(),
                    );
                    icon.image(colors.secondary, 14.0).paint_at(
                        ui,
                        egui::Rect::from_center_size(
                            rect.left_center() + Vec2::new(7.0, 0.0),
                            Vec2::splat(14.0),
                        ),
                    );
                    self.history_text(
                        ui,
                        egui::Rect::from_min_max(rect.min + Vec2::new(19.0, 0.0), rect.max),
                        value.clone(),
                        12.0,
                        false,
                        colors.secondary,
                    );
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Label,
                            true,
                            format!("{}: {value}", self.tr(label)),
                        )
                    });
                }
            });
        });
        self.setting_divider(ui);
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 5.0;
            for (icon, text) in [
                (Icon::Calendar, item.preview_date.clone()),
                (
                    Icon::Info,
                    if item.source.is_empty() {
                        String::new()
                    } else {
                        self.tr("via {}").replace("{}", &item.source)
                    },
                ),
                (Icon::Photo, item.dimensions.clone()),
            ] {
                if !text.is_empty() {
                    self.preview_metadata(ui, icon, &text);
                }
            }
        });
        if matches!(item.status.as_str(), "Saved" | "Failed") {
            self.setting_text(
                ui,
                &item.status,
                12.0,
                false,
                if item.status == "Saved" {
                    Color32::from_rgb(21, 128, 61)
                } else {
                    Color32::from_rgb(183, 55, 49)
                },
            );
        }
    }

    fn preview_metadata(&self, ui: &mut Ui, icon: Icon, text: &str) {
        let colors = self.colors(ui);
        let mut job = egui::text::LayoutJob::simple(
            text.to_owned(),
            egui::FontId::proportional(12.0),
            colors.secondary,
            (ui.available_width() - 20.0).max(1.0),
        );
        job.sections[0].format.line_height = Some(17.0);
        let galley = ui.painter().layout_job(job);
        let (row, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), galley.size().y.max(17.0)),
            egui::Sense::hover(),
        );
        icon.image(colors.secondary, 13.0).paint_at(
            ui,
            egui::Rect::from_center_size(
                row.left_center() + Vec2::new(6.5, 0.0),
                Vec2::splat(13.0),
            ),
        );
        ui.painter()
            .galley(row.min + Vec2::new(20.0, 0.0), galley, colors.secondary);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, text));
    }
}
