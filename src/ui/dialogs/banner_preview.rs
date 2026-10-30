use crate::media::images::{self, Kind};
use crate::ui::components::dialog::child;
use crate::ui::*;

impl Desktop {
    /// Shows the profile banner at its original size with the media viewer's zoom controls.
    pub(in crate::ui) fn banner_preview_overlay(&mut self, ctx: &egui::Context) {
        let Some(url) = self.banner_preview.clone() else {
            return;
        };
        let app = self.app.clone();
        let profile = app.get_account_profile();
        if app.get_selected_tab() != 0 || profile.banner_url.is_empty() {
            self.close_banner_preview(ctx);
            return;
        }
        // The profile card only holds a cropped banner, so the original is loaded here.
        let mut image = images::image(Kind::Preview, &url);
        if image.size().width == 0 {
            images::request(&app, Kind::Preview, url.clone());
            image = images::image(Kind::Preview, &url);
        }
        let loading = image.size().width == 0 && images::pending(Kind::Preview, &url);
        if image.size().width == 0 {
            image = profile.banner_image.clone();
        }
        let has_image = image.size().width > 0;
        self.preview_canvas.info_hidden = true;
        let fullscreen = self.preview_canvas.fullscreen;
        let viewport = ctx.viewport_rect();
        let size = if fullscreen {
            viewport.size()
        } else {
            Vec2::new(
                1600.0_f32.min(viewport.width() - 88.0),
                viewport.height() - 48.0,
            )
        };
        let mut close = false;
        let id = egui::Id::new("banner-preview");
        let modal = egui::Modal::new(id)
            .area(
                egui::Area::new(id)
                    .kind(egui::UiKind::Modal)
                    .sense(egui::Sense::hover())
                    .order(egui::Order::Foreground)
                    .fixed_pos(if fullscreen {
                        viewport.min
                    } else {
                        viewport.center() + Vec2::new(28.0 - size.x / 2.0, -size.y / 2.0)
                    })
                    .constrain(false),
            )
            .backdrop_color(Color32::TRANSPARENT)
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                ui.painter().rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(appearance::RAIL_WIDTH, viewport.top()),
                        viewport.max,
                    ),
                    0,
                    Color32::from_black_alpha(209),
                );
                let (stage, response) = ui.allocate_exact_size(size, egui::Sense::hover());
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Other,
                        true,
                        self.tr("Banner preview"),
                    )
                });
                ui.painter()
                    .rect_filled(stage, if fullscreen { 0 } else { 18 }, Color32::BLACK);
                if has_image {
                    let texture = self.media_texture(ui.ctx(), &image);
                    self.preview_media_canvas(ui, &texture, stage);
                }
                if loading || !has_image {
                    let caption = self.tr(if loading {
                        "Loading preview…"
                    } else {
                        "Preview unavailable"
                    });
                    let font = egui::FontId::proportional(typography::BODY);
                    if has_image {
                        let width = ui
                            .painter()
                            .layout_no_wrap(caption.clone(), font.clone(), Color32::WHITE)
                            .size()
                            .x
                            + 24.0;
                        let pill = egui::Rect::from_center_size(
                            egui::pos2(stage.center().x, stage.bottom() - 32.0),
                            Vec2::new(width, 28.0),
                        );
                        ui.painter()
                            .rect_filled(pill, 14, Color32::from_black_alpha(160));
                        ui.painter().text(
                            pill.center(),
                            egui::Align2::CENTER_CENTER,
                            caption,
                            font,
                            Color32::WHITE,
                        );
                    } else {
                        ui.painter().text(
                            stage.center(),
                            egui::Align2::CENTER_CENTER,
                            caption,
                            font,
                            Color32::from_gray(163),
                        );
                    }
                }
                self.preview_view_controls(ui, stage, has_image, false);
                let mut closing = child(
                    ui,
                    "banner-preview-close",
                    egui::Rect::from_min_size(
                        stage.right_top() + Vec2::new(-44.0, 10.0),
                        Vec2::splat(appearance::CONTROL_HEIGHT),
                    ),
                );
                close |= self
                    .home_icon(
                        &mut closing,
                        Icon::Close,
                        "Close preview",
                        appearance::CONTROL_HEIGHT,
                        self.preview_icon_style(18.0),
                    )
                    .clicked();
            });
        if modal.is_top_modal {
            self.preview_view_shortcuts(ctx, has_image);
        }
        close |= !fullscreen && modal.backdrop_response.clicked();
        if modal.is_top_modal
            && !egui::Popup::is_any_open(ctx)
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            if self.preview_canvas.fullscreen {
                self.preview_canvas.leave_fullscreen(ctx);
            } else {
                close = true;
            }
        }
        if close {
            self.close_banner_preview(ctx);
        }
    }

    fn close_banner_preview(&mut self, ctx: &egui::Context) {
        self.banner_preview = None;
        self.preview_canvas.leave_fullscreen(ctx);
        self.preview_canvas.reset();
        self.preview_canvas.info_hidden = false;
    }
}
