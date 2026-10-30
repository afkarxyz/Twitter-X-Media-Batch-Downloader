use crate::ui::components::dialog::child;
use crate::ui::components::dropdown::SelectOptions;
use crate::ui::components::icon_button::IconStyle;
use crate::ui::*;

macro_rules! setting_switch {
    ($this:expr,$ui:expr,$get:ident,$set:ident,$title:expr,$description:expr,$enabled:expr) => {{
        let mut value = $this.app.$get();
        let response = $this.setting_switch($ui, &mut value, $title, $description, $enabled);
        if response.changed() {
            $this.app.$set(value);
            $this.app.invoke_settings_changed();
        }
        response
    }};
}
macro_rules! setting_choice {
    ($this:expr,$ui:expr,$get:ident,$set:ident,$title:expr,$description:expr,$choices:expr,$enabled:expr) => {{
        let choices = $choices
            .iter()
            .map(|value| $this.tr(value))
            .collect::<Vec<_>>();
        let index = $this.app.$get();
        let next = $this.setting_select(
            $ui,
            $title,
            $description,
            &choices,
            index.max(0) as usize,
            $enabled,
        );
        if next != index.max(0) as usize {
            $this.app.$set(next as i32);
            $this.app.invoke_settings_changed();
        }
    }};
}
macro_rules! setting_pills {
    ($this:expr,$ui:expr,$get:ident,$set:ident,$title:expr,$choices:expr) => {{
        let choices = $choices
            .iter()
            .map(|value| $this.tr(value))
            .collect::<Vec<_>>();
        let index = $this.app.$get();
        let next = $this.setting_pills($ui, $title, &choices, index.max(0) as usize);
        if next != index.max(0) as usize {
            $this.app.$set(next as i32);
            $this.app.invoke_settings_changed();
        }
    }};
}
pub(in crate::ui) use {setting_choice, setting_pills, setting_switch};

