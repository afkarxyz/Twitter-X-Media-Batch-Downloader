use crate::ui::components::dialog::child;
use crate::ui::components::icon_button::IconStyle;
use crate::ui::*;

impl Desktop {
    fn history_back(&self, ui: &mut Ui, rect: egui::Rect, enabled: bool) -> bool {
        let colors = self.colors(ui);
        let mut back = child(ui, "history-back", rect);
        if !enabled {
            back.disable();
        }
        self.home_icon(
            &mut back,
            Icon::Back,
            "Back",
            40.0,
            IconStyle {
                idle: colors.surface,
                hover: colors.control,
                pressed: colors.muted_control,
                foreground: colors.heading,
                border: egui::Stroke::new(1.0, colors.input_border),
                radius: 20,
                icon_size: appearance::NAVIGATION_ICON_SIZE,
                visible: true,
            },
        )
        .clicked()
    }

    pub(in crate::ui) fn fetch_session_details(&mut self, ui: &mut Ui) {
        let app = self.app.clone();
        let bounds = ui.max_rect();
        let colors = self.colors(ui);
        let header = egui::Rect::from_min_size(bounds.min, Vec2::new(bounds.width(), 44.0));
        if self.history_back(
            ui,
            egui::Rect::from_min_size(header.min + Vec2::new(0.0, 2.0), Vec2::splat(40.0)),
            true,
        ) {
            app.set_fetch_sessions_key(String::new());
        }
        self.history_title(
            ui,
            egui::Rect::from_min_max(header.min + Vec2::new(52.0, 0.0), header.max),
            format!(
                "{} · {}",
                app.get_fetch_sessions_title(),
                self.tr("Fetch sessions")
            ),
        );
        let label = crate::ui::components::dialog::galley(
            ui.ctx(),
            app.get_fetch_sessions_label(),
            bounds.width(),
            14.0,
            false,
            colors.secondary,
        );
        ui.painter().galley(
            bounds.min + Vec2::new(0.0, 60.0),
            label.clone(),
            colors.secondary,
        );
        let list = egui::Rect::from_min_max(
            bounds.min + Vec2::new(0.0, 82.0 + label.size().y),
            bounds.max,
        );
        let mut content = child(ui, "fetch-session-list", list);
        egui::ScrollArea::vertical()
            .id_salt("fetch-sessions")
            .max_height(list.height())
            .scroll_bar_rect(list.shrink2(Vec2::new(0.0, 16.0)))
            .auto_shrink([false, false])
            .show_themed(&mut content, |ui| {
                let rows = app.get_fetch_sessions_items();
                let width = ui.available_width();
                let (grid, _) = ui.allocate_exact_size(
                    Vec2::new(width, rows.row_count() as f32 * 80.0),
                    egui::Sense::hover(),
                );
                for (index, row) in rows.iter().enumerate() {
                    self.history_card(
                        ui,
                        egui::Rect::from_min_size(
                            grid.min + Vec2::new(0.0, index as f32 * 80.0),
                            Vec2::new(width, 70.0),
                        ),
                        &row,
                        true,
                    );
                }
            });
    }

