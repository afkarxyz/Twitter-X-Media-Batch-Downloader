pub(in crate::ui) mod account_search;
pub(in crate::ui) mod fetch_options;
pub(in crate::ui) mod preview;
pub(in crate::ui) mod preview_canvas;
pub(in crate::ui) mod xtractor_setup;
use crate::ui::components::dialog::child;
use crate::ui::components::dropdown::SelectOptions;
use crate::ui::*;

impl Desktop {
    pub(in crate::ui) fn dialogs(&mut self, ctx: &egui::Context) {
        let app = self.app.clone();
        self.fetch_options_dialog(ctx);
        self.account_search_dialog(ctx);
        self.settings_dialogs(ctx);
        self.history_labels_dialog(ctx);
        self.archive_dialog(ctx);
        self.update_dialog(ctx);
        self.history_confirm(ctx);
        self.toast(ctx);
        if !app.get_archive_conversion_open() {
            self.keep_original_gifs = true;
        }
    }

    fn archive_dialog(&mut self, ctx: &egui::Context) {
        let app = self.app.clone();
        let first = crate::ui::screens::settings::dialogs::first_open(
            ctx,
            "archive-conversion",
            app.get_archive_conversion_open(),
        );
        if !app.get_archive_conversion_open() {
            self.archive_options = None;
            return;
        }
        let mut keep = self.keep_original_gifs;
        let (mut quality, mut resolution) = self.archive_options.unwrap_or((
            app.get_gif_quality_index().clamp(0, 1) as usize,
            app.get_gif_resolution_index().clamp(0, 3) as usize,
        ));
        let mut closed = !egui::Popup::is_any_open(ctx)
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        closed|=self.preference_modal(ctx,"archive-conversion",520.0,|ui|{
            self.setting_text(ui,"Convert downloaded GIFs",17.0,true,self.colors(ui).heading);
            self.setting_text(ui,&self.tr("Convert {} GIF MP4 files from this download batch.").replace("{}",&app.get_archive_gif_count().to_string()),14.0,false,self.colors(ui).secondary);
            let width=(ui.available_width()-12.0)/2.0;
            ui.horizontal(|ui|{
                ui.spacing_mut().item_spacing.x=12.0;
                for (label,selected,choices) in [("Conversion mode",&mut quality,vec![self.tr("Fast"),self.tr("Better")]),("Resolution",&mut resolution,vec![self.tr("Original"),"800px".into(),"600px".into(),"400px".into()])]{
                    ui.vertical(|ui|{ui.set_width(width);ui.spacing_mut().item_spacing.y=6.0;
                        self.setting_text(ui,label,12.0,false,self.colors(ui).heading);
                        let (rect,_)=ui.allocate_exact_size(Vec2::new(width, appearance::CONTROL_HEIGHT),egui::Sense::hover());
                        *selected=self.choice_picker(ui,rect,&SelectOptions::new(label,&choices),*selected);
                        if first&&label=="Conversion mode"{ui.memory_mut(|m|m.request_focus(ui.id().with(("select",label))));}
                    });
                }
            });
            let (row,_)=ui.allocate_exact_size(Vec2::new(ui.available_width(),22.0),egui::Sense::hover());
            self.history_text(ui,egui::Rect::from_min_max(row.min,row.right_bottom()-Vec2::new(50.0,0.0)),self.tr("Keep original MP4 files"),14.0,false,self.colors(ui).heading);
            self.switch_control(ui,egui::Rect::from_min_size(row.right_top()-Vec2::new(38.0,0.0),Vec2::new(38.0,22.0)),&mut keep,"Keep original MP4 files",true);
            self.setting_caption(ui,if keep{"Downloads will list both the GIF and its original MP4."}else{"Original MP4 files are removed only after conversion and history saving succeed."});
            let mut close=false;let width=(ui.available_width()-10.0)/2.0;
            ui.horizontal(|ui|{
                ui.spacing_mut().item_spacing.x=10.0;
                close|=self.setting_action(ui,"Cancel",None,0,Vec2::new(width, appearance::CONTROL_HEIGHT),true).clicked();
                if self.setting_action(ui,"Convert",None,1,Vec2::new(width, appearance::CONTROL_HEIGHT),!self.busy()&&!app.get_archive_conversion_busy()&&app.get_archive_gif_count()>0).clicked(){
                    app.invoke_convert_archived_gifs(keep,quality as i32,resolution as i32);close=true;
                }
            });close
        });
        self.keep_original_gifs = keep;
        self.archive_options = Some((quality, resolution));
        if closed {
            app.set_archive_conversion_open(false);
        }
    }

