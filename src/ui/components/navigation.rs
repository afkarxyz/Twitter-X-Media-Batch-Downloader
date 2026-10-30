use crate::ui::components::icon_button::interaction_fill;
use crate::ui::*;

pub(in crate::ui) fn focus_visible(ui: &Ui, response: &egui::Response) -> bool {
    let modality = ui.input(|input| {
        input.events.iter().rev().find_map(|event| match event {
            egui::Event::Key { pressed: true, .. } => Some(true),
            egui::Event::PointerButton { pressed: true, .. } => Some(false),
            egui::Event::AccessKitActionRequest(request)
                if request.action == egui::accesskit::Action::Focus =>
            {
                Some(true)
            }
            _ => None,
        })
    });
    let focused = response.has_focus();
    ui.ctx().data_mut(|data| {
        let keyboard = data.get_temp_mut_or(egui::Id::new("keyboard-focus-visible"), true);
        if let Some(modality) = modality {
            *keyboard = modality;
        }
        focused && *keyboard
    })
}

impl Desktop {
    pub(in crate::ui) fn product_rail(&mut self, ui: &mut Ui) {
        let bounds = ui.max_rect();
        let colors = self.colors(ui);
        ui.allocate_rect(bounds, egui::Sense::hover());
        ui.painter().rect_filled(
            egui::Rect::from_min_max(egui::pos2(bounds.right() - 1.0, bounds.top()), bounds.max),
            0,
            colors.border,
        );
        for (tab, label, icon, active_icon, y) in [
            (0, "Home", Icon::Home, Icon::HomeFilled, bounds.top() + 16.0),
            (
                1,
                "History",
                Icon::History,
                Icon::HistoryFilled,
                bounds.top() + 72.0,
            ),
            (
                4,
                "Library",
                Icon::Library,
                Icon::LibraryFilled,
                bounds.top() + 128.0,
            ),
            (
                3,
                "Support",
                Icon::Coffee,
                Icon::CoffeeFilled,
                bounds.bottom() - 112.0,
            ),
            (
                2,
                "Settings",
                Icon::Settings,
                Icon::SettingsFilled,
                bounds.bottom() - 56.0,
            ),
        ] {
            let rect = egui::Rect::from_min_size(
                egui::pos2(bounds.left() + 8.0, y),
                Vec2::splat(appearance::NAVIGATION_SIZE),
            );
            let selected = self.app.get_selected_tab() == tab;
            let response = ui
                .interact(
                    rect,
                    ui.id().with(("destination", tab)),
                    egui::Sense::click(),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            let fill = interaction_fill(
                ui,
                &response,
                if selected {
                    colors.accent_soft
                } else {
                    Color32::TRANSPARENT
                },
                if selected {
                    colors.accent_soft
                } else {
                    colors.control
                },
                colors.accent_soft,
            );
            ui.painter().rect_filled(rect, 6, fill);
            if focus_visible(ui, &response) {
                ui.painter().rect_stroke(
                    rect,
                    6,
                    egui::Stroke::new(2.0, colors.accent),
                    egui::StrokeKind::Inside,
                );
            }
            let center =
                rect.center() + Vec2::new(0.0, f32::from(response.is_pointer_button_down_on()));
            let icon = if selected { active_icon } else { icon };
            icon.image(
                if selected {
                    colors.accent
                } else {
                    colors.secondary
                },
                appearance::NAVIGATION_ICON_SIZE,
            )
            .paint_at(
                ui,
                egui::Rect::from_center_size(center, Vec2::splat(appearance::NAVIGATION_ICON_SIZE)),
            );
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::Button,
                    ui.is_enabled(),
                    selected,
                    self.tr(label),
                )
            });
            if response.hovered() {
                self.rail_tooltip(ui, rect, label);
            }
            if response.clicked() {
                response.request_focus();
                self.app.set_selected_tab(tab);
            }
        }
    }

    fn rail_tooltip(&self, ui: &Ui, button: egui::Rect, label: &str) {
        let painter = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            egui::Id::new("rail-tooltip"),
        ));
        let galley = painter.layout_no_wrap(
            self.tr(label),
            egui::FontId::proportional(13.0),
            Color32::WHITE,
        );
        let rect = egui::Rect::from_min_size(
            egui::pos2(ui.max_rect().right() + 8.0, button.center().y - 14.0),
            Vec2::new(galley.size().x + 16.0, 28.0),
        );
        painter.rect_filled(rect, 5, Color32::from_gray(41));
        painter.galley(rect.center() - galley.size() / 2.0, galley, Color32::WHITE);
    }

    pub(in crate::ui) fn settings_categories(&mut self, ui: &mut Ui) {
        let colors = self.colors(ui);
        egui::Panel::left("settings-categories")
            .resizable(false)
            .exact_size(appearance::SETTINGS_WIDTH)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(colors.sidebar).inner_margin(0))
            .show(ui, |ui| {
                let bounds = ui.max_rect();
                ui.allocate_rect(bounds, egui::Sense::hover());
                ui.painter().rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(bounds.right() - 1.0, bounds.top()),
                        bounds.max,
                    ),
                    0,
                    colors.border,
                );
                let title_rect = egui::Rect::from_min_size(
                    bounds.min + Vec2::new(12.0, 16.0),
                    Vec2::new(bounds.width() - 24.0, 18.0),
                );
                let mut job = egui::text::LayoutJob::simple(
                    self.tr("SETTINGS"),
                    egui::FontId::new(
                        typography::MICRO,
                        egui::FontFamily::Name("cera-bold".into()),
                    ),
                    colors.muted,
                    title_rect.width(),
                );
                job.sections[0].format.extra_letter_spacing = 0.6;
                job.wrap.max_rows = 1;
                let galley = ui.painter().layout_job(job);
                ui.painter().galley(
                    egui::pos2(
                        title_rect.left(),
                        title_rect.center().y - galley.size().y / 2.0,
                    ),
                    galley,
                    colors.muted,
                );
                for (index, label) in ["General", "Cookies", "Downloads", "Storage", "About"]
                    .iter()
                    .enumerate()
                {
                    let rect = egui::Rect::from_min_size(
                        bounds.min + Vec2::new(12.0, 42.0 + 44.0 * index as f32),
                        Vec2::new(bounds.width() - 24.0, appearance::NAVIGATION_SIZE),
                    );
                    let selected = self.app.get_settings_category() == index as i32;
                    let response = ui
                        .interact(
                            rect,
                            ui.id().with(("category", index)),
                            egui::Sense::click(),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    ui.painter().rect_filled(
                        rect,
                        7,
                        if selected || response.hovered() {
                            colors.accent_soft
                        } else {
                            Color32::TRANSPARENT
                        },
                    );
                    if focus_visible(ui, &response) {
                        ui.painter().rect_stroke(
                            rect,
                            7,
                            egui::Stroke::new(2.0, colors.accent),
                            egui::StrokeKind::Inside,
                        );
                    }
                    let mut job = egui::text::LayoutJob::simple(
                        self.tr(label),
                        egui::FontId::new(
                            typography::BODY,
                            egui::FontFamily::Name("cera-medium".into()),
                        ),
                        colors.heading,
                        rect.width() - 22.0,
                    );
                    job.wrap.max_rows = 1;
                    let galley = ui.painter().layout_job(job);
                    ui.painter().galley(
                        egui::pos2(rect.left() + 12.0, rect.center().y - galley.size().y / 2.0),
                        galley,
                        colors.heading,
                    );
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::WidgetType::Button,
                            ui.is_enabled(),
                            selected,
                            self.tr(label),
                        )
                    });
                    if response.clicked() {
                        response.request_focus();
                        self.app.set_settings_category(index as i32);
                    }
                }
            });
    }
}
