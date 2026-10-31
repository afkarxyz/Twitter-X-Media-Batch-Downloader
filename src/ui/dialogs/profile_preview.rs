use crate::media::images::{self, Kind};
use crate::ui::components::dialog::child;
use crate::ui::components::icon_button::IconStyle;
use crate::ui::*;

impl Desktop {
    pub(in crate::ui) fn open_profile_preview(&mut self, kind: &'static str, url: &str) {
        self.profile_preview = Some((
            kind,
            crate::download::engine::original_profile_image_url(kind, url),
        ));
    }

    /// Shows a profile avatar or banner at its original size with the media viewer's
    /// zoom controls and a button to download it.
    pub(in crate::ui) fn profile_preview_overlay(&mut self, ctx: &egui::Context) {
        let Some((kind, url)) = self.profile_preview.clone() else {
            return;
        };
        let app = self.app.clone();
        let profile = app.get_account_profile();
        let (source, fallback) = if kind == "avatar" {
            (profile.avatar_url, profile.avatar_image)
        } else {
            (profile.banner_url, profile.banner_image)
        };
        if app.get_selected_tab() != 0 || source.is_empty() {
            self.close_profile_preview(ctx);
            return;
        }
        // The profile card only holds a reduced copy, so the original is loaded here.
        let mut image = images::image(Kind::Preview, &url);
        if image.size().width == 0 {
            images::request(&app, Kind::Preview, url.clone());
            image = images::image(Kind::Preview, &url);
        }
        let loading = image.size().width == 0 && images::pending(Kind::Preview, &url);
        if image.size().width == 0 {
            image = fallback;
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
        let (title, download) = if kind == "avatar" {
            ("Avatar preview", "Download avatar")
        } else {
            ("Banner preview", "Download banner")
        };
        let mut close = false;
        let id = egui::Id::new("profile-preview");
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
                    egui::WidgetInfo::labeled(egui::WidgetType::Other, true, self.tr(title))
                });
                ui.painter()
                    .rect_filled(stage, if fullscreen { 0 } else { 18 }, Color32::BLACK);
                if has_image {
                    let texture = self.media_texture(ui.ctx(), &image);
                    self.preview_media_canvas(ui, &texture, stage);
                }
                if loading {
                    self.preview_loading(ui, stage, has_image);
                } else if !has_image {
                    ui.painter().text(
                        stage.center(),
                        egui::Align2::CENTER_CENTER,
                        self.tr("Preview unavailable"),
                        egui::FontId::proportional(typography::BODY),
                        Color32::from_gray(163),
                    );
                }
                self.preview_view_controls(ui, stage, has_image, false);
                let mut saving = child(
                    ui,
                    "profile-preview-download",
                    egui::Rect::from_min_size(
                        stage.right_top() + Vec2::new(-124.0, 10.0),
                        Vec2::splat(appearance::CONTROL_HEIGHT),
                    ),
                );
                if self.busy() {
                    saving.disable();
                }
                if self
                    .home_icon(
                        &mut saving,
                        Icon::Download,
                        download,
                        appearance::CONTROL_HEIGHT,
                        IconStyle {
                            radius: 16,
                            icon_size: appearance::CONTROL_ICON_SIZE,
                            ..self.preview_icon_style(18.0)
                        },
                    )
                    .clicked()
                {
                    app.invoke_download_profile_image(kind.into());
                }
                let mut closing = child(
                    ui,
                    "profile-preview-close",
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
            self.close_profile_preview(ctx);
        }
    }

    fn close_profile_preview(&mut self, ctx: &egui::Context) {
        self.profile_preview = None;
        self.preview_canvas.leave_fullscreen(ctx);
        self.preview_canvas.reset();
        self.preview_canvas.info_hidden = false;
    }
}