    fn update_dialog(&self, ctx: &egui::Context) {
        let app = self.app.clone();
        let open = !app.get_startup_update_version().is_empty()
            && app.get_notify_updates()
            && !self.busy();
        let first = crate::ui::screens::settings::dialogs::first_open(ctx, "startup-update", open);
        if !open {
            return;
        }
        let mut close = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        close |= self.preference_modal(ctx, "startup-update", 440.0, |ui| {
            self.setting_text(ui, "Update available", 17.0, true, self.colors(ui).heading);
            self.setting_text(
                ui,
                &self
                    .tr("Version {} is available. Open the release page to download it.")
                    .replace("{}", &app.get_startup_update_version()),
                14.0,
                false,
                self.colors(ui).body,
            );
            let width = (ui.available_width() - 10.0) / 2.0;
            let mut close = false;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let later = self.setting_action(
                    ui,
                    "Later",
                    None,
                    0,
                    Vec2::new(width, appearance::CONTROL_HEIGHT),
                    true,
                );
                if first {
                    later.request_focus();
                }
                close |= later.clicked();
                if self
                    .setting_action(
                        ui,
                        "Open release",
                        None,
                        1,
                        Vec2::new(width, appearance::CONTROL_HEIGHT),
                        true,
                    )
                    .clicked()
                {
                    app.invoke_open_link(
                        "https://github.com/afkarxyz/Twitter-X-Media-Batch-Downloader/releases"
                            .into(),
                    );
                    close = true;
                }
            });
            close
        });
        if close {
            app.set_startup_update_version(String::new());
        }
    }

    fn toast(&self, ctx: &egui::Context) {
        let app = &self.app;
        if app.get_toast_page() >= 0 && app.get_selected_tab() != app.get_toast_page() {
            return;
        }
        let shown = app.get_toast_visible();
        let opacity = ctx.animate_bool_with_time(egui::Id::new("toast-opacity"), shown, 0.14);
        if opacity <= 0.0 || app.get_toast_message().is_empty() {
            return;
        }
        egui::Area::new(egui::Id::new("toast"))
            .anchor(
                egui::Align2::RIGHT_BOTTOM,
                [-16.0, -16.0 + 18.0 * (1.0 - opacity)],
            )
            .order(egui::Order::Tooltip)
            .show(ctx, |ui| {
                ui.set_opacity(opacity);
                if !shown {
                    ui.disable();
                }
                let colors = self.colors(ui);
                let width = 420.0_f32.min(ctx.content_rect().width() - 32.0);
                let error = app.get_toast_kind() == "error";
                let text = crate::ui::components::dialog::galley(
                    ctx,
                    app.get_toast_message(),
                    width - if error { 132.0 } else { 100.0 },
                    14.0,
                    false,
                    colors.body,
                );
                let (rect, _) = ui.allocate_exact_size(
                    Vec2::new(width, (text.size().y + 32.0).max(56.0)),
                    egui::Sense::hover(),
                );
                ui.painter().add(
                    egui::epaint::Shadow {
                        offset: [0, 6],
                        blur: 16,
                        spread: 0,
                        color: Color32::from_black_alpha(56),
                    }
                    .as_shape(rect, 8),
                );
                ui.painter().rect_filled(rect, 8, colors.surface);
                let text_top = rect.center().y - text.size().y / 2.0;
                let icon_y = text_top + 10.0;
                (if error { Icon::Error } else { Icon::Info })
                    .image(
                        if error {
                            Color32::from_rgb(183, 55, 49)
                        } else {
                            colors.accent
                        },
                        20.0,
                    )
                    .paint_at(
                        ui,
                        egui::Rect::from_center_size(
                            egui::pos2(rect.left() + 26.0, icon_y),
                            Vec2::splat(20.0),
                        ),
                    );
                ui.painter()
                    .galley(egui::pos2(rect.left() + 48.0, text_top), text, colors.body);
                for (show, icon, label, offset) in [
                    (error, Icon::Copy, "Copy error details", 76.0),
                    (true, Icon::Close, "Clear", 44.0),
                ] {
                    if show {
                        let target = egui::Rect::from_center_size(
                            egui::pos2(rect.right() - offset + 16.0, icon_y),
                            Vec2::splat(32.0),
                        );
                        let mut control = child(ui, label, target);
                        if self.setting_icon(&mut control, icon, label, 32.0).clicked() {
                            if error && matches!(icon, Icon::Copy) {
                                app.invoke_copy_error();
                            } else {
                                app.invoke_dismiss_toast();
                            }
                        }
                    }
                }
            });
    }
}
