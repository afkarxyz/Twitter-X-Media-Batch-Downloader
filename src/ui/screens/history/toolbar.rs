use crate::ui::components::dialog::child;
use crate::ui::components::icon_button::IconStyle;
use crate::ui::screens::history::HistoryConfirmation;
use crate::ui::*;

impl Desktop {
    pub(in crate::ui) fn history_icon(
        &self,
        ui: &mut Ui,
        rect: egui::Rect,
        icon: (Icon, f32),
        label: &str,
        enabled: bool,
        destructive: bool,
    ) -> egui::Response {
        let colors = self.colors(ui);
        let mut action = child(ui, label, rect);
        if !enabled {
            action.disable();
        }
        self.home_icon(
            &mut action,
            icon.0,
            label,
            rect.width(),
            IconStyle {
                idle: Color32::TRANSPARENT,
                radius: (rect.width() / 2.0) as u8,
                icon_size: icon.1.min(appearance::CONTROL_ICON_SIZE),
                ..if destructive {
                    IconStyle::destructive(colors)
                } else {
                    IconStyle::ghost(colors, false)
                }
            },
        )
    }

    pub(in crate::ui) fn history_button(
        &self,
        ui: &mut Ui,
        rect: egui::Rect,
        label: &str,
        icon: Option<Icon>,
        primary: bool,
        enabled: bool,
    ) -> egui::Response {
        let mut action = child(ui, label, rect);
        if !enabled {
            action.disable();
        }
        self.home_action(
            &mut action,
            label,
            icon.map(|icon| (icon, 20.0)),
            primary,
            rect.size(),
            6,
        )
    }

    pub(in crate::ui) fn history_header(&mut self, ui: &mut Ui, rect: egui::Rect) {
        let app = self.app.clone();
        let enabled = !self.busy();
        let saved = app.get_history_show_saved();
        let mut right = rect.right();
        let mut slot = |width: f32, height: f32| {
            right -= width;
            let slot = egui::Rect::from_min_size(
                egui::pos2(right, rect.center().y - height / 2.0),
                Vec2::new(width, height),
            );
            right -= 8.0;
            slot
        };
        if self
            .history_icon(
                ui,
                slot(appearance::CONTROL_HEIGHT, appearance::CONTROL_HEIGHT),
                (Icon::Trash, 18.0),
                "Clear history",
                enabled
                    && if saved {
                        app.get_history_saved_count() > 0
                    } else {
                        app.get_history_fetch_count() > 0
                    },
                true,
            )
            .clicked()
        {
            self.history_confirmation = Some(HistoryConfirmation {
                id: None,
                title: String::new(),
                group: false,
                saved,
            });
        }
        if !saved
            && self
                .history_icon(
                    ui,
                    slot(appearance::CONTROL_HEIGHT, appearance::CONTROL_HEIGHT),
                    (Icon::Upload, 18.0),
                    "Import history JSON",
                    enabled,
                    false,
                )
                .clicked()
        {
            app.invoke_import_history();
        }

        if self
            .history_icon(
                ui,
                slot(appearance::CONTROL_HEIGHT, appearance::CONTROL_HEIGHT),
                (Icon::Export, 20.0),
                "Export accounts TXT",
                enabled && app.get_history_items().row_count() > 0,
                false,
            )
            .clicked()
        {
            app.invoke_export_accounts_txt();
        }
        self.history_title(
            ui,
            egui::Rect::from_min_max(rect.min, egui::pos2(right, rect.bottom())),
            self.tr("History"),
        );
    }

    pub(in crate::ui) fn history_filters(&mut self, ui: &mut Ui, rect: egui::Rect) {
        let app = self.app.clone();
        let colors = self.colors(ui);
        self.history_text(
            ui,
            egui::Rect::from_min_size(rect.min, Vec2::new(200.0, rect.height())),
            format!("{} ({})", self.tr("Fetches"), app.get_history_fetch_count()),
            14.0,
            true,
            colors.secondary,
        );
        let search = crate::ui::components::account_search::search_rect(rect);
        let mut query = app.get_history_search_query();
        if self.account_search_input(
            ui,
            search,
            "history-search-query",
            "Search accounts…",
            &mut query,
        ) {
            app.set_history_search_query(query.clone());
            app.invoke_search_history(query.clone(), app.get_history_show_saved());
        }
    }
}
