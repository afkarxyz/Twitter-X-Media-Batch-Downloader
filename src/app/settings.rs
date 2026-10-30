use crate::AppTheme;
use crate::AppWindow;
use crate::RendererSettings;
use crate::accounts::tokens as auth_tokens;
use crate::download::engine as downloader;
use crate::download::gifs as gif_conversion;
use crate::fetch::request as fetch_request;
use crate::platform::paths as data_paths;
use std::path::PathBuf;

pub(crate) fn settings_from_app(app: &AppWindow) -> downloader::AppSettings {
    let output_dir = PathBuf::from(app.get_output_dir().as_str());
    let gif_options = gif_conversion::Options::from_indices(
        app.get_gif_quality_index(),
        app.get_gif_resolution_index(),
    );
    let theme = app.global::<AppTheme>();
    downloader::AppSettings {
        legacy_dark_mode: false,
        theme_mode: Some(app.get_theme_mode().to_string()),
        theme_preset: theme.get_preset().to_string(),
        language_mode: app.get_language_mode().to_string(),
        renderer_mode: app.global::<RendererSettings>().get_preferred().to_string(),
        auth_token: app.get_saved_auth_token().to_string(),
        auth_token_history: auth_tokens::snapshot(&app.get_saved_auth_token()).0,
        auth_token_validation: auth_tokens::snapshot(&app.get_saved_auth_token()).1,
        output_dir: data_paths::stored_output_dir(output_dir),
        media_type_index: app.get_media_type_index().clamp(0, 3),
        include_photos: Some(fetch_request::media_flags(app).0),
        include_videos: Some(fetch_request::media_flags(app).1),
        include_gifs: Some(fetch_request::media_flags(app).2),
        retweets_index: app.get_retweets_index().clamp(0, 1),
        no_videos: app.get_no_videos(),
        text_tweets: app.get_text_tweets(),
        video_quality: video_quality_value(app.get_video_quality_index()).to_owned(),
        image_size: image_size_value(app.get_image_size_index()).to_owned(),
        avatar_size: ["orig", "400x400", "200x200", "bigger", "normal"]
            [app.get_avatar_size_index().clamp(0, 4) as usize]
            .into(),
        banner_size: ["orig", "1500x500", "600x200", "300x100"]
            [app.get_banner_size_index().clamp(0, 3) as usize]
            .into(),
        convert_gif: app.get_convert_gif(),
        gif_quality: gif_options.quality_value().into(),
        gif_resolution: gif_options.resolution_value().into(),
        notify_updates: app.get_notify_updates(),
        incremental_fetch: app.get_incremental_fetch(),
        concurrent_downloads: if app.get_concurrent_downloads_index() == 50 {
            0
        } else {
            (app.get_concurrent_downloads_index().clamp(0, 49) + 1) as usize
        },
        retry_attempts: app.get_retry_attempts().clamp(0, 5) as usize,
        filename_format: app.get_filename_format().to_string(),
        folder_name_format: app.get_folder_name_format().to_string(),
        results_view: app.get_results_view().to_string(),
        results_sort: app.get_results_sort().clamp(0, 2),
        separate_media_folders: app.get_separate_media_folders(),
        embed_metadata: app.get_embed_metadata(),
        fetch_timeout_secs: app.get_fetch_timeout_secs().clamp(5, 3600) as u32,
        http_timeout_secs: app.get_http_timeout_secs().clamp(5, 3600) as u32,
        download_speed_kib: app.get_download_speed_kib().max(0) as u32,
        download_delay_ms: app.get_download_delay_ms().max(0) as u32,
        download_jitter_ms: app.get_download_jitter_ms().max(0) as u32,
        fetch_delay_ms: app.get_fetch_delay_ms().max(0) as u32,
        fetch_jitter_ms: app.get_fetch_jitter_ms().max(0) as u32,
        rotate_auth_tokens: app.get_rotate_auth_tokens(),
        proxy_url: app.get_proxy_url().to_string(),
    }
}

pub(crate) fn special_download_folder_name(source_url: &str) -> Option<String> {
    let path = source_url
        .split('?')
        .next()
        .unwrap_or_default()
        .trim_end_matches('/');
    match path
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "bookmarks" => Some("My Bookmarks".to_owned()),
        "likes" => Some("My Likes".to_owned()),
        _ => None,
    }
}

pub(crate) fn video_quality_index(value: &str) -> i32 {
    match value {
        "720p" => 1,
        "480p" => 2,
        "360p" => 3,
        "lowest" => 4,
        _ => 0,
    }
}

pub(crate) fn video_quality_value(index: i32) -> &'static str {
    match index.clamp(0, 4) {
        1 => "720p",
        2 => "480p",
        3 => "360p",
        4 => "lowest",
        _ => "highest",
    }
}

pub(crate) fn image_size_index(value: &str) -> i32 {
    match value {
        "large" => 1,
        "medium" => 2,
        "small" => 3,
        _ => 0,
    }
}

pub(crate) fn image_size_value(index: i32) -> &'static str {
    match index.clamp(0, 3) {
        1 => "large",
        2 => "medium",
        3 => "small",
        _ => "original",
    }
}
