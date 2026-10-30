use crate::ui::components::dropdown::{ChoiceFlag, SelectOptions};
use crate::ui::screens::settings::controls::setting_switch;
use crate::ui::*;

impl Desktop {
    pub(in crate::ui) fn general_settings(&mut self, ui: &mut Ui) {
        self.setting_card(ui,"Restore preferences",10.0,|this,ui|{
            this.setting_heading(ui,"Restore preferences");
            this.setting_caption(ui,"Reset all preferences to defaults. Keeps auth_token, history, account labels, tools and downloaded files.");
            if this.setting_action(ui,"Reset all preferences",Some(Icon::Reset),2,Vec2::new(100.0,32.0),!this.busy()).clicked(){this.reset_setting_inputs(-1);}
        });
        self.theme_editor(ui);
        self.setting_card(ui, "Display language", 10.0, |this, ui| {
            let languages = [
                ("system", "System language"),
                ("en", "English"),
                ("id", "Indonesian"),
                ("es", "Spanish"),
                ("fr", "French"),
                ("de", "German"),
                ("pt", "Portuguese"),
                ("tr", "Turkish"),
                ("zh_Hans", "Simplified Chinese"),
            ];
            let choices = languages.map(|(_, label)| this.tr(label));
            let flags: [Option<ChoiceFlag>; 9] = [
                None,
                Some((
                    "bytes://flag-us.svg",
                    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/flags/us.svg")),
                )),
                Some((
                    "bytes://flag-id.svg",
                    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/flags/id.svg")),
                )),
                Some((
                    "bytes://flag-es.svg",
                    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/flags/es.svg")),
                )),
                Some((
                    "bytes://flag-fr.svg",
                    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/flags/fr.svg")),
                )),
                Some((
                    "bytes://flag-de.svg",
                    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/flags/de.svg")),
                )),
                Some((
                    "bytes://flag-pt.svg",
                    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/flags/pt.svg")),
                )),
                Some((
                    "bytes://flag-tr.svg",
                    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/flags/tr.svg")),
                )),
                Some((
                    "bytes://flag-cn.svg",
                    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/flags/cn.svg")),
                )),
            ];
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 58.0), egui::Sense::hover());
            this.history_text(
                ui,
                egui::Rect::from_min_size(
                    rect.min + Vec2::new(16.0, 16.0),
                    Vec2::new(rect.width() - 260.0, 20.0),
                ),
                this.tr("Display language"),
                14.0,
                true,
                this.colors(ui).heading,
            );
            let picker = egui::Rect::from_min_size(
                rect.right_top() + Vec2::new(-218.0, 8.0),
                Vec2::new(210.0, appearance::CONTROL_HEIGHT),
            );
            let selected = languages
                .iter()
                .position(|(code, _)| *code == this.app.get_language_mode())
                .unwrap_or(0);
            let next = this.choice_picker(
                ui,
                picker,
                &SelectOptions {
                    label: "Display language",
                    choices: &choices,
                    flags: &flags,
                    ..SelectOptions::new("Display language", &choices)
                },
                selected,
            );
            if next != selected {
                this.app.set_language_mode(languages[next].0.into());
                this.app.invoke_save_language(languages[next].0.into());
            }
        });
        self.setting_card(ui, "Renderer", 10.0, |this, ui| {
            let renderer = this.app.global::<RendererSettings>();
            let mut choices = vec![this.tr("OpenGL (compatibility)")];
            if renderer.get_gpu_supported() {
                choices.push(this.tr("GPU — WGPU"));
            }
            let selected = usize::from(renderer.get_preferred() == "gpu").min(choices.len() - 1);
            let next = this.setting_select(ui, "Renderer", "", &choices, selected, true);
            if next != selected {
                renderer.invoke_choose(if next == 1 { "gpu" } else { "cpu" }.into());
            }
            if !renderer.get_gpu_supported() {
                this.setting_caption(ui, "GPU mode is unavailable in this build.");
            }
            if renderer.get_restart_required() {
                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), appearance::CONTROL_HEIGHT),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        if this
                            .setting_action(
                                ui,
                                "Restart now",
                                Some(Icon::Restart),
                                0,
                                Vec2::new(100.0, appearance::CONTROL_HEIGHT),
                                !this.busy() && !renderer.get_restarting(),
                            )
                            .clicked()
                        {
                            renderer.set_prompt_open(true);
                        }
                        this.setting_caption(
                            ui,
                            "Restart required. Your current renderer is still active.",
                        );
                    },
                );
            }
            if !renderer.get_prompt_open() {
                this.setting_error(ui, &renderer.get_error());
            }
        });
        self.setting_card(ui, "Xtractor", 12.0, |this, ui| {
            let app = this.app.clone();
            this.setting_heading(ui, "Xtractor");
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), 18.0),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    ui.label(
                        RichText::new(format!(
                            "{}: {}",
                            this.tr("Latest"),
                            if app.get_xtractor_latest().is_empty() {
                                this.tr("Not checked")
                            } else {
                                app.get_xtractor_latest()
                            }
                        ))
                        .size(12.0)
                        .color(this.colors(ui).secondary),
                    );
                    this.setting_caption(
                        ui,
                        &format!(
                            "{}: {}",
                            this.tr("Installed"),
                            if !app.get_xtractor_installed() {
                                this.tr("Not installed")
                            } else if app.get_xtractor_version().is_empty() {
                                this.tr("Version unknown")
                            } else {
                                app.get_xtractor_version()
                            }
                        ),
                    );
                },
            );
            this.setting_caption(ui, "Updates are installed in the portable data folder.");
            ui.horizontal(|ui| {
                if this
                    .setting_action(
                        ui,
                        "Check for updates",
                        Some(Icon::Restart),
                        0,
                        Vec2::new(100.0, 32.0),
                        !this.busy(),
                    )
                    .clicked()
                {
                    app.invoke_check_xtractor_update();
                }
                if this
                    .setting_action(
                        ui,
                        if app.get_xtractor_installed() {
                            "Install latest xtractor"
                        } else {
                            "Download xtractor"
                        },
                        Some(Icon::Download),
                        1,
                        Vec2::new(100.0, 32.0),
                        !this.busy()
                            && (!app.get_xtractor_installed()
                                || app.get_xtractor_update_available()),
                    )
                    .clicked()
                {
                    app.invoke_update_xtractor();
                }
            });
            if !app.get_xtractor_status().is_empty() {
                this.setting_progress(
                    ui,
                    app.get_xtractor_busy(),
                    &this.tr(&app.get_xtractor_status()),
                    app.get_xtractor_progress(),
                    &app.get_xtractor_detail(),
                );
            }
        });
        self.setting_card(ui, "Diagnostics", 10.0, |this, ui| {
            let app = this.app.clone();
            this.setting_heading(ui, "Diagnostics");
            this.setting_caption(ui, "Session logs are stored in the portable data folder.");
            if !app.get_last_error().is_empty() {
                this.history_text(
                    ui,
                    egui::Rect::from_min_size(
                        ui.next_widget_position(),
                        Vec2::new(ui.available_width(), 18.0),
                    ),
                    app.get_last_error(),
                    12.0,
                    false,
                    this.colors(ui).secondary,
                );
                ui.add_space(18.0);
            }
            ui.horizontal(|ui| {
                if this
                    .setting_action(
                        ui,
                        "Open logs",
                        Some(Icon::Folder),
                        0,
                        Vec2::new(100.0, 32.0),
                        true,
                    )
                    .clicked()
                {
                    app.invoke_open_logs();
                }
                if this
                    .setting_action(
                        ui,
                        "View logs",
                        Some(Icon::File),
                        0,
                        Vec2::new(100.0, 32.0),
                        true,
                    )
                    .clicked()
                {
                    app.invoke_view_logs();
                }
                if this
                    .setting_action(
                        ui,
                        "Copy error details",
                        Some(Icon::Copy),
                        0,
                        Vec2::new(100.0, 32.0),
                        !app.get_last_error().is_empty(),
                    )
                    .clicked()
                {
                    app.invoke_copy_error();
                }
            });
        });
        let (fetch, http) = self.setting_card(ui, "Network timeouts", 12.0, |this, ui| {
            let app = this.app.clone();
            this.setting_heading(ui, "Network timeouts");
            let mut fetch = this.fetch_timeout.clone();
            let mut http = this.http_timeout.clone();
            let width = (ui.available_width() - 12.0) / 2.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                for (label, value) in [
                    ("Fetch page timeout (seconds)", &mut fetch),
                    ("HTTP read timeout (seconds)", &mut http),
                ] {
                    ui.vertical(|ui| {
                        ui.set_width(width);
                        ui.spacing_mut().item_spacing.y = 6.0;
                        this.setting_caption(ui, label);
                        ui.add_enabled_ui(!this.busy(), |ui| {
                            this.setting_input(ui, (label, ""), value, false);
                        });
                    });
                }
            });
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), 32.0),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    if this
                        .setting_action(
                            ui,
                            "Apply",
                            Some(Icon::Check),
                            0,
                            Vec2::new(100.0, 32.0),
                            !this.busy(),
                        )
                        .clicked()
                    {
                        app.invoke_apply_timeouts(fetch.clone(), http.clone());
                    }
                    this.setting_caption(
                        ui,
                        "5–3600 seconds. HTTP applies to downloads, previews and thumbnails.",
                    );
                },
            );
            this.setting_error(ui, &app.get_timeout_error());
            (fetch, http)
        });
        self.fetch_timeout = fetch;
        self.http_timeout = http;
        self.setting_card(ui, "Application and tool updates", 12.0, |this, ui| {
            let app = this.app.clone();
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), 32.0),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    if this
                        .setting_action(
                            ui,
                            if app.get_updates_busy() {
                                "Checking…"
                            } else {
                                "Check for updates"
                            },
                            Some(Icon::Restart),
                            0,
                            Vec2::new(100.0, 32.0),
                            !app.get_updates_busy(),
                        )
                        .clicked()
                    {
                        app.invoke_check_all_updates();
                    }
                    this.setting_heading(ui, "Application and tool updates");
                },
            );
            this.setting_caption(
                ui,
                "Install the latest tools in the portable data folder. Existing system installations are preserved.",
            );
            setting_switch!(
                this,
                ui,
                get_notify_updates,
                set_notify_updates,
                "Notify about updates on startup",
                "Check for a new stable application release when the app opens.",
                true
            );
            for item in app.get_update_items().iter() {
                ui.push_id(&item.name, |ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(ui.available_width(), 48.0),
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            if this
                                .setting_action(
                                    ui,
                                    "Open release",
                                    Some(Icon::External),
                                    0,
                                    Vec2::new(100.0, 32.0),
                                    !item.url.is_empty(),
                                )
                                .clicked()
                            {
                                app.invoke_open_link(item.url.clone());
                            }
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 4.0;
                                this.setting_text(
                                    ui,
                                    &format!(
                                        "{} · {}: {} · {}: {}",
                                        this.tr(&item.name),
                                        this.tr("Installed"),
                                        this.tr(&item.installed),
                                        this.tr("Latest"),
                                        item.latest
                                    ),
                                    14.0,
                                    false,
                                    this.colors(ui).heading,
                                );
                                this.setting_caption(ui, &this.tr(&item.status));
                            });
                        },
                    );
                });
            }
            ui.horizontal(|ui| {
                for name in ["FFmpeg", "ExifTool"] {
                    if this
                        .setting_action(
                            ui,
                            &format!("Update / Reinstall {name}"),
                            Some(Icon::Download),
                            0,
                            Vec2::new(100.0, 32.0),
                            !this.busy() && !app.get_updates_busy(),
                        )
                        .clicked()
                    {
                        app.invoke_update_tool(name.into());
                    }
                }
            });
            if app.get_tool_update_busy() || !app.get_tool_update_status().is_empty() {
                this.setting_progress(
                    ui,
                    app.get_tool_update_busy(),
                    &app.get_tool_update_status(),
                    app.get_tool_update_progress(),
                    &app.get_tool_update_detail(),
                );
                if app.get_tool_update_busy()
                    && this
                        .setting_action(ui, "Cancel", Some(Icon::Close), 0, Vec2::new(100.0, 32.0), true)
                        .clicked()
                {
                    app.invoke_cancel_tool_update();
                }
            }
            this.setting_error(ui, &app.get_tool_update_error());
            this.setting_error(ui, &app.get_updates_error());
        });
        self.proxy=self.setting_card(ui,"Manual proxy",12.0,|this,ui|{
            let app=this.app.clone();let mut proxy=this.proxy.clone();this.setting_heading(ui,"Manual proxy");
            ui.add_enabled_ui(!this.busy(),|ui|{this.setting_input(ui,("Proxy URL","http://127.0.0.1:7890"),&mut proxy, false);});
            this.setting_caption(ui,"HTTP, HTTPS, SOCKS5 and SOCKS5H. Empty uses the system/environment proxy. Local preview traffic stays local.");
            ui.horizontal(|ui|{if this.setting_action(ui,"Apply",Some(Icon::Check),0,Vec2::new(100.0,32.0),!this.busy()).clicked(){app.invoke_apply_proxy(proxy.clone());}
                if this.setting_action(ui,"Clear",Some(Icon::Trash),2,Vec2::new(100.0,32.0),!this.busy()).clicked()&&app.invoke_apply_proxy(String::new()){proxy.clear();}
            });this.setting_error(ui,&app.get_proxy_error());proxy
        });
    }

    pub(in crate::ui) fn cookies(&mut self, ui: &mut Ui) {
        self.setting_card(ui, "X/Twitter authentication", 10.0, |this, ui| {
            let app = this.app.clone();
            this.setting_heading(ui, "X/Twitter authentication");
            this.setting_text(
                ui,
                "Save your X auth_token locally. Up to 10 previous tokens are kept in the portable config.json.",
                13.0,
                false,
                this.colors(ui).secondary,
            );
            let mut token = app.get_auth_token();
            let enabled = !this.busy() && !app.get_auth_check_busy();
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.vertical(|ui| {
                    ui.set_width((ui.available_width() - 120.0).max(100.0));
                    if this
                        .setting_input(ui, ("X auth_token", "auth_token (required)"), &mut token, false)
                        .changed()
                    {
                        app.set_auth_token(token.clone());
                    }
                });
                let clear =
                    !app.get_saved_auth_token().is_empty() && (token.is_empty() || token == app.get_saved_auth_token());
                if this
                    .setting_action(
                        ui,
                        if clear { "Clear" } else { "Save" },
                        Some(if clear { Icon::Trash } else { Icon::Save }),
                        if clear { 2 } else { 1 },
                        Vec2::new(112.0, appearance::CONTROL_HEIGHT),
                        enabled && (!token.is_empty() || !app.get_saved_auth_token().is_empty()),
                    )
                    .clicked()
                {
                    if clear {
                        app.set_auth_token(String::new());
                    }
                    app.invoke_save_auth_token();
                }
            });
            this.active_token_status(ui, enabled);
        });
        if self.app.get_auth_token_history().row_count() > 0 {
            self.setting_card(ui, "Previous auth_tokens", 10.0, |this, ui| {
                let app = this.app.clone();
                this.setting_heading(
                    ui,
                    &format!(
                        "{} ({}/{})",
                        this.tr("Previous auth_tokens"),
                        app.get_auth_token_history().row_count(),
                        crate::accounts::tokens::HISTORY_LIMIT
                    ),
                );
                this.setting_caption(
                    ui,
                    "Check a previous token, use it as the active token, or remove it from local history.",
                );
                // Usable during a fetch: it takes effect at the next rate limit.
                setting_switch!(
                    this,
                    ui,
                    get_rotate_auth_tokens,
                    set_rotate_auth_tokens,
                    "Switch tokens when rate limited",
                    "When X rate limits a fetch, continue with the next saved token that is not invalid or cooling down. That token becomes the active one.",
                    true
                );
                for item in app.get_auth_token_history().iter() {
                    ui.push_id(item.index, |ui| {
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::new(ui.available_width(), 82.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 8, this.colors(ui).surface);
                        let enabled = !this.busy() && !app.get_auth_check_busy();
                        let mut row = crate::ui::components::dialog::child(ui, "previous-token", rect.shrink(12.0));
                        row.spacing_mut().item_spacing.x = 10.0;
                        row.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_enabled_ui(enabled, |ui| {
                                if this
                                    .setting_icon(ui, Icon::Trash, "Remove from token history", 32.0)
                                    .clicked()
                                {
                                    app.invoke_remove_auth_token_history(item.index);
                                }
                            });
                            if this
                                .setting_action(ui, "Use", Some(Icon::Check), 0, Vec2::new(64.0, 32.0), enabled)
                                .clicked()
                            {
                                app.invoke_use_auth_token_history(item.index);
                            }
                            if this
                                .setting_action(
                                    ui,
                                    if app.get_auth_check_busy() && app.get_auth_check_index() == item.index {
                                        "Checking…"
                                    } else {
                                        "Check"
                                    },
                                    Some(Icon::Check),
                                    0,
                                    Vec2::new(72.0, 32.0),
                                    enabled,
                                )
                                .clicked()
                            {
                                app.invoke_check_auth_token(item.index);
                            }
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 3.0;
                                this.setting_text(ui, &item.token, 14.0, true, this.colors(ui).heading);
                                this.setting_text(
                                    ui,
                                    &format!(
                                        "{}{}",
                                        this.auth_status(&item.status),
                                        if item.username.is_empty() {
                                            String::new()
                                        } else {
                                            format!(" · @{}", item.username)
                                        }
                                    ),
                                    12.0,
                                    false,
                                    if item.status == "invalid" {
                                        Color32::from_rgb(183, 55, 49)
                                    } else {
                                        this.colors(ui).secondary
                                    },
                                );
                                if !item.checked_at.is_empty() {
                                    this.setting_text(
                                        ui,
                                        &format!("{}: {}", this.tr("Last checked"), item.checked_at),
                                        10.0,
                                        false,
                                        this.colors(ui).secondary,
                                    );
                                }
                            });
                        });
                    });
                }
            });
        }
    }

    fn active_token_status(&self, ui: &mut Ui, enabled: bool) {
        let app = &self.app;
        let label = if app.get_auth_check_busy() && app.get_auth_check_index() == -1 {
            "Checking…"
        } else {
            "Check active token"
        };
        let status = format!(
            "{} · {}{}",
            self.tr("Active token"),
            self.auth_status(&app.get_auth_validation_status()),
            if app.get_auth_validation_username().is_empty() {
                String::new()
            } else {
                format!(" · @{}", app.get_auth_validation_username())
            }
        );
        let caption_color = if app.get_auth_validation_status() == "invalid" {
            Color32::from_rgb(183, 55, 49)
        } else {
            self.colors(ui).secondary
        };
        let measure = ui.painter().layout_no_wrap(
            self.tr(label),
            egui::FontId::new(typography::BODY, egui::FontFamily::Name("cera-bold".into())),
            caption_color,
        );
        let text_width = (ui.available_width() - measure.size().x.max(76.0) - 60.0).max(1.0);
        let mut job = egui::text::LayoutJob::simple(
            status.clone(),
            egui::FontId::proportional(12.0),
            caption_color,
            text_width,
        );
        job.sections[0].format.line_height = Some(17.0);
        let text = ui.painter().layout_job(job);
        let checked = !app.get_auth_validation_checked_at().is_empty();
        let status_height = text.size().y + if checked { 17.0 } else { 0.0 };
        let (row, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), status_height.max(32.0)),
            egui::Sense::hover(),
        );
        let mut actions = crate::ui::components::dialog::child(ui, "active-token-check", row);
        actions.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if self
                .setting_action(
                    ui,
                    label,
                    Some(Icon::Check),
                    0,
                    Vec2::new(100.0, 32.0),
                    enabled && !app.get_saved_auth_token().is_empty(),
                )
                .clicked()
            {
                app.invoke_check_auth_token(-1);
            }
        });
        let top = row.center().y - status_height / 2.0;
        ui.painter()
            .galley(egui::pos2(row.left(), top), text.clone(), caption_color);
        let response = ui.interact(
            egui::Rect::from_min_size(egui::pos2(row.left(), top), text.size()),
            ui.id().with("active-token-status"),
            egui::Sense::hover(),
        );
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &status));
        if checked {
            self.history_text(
                ui,
                egui::Rect::from_min_size(
                    egui::pos2(row.left(), top + text.size().y + 3.0),
                    Vec2::new(text_width, 14.0),
                ),
                format!(
                    "{}: {}",
                    self.tr("Last checked"),
                    app.get_auth_validation_checked_at()
                ),
                9.96,
                false,
                self.colors(ui).secondary,
            );
        }
    }

    fn auth_status(&self, status: &str) -> String {
        self.tr(match status {
            "valid" => "Valid",
            "invalid" => "Invalid / expired",
            "retryable" => "Temporary error — check again",
            "unknown" => "Could not determine validity",
            _ => "Not checked",
        })
    }
}