    pub(in crate::ui) fn download_details(&mut self, ui: &mut Ui) {
        let app = self.app.clone();
        let colors = self.colors(ui);
        let bounds = ui.max_rect();
        let enabled = !self.busy();
        let header = egui::Rect::from_min_size(bounds.min, Vec2::new(bounds.width(), 44.0));
        if self.history_back(
            ui,
            egui::Rect::from_min_size(header.min + Vec2::new(0.0, 2.0), Vec2::splat(40.0)),
            enabled,
        ) {
            app.update_library(|data| data.files_view = false);
        }
        let painter = ui.painter().clone();
        let action_width = |label: &str| {
            painter
                .layout_no_wrap(
                    self.tr(label),
                    egui::FontId::new(
                        typography::BODY,
                        egui::FontFamily::Name("cera-medium".into()),
                    ),
                    colors.heading,
                )
                .size()
                .x
                + 47.0
        };
        let folder_width = action_width("Open folder").max(116.0);
        let check_width = action_width("Check files").max(116.0);
        let folder = egui::Rect::from_min_size(
            egui::pos2(header.right() - folder_width, header.top() + 6.0),
            Vec2::new(folder_width, 32.0),
        );
        let check = egui::Rect::from_min_size(
            egui::pos2(folder.left() - 12.0 - check_width, folder.top()),
            Vec2::new(check_width, 32.0),
        );
        if self
            .history_button(
                ui,
                folder,
                &self.tr("Open folder"),
                Some(Icon::Folder),
                false,
                enabled && !app.get_download_batch_folder().is_empty(),
            )
            .clicked()
        {
            app.invoke_open_download_path(app.get_download_batch_folder(), true);
        }
        if self
            .history_button(
                ui,
                check,
                &self.tr("Check files"),
                Some(Icon::Verified),
                false,
                enabled,
            )
            .clicked()
        {
            app.invoke_library_refresh();
        }
        self.history_title(
            ui,
            egui::Rect::from_min_max(
                header.min + Vec2::new(52.0, 0.0),
                egui::pos2(check.left() - 12.0, header.bottom()),
            ),
            app.get_download_batch_title(),
        );
        let mut y = header.bottom() + 16.0;
        self.history_text(
            ui,
            egui::Rect::from_min_size(
                egui::pos2(bounds.left(), y),
                Vec2::new(bounds.width(), 20.0),
            ),
            format!(
                "{} · {}",
                self.tr("{} available")
                    .replace("{}", &app.get_library().available.to_string()),
                app.get_download_batch_size()
            ),
            14.0,
            true,
            colors.heading,
        );
        y += 27.0;
        self.history_text(
            ui,
            egui::Rect::from_min_size(
                egui::pos2(bounds.left(), y),
                Vec2::new(bounds.width(), 17.0),
            ),
            app.get_download_batch_folder(),
            12.0,
            false,
            colors.secondary,
        );
        y += 38.0;
        if app.get_archive_conversion_busy() {
            let rect = egui::Rect::from_min_size(
                egui::pos2(bounds.left(), y),
                Vec2::new(bounds.width(), 56.0),
            );
            let progress_rect =
                egui::Rect::from_min_max(rect.min, rect.max - Vec2::new(128.0, 0.0));
            let mut progress = child(ui, "conversion-progress", progress_rect);
            progress.label(app.get_library().status);
            self.progress_bar(&mut progress, app.get_library().progress);
            self.setting_caption(&mut progress, &app.get_library().progress_detail);
            let stop = egui::Rect::from_min_size(
                rect.right_top() - Vec2::new(116.0, -12.0),
                Vec2::new(116.0, 32.0),
            );
            if self
                .history_button(ui, stop, &self.tr("Stop"), None, false, true)
                .clicked()
            {
                app.invoke_cancel_operation();
            }
            y += 72.0;
        } else if app.get_archive_gif_count() > 0 {
            let width = action_width("Convert GIFs").max(116.0);
            let rect =
                egui::Rect::from_min_size(egui::pos2(bounds.left(), y), Vec2::new(width, 32.0));
            if self
                .history_button(
                    ui,
                    rect,
                    &self.tr("Convert GIFs"),
                    Some(Icon::Film),
                    false,
                    enabled,
                )
                .clicked()
            {
                app.set_archive_conversion_open(true);
            }
            self.history_text(
                ui,
                egui::Rect::from_min_max(
                    egui::pos2(rect.right() + 12.0, y),
                    egui::pos2(bounds.right(), y + 32.0),
                ),
                self.tr("{} GIF MP4 files")
                    .replace("{}", &app.get_archive_gif_count().to_string()),
                12.0,
                false,
                colors.secondary,
            );
            y += 48.0;
        }
        let list = egui::Rect::from_min_max(egui::pos2(bounds.left(), y), bounds.max);
        let mut content = child(ui, "download-files", list);
        egui::ScrollArea::vertical()
            .id_salt("download-files")
            .max_height(list.height())
            .scroll_bar_rect(list.shrink2(Vec2::new(0.0, 16.0)))
            .auto_shrink([false, false])
            .show_themed(&mut content, |ui| {
                let files = app.get_download_batch_files();
                let width = ui.available_width();
                let (grid, _) = ui.allocate_exact_size(
                    Vec2::new(width, files.row_count() as f32 * 86.0),
                    egui::Sense::hover(),
                );
                let first = (((ui.clip_rect().top() - grid.top()) / 86.0)
                    .floor()
                    .max(0.0) as usize)
                    .saturating_sub(1)
                    .min(files.row_count());
                let last = ((((ui.clip_rect().bottom() - grid.top()) / 86.0)
                    .ceil()
                    .max(0.0) as usize)
                    + 1)
                .min(files.row_count());
                for index in first..last {
                    let file = files.row_data(index).unwrap();
                    let rect = egui::Rect::from_min_size(
                        grid.min + Vec2::new(0.0, index as f32 * 86.0),
                        Vec2::new(width, 76.0),
                    );
                    ui.painter().rect_filled(rect, 12, colors.surface);
                    ui.painter().rect_stroke(
                        rect,
                        12,
                        egui::Stroke::new(1.0, colors.border),
                        egui::StrokeKind::Inside,
                    );
                    let icon = match file.kind.as_str() {
                        "photo" | "image" => Icon::Photo,
                        "video" => Icon::Video,
                        "film" | "animated_gif" => Icon::Film,
                        _ => Icon::Text,
                    };
                    icon.image(colors.secondary, 22.0).paint_at(
                        ui,
                        egui::Rect::from_center_size(
                            egui::pos2(rect.left() + 25.0, rect.center().y),
                            Vec2::splat(22.0),
                        ),
                    );
                    let folder = egui::Rect::from_min_size(
                        egui::pos2(rect.right() - 14.0 - folder_width, rect.center().y - 16.0),
                        Vec2::new(folder_width, 32.0),
                    );
                    let open_width = action_width(if file.present {
                        "Open file"
                    } else {
                        "Download again"
                    })
                    .max(116.0);
                    let open = egui::Rect::from_min_size(
                        egui::pos2(folder.left() - 14.0 - open_width, folder.top()),
                        Vec2::new(open_width, 32.0),
                    );
                    let mut row_ui = child(ui, (&file.path, index), rect);
                    let folder_response = self.history_button(
                        &mut row_ui,
                        folder,
                        &self.tr("Open folder"),
                        Some(Icon::Folder),
                        false,
                        enabled && !file.folder.is_empty(),
                    );
                    folder_response.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Button,
                            enabled && !file.folder.is_empty(),
                            format!("{} · {}", self.tr("Open folder"), file.filename),
                        )
                    });
                    if folder_response.clicked() {
                        app.invoke_open_download_path(file.folder.clone(), true);
                    }
                    let open_label = if file.present {
                        "Open file"
                    } else {
                        "Download again"
                    };
                    let open_response = self.history_button(
                        &mut row_ui,
                        open,
                        &self.tr(open_label),
                        Some(if file.present {
                            Icon::External
                        } else {
                            Icon::Download
                        }),
                        false,
                        enabled,
                    );
                    open_response.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Button,
                            enabled,
                            format!("{} · {}", self.tr(open_label), file.filename),
                        )
                    });
                    if open_response.clicked() {
                        if file.present {
                            app.invoke_open_download_path(file.path.clone(), false);
                        } else {
                            app.invoke_library_download(file.media_index);
                        }
                    }
                    let text = egui::Rect::from_min_max(
                        egui::pos2(rect.left() + 50.0, rect.center().y - 20.0),
                        egui::pos2(open.left() - 14.0, rect.center().y + 20.0),
                    );
                    self.history_text(
                        ui,
                        egui::Rect::from_min_size(text.min, Vec2::new(text.width(), 20.0)),
                        file.filename.clone(),
                        14.0,
                        true,
                        colors.heading,
                    );
                    let description = if file.path.is_empty() {
                        self.tr("Not downloaded")
                    } else if file.present {
                        file.size_text.clone()
                    } else {
                        self.tr(if file.integrity_state.is_empty() {
                            "File missing"
                        } else {
                            &file.integrity_state
                        })
                    };
                    self.history_text(
                        ui,
                        egui::Rect::from_min_max(
                            text.min + Vec2::new(0.0, 24.0),
                            text.max + Vec2::new(0.0, 1.0),
                        ),
                        format!("{description} · {}", file.folder),
                        12.0,
                        false,
                        if file.present {
                            colors.secondary
                        } else {
                            ui.visuals().error_fg_color
                        },
                    );
                }
            });
    }
}
