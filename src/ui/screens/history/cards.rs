use crate::ui::components::dialog::child;
use crate::ui::components::icon_button::IconStyle;
use crate::ui::screens::history::HistoryConfirmation;
use crate::ui::*;

impl Desktop {
    pub(in crate::ui) fn history_card(
        &mut self,
        ui: &mut Ui,
        rect: egui::Rect,
        row: &crate::HistoryItem,
        sessions: bool,
    ) {
        let app = self.app.clone();
        let colors = self.colors(ui);
        let enabled = !self.busy();
        let saved = !sessions && app.get_history_show_saved();
        let mut card = child(ui, (&row.id, sessions), rect);
        let response = card.interact(
            rect,
            card.id().with("activate"),
            if enabled {
                egui::Sense::click()
            } else {
                egui::Sense::hover()
            },
        );
        let response = if enabled {
            response.on_hover_cursor(egui::CursorIcon::PointingHand)
        } else {
            response
        };
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &row.title)
        });
        let focus = crate::ui::components::navigation::focus_visible(&card, &response);
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
                if focus { 2.0 } else { 1.0 },
                if focus || response.hovered() {
                    colors.accent
                } else {
                    colors.border
                },
            ),
            egui::StrokeKind::Inside,
        );
        let avatar = egui::Rect::from_center_size(
            egui::pos2(rect.left() + 36.0, rect.center().y),
            Vec2::splat(48.0),
        );
        card.painter()
            .circle_filled(avatar.center(), 24.0, colors.surface);
        if row.has_avatar {
            self.paint_media(&mut card, &row.avatar_image, avatar, 24);
        } else {
            Icon::User.image(colors.secondary, 20.0).paint_at(
                &card,
                egui::Rect::from_center_size(avatar.center(), Vec2::splat(20.0)),
            );
        }
        let mut lines = vec![(row.title.clone(), 14.04, true, colors.heading)];
        if saved && !row.handle.is_empty() {
            lines.push((format!("@{}", row.handle), 12.0, false, colors.secondary));
        }
        let age = if row.earlier_download {
            self.tr("Earlier downloads")
        } else if row.age_key.is_empty() {
            row.fetched_at.clone()
        } else {
            self.tr(&row.age_key)
                .replace("{}", &row.age_count.to_string())
        };
        let prefix = if saved {
            format!(
                "{} · {}",
                self.tr("{} downloaded")
                    .replace("{}", &row.media_count.to_string()),
                row.size_text
            )
        } else {
            format!(
                "{}{}",
                if row.handle.is_empty() {
                    String::new()
                } else {
                    format!("@{} · ", row.handle)
                },
                self.tr("{} media")
                    .replace("{}", &row.media_count.to_string())
            )
        };
        lines.push((format!("{prefix} · {age}"), 12.0, false, colors.secondary));
        if saved {
            let available = if row.saved_count < row.media_count {
                format!(
                    "{} · ",
                    self.tr("{} available")
                        .replace("{}", &row.saved_count.to_string())
                )
            } else {
                String::new()
            };
            lines.push((
                format!("{available}{}", row.output_dir),
                12.0,
                false,
                colors.secondary,
            ));
        } else if row.session_count > 0 {
            let sessions = if row.session_count == 1 {
                self.tr("1 fetch session")
            } else {
                self.tr("{} fetch sessions")
                    .replace("{}", &row.session_count.to_string())
            };
            lines.push((
                format!(
                    "{} · {sessions} · {}",
                    self.tr("Merged archive"),
                    row.group_label
                ),
                12.0,
                false,
                colors.secondary,
            ));
        }
        let label_color = if row.account_color.a() == 0 {
            colors.accent
        } else if ui.visuals().dark_mode {
            if row
                .account_color
                .r()
                .max(row.account_color.g())
                .max(row.account_color.b())
                < 153
            {
                appearance::mix(row.account_color, Color32::WHITE, 0.6)
            } else {
                row.account_color
            }
        } else {
            appearance::mix(row.account_color, Color32::BLACK, 0.55)
        };
        if !row.account_labels.is_empty() {
            lines.push((row.account_labels.clone(), 12.0, false, label_color));
        }
        let line_height = if lines.len() > 4 { 15.0 } else { 17.0 };
        let total = 20.0 + (lines.len() - 1) as f32 * (line_height + 3.0);
        let mut y = rect.center().y - total / 2.0;
        let reserve = if row.session_count > 1 { 110.0 } else { 70.0 };
        let text_left = rect.left() + 72.0;
        let text_right = rect.right() - 12.0 - reserve - 12.0;
        for (index, (text, size, bold, color)) in lines.iter().enumerate() {
            let is_labels = !row.account_labels.is_empty() && index == lines.len() - 1;
            let dot = is_labels && row.account_color.a() > 0;
            let height = if index == 0 { 20.0 } else { line_height };
            if dot {
                card.painter().circle_filled(
                    egui::pos2(text_left + 5.0, y + height / 2.0),
                    5.0,
                    row.account_color,
                );
            }
            self.history_text(
                &card,
                egui::Rect::from_min_max(
                    egui::pos2(text_left + if dot { 16.0 } else { 0.0 }, y),
                    egui::pos2(text_right, y + height),
                ),
                text.clone(),
                *size,
                *bold,
                *color,
            );
            y += height + 3.0;
        }
        let mut child_clicked = false;
        for (icon, label, offset, available, destructive) in [
            (
                Icon::History,
                "View fetch sessions",
                118.0,
                !saved && !sessions && row.session_count > 1,
                false,
            ),
            (
                if saved { Icon::Folder } else { Icon::Export },
                if saved { "Open folder" } else { "Export JSON" },
                80.0,
                true,
                false,
            ),
            (Icon::Trash, "Remove from history", 42.0, true, true),
        ] {
            if !available {
                continue;
            }
            let action = egui::Rect::from_center_size(
                egui::pos2(rect.right() - offset + 18.0, rect.center().y),
                Vec2::splat(appearance::CONTROL_HEIGHT),
            );
            let full_label = format!("{} · {}", self.tr(label), row.title);
            let mut action_ui = child(&mut card, label, action);
            if !enabled || (saved && !destructive && row.output_dir.is_empty()) {
                action_ui.disable();
            }
            let response = self.home_icon(
                &mut action_ui,
                icon,
                &full_label,
                appearance::CONTROL_HEIGHT,
                IconStyle {
                    radius: 16,
                    icon_size: appearance::CONTROL_ICON_SIZE,
                    ..if destructive {
                        IconStyle::destructive(colors)
                    } else {
                        IconStyle::ghost(colors, false)
                    }
                },
            );
            if response.clicked() {
                child_clicked = true;
                match label {
                    "View fetch sessions" => app.invoke_open_fetch_sessions(row.id.clone()),
                    "Open folder" => app.invoke_open_download_path(row.output_dir.clone(), true),
                    "Export JSON" => app.invoke_export_history(row.id.clone()),
                    _ => {
                        self.history_confirmation = Some(HistoryConfirmation {
                            id: Some(row.id.clone()),
                            title: row.title.clone(),
                            group: !saved && !sessions,
                            saved,
                        })
                    }
                }
            }
        }
        if response.clicked() && !child_clicked {
            response.request_focus();
            if saved {
                app.invoke_open_download_batch(row.id.clone());
            } else {
                app.invoke_open_history(row.id.clone());
            }
        }
    }
}
