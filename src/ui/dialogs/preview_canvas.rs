use crate::ui::components::dialog::child;
use crate::ui::components::icon_button::IconStyle;
use crate::ui::*;

pub(in crate::ui) struct PreviewCanvas {
    pub fullscreen: bool,
    pub info_hidden: bool,
    pub zoom: f32,
    pub pan: Vec2,
    was_fullscreen: bool,
}

impl Default for PreviewCanvas {
    fn default() -> Self {
        Self {
            fullscreen: false,
            info_hidden: false,
            zoom: 1.0,
            pan: Vec2::ZERO,
            was_fullscreen: false,
        }
    }
}

impl PreviewCanvas {
    pub fn reset(&mut self) {
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
    }

    pub fn toggle_fullscreen(&mut self, ctx: &egui::Context) {
        if self.fullscreen {
            self.leave_fullscreen(ctx);
        } else {
            self.was_fullscreen = ctx.input(|input| input.viewport().fullscreen.unwrap_or(false));
            self.fullscreen = true;
            self.reset();
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
            ctx.request_repaint();
        }
    }

    pub fn leave_fullscreen(&mut self, ctx: &egui::Context) {
        if self.fullscreen {
            self.fullscreen = false;
            self.reset();
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.was_fullscreen));
            ctx.request_repaint();
        }
    }

    pub fn zoom_at(&mut self, factor: f32, anchor: Vec2) {
        let next = (self.zoom * factor).clamp(0.25, 16.0);
        self.pan = anchor + (self.pan - anchor) * (next / self.zoom);
        self.zoom = next;
    }

    fn clamp_pan(&mut self, fitted: Vec2, viewport: Vec2) {
        let maximum = ((fitted * self.zoom - viewport) / 2.0).max(Vec2::ZERO);
        self.pan = self.pan.clamp(-maximum, maximum);
    }
}

#[derive(Default)]
pub(in crate::ui) struct PreviewTexture {
    frame: Option<std::sync::Arc<egui::ColorImage>>,
    texture: Option<egui::TextureHandle>,
}

impl PreviewTexture {
    pub(in crate::ui) fn update(
        &mut self,
        ctx: &egui::Context,
        frame: &std::sync::Arc<egui::ColorImage>,
    ) -> egui::TextureHandle {
        if self
            .frame
            .as_ref()
            .is_none_or(|old| !std::sync::Arc::ptr_eq(old, frame))
        {
            let data = egui::ImageData::Color(frame.clone());
            if let Some(texture) = &mut self.texture {
                if texture.size() == frame.size {
                    // A full set recreates the GPU texture in egui-wgpu. A partial
                    // update covering the whole image retains its allocation.
                    texture.set_partial([0, 0], data, egui::TextureOptions::LINEAR);
                } else {
                    texture.set(data, egui::TextureOptions::LINEAR);
                }
            } else {
                self.texture =
                    Some(ctx.load_texture("preview-video", data, egui::TextureOptions::LINEAR));
            }
            self.frame = Some(frame.clone());
        }
        self.texture.as_ref().unwrap().clone()
    }
}

