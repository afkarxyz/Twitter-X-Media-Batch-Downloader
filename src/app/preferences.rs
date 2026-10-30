use crate::AppTheme;
use crate::AppWindow;
use crate::RendererSettings;
use crate::download::engine::AppSettings;

// Scope IDs match Settings categories; 5 is Fetch options and -1 is all preferences.
fn planned(current: AppSettings, scope: i32, gpu_supported: bool) -> Option<AppSettings> {
    if !matches!(scope, -1 | 0 | 2 | 3 | 5) {
        return None;
    }
    let defaults = AppSettings::default();
    let mut result = current;
    macro_rules! reset { ($($field:ident),+ $(,)?) => { $(result.$field = defaults.$field.clone();)+ }; }
    if scope == -1 || scope == 0 {
        reset!(
            legacy_dark_mode,
            theme_mode,
            theme_preset,
            language_mode,
            renderer_mode,
            fetch_timeout_secs,
            http_timeout_secs,
            proxy_url,
            notify_updates,
            results_view,
            results_sort
        );
        if !gpu_supported {
            result.renderer_mode = "cpu".into();
        }
    }
    if scope == -1 || scope == 2 {
        reset!(
            media_type_index,
            include_photos,
            include_videos,
            include_gifs,
            retweets_index,
            no_videos,
            text_tweets,
            video_quality,
            image_size,
            avatar_size,
            banner_size,
            convert_gif,
            gif_quality,
            gif_resolution,
            concurrent_downloads,
            retry_attempts,
            filename_format,
            folder_name_format,
            separate_media_folders,
            embed_metadata,
            download_speed_kib,
            download_delay_ms,
            download_jitter_ms
        );
    }
    if scope == -1 || scope == 3 {
        result.output_dir = None;
    }
    if scope == -1 || scope == 5 {
        result.incremental_fetch = defaults.incremental_fetch;
    }
    Some(result)
}

pub fn register(app: &AppWindow) {
    register_with_writer(app, crate::download::engine::save_settings);
}