impl Desktop {
    pub(in crate::ui) fn setting_card<R>(
        &mut self,
        ui: &mut Ui,
        label: &str,
        spacing: f32,
        content: impl FnOnce(&mut Self, &mut Ui) -> R,
    ) -> R {
        let colors = self.colors(ui);
        let width = ui.available_width();
        let result = egui::Frame::new()
            .fill(colors.surface)
            .stroke(egui::Stroke::new(1.0, colors.border))
            .corner_radius(8)
            .inner_margin(15)
            .show(ui, |ui| {
                ui.set_width(width - 32.0);
                ui.spacing_mut().item_spacing.y = spacing;
                ui.push_id(label, |ui| content(self, ui)).inner
            });
        result.response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Other,
                true,
                format!("{label} settings card"),
            )
        });
        result.inner
    }

    pub(in crate::ui) fn setting_text(
        &self,
        ui: &mut Ui,
        text: &str,
        size: f32,
        bold: bool,
        color: Color32,
    ) {
        let text = self.tr(text);
        let font = egui::FontId::new(
            typography::size(size),
            if bold {
                egui::FontFamily::Name("cera-bold".into())
            } else {
                egui::FontFamily::Proportional
            },
        );
        let mut job =
            egui::text::LayoutJob::simple(text.clone(), font, color, ui.available_width());
        job.sections[0].format.line_height = Some(if size == 14.04 {
            21.0
        } else if size == 16.0 {
            24.0
        } else if size == 13.0 {
            18.0
        } else if size == 12.0 {
            17.0
        } else {
            size + 6.0
        });
        let galley = ui.painter().layout_job(job);
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), galley.size().y),
            egui::Sense::hover(),
        );
        ui.painter().galley(rect.min, galley, color);
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, text.clone()));
    }

    pub(in crate::ui) fn setting_heading(&self, ui: &mut Ui, title: &str) {
        self.setting_text(ui, title, 16.0, true, self.colors(ui).heading);
    }
    pub(in crate::ui) fn setting_caption(&self, ui: &mut Ui, text: &str) {
        self.setting_text(ui, text, 12.0, false, self.colors(ui).secondary);
    }
    pub(in crate::ui) fn setting_error(&self, ui: &mut Ui, text: &str) {
        if !text.is_empty() {
            self.setting_text(ui, text, 12.0, false, Color32::from_rgb(183, 55, 49));
        }
    }
    pub(in crate::ui) fn setting_divider(&self, ui: &mut Ui) {
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 0, self.colors(ui).border);
    }

    pub(in crate::ui) fn setting_action(
        &self,
        ui: &mut Ui,
        label: &str,
        icon: Option<Icon>,
        tone: i32,
        size: Vec2,
        enabled: bool,
    ) -> egui::Response {
        ui.add_enabled_ui(enabled, |ui| {
            self.action_button(
                ui,
                &self.tr(label),
                ActionStyle {
                    icon: icon.map(|icon| (icon, appearance::CONTROL_ICON_SIZE)),
                    tone: match tone {
                        1 => ButtonTone::Primary,
                        2 => ButtonTone::Destructive,
                        _ => ButtonTone::Neutral,
                    },
                    ..ActionStyle::new(Vec2::new(size.x, appearance::CONTROL_HEIGHT))
                },
            )
        })
        .inner
    }

    pub(in crate::ui) fn setting_input(
        &self,
        ui: &mut Ui,
        label: (&str, &str),
        value: &mut String,
        read_only: bool,
    ) -> egui::Response {
        let colors = self.colors(ui);
        let (rect, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), appearance::CONTROL_HEIGHT),
            egui::Sense::hover(),
        );
        ui.painter().rect_filled(rect, 6, colors.surface);
        let border = ui.painter().add(egui::Shape::Noop);
        let inner = rect.shrink2(Vec2::new(12.0, 6.0));
        let mut field = child(ui, label.0, inner);
        let id = field.id().with("edit");
        let mut read_buffer = value.as_str();
        let edit = if read_only {
            egui::TextEdit::singleline(&mut read_buffer)
        } else {
            egui::TextEdit::singleline(value)
        };
        let mut response = field.add(
            edit.id(id)
                .frame(egui::Frame::NONE)
                .margin(egui::Margin::ZERO)
                .desired_width(inner.width())
                .min_size(inner.size())
                .vertical_align(egui::Align::Center)
                .text_color(colors.heading)
                .hint_text(RichText::new(self.tr(label.1)).color(colors.placeholder)),
        );
        ui.painter().set(
            border,
            egui::Shape::rect_stroke(
                rect,
                6,
                egui::Stroke::new(
                    1.0,
                    if response.has_focus() {
                        colors.accent
                    } else {
                        colors.input_border
                    },
                ),
                egui::StrokeKind::Inside,
            ),
        );
        if self.text_context_menu(&response, value, read_only) {
            response.mark_changed();
        }
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::TextEdit,
                ui.is_enabled(),
                self.tr(label.0),
            )
        });
        response
    }

    pub(in crate::ui) fn setting_select(
        &self,
        ui: &mut Ui,
        title: &str,
        description: &str,
        choices: &[String],
        selected: usize,
        enabled: bool,
    ) -> usize {
        let colors = self.colors(ui);
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 58.0), egui::Sense::hover());
        self.history_text(
            ui,
            egui::Rect::from_min_size(
                rect.min + Vec2::new(16.0, 16.0),
                Vec2::new(rect.width() - 260.0, 20.0),
            ),
            self.tr(title),
            14.0,
            true,
            colors.heading,
        );
        self.history_text(
            ui,
            egui::Rect::from_min_size(
                rect.min + Vec2::new(16.0, 38.0),
                Vec2::new(rect.width() - 260.0, 17.0),
            ),
            self.tr(description),
            12.0,
            false,
            colors.secondary,
        );
        let picker = egui::Rect::from_min_size(
            rect.right_top() + Vec2::new(-218.0, 8.0),
            Vec2::new(210.0, appearance::CONTROL_HEIGHT),
        );
        let mut input = child(ui, title, picker);
        if !enabled {
            input.disable();
        }
        self.choice_picker(
            &mut input,
            picker,
            &SelectOptions::new(title, choices),
            selected,
        )
    }

    pub(in crate::ui) fn setting_switch(
        &self,
        ui: &mut Ui,
        value: &mut bool,
        title: &str,
        description: &str,
        enabled: bool,
    ) -> egui::Response {
        let colors = self.colors(ui);
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 48.0), egui::Sense::hover());
        let mut row = child(ui, title, rect);
        if !enabled {
            row.disable();
        }
        self.history_text(
            &row,
            egui::Rect::from_min_size(
                rect.min + Vec2::new(16.0, 16.0),
                Vec2::new(rect.width() - 86.0, 17.0),
            ),
            self.tr(title),
            14.0,
            true,
            colors.heading,
        );
        self.history_text(
            &row,
            egui::Rect::from_min_size(
                rect.min + Vec2::new(16.0, 35.0),
                Vec2::new(rect.width() - 86.0, 17.0),
            ),
            self.tr(description),
            12.0,
            false,
            colors.secondary,
        );
        let switch = egui::Rect::from_min_size(
            rect.right_top() + Vec2::new(-46.0, 8.0),
            Vec2::new(38.0, 22.0),
        );
        self.switch_control(ui, switch, value, title, enabled)
    }

    pub(in crate::ui) fn setting_pills(
        &self,
        ui: &mut Ui,
        title: &str,
        choices: &[String],
        mut selected: usize,
    ) -> usize {
        ui.push_id(title, |ui| {
            let group = ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                let (label, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 21.0),
                    egui::Sense::hover(),
                );
                self.history_text(
                    ui,
                    label,
                    self.tr(title),
                    14.04,
                    true,
                    self.colors(ui).heading,
                );
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    for (index, label) in choices.iter().enumerate() {
                        if self
                            .setting_action(
                                ui,
                                label,
                                None,
                                i32::from(index == selected),
                                Vec2::new(64.0, appearance::CONTROL_HEIGHT),
                                ui.is_enabled(),
                            )
                            .clicked()
                        {
                            selected = index;
                        }
                    }
                });
            });
            group.response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Other, true, format!("{title} options"))
            });
        });
        selected
    }

    pub(in crate::ui) fn setting_icon(
        &self,
        ui: &mut Ui,
        icon: Icon,
        label: &str,
        size: f32,
    ) -> egui::Response {
        let colors = self.colors(ui);
        let destructive = matches!(icon, Icon::Trash);
        self.home_icon(
            ui,
            icon,
            label,
            size,
            IconStyle {
                idle: Color32::TRANSPARENT,
                radius: (size / 2.0) as u8,
                icon_size: appearance::CONTROL_ICON_SIZE,
                ..if destructive {
                    IconStyle::destructive(colors)
                } else {
                    IconStyle::ghost(colors, false)
                }
            },
        )
    }

    pub(in crate::ui) fn setting_progress(
        &self,
        ui: &mut Ui,
        busy: bool,
        status: &str,
        progress: f32,
        detail: &str,
    ) {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            self.setting_text(ui, status, 14.0, false, self.colors(ui).heading);
            if busy {
                self.progress_bar(ui, progress);
            }
            if !detail.is_empty() {
                self.setting_caption(ui, detail);
            }
        });
    }
}
