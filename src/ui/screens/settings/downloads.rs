use crate::ui::screens::settings::controls::{setting_choice, setting_pills, setting_switch};
use crate::ui::*;

impl Desktop {
    pub(in crate::ui) fn download_settings(&mut self, ui: &mut Ui) {
        self.setting_card(ui, "Extraction defaults", 12.0, |this, ui| {
            let app = this.app.clone();
            let enabled = !this.busy();
            this.setting_heading(ui, "Extraction defaults");
            ui.vertical(|ui| {
                let (group, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 58.0),
                    egui::Sense::hover(),
                );
                this.history_text(
                    ui,
                    egui::Rect::from_min_size(group.min, Vec2::new(group.width(), 20.0)),
                    this.tr("Media types"),
                    14.0,
                    true,
                    this.colors(ui).heading,
                );
                let flags = crate::fetch::request::media_flags(&app);
                let mut values = [flags.0, flags.1, flags.2, app.get_text_tweets()];
                let mut changed = false;
                {
                    for (index, (label, icon)) in [
                        ("Image", Icon::Photo),
                        ("Video", Icon::Video),
                        ("GIF", Icon::Film),
                        ("Text", Icon::Text),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let mut option = crate::ui::components::dialog::child(
                            ui,
                            label,
                            egui::Rect::from_min_size(
                                group.min + Vec2::new(index as f32 * 118.0, 26.0),
                                Vec2::new(110.0, 32.0),
                            ),
                        );
                        if !enabled {
                            option.disable();
                        }
                        let response = this.action_button(
                            &mut option,
                            &this.tr(label),
                            ActionStyle {
                                icon: Some((icon, appearance::CONTROL_ICON_SIZE)),
                                tone: if values[index] {
                                    ButtonTone::Selected
                                } else {
                                    ButtonTone::Neutral
                                },
                                exact_width: true,
                                ..ActionStyle::new(Vec2::new(110.0, 32.0))
                            },
                        );
                        response.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::WidgetType::Checkbox,
                                enabled,
                                values[index],
                                this.tr(label),
                            )
                        });
                        if response.clicked() {
                            values[index] = !values[index];
                            changed = true;
                        }
                    }
                }
                if changed {
                    app.set_include_photos(values[0]);
                    app.set_include_videos(values[1]);
                    app.set_include_gifs(values[2]);
                    app.set_text_tweets(values[3]);
                    app.set_media_filters_custom(true);
                    app.set_no_videos(false);
                    app.invoke_settings_changed();
                }
            });
            setting_choice!(
                this,
                ui,
                get_retweets_index,
                set_retweets_index,
                "Retweets",
                "Choose whether xtractor skips or includes reposts.",
                ["Skip", "Include"],
                true
            );
            this.setting_divider(ui);
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 48.0), egui::Sense::hover());
            this.history_text(
                ui,
                rect,
                this.tr("Choose one or more media types. Result filters can also be combined."),
                12.0,
                false,
                this.colors(ui).secondary,
            );
            this.setting_caption(
                ui,
                "Show text-only posts as cards and save them as UTF-8 TXT files.",
            );
        });
        self.setting_card(ui, "Media quality", 12.0, |this, ui| {
            let app = this.app.clone();
            this.setting_heading(ui, "Media quality");
            let before = app.get_convert_gif();
            setting_switch!(
                this,
                ui,
                get_convert_gif,
                set_convert_gif,
                "Auto Convert GIF",
                "Save X GIF videos as animated .gif files using FFmpeg.",
                !app.get_ffmpeg_busy()
            );
            if !before && app.get_convert_gif() {
                app.invoke_ensure_ffmpeg();
            }
            if app.get_convert_gif() {
                if (app.get_ffmpeg_busy() || !app.get_ffmpeg_ready())
                    && !app.get_ffmpeg_status().is_empty()
                {
                    this.setting_progress(
                        ui,
                        app.get_ffmpeg_busy(),
                        &this.tr(&app.get_ffmpeg_status()),
                        app.get_ffmpeg_progress(),
                        &app.get_ffmpeg_detail(),
                    );
                }
                if !app.get_ffmpeg_busy()
                    && !app.get_ffmpeg_ready()
                    && !app.get_ffmpeg_status().is_empty()
                    && this
                        .setting_action(
                            ui,
                            "Retry FFmpeg download",
                            None,
                            0,
                            Vec2::new(100.0, 32.0),
                            true,
                        )
                        .clicked()
                {
                    app.invoke_ensure_ffmpeg();
                }
                setting_pills!(
                    this,
                    ui,
                    get_gif_quality_index,
                    set_gif_quality_index,
                    "GIF conversion mode",
                    ["Fast", "Better"]
                );
                setting_pills!(
                    this,
                    ui,
                    get_gif_resolution_index,
                    set_gif_resolution_index,
                    "GIF resolution",
                    ["Original", "High (800px)", "Medium (600px)", "Low (400px)"]
                );
                this.setting_caption(
                    ui,
                    &if app.get_gif_quality_index() == 0 {
                        this.tr("Fast keeps the source frame rate.")
                    } else {
                        format!(
                            "{} {}",
                            this.tr("Better uses an optimized color palette."),
                            this.tr("{} FPS").replace(
                                "{}",
                                match app.get_gif_resolution_index() {
                                    2 => "10",
                                    3 => "8",
                                    _ => "15",
                                }
                            )
                        )
                    },
                );
            }
            let choices = (1..=50)
                .map(|value| value.to_string())
                .chain(std::iter::once(this.tr("Auto")))
                .collect::<Vec<_>>();
            let current = app.get_concurrent_downloads_index().clamp(0, 50) as usize;
            let selected = this.setting_select(
                ui,
                "Concurrent downloads",
                "Choose 1–50 downloads, or Auto: images 8, video/GIF 3, mixed 10.",
                &choices,
                current,
                true,
            );
            if selected != current {
                app.set_concurrent_downloads_index(selected as i32);
                app.invoke_settings_changed();
            }
            ui.add_space(-1.0);
            setting_pills!(
                this,
                ui,
                get_video_quality_index,
                set_video_quality_index,
                "Video quality",
                ["Highest", "720p", "480p", "360p", "Lowest"]
            );
            setting_pills!(
                this,
                ui,
                get_retry_attempts,
                set_retry_attempts,
                "Download retries",
                ["0", "1", "2", "3", "4", "5"]
            );
            setting_pills!(
                this,
                ui,
                get_image_size_index,
                set_image_size_index,
                "Image size",
                ["Original", "Large", "Medium", "Small"]
            );
            setting_pills!(
                this,
                ui,
                get_avatar_size_index,
                set_avatar_size_index,
                "Avatar size",
                ["Original", "400px", "200px", "Bigger", "Normal"]
            );
            setting_pills!(
                this,
                ui,
                get_banner_size_index,
                set_banner_size_index,
                "Banner size",
                ["Original", "1500 × 500", "600 × 200", "300 × 100"]
            );
        });
        self.setting_format(ui, false);
        self.setting_format(ui, true);
        let (speed,delay,jitter)=self.setting_card(ui,"Download organization and limits",12.0,|this,ui|{
            let app=this.app.clone();let enabled=!this.busy();this.setting_heading(ui,"Download organization and limits");
            setting_switch!(this,ui,get_separate_media_folders,set_separate_media_folders,"Separate media folders","Store media in Image, Video and GIF subfolders inside each account folder.",enabled);
            setting_switch!(this,ui,get_embed_metadata,set_embed_metadata,"Embed post metadata","Write the post URL, original media name and post text into JPG/MP4 comments.",enabled);
            if app.get_embed_metadata(){this.setting_caption(ui,"ExifTool is downloaded into the portable data folder when missing. Applies to new JPG/MP4 downloads; converted GIFs are excluded.");}
            let mut speed=this.speed.clone();let mut delay=this.delay.clone();let mut jitter=this.jitter.clone();let mut enter=false;
            let width=(ui.available_width()-12.0)/2.0;
            ui.horizontal(|ui|{ui.spacing_mut().item_spacing.x=12.0;for (label,value) in [("Total speed limit (KiB/s)",&mut speed),("Delay between requests (ms)",&mut delay)] {ui.vertical(|ui|{
                ui.set_width(width);ui.spacing_mut().item_spacing.y=6.0;this.setting_caption(ui,label);ui.add_enabled_ui(enabled,|ui|{let response=this.setting_input(ui,(label,""),value, false);enter|=response.lost_focus()&&ui.input(|input|input.key_pressed(egui::Key::Enter));});
            });}});
            ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), appearance::CONTROL_HEIGHT), egui::Layout::right_to_left(egui::Align::Center),|ui|{
                ui.add_enabled_ui(enabled,|ui|{ui.set_max_width(160.0);let response=this.setting_input(ui,("Random extra delay (ms)",""),&mut jitter, false);enter|=response.lost_focus()&&ui.input(|input|input.key_pressed(egui::Key::Enter));});
                this.setting_caption(ui,"Random extra delay (ms)");
            });
            this.setting_caption(ui,"Adds a random 0 to this many milliseconds between download requests, including retries. 0 disables jitter.");
            ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), 32.0), egui::Layout::right_to_left(egui::Align::Center),|ui|{
                let clicked=this.setting_action(ui,"Apply",Some(Icon::Check),0,Vec2::new(100.0,32.0),enabled).clicked();
                if enabled&&(clicked||enter){app.invoke_apply_download_limits(speed.clone(),delay.clone(),jitter.clone());}
                this.setting_caption(ui,"0 disables the limit. The speed budget is shared by parallel downloads.");
            });this.setting_error(ui,&app.get_download_limits_error());(speed,delay,jitter)
        });
        self.speed = speed;
        self.delay = delay;
        self.jitter = jitter;
    }

    fn setting_format(&mut self, ui: &mut Ui, folder: bool) {
        self.setting_card(ui,if folder{"Folder name format"}else{"Filename format"},10.0,|this,ui|{
            let app=this.app.clone();let title=if folder{"Folder name format"}else{"Filename format"};this.setting_heading(ui,title);
            let mut value=if folder{app.get_folder_name_format()}else{app.get_filename_format()};let mut changed=false;let mut field_id=None;
            let default=if folder{"{handle}"}else{"{date}_{handle}_{tweet_id}_{index}"};
            ui.horizontal(|ui|{ui.spacing_mut().item_spacing.x=8.0;
                ui.vertical(|ui|{ui.set_width((ui.available_width()-108.0).max(100.0));let response=this.setting_input(ui,(if folder{"Folder name format"}else{"Downloaded media filename format"},default),&mut value, false);changed|=response.changed();field_id=Some(response.id);});
                if this.setting_action(ui,"Reset",Some(Icon::Reset),2,Vec2::new(100.0, appearance::CONTROL_HEIGHT),true).clicked(){value=default.into();changed=true;}
            });
            let rows:Vec<&[&str]>=if folder{vec![&["{handle}","{name}","{date}"]]}else{vec![&["{handle}","{tweet_id}","{index}","{type}","{date}"],&["{post_date}","{post_time}","{post_datetime}","{name}","{media_id}"]]};
            for tokens in rows{ui.horizontal(|ui|{ui.spacing_mut().item_spacing.x=6.0;for token in tokens{
                if this.action_button(ui,token,ActionStyle{measure_regular:true,measure_bold:false,..ActionStyle::new(Vec2::new(0.0,appearance::CONTROL_HEIGHT))}).clicked(){value.push_str(token);changed=true;
                    if let Some(id)=field_id{let mut state=egui::text_edit::TextEditState::load(ui.ctx(),id).unwrap_or_default();state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(value.chars().count()))));state.store(ui.ctx(),id);ui.memory_mut(|m|m.request_focus(id));}
                }
            }});}
            if changed{if folder{app.set_folder_name_format(value);}else{app.set_filename_format(value);}app.invoke_settings_changed();}
            this.setting_caption(ui,if folder{"Folder name tokens: {handle} (author handle), {name} (display name), and {date} (download date in YYYYMMDD format)."}else{"Post date/time use UTC. Date uses the download date. Unknown post dates use unknown. The extension is added automatically."});
            let (rect,_)=ui.allocate_exact_size(Vec2::new(ui.available_width(),52.0),egui::Sense::hover());let colors=this.colors(ui);
            ui.painter().rect_filled(rect,6,colors.surface);ui.painter().rect_stroke(rect,6,egui::Stroke::new(1.0,colors.border),egui::StrokeKind::Inside);
            this.history_text(ui,egui::Rect::from_min_size(rect.min+Vec2::new(10.0,6.0),Vec2::new(rect.width()-20.0,16.0)),this.tr("Preview"),10.0,true,colors.secondary);
            this.history_text(ui,egui::Rect::from_min_size(rect.min+Vec2::new(10.0,26.0),Vec2::new(rect.width()-20.0,17.0)),if folder{app.get_folder_name_preview()}else{app.get_filename_preview()},12.0,false,colors.heading);
        });
    }
}
