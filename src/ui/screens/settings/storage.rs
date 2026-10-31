use crate::ui::*;

impl Desktop {
    pub(in crate::ui) fn storage_settings(&mut self, ui: &mut Ui) {
        self.setting_card(ui, "Download folder", 10.0, |this, ui| {
            let app = this.app.clone();
            this.setting_heading(ui, "Download folder");
            ui.add_space(1.0);
            if this
                .setting_action(
                    ui,
                    "Use portable Downloads folder",
                    Some(Icon::Reset),
                    2,
                    Vec2::new(100.0, 32.0),
                    !this.busy(),
                )
                .clicked()
            {
                this.reset_setting_inputs(3);
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.vertical(|ui| {
                    ui.set_width((ui.available_width() - 216.0).max(100.0));
                    let mut folder = app.get_output_dir();
                    this.setting_input(ui, ("Download folder", ""), &mut folder, true);
                });
                if this
                    .setting_action(
                        ui,
                        "Browse",
                        Some(Icon::FolderPlus),
                        0,
                        Vec2::new(100.0, appearance::CONTROL_HEIGHT),
                        true,
                    )
                    .clicked()
                {
                    app.invoke_browse_output_folder();
                }
                if this
                    .setting_action(
                        ui,
                        "Open",
                        Some(Icon::Folder),
                        0,
                        Vec2::new(100.0, appearance::CONTROL_HEIGHT),
                        true,
                    )
                    .clicked()
                {
                    app.invoke_open_output_folder();
                }
            });
            // A periodic refresh keeps the previous figures until the new ones arrive.
            let calculating = app.get_storage_busy() && !app.get_storage_ready();
            let used = if calculating {
                this.tr("Calculating…")
            } else {
                app.get_storage_used()
            };
            let free = if calculating {
                this.tr("Calculating…")
            } else {
                app.get_storage_free()
            };
            let files = this
                .tr("{} files")
                .replace("{}", &app.get_storage_files().to_string());
            let total = if app.get_storage_capacity_known() {
                this.tr("{} total on drive")
                    .replace("{}", &app.get_storage_total())
            } else if calculating {
                String::new()
            } else {
                this.tr("Storage capacity unavailable")
            };
            let pref = |title: &str, value: &str, detail: &str| -> f32 {
                [
                    (this.tr(title), 12.0, false),
                    (value.to_owned(), 17.04, true),
                    (detail.to_owned(), 12.0, false),
                ]
                .into_iter()
                .map(|(text, size, bold)| {
                    ui.painter()
                        .layout_no_wrap(
                            text,
                            egui::FontId::new(
                                size,
                                if bold {
                                    egui::FontFamily::Name("cera-bold".into())
                                } else {
                                    egui::FontFamily::Proportional
                                },
                            ),
                            this.colors(ui).secondary,
                        )
                        .size()
                        .x
                })
                .fold(0.0, f32::max)
                    + 24.0
            };
            let left_pref = pref("Download folder usage", &used, &files);
            let right_pref = pref("Free space", &free, &total);
            let width = ((ui.available_width() - 12.0 + left_pref - right_pref) / 2.0)
                .clamp(120.0, ui.available_width() - 132.0);
            let other_width = ui.available_width() - 12.0 - width;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                this.storage_metric(ui, width, "Download folder usage", used, files);
                this.storage_metric(ui, other_width, "Free space", free, total);
            });
            this.storage_capacity(ui);
            ui.add_space(1.0);
            let (row, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), egui::Sense::hover());
            let mut refresh = crate::ui::components::dialog::child(
                ui,
                "storage-refresh",
                egui::Rect::from_min_size(
                    row.right_top() - Vec2::new(100.0, 0.0),
                    Vec2::new(100.0, 32.0),
                ),
            );
            if this
                .setting_action(
                    &mut refresh,
                    "Refresh",
                    Some(Icon::Restart),
                    0,
                    Vec2::new(100.0, 32.0),
                    !calculating,
                )
                .clicked()
            {
                app.invoke_refresh_storage();
            }
            let mut caption = crate::ui::components::dialog::child(
                ui,
                "storage-counts",
                egui::Rect::from_min_max(row.min, row.right_bottom() - Vec2::new(112.0, 0.0)),
            );
            this.setting_caption(
                &mut caption,
                if app.get_storage_partial() {
                    "Some files could not be read."
                } else {
                    "Counts final files in the selected folder, including subfolders."
                },
            );
        });
        self.setting_card(ui,"Data and file checks",12.0,|this,ui|{
            let app=this.app.clone();this.setting_heading(ui,"Data and file checks");
            this.setting_caption(ui,"Back up history, account labels and download records. Downloaded media and auth tokens are not included.");
            ui.horizontal(|ui|{
                if this.setting_action(ui,"Check downloads",Some(Icon::Check),0,Vec2::new(100.0,32.0),!this.busy()).clicked(){app.set_download_batch_id(String::new());app.invoke_check_downloads(false);}
                if this.setting_action(ui,"Backup database",Some(Icon::Save),0,Vec2::new(100.0,32.0),!this.busy()&&!app.get_backup_busy()).clicked(){app.invoke_backup_database();}
                if this.setting_action(ui,"Restore backup",Some(Icon::Upload),2,Vec2::new(100.0,32.0),!this.busy()&&!app.get_backup_busy()).clicked(){app.invoke_choose_database_backup();}
            });if app.get_backup_busy(){this.setting_caption(ui,"Working on database backup…");}this.setting_error(ui,&app.get_backup_error());
        });
    }

    fn storage_metric(&self, ui: &mut Ui, width: f32, title: &str, value: String, detail: String) {
        let colors = self.colors(ui);
        let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 84.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 8, colors.surface);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Other, true, format!("{title} metric"))
        });
        self.history_text(
            ui,
            egui::Rect::from_min_size(
                rect.min + Vec2::new(12.0, 12.0),
                Vec2::new(width - 24.0, 17.0),
            ),
            self.tr(title),
            12.0,
            false,
            colors.secondary,
        );
        self.history_text(
            ui,
            egui::Rect::from_min_size(
                rect.min + Vec2::new(12.0, 33.0),
                Vec2::new(width - 24.0, 24.0),
            ),
            value,
            17.0,
            true,
            colors.heading,
        );
        self.history_text(
            ui,
            egui::Rect::from_min_size(
                rect.min + Vec2::new(12.0, 61.0),
                Vec2::new(width - 24.0, 17.0),
            ),
            detail,
            12.0,
            false,
            colors.secondary,
        );
    }

    fn storage_capacity(&self, ui: &mut Ui) {
        let app = &self.app;
        let colors = self.colors(ui);
        let dark = ui.visuals().dark_mode;
        let known = app.get_storage_capacity_known();
        let busy = app.get_storage_busy() && !app.get_storage_ready();
        let downloads = swatches::color("amber", if dark { 4 } else { 6 });
        let other = swatches::color("zinc", if dark { 5 } else { 4 });
        let free = swatches::color("zinc", if dark { 8 } else { 2 });
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 10.0;
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), 21.0),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    if known {
                        ui.label(
                            RichText::new(self.tr("{}% used").replace(
                                "{}",
                                &format!("{:.0}", app.get_storage_used_ratio() * 100.0),
                            ))
                            .size(12.0)
                            .color(colors.secondary),
                        );
                    }
                    self.setting_text(ui, "Drive storage", 14.0, true, colors.heading);
                },
            );
            let (bar, response) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 14.0), egui::Sense::hover());
            ui.painter().rect_filled(
                bar,
                7,
                if known {
                    free
                } else {
                    appearance::mix(free, colors.surface, 0.45)
                },
            );
            if known {
                let used = app.get_storage_used_ratio().clamp(0.0, 1.0);
                let ratio = app.get_storage_downloads_ratio().clamp(0.0, used);
                for (end, color) in [(used, other), (ratio, downloads)] {
                    if end > 0.0 {
                        let clip = egui::Rect::from_min_size(
                            bar.min,
                            Vec2::new(bar.width() * end, bar.height()),
                        );
                        ui.painter()
                            .with_clip_rect(clip.intersect(ui.clip_rect()))
                            .rect_filled(bar, 7, color);
                    }
                }
            }
            response.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Image,
                    true,
                    format!(
                        "{}: {} {}, {} {}, {} {}",
                        self.tr("Drive storage"),
                        self.tr("Downloads"),
                        app.get_storage_used(),
                        self.tr("Other files"),
                        app.get_storage_other_used(),
                        self.tr("Free space"),
                        app.get_storage_free()
                    ),
                )
            });
            let legends = [
                ("Downloads", app.get_storage_used(), downloads),
                (
                    "Other files",
                    if known {
                        app.get_storage_other_used()
                    } else {
                        "—".into()
                    },
                    other,
                ),
                ("Free space", app.get_storage_free(), free),
            ];
            let preferred = legends
                .iter()
                .map(|(label, value, _)| {
                    let label = ui
                        .painter()
                        .layout_no_wrap(
                            self.tr(label),
                            egui::FontId::proportional(12.0),
                            colors.secondary,
                        )
                        .size()
                        .x;
                    let value = ui
                        .painter()
                        .layout_no_wrap(
                            if busy {
                                self.tr("Calculating…")
                            } else {
                                value.clone()
                            },
                            egui::FontId::new(12.0, egui::FontFamily::Name("cera-bold".into())),
                            colors.heading,
                        )
                        .size()
                        .x;
                    label.max(value) + 16.0
                })
                .collect::<Vec<_>>();
            let surplus = (ui.available_width() - 24.0 - preferred.iter().sum::<f32>()) / 3.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                for (index, (label, value, marker)) in legends.into_iter().enumerate() {
                    let width = (preferred[index] + surplus).max(32.0);
                    let (rect, _) =
                        ui.allocate_exact_size(Vec2::new(width, 40.0), egui::Sense::hover());
                    ui.painter()
                        .circle_filled(rect.min + Vec2::new(4.5, 7.5), 4.5, marker);
                    self.history_text(
                        ui,
                        egui::Rect::from_min_size(
                            rect.min + Vec2::new(16.0, 0.0),
                            Vec2::new(width - 16.0, 17.0),
                        ),
                        self.tr(label),
                        12.0,
                        false,
                        colors.secondary,
                    );
                    self.history_text(
                        ui,
                        egui::Rect::from_min_size(
                            rect.min + Vec2::new(16.0, 21.0),
                            Vec2::new(width - 16.0, 17.0),
                        ),
                        if busy {
                            self.tr("Calculating…")
                        } else {
                            value
                        },
                        12.0,
                        true,
                        colors.heading,
                    );
                }
            });
            self.setting_caption(
                ui,
                &if known {
                    self.tr("{} total on drive")
                        .replace("{}", &app.get_storage_total())
                } else if busy {
                    self.tr("Calculating…")
                } else {
                    self.tr("Storage capacity unavailable")
                },
            );
        });
    }

    pub(in crate::ui) fn about(&mut self, ui: &mut Ui) {
        if self.about_logo.is_none() {
            let image =
                image::load_from_memory(include_bytes!(concat!(env!("OUT_DIR"), "/logo-ui.png")))
                    .expect("embedded application logo is valid")
                    .to_rgba8();
            self.about_logo = Some(ui.ctx().load_texture(
                "about-logo",
                egui::ColorImage::from_rgba_unmultiplied(
                    [image.width() as usize, image.height() as usize],
                    image.as_raw(),
                ),
                egui::TextureOptions::LINEAR,
            ));
        }
        egui::Frame::new().inner_margin(12).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 12.0;
            let (header, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 44.0), egui::Sense::hover());
            self.history_title(ui, header, self.tr("About"));
            self.setting_card(ui, "About", 10.0, |this, ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(48.0), egui::Sense::hover());
                egui::Image::new(this.about_logo.as_ref().unwrap()).paint_at(ui, rect);
                this.setting_heading(ui, "Twitter/X Media Batch Downloader");
                this.setting_text(
                    ui,
                    &this
                        .tr("Version {}")
                        .replace("{}", &this.app.get_app_version()),
                    13.0,
                    false,
                    this.colors(ui).secondary,
                );
                this.setting_divider(ui);
                this.setting_text(
                    ui,
                    "Downloads original-quality images and videos from Twitter/X accounts.",
                    14.0,
                    false,
                    this.colors(ui).body,
                );
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    if this
                        .setting_icon(
                            ui,
                            Icon::Github,
                            "GitHub repository",
                            appearance::CONTROL_HEIGHT,
                        )
                        .clicked()
                    {
                        this.app.invoke_open_link(
                            "https://github.com/afkarxyz/Twitter-X-Media-Batch-Downloader".into(),
                        );
                    }
                    if this
                        .setting_icon(ui, Icon::Globe, "twitterdl.app", appearance::CONTROL_HEIGHT)
                        .clicked()
                    {
                        this.app.invoke_open_link("https://twitterdl.app".into());
                    }
                });
                this.setting_divider(ui);
                this.setting_text(
                    ui,
                    "Independent open-source project, not affiliated with or endorsed by X Corp. Media is downloaded directly from X to your device. You are responsible for how you use it.",
                    12.0,
                    false,
                    this.colors(ui).secondary,
                );
            });
        });
    }
}