pub(crate) fn register_with_writer(app: &AppWindow, write: impl Fn(AppSettings) -> bool + 'static) {
    let ui = app.as_weak();
    app.on_reset_settings(move |scope| {
        let Some(app) = ui.upgrade() else {
            return false;
        };
        if app.get_is_busy()
            || app.get_auth_check_busy()
            || app.get_batch_running()
            || app.get_history_busy()
            || app.get_ffmpeg_busy()
            || app.get_xtractor_busy()
            || app.get_tool_update_busy()
            || app.get_backup_busy()
            || app.get_archive_conversion_busy()
            || app.get_integrity_busy()
        {
            return false;
        }
        let Some(settings) = planned(
            crate::app::settings::settings_from_app(&app),
            scope,
            app.global::<RendererSettings>().get_gpu_supported(),
        ) else {
            return false;
        };
        let output = if scope == -1 || scope == 3 {
            match crate::platform::paths::output_dir(None) {
                Ok(path) => Some(path),
                Err(_) => {
                    crate::app::notifications::show_toast(
                        &app,
                        "Could not prepare the portable Downloads folder.",
                        "error",
                    );
                    return false;
                }
            }
        } else {
            None
        };
        // Failed writes leave current preferences intact.
        if !write(settings.clone()) {
            crate::app::notifications::show_toast(
                &app,
                "Could not save default settings. Please try again.",
                "error",
            );
            return false;
        }
        if scope == -1 || scope == 0 {
            app.global::<AppTheme>()
                .set_preset(settings.theme_preset.clone());
            app.set_theme_mode("system".into());
            crate::i18n::apply_language(&app, &settings.language_mode);
            app.set_notify_updates(settings.notify_updates);
            app.set_results_view(settings.results_view.clone());
            app.set_results_sort(settings.results_sort);
            app.set_fetch_timeout_secs(settings.fetch_timeout_secs as i32);
            app.set_http_timeout_secs(settings.http_timeout_secs as i32);
            crate::network::timeouts::configure(
                settings.fetch_timeout_secs,
                settings.http_timeout_secs,
            );
            let _ = crate::network::proxy::configure(&settings.proxy_url);
            app.set_proxy_url(settings.proxy_url.clone());
            app.set_proxy_error("".into());
            app.set_timeout_error("".into());
            crate::platform::renderer::choose(&app, &settings.renderer_mode, || true);
        }
        if scope == -1 || scope == 2 {
            app.set_media_type_index(settings.media_type_index);
            app.set_media_filters_custom(false);
            app.set_include_photos(true);
            app.set_include_videos(true);
            app.set_include_gifs(true);
            app.set_retweets_index(settings.retweets_index);
            app.set_no_videos(settings.no_videos);
            app.set_text_tweets(settings.text_tweets);
            app.set_video_quality_index(crate::app::settings::video_quality_index(
                &settings.video_quality,
            ));
            app.set_image_size_index(crate::app::settings::image_size_index(&settings.image_size));
            app.set_avatar_size_index(0);
            app.set_banner_size_index(0);
            app.set_convert_gif(settings.convert_gif);
            let gif = crate::download::gifs::Options::from_values(
                &settings.gif_quality,
                &settings.gif_resolution,
            );
            app.set_gif_quality_index(gif.quality_index());
            app.set_gif_resolution_index(gif.resolution_index());
            app.set_concurrent_downloads_index(settings.concurrent_downloads as i32 - 1);
            app.set_retry_attempts(settings.retry_attempts as i32);
            app.set_filename_format(settings.filename_format.clone());
            app.set_filename_preview(crate::download::engine::filename_preview(
                &settings.filename_format,
            ));
            app.set_folder_name_format(settings.folder_name_format.clone());
            app.set_folder_name_preview(crate::download::engine::folder_name_preview(
                &settings.folder_name_format,
            ));
            app.set_separate_media_folders(settings.separate_media_folders);
            app.set_embed_metadata(settings.embed_metadata);
            app.set_download_speed_kib(settings.download_speed_kib as i32);
            app.set_download_delay_ms(settings.download_delay_ms as i32);
            app.set_download_jitter_ms(settings.download_jitter_ms as i32);
            app.set_download_limits_error("".into());
        }
        if let Some(output) = output {
            app.set_output_dir(output.to_string_lossy().into_owned());
        }
        if scope == -1 || scope == 5 {
            if scope == -1 {
                app.invoke_choose_fetch_source(0);
                app.invoke_clear_batch();
                app.set_batch_mode(false);
                app.set_batch_sources_text("".into());
            } else if !app.get_batch_mode() {
                app.invoke_choose_fetch_source(0);
            }
            app.set_date_filter_enabled(false);
            app.set_fetch_start_date("".into());
            app.set_fetch_end_date("".into());
            app.set_incremental_fetch(settings.incremental_fetch);
            app.set_fetch_options_error("".into());
            if scope == 5 && app.get_batch_mode() && !app.get_batch_sources_text().trim().is_empty()
            {
                app.invoke_apply_fetch_options(
                    true,
                    false,
                    String::new(),
                    String::new(),
                    app.get_batch_sources_text(),
                );
            }
        }
        crate::app::notifications::show_toast(&app, "Default settings restored.", "info");
        true
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scoped_resets_preserve_authentication_and_other_categories() {
        let current = AppSettings {
            auth_token: "fixture-token".into(),
            theme_preset: "github".into(),
            retry_attempts: 5,
            proxy_url: "http://localhost:7890".into(),
            output_dir: Some("custom-downloads".into()),
            ..Default::default()
        };
        let downloads = planned(current.clone(), 2, true).unwrap();
        assert_eq!(downloads.auth_token, current.auth_token);
        assert_eq!(downloads.theme_preset, "github");
        assert_eq!(downloads.output_dir, current.output_dir);
        assert_eq!(
            downloads.retry_attempts,
            AppSettings::default().retry_attempts
        );
        let all = planned(current.clone(), -1, false).unwrap();
        assert_eq!(all.auth_token, current.auth_token);
        assert_eq!(all.theme_preset, "cacao");
        assert!(all.output_dir.is_none());
        assert_eq!(all.renderer_mode, "cpu");
        assert!(planned(current, 1, true).is_none()); // Cookies are deliberately not reset.
    }
}
