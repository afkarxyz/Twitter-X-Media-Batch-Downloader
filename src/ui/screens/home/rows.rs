use crate::ui::components::icon_button::IconStyle;
use crate::ui::*;

impl Desktop {
    pub(in crate::ui) fn virtual_media_rows(
        &mut self,
        ui: &mut Ui,
        rows: &ui_runtime::ModelRc<MediaItem>,
    ) {
        let list = self.app.get_selected_tab() != crate::library::TAB
            && self.app.get_results_view() == "list";
        let columns = if list {
            1
        } else {
            ((ui.available_width() + 10.0) / 130.0).floor().max(1.0) as usize
        };
        let height = if list {
            96.0
        } else {
            (ui.available_width() - (columns - 1) as f32 * 10.0) / columns as f32
        };
        let count = rows.row_count().div_ceil(columns);
        let pitch = height + 10.0;
        let (area, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), count as f32 * pitch - 10.0),
            egui::Sense::hover(),
        );
        let clip = ui.clip_rect();
        let first = (((clip.top() - area.top()) / pitch).floor().max(0.0) as usize)
            .saturating_sub(1)
            .min(count);
        let last =
            ((((clip.bottom() - area.top()) / pitch).ceil().max(0.0) as usize) + 1).min(count);
        for row in first..last {
            let rect = egui::Rect::from_min_size(
                area.min + Vec2::new(0.0, row as f32 * pitch),
                Vec2::new(area.width(), height),
            );
            let mut child = crate::ui::components::dialog::child(ui, ("media-row", row), rect);
            child.spacing_mut().item_spacing.x = 10.0;
            if list {
                let item = rows.row_data(row).unwrap();
                child.push_id(item.media_index, |ui| self.home_media_row(ui, &item, row));
            } else {
                child.horizontal_top(|ui| {
                    for position in row * columns..((row + 1) * columns).min(rows.row_count()) {
                        let item = rows.row_data(position).unwrap();
                        ui.push_id(item.media_index, |ui| {
                            self.media_tile(ui, &item, height, position)
                        });
                    }
                });
            }
        }
    }

    pub(in crate::ui) fn media_number(&self, ui: &Ui, area: egui::Rect, position: usize) {
        let caption = format!("#{}", position + 1);
        let mut job = egui::text::LayoutJob::simple(
            caption.clone(),
            egui::FontId::new(11.0, egui::FontFamily::Name("cera-medium".into())),
            Color32::WHITE,
            (area.width() - 8.0).max(1.0),
        );
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        let galley = ui.painter().layout_job(job);
        let badge =
            egui::Rect::from_center_size(area.center(), Vec2::new(galley.size().x + 8.0, 20.0));
        ui.painter()
            .rect_filled(badge, 5, Color32::from_black_alpha(180));
        ui.painter()
            .galley(badge.center() - galley.size() / 2.0, galley, Color32::WHITE);
        ui.interact(badge, ui.id().with("media-number"), egui::Sense::hover())
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &caption));
    }

    pub(in crate::ui) fn home_media_row(&mut self, ui: &mut Ui, item: &MediaItem, position: usize) {
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
            .any(|index| index == item.media_index);
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 96.0), egui::Sense::click());
        let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &item.title));
        ui.painter().rect_filled(rect, 8.0, colors.surface);
        let thumbnail =
            egui::Rect::from_min_size(rect.min + Vec2::splat(1.0), Vec2::new(92.0, 94.0));
        if item.preview_image.size().width > 0 {
            self.paint_media(ui, &item.preview_image, thumbnail, 7);
        } else {
            let icon = match item.media_type.as_str() {
                "photo" => Icon::Photo,
                "video" => Icon::Video,
                "animated_gif" => Icon::Film,
                _ => Icon::Text,
            };
            icon.image(colors.secondary, 24.0).paint_at(
                ui,
                egui::Rect::from_center_size(thumbnail.center(), Vec2::splat(24.0)),
            );
        }
        self.media_number(
            ui,
            egui::Rect::from_min_max(
                thumbnail.left_bottom() + Vec2::new(6.0, -26.0),
                thumbnail.right_bottom() + Vec2::new(-6.0, -6.0),
            ),
            position,
        );
        let focus = crate::ui::components::navigation::focus_visible(ui, &response);
        ui.painter().rect_stroke(
            rect,
            8.0,
            egui::Stroke::new(
                if focus { 2.0 } else { 1.0 },
                if selected || focus {
                    colors.accent
                } else {
                    colors.border
                },
            ),
            egui::StrokeKind::Inside,
        );
        let title = format!(
            "{}{}",
            item.title,
            if item.is_new {
                format!(" · {}", self.tr("New"))
            } else {
                String::new()
            }
        );
        let detail = [
            item.preview_date.clone(),
            item.dimensions.clone(),
            self.tr(&item.status),
        ]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
        for (key, text, top, height, font, color) in [
            (
                "title",
                title,
                14.0,
                21.0,
                egui::FontId::new(
                    typography::BODY,
                    egui::FontFamily::Name("cera-medium".into()),
                ),
                colors.heading,
            ),
            (
                "detail",
                detail,
                40.0,
                18.0,
                egui::FontId::proportional(12.0),
                colors.secondary,
            ),
            (
                "text",
                item.tweet_text.to_string(),
                63.0,
                18.0,
                egui::FontId::proportional(12.0),
                colors.secondary,
            ),
        ] {
            let bounds = egui::Rect::from_min_size(
                rect.min + Vec2::new(106.0, top),
                Vec2::new((rect.width() - 212.0).max(0.0), height),
            );
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt((item.media_index, key))
                    .max_rect(bounds)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            child.add(egui::Label::new(RichText::new(text).font(font).color(color)).truncate());
        }
        for (key, icon, left, enabled) in [
            (
                "select",
                if selected {
                    Icon::SquareCheck
                } else {
                    Icon::Square
                },
                rect.right() - 88.0,
                !self.busy() && item.status != "Saved",
            ),
            (
                "download",
                if app.get_selected_tab() == crate::library::TAB && item.status == "Saved" {
                    Icon::Folder
                } else if item.status == "Saved" {
                    Icon::Check
                } else {
                    Icon::Download
                },
                rect.right() - 44.0,
                !self.busy()
                    && (app.get_selected_tab() == crate::library::TAB || item.status != "Saved"),
            ),
        ] {
            if key == "select"
                && (item.status == "Saved" || app.get_selected_tab() == crate::library::TAB)
            {
                continue;
            }
            let bounds = egui::Rect::from_min_size(
                egui::pos2(left, rect.center().y - 16.0),
                Vec2::splat(32.0),
            );
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt((item.media_index, key))
                    .max_rect(bounds)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            let caption = format!(
                "{} · {}",
                self.tr(if key == "select" {
                    if selected {
                        "Deselect media"
                    } else {
                        "Select media"
                    }
                } else {
                    if app.get_selected_tab() == crate::library::TAB && item.status == "Saved" {
                        "Open folder"
                    } else if item.status == "Missing" {
                        "Download again"
                    } else {
                        "Download file"
                    }
                }),
                item.title
            );
            let style = IconStyle {
                foreground: if key == "select" {
                    colors.accent
                } else if item.status == "Saved" {
                    Color32::from_rgb(21, 128, 61)
                } else {
                    colors.heading
                },
                radius: 6,
                ..IconStyle::ghost(colors, false)
            };
            child.add_enabled_ui(enabled, |ui| {
                if self.home_icon(ui, icon, &caption, 32.0, style).clicked() {
                    if key == "select" {
                        app.global::<MediaSelection>()
                            .invoke_toggle(item.media_index);
                    } else {
                        self.media_download_or_open(item);
                    }
                }
            });
        }
        if response.clicked() && !item.payload_pending {
            app.set_selected_media_index(item.media_index);
            app.invoke_load_media_preview(item.media_index);
        }
    }
}
