use super::palette;
use crate::AppTheme;
use crate::AppWindow;
use crate::I18n;
use crate::MediaSelection;
use crate::RendererSettings;
use crate::accounts::labels as account_labels;
use crate::accounts::search as account_search;
use crate::accounts::tokens as auth_tokens;
use crate::app::preferences as settings_reset;
use crate::download::commands as downloads;
use crate::download::conversion as archive_conversion;
use crate::download::engine as downloader;
use crate::download::gifs as gif_conversion;
use crate::download::integrity as file_integrity;
use crate::download::policy as download_policy;
use crate::download::saved_files;
use crate::fetch::archives as fetch_archives;
use crate::fetch::batch as batch_fetch;
use crate::fetch::request as fetch_request;
use crate::fetch::sources as fetch_sources;
use crate::history::backup as database_backup;
use crate::history::db as history_db;
use crate::history::refresh as history_refresh;
use crate::history::view_model as history_ui;
use crate::i18n;
use crate::media::images as remote_images;
use crate::media::preview as preview_player;
use crate::media::results as media_results;
use crate::network::proxy as network_proxy;
use crate::network::timeouts as network_timeouts;
use crate::platform::logging as session_log;
use crate::platform::logs as log_viewer;
use crate::platform::paths as data_paths;
use crate::platform::renderer as renderer_preferences;
use crate::platform::storage;
use crate::updates::checks as update_checks;
use crate::updates::dependencies as dependency_updates;
use crate::updates::ffmpeg as ffmpeg_setup;
use crate::updates::worker as xtractor_worker;
use crate::updates::xtractor as xtractor_update;
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use super::fetch::{
    current_profile_from_ui, empty_media_model, same_extraction_filters, spawn_extraction,
};
use super::notifications::{dismiss_toast, show_toast};
use super::settings::{
    image_size_index, image_size_value, settings_from_app, special_download_folder_name,
    video_quality_index, video_quality_value,
};
use copypasta::{ClipboardContext, ClipboardProvider};