impl Desktop {
    pub(in crate::ui) fn preview_media_canvas(
        &mut self,
        ui: &mut Ui,
        texture: &egui::TextureHandle,
        stage: egui::Rect,
    ) {
        let response = ui.interact(
            stage,
            ui.id().with("media-canvas"),
            egui::Sense::click_and_drag(),
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Image, true, self.tr("Media canvas"))
        });
        if response.hovered() {
            let (wheel, pinch, anchor) = ui.input_mut(|input| {
                let values = (
                    input.smooth_scroll_delta.y,
                    input.zoom_delta(),
                    input.pointer.hover_pos().unwrap_or(stage.center()) - stage.center(),
                );
                input.smooth_scroll_delta = Vec2::ZERO;
                values
            });
            if wheel != 0.0 || pinch != 1.0 {
                self.preview_canvas
                    .zoom_at((wheel / 480.0).exp().clamp(0.5, 2.0) * pinch, anchor);
            }
        }
        if response.double_clicked() {
            if self.preview_canvas.zoom > 1.0 {
                self.preview_canvas.reset();
            } else {
                self.preview_canvas.zoom_at(
                    2.0 / self.preview_canvas.zoom,
                    response.interact_pointer_pos().unwrap_or(stage.center()) - stage.center(),
                );
            }
        }
        if response.dragged() {
            self.preview_canvas.pan += ui.input(|input| input.pointer.delta());
        }
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(if response.dragged() {
                egui::CursorIcon::Grabbing
            } else if self.preview_canvas.zoom <= 1.0 {
                egui::CursorIcon::Default
            } else {
                egui::CursorIcon::Grab
            });
        }
        let size = texture.size_vec2();
        let factor = (stage.width() / size.x).min(stage.height() / size.y);
        let fitted = Vec2::new(size.x * factor, size.y * factor);
        self.preview_canvas.clamp_pan(fitted, stage.size());
        let mut canvas = child(ui, "zoomed-media", stage);
        canvas.set_clip_rect(stage.intersect(ui.clip_rect()));
        let image_rect = egui::Rect::from_center_size(
            stage.center() + self.preview_canvas.pan,
            fitted * self.preview_canvas.zoom,
        );
        let visible = image_rect.intersect(stage);
        let uv = egui::Rect::from_min_max(
            ((visible.min - image_rect.min) / image_rect.size()).to_pos2(),
            ((visible.max - image_rect.min) / image_rect.size()).to_pos2(),
        );
        let radius = if self.preview_canvas.fullscreen {
            0
        } else {
            18
        };
        let corners = egui::CornerRadius {
            nw: if visible.left() == stage.left() && visible.top() == stage.top() {
                radius
            } else {
                0
            },
            ne: if (self.preview_canvas.info_hidden || self.preview_canvas.fullscreen)
                && visible.right() == stage.right()
                && visible.top() == stage.top()
            {
                radius
            } else {
                0
            },
            sw: if visible.left() == stage.left() && visible.bottom() == stage.bottom() {
                radius
            } else {
                0
            },
            se: if self.preview_canvas.info_hidden
                && visible.right() == stage.right()
                && visible.bottom() == stage.bottom()
            {
                radius
            } else {
                0
            },
        };
        egui::Image::new(texture)
            .uv(uv)
            .corner_radius(corners)
            .fit_to_exact_size(visible.size())
            .maintain_aspect_ratio(false)
            .paint_at(&canvas, visible);
    }

    pub(in crate::ui) fn preview_view_controls(
        &mut self,
        ui: &mut Ui,
        stage: egui::Rect,
        has_image: bool,
        text_post: bool,
    ) {
        let style = IconStyle {
            idle: Color32::from_black_alpha(140),
            hover: Color32::from_black_alpha(204),
            pressed: Color32::from_black_alpha(230),
            foreground: Color32::WHITE,
            border: egui::Stroke::NONE,
            radius: 16,
            icon_size: appearance::CONTROL_ICON_SIZE,
            visible: true,
        };
        let mut fullscreen = child(
            ui,
            "preview-fullscreen",
            egui::Rect::from_min_size(
                stage.right_top()
                    + Vec2::new(
                        if self.preview_canvas.fullscreen || self.preview_canvas.info_hidden {
                            -84.0
                        } else {
                            -44.0
                        },
                        10.0,
                    ),
                Vec2::splat(appearance::CONTROL_HEIGHT),
            ),
        );
        if self
            .home_icon(
                &mut fullscreen,
                if self.preview_canvas.fullscreen {
                    Icon::ExitFullscreen
                } else {
                    Icon::Fullscreen
                },
                if self.preview_canvas.fullscreen {
                    "Exit full screen"
                } else {
                    "Enter full screen"
                },
                appearance::CONTROL_HEIGHT,
                style,
            )
            .clicked()
        {
            self.preview_canvas.toggle_fullscreen(ui.ctx());
        }
        if text_post {
            return;
        }
        let toolbar = egui::Rect::from_center_size(
            egui::pos2(stage.center().x, stage.top() + 27.0),
            Vec2::new(178.0, appearance::CONTROL_HEIGHT),
        );
        ui.painter()
            .rect_filled(toolbar, 17, Color32::from_black_alpha(140));
        let caption = format!("{:.0}%", self.preview_canvas.zoom * 100.0);
        let readout = egui::Rect::from_min_size(
            toolbar.min + Vec2::new(36.0, 0.0),
            Vec2::new(66.0, appearance::CONTROL_HEIGHT),
        );
        ui.painter().text(
            readout.center(),
            egui::Align2::CENTER_CENTER,
            caption,
            egui::FontId::proportional(12.0),
            Color32::WHITE,
        );
        ui.interact(
            readout,
            ui.id().with("media-zoom-value"),
            egui::Sense::hover(),
        )
        .widget_info(|| {
            let mut info =
                egui::WidgetInfo::labeled(egui::WidgetType::Label, true, self.tr("Media zoom"));
            info.value = Some((self.preview_canvas.zoom * 100.0) as f64);
            info
        });
        for (label, icon, x, enabled) in [
            (
                "Zoom out",
                Icon::ZoomOut,
                2.0,
                has_image && self.preview_canvas.zoom > 0.25,
            ),
            (
                "Zoom in",
                Icon::ZoomIn,
                104.0,
                has_image && self.preview_canvas.zoom < 16.0,
            ),
            ("Fit to screen", Icon::Fit, 142.0, has_image),
        ] {
            let mut control = child(
                ui,
                label,
                egui::Rect::from_min_size(
                    toolbar.min + Vec2::new(x, 2.0),
                    Vec2::splat(appearance::COMPACT_CONTROL_SIZE),
                ),
            );
            control.add_enabled_ui(enabled, |ui| {
                if self
                    .home_icon(
                        ui,
                        icon,
                        label,
                        appearance::COMPACT_CONTROL_SIZE,
                        IconStyle {
                            idle: Color32::TRANSPARENT,
                            radius: 14,
                            ..style
                        },
                    )
                    .clicked()
                {
                    match label {
                        "Zoom in" => self.preview_canvas.zoom_at(1.25, Vec2::ZERO),
                        "Zoom out" => self.preview_canvas.zoom_at(0.8, Vec2::ZERO),
                        _ => self.preview_canvas.reset(),
                    }
                    ui.ctx().request_repaint();
                }
            });
        }
    }

    pub(in crate::ui) fn preview_view_shortcuts(&mut self, ctx: &egui::Context, has_image: bool) {
        if egui::Popup::is_any_open(ctx)
            || ctx.text_edit_focused()
            || ctx.input(|input| {
                input.modifiers.ctrl || input.modifiers.command || input.modifiers.alt
            })
        {
            return;
        }
        if ctx.input_mut(|input| {
            input.consume_key(egui::Modifiers::NONE, egui::Key::F)
                | input.consume_key(egui::Modifiers::NONE, egui::Key::F11)
        }) {
            self.preview_canvas.toggle_fullscreen(ctx);
        }
        if has_image {
            let mut changed = false;
            ctx.input_mut(|input| {
                for modifiers in [egui::Modifiers::NONE, egui::Modifiers::SHIFT] {
                    if input.consume_key(modifiers, egui::Key::Plus)
                        | input.consume_key(modifiers, egui::Key::Equals)
                    {
                        self.preview_canvas.zoom_at(1.25, Vec2::ZERO);
                        changed = true;
                    }
                    if input.consume_key(modifiers, egui::Key::Minus) {
                        self.preview_canvas.zoom_at(0.8, Vec2::ZERO);
                        changed = true;
                    }
                }
                if input.consume_key(egui::Modifiers::NONE, egui::Key::Num0) {
                    self.preview_canvas.reset();
                    changed = true;
                }
            });
            if changed {
                ctx.request_repaint();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_updates_reuse_the_gpu_allocation_and_share_the_frame_buffer() {
        use std::sync::Arc;
        let ctx = egui::Context::default();
        let mut cache = PreviewTexture::default();
        let frame = Arc::new(egui::ColorImage::filled([96, 54], Color32::RED));
        let texture = cache.update(&ctx, &frame);
        let id = texture.id();
        let changes = ctx.tex_manager().write().take_delta();
        let delta = &changes.set[&id][0];
        assert_eq!(delta.pos, None);
        let egui::ImageData::Color(upload) = &delta.image;
        assert!(Arc::ptr_eq(upload, &frame));
        let _ = cache.update(&ctx, &frame);
        assert!(ctx.tex_manager().write().take_delta().set.is_empty());
        for index in 0..120 {
            let frame = Arc::new(egui::ColorImage::filled(
                [96, 54],
                Color32::from_rgb(index, 55, 88),
            ));
            assert_eq!(cache.update(&ctx, &frame).id(), id);
            let changes = ctx.tex_manager().write().take_delta();
            assert_eq!(changes.set.len(), 1);
            assert!(changes.free.is_empty());
            let delta = &changes.set[&id][0];
            assert_eq!(
                delta.pos,
                Some([0, 0]),
                "Same-size frames must retain the GPU texture"
            );
            let egui::ImageData::Color(upload) = &delta.image;
            assert!(
                Arc::ptr_eq(upload, &frame),
                "The UI must not copy video pixels"
            );
        }
        let resized = Arc::new(egui::ColorImage::filled([54, 96], Color32::BLUE));
        let resized_texture = cache.update(&ctx, &resized);
        assert_eq!(resized_texture.id(), id);
        assert_eq!(resized_texture.size(), [54, 96]);
        let changes = ctx.tex_manager().write().take_delta();
        assert_eq!(
            changes.set[&id][0].pos, None,
            "A size change requires reallocation"
        );
        drop(texture);
        drop(resized_texture);
        drop(cache);
        assert!(ctx.tex_manager().write().take_delta().free.contains(&id));
    }

    #[test]
    fn zoom_preserves_the_anchor_limits_scale_and_clamps_panning() {
        let mut canvas = PreviewCanvas::default();
        let anchor = Vec2::new(100.0, -50.0);
        canvas.zoom_at(2.0, anchor);
        assert_eq!((anchor - canvas.pan) / canvas.zoom, anchor);
        canvas.pan = Vec2::new(999.0, -999.0);
        canvas.clamp_pan(Vec2::new(400.0, 300.0), Vec2::new(400.0, 300.0));
        assert_eq!(canvas.pan, Vec2::new(200.0, -150.0));
        canvas.zoom_at(1000.0, Vec2::ZERO);
        assert_eq!(canvas.zoom, 16.0);
        canvas.zoom_at(0.0001, Vec2::ZERO);
        assert_eq!(canvas.zoom, 0.25);
        canvas.clamp_pan(Vec2::new(400.0, 300.0), Vec2::new(400.0, 300.0));
        assert_eq!(canvas.pan, Vec2::ZERO);
        canvas.reset();
        assert_eq!(canvas.zoom, 1.0);
    }

    #[test]
    fn exiting_the_viewer_preserves_an_already_fullscreen_window() {
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .fullscreen = Some(true);
        let output = ctx.run_ui(input, |ui| {
            let ctx = ui.ctx();
            let mut canvas = PreviewCanvas::default();
            canvas.toggle_fullscreen(ctx);
            canvas.leave_fullscreen(ctx);
            assert!(!canvas.fullscreen);
        });
        let modes = output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .iter()
            .filter_map(|command| {
                if let egui::ViewportCommand::Fullscreen(value) = command {
                    Some(*value)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(modes, [true, true]);
    }
}
