pub(in crate::ui) mod controls;
pub(in crate::ui) mod dialogs;
pub(in crate::ui) mod downloads;
pub(in crate::ui) mod general;
pub(in crate::ui) mod storage;
use crate::ui::*;

pub(in crate::ui) struct InputSources {
    proxy: String,
    speed: i32,
    delay: i32,
    jitter: i32,
    fetch_delay: i32,
    fetch_jitter: i32,
    fetch: i32,
    http: i32,
}

impl Desktop {
    pub(in crate::ui) fn settings(&mut self, ui: &mut Ui) {
        self.settings_categories(ui);
        self.sync_setting_inputs();
        let category = self.app.get_settings_category().clamp(0, 4);
        let changed = self.settings_category != Some(category);
        self.settings_category = Some(category);
        if category == 3 && (changed || self.settings_output_seen != self.app.get_output_dir()) {
            self.settings_output_seen = self.app.get_output_dir();
            self.app.invoke_refresh_storage();
        }
        let bounds = ui.available_rect_before_wrap();
        let area = egui::ScrollArea::vertical()
            .id_salt("settings-page")
            .auto_shrink([false, false])
            .scroll_bar_rect(bounds.shrink2(Vec2::new(0.0, 16.0)));
        let area = if changed {
            area.vertical_scroll_offset(0.0)
        } else {
            area
        };
        area.show_themed(ui, |ui| {
            let width = (ui.available_width() - 24.0).min(appearance::SETTINGS_CONTENT_WIDTH);
            let column_rect = egui::Rect::from_min_size(
                ui.next_widget_position()
                    + Vec2::new(((ui.available_width() - width - 24.0) / 2.0).max(0.0), 0.0),
                Vec2::new(width + 24.0, ui.available_height()),
            );
            let mut column =
                crate::ui::components::dialog::child(ui, "settings-column", column_rect);
            let rendered = egui::Frame::new().inner_margin(12).show(&mut column, |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = 12.0;
                if category != 4 {
                    let (header, _) =
                        ui.allocate_exact_size(Vec2::new(width, 44.0), egui::Sense::hover());
                    self.history_title(
                        ui,
                        header,
                        self.tr(["General", "Cookies", "Downloads", "Storage", "About"]
                            [category as usize]),
                    );
                    if matches!(category, 0 | 2) {
                        let mut actions = crate::ui::components::dialog::child(
                            ui,
                            "settings-reset",
                            egui::Rect::from_min_size(
                                header.right_top() + Vec2::new(-180.0, 6.0),
                                Vec2::new(180.0, 32.0),
                            ),
                        );
                        actions.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if self
                                    .setting_action(
                                        ui,
                                        "Reset defaults",
                                        Some(Icon::Reset),
                                        2,
                                        Vec2::new(100.0, 32.0),
                                        !self.busy(),
                                    )
                                    .clicked()
                                {
                                    self.reset_setting_inputs(category);
                                }
                            },
                        );
                    }
                }
                match category {
                    1 => self.cookies(ui),
                    2 => self.download_settings(ui),
                    3 => self.storage_settings(ui),
                    4 => self.about(ui),
                    _ => self.general_settings(ui),
                }
            });
            ui.advance_cursor_after_rect(rendered.response.rect);
        });
    }

    pub(in crate::ui) fn sync_setting_inputs(&mut self) {
        let app = &self.app;
        let current = InputSources {
            proxy: app.get_proxy_url(),
            speed: app.get_download_speed_kib(),
            delay: app.get_download_delay_ms(),
            jitter: app.get_download_jitter_ms(),
            fetch_delay: app.get_fetch_delay_ms(),
            fetch_jitter: app.get_fetch_jitter_ms(),
            fetch: app.get_fetch_timeout_secs(),
            http: app.get_http_timeout_secs(),
        };
        let old = self.settings_sources.as_ref();
        if old.is_none_or(|old| old.proxy != current.proxy) {
            self.proxy = current.proxy.clone();
        }
        if old.is_none_or(|old| old.speed != current.speed) {
            self.speed = current.speed.to_string();
        }
        if old.is_none_or(|old| old.delay != current.delay) {
            self.delay = current.delay.to_string();
        }
        if old.is_none_or(|old| old.jitter != current.jitter) {
            self.jitter = current.jitter.to_string();
        }
        if old.is_none_or(|old| old.fetch_delay != current.fetch_delay) {
            self.fetch_delay = current.fetch_delay.to_string();
        }
        if old.is_none_or(|old| old.fetch_jitter != current.fetch_jitter) {
            self.fetch_jitter = current.fetch_jitter.to_string();
        }
        if old.is_none_or(|old| old.fetch != current.fetch) {
            self.fetch_timeout = current.fetch.to_string();
        }
        if old.is_none_or(|old| old.http != current.http) {
            self.http_timeout = current.http.to_string();
        }
        self.settings_sources = Some(current);
    }

    pub(in crate::ui) fn reset_setting_inputs(&mut self, scope: i32) {
        if self.app.invoke_reset_settings(scope) {
            self.settings_sources = None;
            self.sync_setting_inputs();
            self.settings_category = None;
        }
    }
}