pub async fn run() -> Result<(), crate::ui::runtime::PlatformError> {
    let migrate_legacy = std::env::args().any(|argument| argument == "--migrate-portable-data");
    let directory = if migrate_legacy {
        data_paths::migrate_legacy_data()
    } else {
        data_paths::prepare()
    }
    .map_err(|error| {
        crate::ui::runtime::PlatformError::Other(format!(
            "Could not prepare portable app data beside the executable: {error}"
        ))
    })?;
    if migrate_legacy || std::env::args().any(|argument| argument == "--prepare-portable-data") {
        println!("Portable data ready: {}", directory.display());
        return Ok(());
    }
    let _session_log = session_log::initialize(&directory)
        .map_err(|error| {
            eprintln!("Could not initialize session log: {error}");
        })
        .ok();
    i18n::initialize();
    let settings = downloader::load_settings();
    auth_tokens::initialize(&settings);
    let proxy_error = network_proxy::configure(&settings.proxy_url).err();
    network_timeouts::configure(settings.fetch_timeout_secs, settings.http_timeout_secs);
    let (active_renderer, renderer_error) =
        renderer_preferences::select_backend(&settings.renderer_mode).map_err(|error| {
            session_log::error(format!("Could not initialize renderer: {error}"));
            error
        })?;
    let startup_renderer_error = renderer_error.clone();
    let app = AppWindow::new().map_err(|error| {
        session_log::error(format!("Could not create app window: {error}"));
        error
    })?;
    media_results::register(&app);
    remote_images::register(&app);
    let _storage_monitor = storage::register(&app);
    ffmpeg_setup::register(&app);
    xtractor_update::register(&app);
    update_checks::register(&app);
    dependency_updates::register(&app);
    app.set_proxy_url(settings.proxy_url.clone());
    if let Some(error) = proxy_error {
        app.set_proxy_error(error.into());
    }
    let _log_monitor = log_viewer::register(&app, directory.join("logs"));
    {
        let ui = app.as_weak();
        app.on_copy_address(move |address| {
            if let Some(app) = ui.upgrade() {
                match ClipboardContext::new()
                    .and_then(|mut clipboard| clipboard.set_contents(address.to_string()))
                {
                    Ok(()) => show_toast(&app, "Wallet address copied.", "info"),
                    Err(_) => show_toast(&app, "Could not copy wallet address.", "error"),
                }
            }
        });
    }
    {
        let ui = app.as_weak();
        app.on_copy_error(move || {
            let Some(app) = ui.upgrade() else {
                return;
            };
            let error = app.get_last_error();
            if error.is_empty() {
                return;
            }
            let copied = ClipboardContext::new()
                .and_then(|mut clipboard| clipboard.set_contents(error.to_string()))
                .is_ok();
            show_toast(
                &app,
                if copied {
                    "Error details copied."
                } else {
                    "Could not copy error details."
                },
                "info",
            );
        });
        let ui = app.as_weak();
        app.on_open_logs(move || {
            let Some(app) = ui.upgrade() else {
                return;
            };
            let Some(path) = downloader::app_data_dir().map(|directory| directory.join("logs"))
            else {
                return;
            };
            if std::fs::create_dir_all(&path).is_err() || open::that(&path).is_err() {
                show_toast(&app, "Could not open the logs folder.", "error");
            }
        });
    }
    {
        let ui = app.as_weak();
        app.on_apply_download_limits(move |speed, delay, jitter| {
            let Some(app) = ui.upgrade() else {
                return false;
            };
            if app.get_is_busy() {
                return false;
            }
            let (speed, delay, jitter) =
                match download_policy::parse_limits(&speed, &delay, &jitter) {
                    Ok(values) => values,
                    Err(error) => {
                        app.set_download_limits_error(error.into());
                        return false;
                    }
                };
            app.set_download_speed_kib(speed as i32);
            app.set_download_delay_ms(delay as i32);
            app.set_download_jitter_ms(jitter as i32);
            app.set_download_limits_error("".into());
            app.invoke_settings_changed();
            true
        });
    }
    {
        let ui = app.as_weak();
        app.on_apply_timeouts(move |fetch, http| {
            let Some(app) = ui.upgrade() else {
                return false;
            };
            if app.get_is_busy() {
                return false;
            }
            match network_timeouts::parse(&fetch, &http) {
                Ok((fetch, http)) => {
                    network_timeouts::configure(fetch, http);
                    app.set_fetch_timeout_secs(fetch as i32);
                    app.set_http_timeout_secs(http as i32);
                    app.set_timeout_error("".into());
                    app.invoke_settings_changed();
                    true
                }
                Err(error) => {
                    app.set_timeout_error(error.into());
                    false
                }
            }
        });
    }
    app.set_account_profile(downloader::empty_account_profile());
    let ui = app.as_weak();
    i18n::register(&app);
    app.global::<I18n>().set_language("initializing".into());
    let backend = Arc::new(Mutex::new(downloader::BackendState::default()));
    let preview_player = preview_player::register(&app);
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let ui = ui.clone();
        let backend = backend.clone();
        let cancel = cancel.clone();
        app.on_download_profile_image(move |kind| {
            let Some(app) = ui.upgrade() else { return };
            if app.get_is_busy()
                || (app.get_xtractor_busy() || app.get_tool_update_busy())
                || !matches!(kind.as_str(), "avatar" | "banner")
            {
                return;
            }
            let json = backend
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .json
                .clone();
            let size = if kind == "avatar" {
                ["orig", "400x400", "200x200", "bigger", "normal"]
                    [app.get_avatar_size_index().clamp(0, 4) as usize]
            } else {
                ["orig", "1500x500", "600x200", "300x100"]
                    [app.get_banner_size_index().clamp(0, 3) as usize]
            }
            .to_owned();
            let output = PathBuf::from(app.get_output_dir().as_str());
            let folder_format = app.get_folder_name_format().to_string();
            let retry_attempts = app.get_retry_attempts().clamp(0, 5) as usize;
            let download_policy = download_policy::from_app(&app);
            cancel.store(false, Ordering::Relaxed);
            app.set_is_busy(true);
            app.set_progress(0.0);
            app.set_progress_detail("".into());
            app.set_status(format!("Downloading {kind}…"));
            let ui = ui.clone();
            let cancel = cancel.clone();
            tokio::spawn(async move {
                let result = downloader::download_profile_image(
                    json,
                    kind.to_string(),
                    size,
                    output,
                    folder_format,
                    retry_attempts,
                    ui.clone(),
                    cancel,
                    download_policy,
                )
                .await;
                let _ = crate::ui::runtime::invoke_from_event_loop(move || {
                    if let Some(app) = ui.upgrade() {
                        app.set_is_busy(false);
                        match result {
                            Ok(path) => {
                                app.set_progress(100.0);
                                show_toast(&app, format!("Saved {}", path.display()), "info");
                            }
                            Err(error) => {
                                show_toast(&app, error, "error");
                            }
                        }
                        app.set_status("Ready for an X/Twitter URL.".into());
                    }
                });
            });
        });
    }
    {
        let ui = ui.clone();
        let backend = backend.clone();
        app.on_clear_results(move || {
            let Some(app) = ui.upgrade() else { return };
            if app.get_is_busy() {
                return;
            }
            app.set_selected_media_index(-1);
            *backend.lock().unwrap_or_else(|error| error.into_inner()) =
                downloader::BackendState::default();
            app.set_media_items(empty_media_model());
            app.global::<MediaSelection>().invoke_clear();
            app.set_media_count(0);
            app.set_media_count_text("0".into());
            app.set_account_profile(downloader::empty_account_profile());
            app.set_has_output(false);
            app.set_has_more(false);
            app.set_progress(0.0);
            app.set_progress_detail("".into());
            app.set_source_url("".into());
            app.set_status(i18n::translate(
                &app.global::<I18n>().get_language(),
                "Ready for an X/Twitter URL.",
            ));
        });
    }

    {
        let renderer = app.global::<RendererSettings>();
        renderer.set_preferred(
            if settings.renderer_mode == "gpu" && renderer_preferences::GPU_SUPPORTED {
                "gpu"
            } else {
                "cpu"
            }
            .into(),
        );
        renderer.set_active(active_renderer.into());
        renderer.set_gpu_supported(renderer_preferences::GPU_SUPPORTED);
        renderer.set_error(renderer_error);
    }
    if !startup_renderer_error.is_empty() {
        show_toast(
            &app,
            format!(
                "GPU renderer could not start. Using CPU for this session: {startup_renderer_error}"
            ),
            "info",
        );
    }
    {
        let app_theme = app.global::<AppTheme>();
        let preset = palette::normalize(&settings.theme_preset);
        app_theme.set_preset(preset.into());
    }
    let theme_mode = settings
        .theme_mode
        .as_deref()
        .unwrap_or(if settings.legacy_dark_mode {
            "dark"
        } else {
            "system"
        });
    app.set_theme_mode(theme_mode.into());
    i18n::apply_language(&app, &settings.language_mode);
    app.set_status(i18n::translate(
        &app.global::<I18n>().get_language(),
        "Ready for an X/Twitter URL.",
    ));
    app.set_auth_token(settings.auth_token.clone());
    app.set_saved_auth_token(settings.auth_token);
    app.set_media_type_index(settings.media_type_index.clamp(0, 3));
    // The removed Original mode (2) returned the same posts as Include.
    app.set_retweets_index(settings.retweets_index.clamp(0, 1));
    app.set_no_videos(settings.no_videos);
    if settings.include_photos.is_some()
        || settings.include_videos.is_some()
        || settings.include_gifs.is_some()
    {
        let (photo, video, gif) = fetch_request::media_flags(&app);
        app.set_include_photos(settings.include_photos.unwrap_or(photo));
        app.set_include_videos(settings.include_videos.unwrap_or(video));
        app.set_include_gifs(settings.include_gifs.unwrap_or(gif));
        app.set_no_videos(false);
        app.set_media_filters_custom(true);
    }
    app.set_text_tweets(settings.text_tweets);
    app.set_video_quality_index(video_quality_index(&settings.video_quality));
    app.set_image_size_index(image_size_index(&settings.image_size));
    app.set_avatar_size_index(
        ["orig", "400x400", "200x200", "bigger", "normal"]
            .iter()
            .position(|size| *size == settings.avatar_size)
            .unwrap_or(0) as i32,
    );
    app.set_banner_size_index(
        ["orig", "1500x500", "600x200", "300x100"]
            .iter()
            .position(|size| *size == settings.banner_size)
            .unwrap_or(0) as i32,
    );
    let gif_options =
        gif_conversion::Options::from_values(&settings.gif_quality, &settings.gif_resolution);
    app.set_gif_quality_index(gif_options.quality_index());
    app.set_gif_resolution_index(gif_options.resolution_index());
    app.set_convert_gif(settings.convert_gif);
    if settings.convert_gif {
        app.invoke_ensure_ffmpeg();
    }
    app.set_notify_updates(settings.notify_updates);
    app.set_incremental_fetch(settings.incremental_fetch);
    app.set_concurrent_downloads_index(if settings.concurrent_downloads == 0 {
        50
    } else {
        settings.concurrent_downloads.clamp(1, 50) as i32 - 1
    });
    app.set_retry_attempts(settings.retry_attempts.min(5) as i32);
    app.set_results_view(
        if settings.results_view == "list" {
            "list"
        } else {
            "grid"
        }
        .into(),
    );
    app.set_results_sort(settings.results_sort.clamp(0, 2));
    app.set_separate_media_folders(settings.separate_media_folders);
    app.set_embed_metadata(settings.embed_metadata);
    app.set_fetch_timeout_secs(settings.fetch_timeout_secs.clamp(5, 3600) as i32);
    app.set_http_timeout_secs(settings.http_timeout_secs.clamp(5, 3600) as i32);
    app.set_download_speed_kib(settings.download_speed_kib.min(1_048_576) as i32);
    app.set_download_delay_ms(settings.download_delay_ms.min(3_600_000) as i32);
    app.set_download_jitter_ms(settings.download_jitter_ms.min(3_600_000) as i32);
    let filename_format = if settings.filename_format.trim().is_empty() {
        "{date}_{handle}_{tweet_id}_{index}".to_owned()
    } else {
        settings.filename_format.clone()
    };
    app.set_filename_format(filename_format.clone());
    app.set_filename_preview(downloader::filename_preview(&filename_format));
    let folder_name_format = if settings.folder_name_format.trim().is_empty() {
        "{handle}".to_owned()
    } else {
        settings.folder_name_format.clone()
    };
    app.set_folder_name_format(folder_name_format.clone());
    app.set_folder_name_preview(downloader::folder_name_preview(&folder_name_format));
    app.set_output_dir(
        data_paths::output_dir(settings.output_dir.as_deref())
            .map_err(|error| {
                crate::ui::runtime::PlatformError::Other(format!(
                    "Could not prepare the portable Downloads folder: {error}"
                ))
            })?
            .to_string_lossy()
            .into_owned(),
    );
    app.set_app_version(env!("CARGO_PKG_VERSION").into());

    account_labels::initialize();
    account_search::register(&app);
    let history = Arc::new(Mutex::new(Vec::new()));
    app.set_history_busy(true);
    let history_filter = Arc::new(Mutex::new((String::new(), false)));
    let _saved_file_monitor = saved_files::watch(
        &app,
        backend.clone(),
        history.clone(),
        history_filter.clone(),
    );
    history_ui::register(&app, history.clone(), history_filter.clone());
    history_refresh::register(&app, history.clone());
    account_labels::register(&app, history.clone(), history_filter.clone());
    downloads::register(&app, history.clone());
    crate::library::register(
        &app,
        backend.clone(),
        history.clone(),
        history_filter.clone(),
        cancel.clone(),
    );
    database_backup::register(
        &app,
        directory.clone(),
        history.clone(),
        backend.clone(),
        history_filter.clone(),
    );
    file_integrity::register(
        &app,
        history.clone(),
        backend.clone(),
        history_filter.clone(),
        cancel.clone(),
    );
    archive_conversion::register(
        &app,
        history.clone(),
        backend.clone(),
        history_filter.clone(),
        cancel.clone(),
    );
    {
        let ui = app.as_weak();
        let history = history.clone();
        let filter = history_filter.clone();
        tokio::task::spawn_blocking(move || {
            let loaded = history_db::load_summaries();
            let _ = ui.upgrade_in_event_loop(move |app| {
                app.set_history_busy(false);
                match loaded {
                    Ok(entries) => {
                        *history.lock().unwrap() = entries;
                        app.invoke_history_loaded();
                        let entries = history.lock().unwrap();
                        let filter = filter.lock().unwrap();
                        history_ui::refresh(&app, &entries, &filter.0, filter.1);
                    }
                    Err(error) => {
                        show_toast(&app, format!("Could not load history: {error}"), "error")
                    }
                }
            });
        });
    }

    {
        let ui = ui.clone();
        let cancel = cancel.clone();
        let backend = backend.clone();
        let history = history.clone();
        let history_filter = history_filter.clone();
        app.on_continue_extraction(move || {
            let Some(app) = ui.upgrade() else { return };
            if app.get_is_busy()
                || app.get_history_busy()
                || app.get_auth_check_busy()
                || app.get_xtractor_busy()
                || app.get_tool_update_busy()
            {
                return;
            }
            if app.get_saved_auth_token().trim().is_empty() {
                let message = i18n::translate(
                    &app.global::<I18n>().get_language(),
                    "An X auth_token is required before extraction.",
                );
                show_toast(&app, message, "error");
                return;
            }
            let pending={let state=backend.lock().unwrap();state.stored_view.clone().filter(|_|state.fetch_json.is_none())};
            if let Some(view)=pending {
                cancel.store(false,Ordering::Relaxed);app.set_is_busy(true);app.set_status("Preparing xtractor…".into());
                let backend=backend.clone();let ui=ui.clone();let cancel=cancel.clone();
                tokio::task::spawn_blocking(move|| {
                    let result=view.response(true);
                    let _=ui.upgrade_in_event_loop(move|app|{
                        app.set_is_busy(false);
                        if cancel.load(Ordering::Relaxed){return;}
                        match result {Ok(json)=>{backend.lock().unwrap().fetch_json=Some(json);app.invoke_continue_extraction();},Err(error)=>show_toast(&app,format!("Could not restore fetch cursor data: {error}"),"error")}
                    });
                });return;
            }
            let (profile, cursor, previous_json, has_more) = {
                let data = backend.lock().unwrap_or_else(|e| e.into_inner());
                (
                    data.profile.clone(),
                    data.next_cursor.clone(),
                    data.pagination_json(),
                    data.has_more,
                )
            };
            let Some(mut profile) = profile else {
                let message = "Extract a timeline before continuing.";
                show_toast(&app, message, "error");
                return;
            };
            if !has_more || cursor.is_empty() {
                let message = "There are no more pages to continue.";
                show_toast(&app, message, "info");
                return;
            }
            if !same_extraction_filters(&current_profile_from_ui(&app), &profile) {
                let message =
                    "Restore the source and filters from the previous extraction before continuing.";
                show_toast(&app, message, "error");
                return;
            }
            profile.page_size = downloader::POSTS_PER_REQUEST;

            let options = downloader::ExtractionOptions {
                url: profile.url.clone(),
                media_timeline: profile.media_timeline,
                media_reposts: profile.media_reposts,
                auth_token: app.get_saved_auth_token().trim().to_owned(),
                max_posts: usize::MAX,
                page_size: downloader::POSTS_PER_REQUEST,
                media_type: profile.media_type.clone(),
                retweets: profile.retweets.clone(),
                no_videos: profile.no_videos,
                text_tweets: profile.text_tweets,
                cursor,
                previous_json: Some(previous_json),
                refresh_boundary: Default::default(),
            };
            session_log::info(format!(
                "Continuing extraction for {}.",
                options.url
            ));
            cancel.store(false, Ordering::Relaxed);
            app.set_is_busy(true);
            app.set_is_extracting(true);
            app.set_progress(0.0);
            app.set_progress_detail("".into());
            app.global::<MediaSelection>().invoke_clear();
            app.set_selected_media_index(-1);
            app.set_status("Continuing with the saved xtractor cursor…".into());
            spawn_extraction(
                ui.clone(),
                options,
                profile,
                backend.clone(),
                history.clone(),
                history_filter.clone(),
                cancel.clone(),
            );
        });
    }

    {
        let ui = ui.clone();
        let cancel = cancel.clone();
        let backend = backend.clone();
        let history = history.clone();
        let history_filter = history_filter.clone();
        app.on_start_extraction(move || {
            let Some(app) = ui.upgrade() else { return };
            app.invoke_sync_fetch_source(app.get_source_url());
            if app.get_batch_mode() {
                app.invoke_start_batch();
                return;
            }
            if app.get_is_busy()
                || app.get_history_busy()
                || app.get_auth_check_busy()
                || app.get_xtractor_busy()
                || app.get_tool_update_busy()
            {
                return;
            }
            if app.get_saved_auth_token().trim().is_empty() {
                let message = i18n::translate(
                    &app.global::<I18n>().get_language(),
                    "An X auth_token is required before extraction.",
                );
                show_toast(&app, message, "error");
                return;
            }

            let input = app.get_source_url().trim().to_owned();
            if input.is_empty() {
                let message = "Enter an X/Twitter profile, search, or bookmarks URL.";
                show_toast(&app, message, "error");
                return;
            }

            let range = fetch_request::range_from_app(&app);
            let profile = match fetch_request::profile_from_app(&app, &input, &range) {
                Ok(profile) => profile,
                Err(error) => {
                    show_toast(&app, error, "error");
                    return;
                }
            };
            let options = downloader::ExtractionOptions {
                url: profile.url.clone(),
                media_timeline: profile.media_timeline,
                media_reposts: profile.media_reposts,
                auth_token: app.get_saved_auth_token().trim().to_owned(),
                max_posts: usize::MAX,
                page_size: downloader::POSTS_PER_REQUEST,
                media_type: profile.media_type.clone(),
                retweets: profile.retweets.clone(),
                no_videos: profile.no_videos,
                text_tweets: profile.text_tweets,
                cursor: String::new(),
                previous_json: None,
                refresh_boundary: Default::default(),
            };
            session_log::info(format!("Extraction started for {}.", options.url));

            cancel.store(false, Ordering::Relaxed);
            app.set_is_busy(true);
            app.set_is_extracting(true);
            app.set_progress(0.0);
            app.set_progress_detail("".into());
            app.set_status("Preparing xtractor…".into());
            app.set_media_items(empty_media_model());
            app.global::<MediaSelection>().invoke_clear();
            app.set_media_count(0);
            app.set_media_count_text("0".into());
            app.set_has_output(false);
            app.set_has_more(false);
            app.set_account_profile(downloader::empty_account_profile());
            app.set_selected_media_index(-1);
            {
                let mut data = backend.lock().unwrap_or_else(|e| e.into_inner());
                data.candidates.clear();
                data.json.clear();
                data.profile = None;
                data.next_cursor.clear();
                data.has_more = false;
                data.tweet_count = 0;
                data.history_id.clear();
                data.fetch_json = None;
                data.stored_view = None;
                data.prepared_download = false;
            }

            spawn_extraction(
                ui.clone(),
                options,
                profile,
                backend.clone(),
                history.clone(),
                history_filter.clone(),
                cancel.clone(),
            );
        });
    }

    {
        let ui = ui.clone();
        app.on_dismiss_status(move || {
            if let Some(app) = ui.upgrade() {
                if app.get_is_busy() {
                    return;
                }
                let ready_status = i18n::translate(
                    &app.global::<I18n>().get_language(),
                    "Ready for an X/Twitter URL.",
                );
                app.set_status(ready_status);
                app.set_progress(0.0);
                app.set_progress_detail("".into());
            }
        });
    }

    {
        let ui = ui.clone();
        app.on_dismiss_toast(move || {
            if let Some(app) = ui.upgrade() {
                dismiss_toast(&app);
            }
        });
    }

    {
        let ui = ui.clone();
        app.on_paste_from_clipboard(move || {
            let Some(app) = ui.upgrade() else { return };
            let clipboard_text =
                ClipboardContext::new().and_then(|mut clipboard| clipboard.get_contents());
            let text = match clipboard_text {
                Ok(text) if !text.trim().is_empty() => text.trim().to_owned(),
                Ok(_) => {
                    show_toast(&app, "Clipboard is empty.", "info");
                    return;
                }
                Err(_) => {
                    show_toast(&app, "Could not read the clipboard.", "error");
                    return;
                }
            };
            app.set_source_url(text);
            app.invoke_start_extraction();
        });
    }

    {
        let ui = ui.clone();
        let cancel = cancel.clone();
        let backend = backend.clone();
        let history = history.clone();
        let history_filter = history_filter.clone();
        app.on_start_downloads(move |kind| {
            let Some(app) = ui.upgrade() else { return };
            if app.get_is_busy() || (app.get_xtractor_busy() || app.get_tool_update_busy()) {
                return;
            }
            let prepared = backend.lock().unwrap().prepared_download;
            if !prepared {
                crate::history::open::prepare_download(
                    &app,
                    backend.clone(),
                    cancel.clone(),
                    kind.to_string(),
                    None,
                );
                return;
            }
            let candidates = {
                let mut state = backend.lock().unwrap();
                state.prepared_download = false;
                std::mem::take(&mut state.candidates)
            };
            if candidates.is_empty() {
                let message = "Extract media URLs before starting a download.";
                show_toast(&app, message, "error");
                return;
            }
            let media_filter = kind.to_string();
            let selected_indices = app
                .global::<MediaSelection>()
                .get_indices()
                .iter()
                .filter(|index| *index >= 0)
                .map(|index| index as usize)
                .collect::<Vec<_>>();
            let selected = selected_indices
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>();
            let pending_count = candidates
                .iter()
                .enumerate()
                .filter(|(index, item)| {
                    downloader::matches_download_scope(*index, item, &media_filter, &selected)
                        && item.status != "Saved"
                })
                .count();
            if pending_count == 0 {
                show_toast(&app, "All selected media files are already saved.", "info");
                return;
            }
            session_log::info(format!(
                "Downloading {pending_count} media item(s) from the {media_filter} filter."
            ));

            let output_dir = PathBuf::from(app.get_output_dir().as_str());
            let filename_format = app.get_filename_format().to_string();
            let folder_name_format = app.get_folder_name_format().to_string();
            let folder_name_override = special_download_folder_name(&app.get_source_url());
            let video_quality = video_quality_value(app.get_video_quality_index()).to_owned();
            let image_size = image_size_value(app.get_image_size_index()).to_owned();
            let convert_gif = app.get_convert_gif();
            let gif_options = gif_conversion::Options::from_indices(
                app.get_gif_quality_index(),
                app.get_gif_resolution_index(),
            );
            let concurrent_downloads = if app.get_concurrent_downloads_index() == 50 {
                0
            } else {
                (app.get_concurrent_downloads_index().clamp(0, 49) + 1) as usize
            };
            let retry_attempts = app.get_retry_attempts().clamp(0, 5) as usize;
            let download_policy = download_policy::from_app(&app);
            cancel.store(false, Ordering::Relaxed);
            app.set_is_busy(true);
            app.set_is_extracting(false);
            app.set_progress(0.0);
            app.set_progress_detail("".into());
            app.set_status(format!("Downloading {pending_count} media files…"));
            downloader::refresh_ui_model(&app, &candidates);

            let ui = ui.clone();
            let cancel = cancel.clone();
            let backend = backend.clone();
            let history = history.clone();
            let history_filter = history_filter.clone();
            let previously_saved = candidates
                .iter()
                .filter(|item| item.status == "Saved")
                .map(|item| item.saved_path.clone())
                .collect::<std::collections::HashSet<_>>();
            tokio::spawn(async move {
                let summary = downloader::download_all(
                    candidates,
                    &media_filter,
                    output_dir,
                    filename_format,
                    folder_name_format,
                    folder_name_override,
                    video_quality,
                    image_size,
                    concurrent_downloads,
                    retry_attempts,
                    selected_indices,
                    convert_gif,
                    gif_options,
                    backend.clone(),
                    ui.clone(),
                    cancel.clone(),
                    download_policy,
                )
                .await;
                let (history_id, updated_media) = {
                    let state = backend.lock().unwrap_or_else(|e| e.into_inner());
                    (state.history_id.clone(), state.candidates.clone())
                };
                let history_snapshot = if history_id.is_empty() {
                    None
                } else {
                    let mut entries = history.lock().unwrap_or_else(|e| e.into_inner()).clone();
                    for (id, media) in fetch_archives::update_saved(&mut entries, &updated_media) {
                        history_db::save_media(id, media);
                    }
                    let saved = updated_media
                        .into_iter()
                        .filter(|item| {
                            saved_files::exists(item)
                                && !previously_saved.contains(&item.saved_path)
                        })
                        .collect();
                    downloads::record(&mut entries, &history_id, saved, &media_filter);
                    Some(entries.clone())
                };
                let history_snapshot =
                    crate::history::open::settle_history(&history, history_snapshot).await;
                if backend.lock().unwrap().stored_view.is_some() {
                    backend.lock().unwrap().candidates.clear();
                }
                let filter = history_filter
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                let _ = crate::ui::runtime::invoke_from_event_loop(move || {
                    if let Some(app) = ui.upgrade() {
                        if let Some(entries) = history_snapshot {
                            history_ui::refresh(&app, &entries, &filter.0, filter.1);
                        }
                        let toast_kind = if summary.message.contains(" failed.")
                            || summary.message.starts_with("Could not")
                        {
                            "error"
                        } else {
                            "info"
                        };
                        show_toast(&app, summary.message.clone(), toast_kind);
                        if toast_kind == "info" {
                            app.set_status(summary.message);
                        }
                        app.set_progress(summary.progress);
                        app.set_is_busy(false);
                        media_results::retain_pending_selection(&app);
                    }
                });
            });
        });
    }

    {
        let ui = ui.clone();
        let cancel = cancel.clone();
        let backend = backend.clone();
        let history = history.clone();
        let history_filter = history_filter.clone();
        app.on_download_selected_media(move |media_index| {
            let Some(app) = ui.upgrade() else { return };
            if app.get_is_busy() || (app.get_xtractor_busy() || app.get_tool_update_busy()) {
                return;
            }
            let needs_prepare = {
                let state = backend.lock().unwrap();
                state.stored_view.is_some() && !state.prepared_download
            };
            if needs_prepare {
                crate::history::open::prepare_download(
                    &app,
                    backend.clone(),
                    cancel.clone(),
                    "selected".into(),
                    Some(media_index),
                );
                return;
            }
            backend.lock().unwrap().prepared_download = false;
            let (candidate, candidate_index, history_id) = {
                let mut state = backend.lock().unwrap_or_else(|e| e.into_inner());
                let Some((index, item)) = state
                    .candidates
                    .iter_mut()
                    .enumerate()
                    .find(|(_, item)| item.media_index == media_index.max(0) as usize)
                else {
                    let message = "This media item is no longer available.";
                    show_toast(&app, message, "error");
                    return;
                };
                saved_files::reconcile(std::slice::from_mut(item));
                (item.clone(), index, state.history_id.clone())
            };
            if saved_files::exists(&candidate) {
                return;
            }
            let output_dir = PathBuf::from(app.get_output_dir().as_str());
            let filename_format = app.get_filename_format().to_string();
            let folder_name_format = app.get_folder_name_format().to_string();
            let folder_name_override = special_download_folder_name(&app.get_source_url());
            let video_quality = video_quality_value(app.get_video_quality_index()).to_owned();
            let image_size = image_size_value(app.get_image_size_index()).to_owned();
            let convert_gif = app.get_convert_gif();
            let retry_attempts = app.get_retry_attempts().clamp(0, 5) as usize;
            let download_policy = download_policy::from_app(&app);
            let gif_options = gif_conversion::Options::from_indices(
                app.get_gif_quality_index(),
                app.get_gif_resolution_index(),
            );
            cancel.store(false, Ordering::Relaxed);
            app.set_is_busy(true);
            app.set_is_extracting(false);
            app.set_progress(0.0);
            app.set_progress_detail("".into());
            app.set_status(format!("Downloading {}…", candidate.tweet_id));

            let ui = ui.clone();
            let cancel = cancel.clone();
            let backend = backend.clone();
            let history = history.clone();
            let history_filter = history_filter.clone();
            tokio::spawn(async move {
                let result = downloader::download_selected_file(
                    candidate,
                    candidate_index,
                    output_dir,
                    filename_format,
                    folder_name_format,
                    folder_name_override,
                    video_quality,
                    image_size,
                    convert_gif,
                    gif_options,
                    retry_attempts,
                    ui.clone(),
                    cancel.clone(),
                    download_policy,
                )
                .await;
                let signature = if let Ok((path, _)) = &result {
                    file_integrity::signature(path.clone(), cancel.clone()).await
                } else {
                    None
                };
                let download_succeeded = result.is_ok();
                let metadata_failed = result.as_ref().is_ok_and(|(_, warning)| warning.is_some());
                let (message, media) = match result {
                    Ok((path, warning)) => {
                        let media = {
                            let mut state = backend.lock().unwrap_or_else(|e| e.into_inner());
                            if let Some(item) = state.candidates.get_mut(candidate_index) {
                                item.status = "Saved".into();
                                item.integrity_error.clear();
                                item.saved_path = path.to_string_lossy().into_owned();
                                if let Some(signature) = &signature {
                                    signature.apply(item);
                                }
                            }
                            state.candidates.clone()
                        };
                        (
                            match warning {
                                Some(error) => {
                                    format!("Saved {}. Metadata failed: {error}", path.display())
                                }
                                None => format!("Saved {}", path.display()),
                            },
                            Some(media),
                        )
                    }
                    Err(error) => (error, None),
                };
                let history_snapshot = if let Some(media) = media.as_ref() {
                    let mut entries = history.lock().unwrap_or_else(|e| e.into_inner()).clone();
                    for (id, media) in fetch_archives::update_saved(&mut entries, media) {
                        history_db::save_media(id, media);
                    }
                    downloads::record(
                        &mut entries,
                        &history_id,
                        vec![media[candidate_index].clone()],
                        "single",
                    );
                    Some(entries.clone())
                } else {
                    None
                };
                let history_snapshot =
                    crate::history::open::settle_history(&history, history_snapshot).await;
                if backend.lock().unwrap().stored_view.is_some() {
                    backend.lock().unwrap().candidates.clear();
                }
                let filter = history_filter
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                let _ = crate::ui::runtime::invoke_from_event_loop(move || {
                    if let Some(app) = ui.upgrade() {
                        if let Some(media) = media.as_ref() {
                            downloader::refresh_ui_model(&app, media);
                            media_results::retain_pending_selection(&app);
                            app.set_progress(100.0);
                        }
                        if let Some(entries) = history_snapshot {
                            history_ui::refresh(&app, &entries, &filter.0, filter.1);
                        }
                        let toast_kind = if metadata_failed {
                            "error"
                        } else if download_succeeded {
                            "info"
                        } else if message.to_ascii_lowercase().contains("cancelled") {
                            "info"
                        } else {
                            "error"
                        };
                        show_toast(&app, message.clone(), toast_kind);
                        if download_succeeded || toast_kind == "info" {
                            app.set_status(message);
                        }
                        app.set_is_busy(false);
                    }
                });
            });
        });
    }

    {
        let ui = ui.clone();
        app.on_open_link(move |url| {
            if let Err(error) = open::that(url.as_str())
                && let Some(app) = ui.upgrade()
            {
                let message = format!("Could not open media: {error}");
                show_toast(&app, message, "error");
            }
        });
    }

    {
        let ui = ui.clone();
        app.on_open_post(move |url| {
            if let Err(error) = open::that(url.as_str())
                && let Some(app) = ui.upgrade()
            {
                let message = format!("Could not open post: {error}");
                show_toast(&app, message, "error");
            }
        });
    }

    crate::history::open::register(&app, history.clone(), backend.clone());

    {
        let ui = ui.clone();
        let history = history.clone();
        app.on_clear_history(move || {
            if ui
                .upgrade()
                .is_some_and(|app| app.get_is_busy() || app.get_history_busy())
            {
                return;
            }
            let mut entries = history.lock().unwrap_or_else(|e| e.into_inner());
            let Some(app) = ui.upgrade() else { return };
            let downloads = app.get_history_show_saved();
            entries.retain(|entry| entry.download.is_some() != downloads);
            history_db::clear_history(downloads);
            if let Some(app) = ui.upgrade() {
                history_ui::refresh(
                    &app,
                    &entries,
                    app.get_history_search_query().as_str(),
                    downloads,
                );
            }
        });
    }

    {
        let ui = ui.clone();
        let history = history.clone();
        let history_filter = history_filter.clone();
        app.on_search_history(move |query, saved_only| {
            let mut filter = history_filter.lock().unwrap_or_else(|e| e.into_inner());
            filter.0 = query.to_string();
            filter.1 = saved_only;
            let entries = history.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(app) = ui.upgrade() {
                history_ui::refresh(&app, &entries, &filter.0, filter.1);
            }
        });
    }

    {
        let cancel = cancel.clone();
        app.on_cancel_operation(move || cancel.store(true, Ordering::Relaxed));
    }

    {
        let ui = ui.clone();
        app.on_browse_output_folder(move || {
            let initial = ui
                .upgrade()
                .map(|app| PathBuf::from(app.get_output_dir().as_str()))
                .filter(|path| path.is_dir());
            let mut dialog = rfd::FileDialog::new();
            if let Some(initial) = initial {
                dialog = dialog.set_directory(initial);
            }
            if let Some(path) = dialog.pick_folder()
                && let Some(app) = ui.upgrade()
            {
                app.set_output_dir(path.to_string_lossy().into_owned());
                downloader::save_settings(settings_from_app(&app));
            }
        });
    }

    {
        let ui = ui.clone();
        let backend = backend.clone();
        app.on_export_json(move || {
            let (json, view) = {
                let state = backend.lock().unwrap();
                (state.json.clone(), state.stored_view.clone())
            };
            if json.is_empty() {
                if let Some(app) = ui.upgrade() {
                    let message = "There is no extraction JSON to export yet.";
                    show_toast(&app, message, "error");
                }
                return;
            }
            if let Some(path) = rfd::FileDialog::new()
                .set_file_name("xtractor-output.json")
                .add_filter("JSON", &["json"])
                .save_file()
            {
                let ui = ui.clone();
                tokio::spawn(async move {
                    let result = tokio::task::spawn_blocking(move || {
                        let json = if let Some(view) = view {
                            view.response(false).map_err(|error| error.to_string())?
                        } else {
                            json
                        };
                        std::fs::write(&path, json).map_err(|error| error.to_string())?;
                        Ok::<_, String>(path)
                    })
                    .await;
                    let message = match result {
                        Ok(Ok(path)) => format!("JSON exported to {}", path.display()),
                        Ok(Err(error)) => format!("Could not export JSON: {error}"),
                        Err(error) => format!("Could not export JSON: {error}"),
                    };
                    let _ = crate::ui::runtime::invoke_from_event_loop(move || {
                        if let Some(app) = ui.upgrade() {
                            let toast_kind = if message.starts_with("Could not") {
                                "error"
                            } else {
                                "info"
                            };
                            show_toast(&app, message.clone(), toast_kind);
                            if toast_kind == "info" {
                                app.set_status(message);
                            }
                        }
                    });
                });
            }
        });
    }

    {
        let ui = ui.clone();
        app.on_open_output_folder(move || {
            let Some(app) = ui.upgrade() else { return };
            let path = PathBuf::from(app.get_output_dir().as_str());
            if let Err(error) = open::that(&path) {
                let message = format!("Could not open output folder: {error}");
                show_toast(&app, message, "error");
            }
        });
    }

    {
        let ui = ui.clone();
        app.on_save_theme_mode(move |mode| {
            if let Some(app) = ui.upgrade() {
                app.set_theme_mode(mode);
                downloader::save_settings(settings_from_app(&app));
            }
        });
    }

    {
        let ui = ui.clone();
        app.on_save_theme_preset(move |preset| {
            if let Some(app) = ui.upgrade() {
                app.global::<AppTheme>().set_preset(preset);
                downloader::save_settings(settings_from_app(&app));
            }
        });
    }

    {
        let ui = ui.clone();
        app.on_save_language(move |language| {
            if let Some(app) = ui.upgrade() {
                i18n::apply_language(&app, language.as_str());
                downloader::save_settings(settings_from_app(&app));
            }
        });
    }

    {
        let ui = ui.clone();
        app.global::<RendererSettings>().on_choose(move |mode| {
            let Some(app) = ui.upgrade() else { return };
            renderer_preferences::choose(&app, mode.as_str(), || {
                downloader::save_settings(settings_from_app(&app))
            });
        });
    }

    {
        let ui = ui.clone();
        let cancel = cancel.clone();
        app.global::<RendererSettings>().on_restart_now(move || {
            let Some(app) = ui.upgrade() else { return };
            let renderer = app.global::<RendererSettings>();
            if !renderer.get_restart_required()
                || renderer.get_restarting()
                || app.get_is_busy()
                || app.get_ffmpeg_busy()
                || app.get_history_busy()
                || (app.get_xtractor_busy() || app.get_tool_update_busy())
            {
                return;
            }
            renderer.set_restarting(true);
            history_db::flush();
            match renderer_preferences::launch_restart() {
                Ok(_) => {
                    session_log::info("Restarting application to apply renderer preference.");
                    cancel.store(true, Ordering::Relaxed);
                    if let Err(error) = crate::ui::runtime::quit_event_loop() {
                        session_log::error(format!(
                            "Could not stop the previous app session: {error}"
                        ));
                        renderer.set_restarting(false);
                    }
                }
                Err(error) => {
                    session_log::error(format!("Could not restart application: {error}"));
                    renderer.set_restarting(false);
                    renderer.set_error("Could not restart the app. Please try again.".into());
                }
            }
        });
    }

    {
        let ui = ui.clone();
        app.on_settings_changed(move || {
            if let Some(app) = ui.upgrade() {
                let filename_format = app.get_filename_format().to_string();
                app.set_filename_preview(downloader::filename_preview(&filename_format));
                let folder_name_format = app.get_folder_name_format().to_string();
                app.set_folder_name_preview(downloader::folder_name_preview(&folder_name_format));
                downloader::save_settings(settings_from_app(&app));
            }
        });
    }

    auth_tokens::register(&app);

    let _batch_queue_store = batch_fetch::register(
        &app,
        backend.clone(),
        history.clone(),
        history_filter.clone(),
        cancel.clone(),
    );
    fetch_sources::register(&app);
    settings_reset::register(&app);
    update_checks::startup(&app);
    let result = app.run();
    cancel.store(true, Ordering::Relaxed);
    preview_player.shutdown();
    xtractor_worker::shutdown().await;
    history_db::flush();
    match &result {
        Ok(()) => session_log::info("Application stopped."),
        Err(error) => session_log::error(format!("Application stopped with an error: {error}")),
    }
    result
}
