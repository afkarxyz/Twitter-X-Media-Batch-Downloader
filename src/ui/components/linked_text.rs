use crate::ui::*;
use std::ops::Range;

struct TextLink {
    bytes: Range<usize>,
    url: String,
}

fn links(text: &str) -> Vec<TextLink> {
    let mut finder = linkify::LinkFinder::new();
    finder.kinds(&[linkify::LinkKind::Url]);
    finder.url_must_have_scheme(false);
    finder
        .links(text)
        .filter_map(|link| {
            let raw = link.as_str();
            let url = if raw.contains("://") {
                raw.to_owned()
            } else {
                format!("https://{raw}")
            };
            let parsed = reqwest::Url::parse(&url).ok()?;
            (matches!(parsed.scheme(), "http" | "https") && parsed.host_str().is_some()).then(
                || TextLink {
                    bytes: link.start()..link.end(),
                    url: parsed.to_string(),
                },
            )
        })
        .collect()
}

fn link_rects(galley: &egui::Galley, chars: Range<usize>, origin: egui::Pos2) -> Vec<egui::Rect> {
    let mut offset = 0;
    let mut rects = Vec::new();
    for row in &galley.rows {
        let start = chars.start.saturating_sub(offset).min(row.glyphs.len());
        let end = chars.end.saturating_sub(offset).min(row.glyphs.len());
        if start < end {
            let first = &row.glyphs[start];
            let last = &row.glyphs[end - 1];
            rects.push(egui::Rect::from_min_max(
                origin + row.pos.to_vec2() + Vec2::new(first.pos.x, 0.0),
                origin + row.pos.to_vec2() + Vec2::new(last.pos.x + last.advance_width, row.size.y),
            ));
        }
        offset += row.glyphs.len() + usize::from(row.ends_with_newline);
    }
    rects
}

fn hover_shape(rect: egui::Rect, accent: Color32, pressed: bool) -> egui::Shape {
    egui::Shape::rect_filled(
        rect.expand2(Vec2::new(2.0, 0.0)),
        3,
        accent.gamma_multiply(if pressed { 0.22 } else { 0.12 }),
    )
}

impl Desktop {
    pub(in crate::ui) fn text_link(
        &self,
        ui: &mut Ui,
        text: impl Into<egui::WidgetText>,
    ) -> egui::Response {
        let text = text.into();
        let label = text.text().to_owned();
        let (pos, galley, response) = egui::Label::new(text)
            .sense(egui::Sense::click())
            .layout_in_ui(ui);
        let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Link, true, &label));
        self.text_link_feedback(ui, &response);
        ui.painter()
            .galley(pos, galley, ui.visuals().hyperlink_color);
        response
    }

    pub(in crate::ui) fn text_link_feedback(&self, ui: &Ui, response: &egui::Response) {
        let accent = ui.visuals().hyperlink_color;
        if response.hovered() {
            ui.painter().add(hover_shape(
                response.rect,
                accent,
                response.is_pointer_button_down_on(),
            ));
        }
        if crate::ui::components::navigation::focus_visible(ui, response) {
            ui.painter().rect_stroke(
                response.rect.expand(1.0),
                2,
                egui::Stroke::new(1.0, accent),
                egui::StrokeKind::Outside,
            );
        }
    }

    pub(in crate::ui) fn linked_text(&self, ui: &mut Ui, text: &str, size: f32, color: Color32) {
        let links = links(text);
        let mut job = egui::text::LayoutJob::default();
        job.wrap.max_width = ui.available_width().max(1.0);
        let format = egui::TextFormat {
            font_id: egui::FontId::proportional(typography::size(size)),
            color,
            line_height: Some(if size == 14.0 || size == 14.04 {
                21.0
            } else {
                size + 6.0
            }),
            ..Default::default()
        };
        let accent = ui.visuals().hyperlink_color;
        let mut offset = 0;
        for link in &links {
            job.append(&text[offset..link.bytes.start], 0.0, format.clone());
            job.append(
                &text[link.bytes.clone()],
                0.0,
                egui::TextFormat {
                    color: accent,
                    ..format.clone()
                },
            );
            offset = link.bytes.end;
        }
        job.append(&text[offset..], 0.0, format);
        let galley = ui.painter().layout_job(job);
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), galley.size().y),
            egui::Sense::hover(),
        );
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, text));
        let background = ui.painter().add(egui::Shape::Noop);
        let mut highlights = Vec::new();
        ui.painter().galley(rect.min, galley.clone(), color);
        for (index, link) in links.iter().enumerate() {
            let chars =
                text[..link.bytes.start].chars().count()..text[..link.bytes.end].chars().count();
            let rects = link_rects(&galley, chars, rect.min);
            let primary = response.id.with(("link", index, 0));
            let mut focused = false;
            let mut hovered = false;
            let mut pressed = false;
            for (row, bounds) in rects.iter().enumerate() {
                let response = ui
                    .interact(
                        *bounds,
                        response.id.with(("link", index, row)),
                        if row == 0 {
                            egui::Sense::click()
                        } else {
                            egui::Sense::CLICK
                        },
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                hovered |= response.hovered();
                pressed |= response.is_pointer_button_down_on();
                if row == 0 {
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Link,
                            true,
                            &text[link.bytes.clone()],
                        )
                    });
                    focused = crate::ui::components::navigation::focus_visible(ui, &response);
                }
                self.control_tooltip(ui, &response, &link.url);
                if response.clicked() || response.middle_clicked() {
                    ui.memory_mut(|memory| memory.request_focus(primary));
                    ui.open_url(egui::OpenUrl::new_tab(&link.url));
                }
            }
            if hovered {
                highlights.extend(
                    rects
                        .iter()
                        .map(|bounds| hover_shape(*bounds, accent, pressed)),
                );
            }
            if focused {
                for bounds in rects {
                    ui.painter().rect_stroke(
                        bounds.expand(1.0),
                        2,
                        egui::Stroke::new(1.0, accent),
                        egui::StrokeKind::Outside,
                    );
                }
            }
        }
        ui.painter().set(background, egui::Shape::Vec(highlights));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_preserve_unicode_punctuation_and_only_open_web_addresses() {
        let text = "写真 (https://example.com/a_(b)), www.example.org/end. https://例え.jp/猫\nfile:///C:/secret javascript:alert(1)";
        let found = links(text);
        assert_eq!(found.len(), 3);
        assert_eq!(&text[found[0].bytes.clone()], "https://example.com/a_(b)");
        assert_eq!(found[1].url, "https://www.example.org/end");
        assert!(found[2].url.starts_with("https://xn--"));
    }
}
