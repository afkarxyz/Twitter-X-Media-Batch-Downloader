use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use crate::ui::runtime::{Image, Model, ModelRc, VecModel};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{io::AsyncWriteExt, time::sleep};

use crate::AccountProfile;
use crate::AppWindow;
use crate::HistoryItem;
use crate::MediaItem;

use crate::media::images::{self as remote_images, Kind};
use crate::platform::paths as data_paths;
pub const POSTS_PER_REQUEST: usize = 100;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MediaCandidate {
    pub url: String,
    pub media_type: String,
    pub tweet_id: String,
    pub tweet_url: String,
    pub username: String,
    pub author_name: String,
    pub author_verified: bool,
    pub author_avatar_url: String,
    pub author_avatar_square: bool,
    pub alt_text: String,
    pub quotes: u64,
    pub bookmarks: u64,
    pub is_repost: bool,
    pub reposted_by: String,
    pub tweet_text: String,
    pub created_at: String,
    pub source: String,
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
    pub replies: u64,
    pub retweets: u64,
    pub likes: u64,
    pub views: u64,
    pub preview_url: String,
    pub thumb_url: String,
    pub variants: Vec<VideoVariant>,
    pub image_variants: Vec<ImageVariant>,
    pub status: String,
    pub saved_path: String,
    pub saved_bytes: u64,
    pub saved_sha256: String,
    pub integrity_error: String,
    pub media_index: usize,
    pub is_new: bool,
    pub gif_output_path: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoVariant {
    pub url: String,
    pub bitrate: u64,
    pub resolution: String,
    pub height: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageVariant {
    pub name: String,
    pub url: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HistoryEntry {
    pub id: String,
    pub fetched_at: String,
    pub source_url: String,
    pub account_name: String,
    pub username: String,
    pub tweet_count: usize,
    pub media: Vec<MediaCandidate>,
    pub json: String,
    pub profile: Option<ExtractionProfile>,
    pub next_cursor: String,
    pub has_more: bool,
    pub download: Option<DownloadRecord>,
    #[serde(skip)]
    pub stored: Option<crate::history::storage::StoredEntry>,
}

impl HistoryEntry {
    pub fn media_count(&self) -> usize {
        self.stored
            .as_ref()
            .map_or(self.media.len(), |stored| stored.summary.count)
    }
    pub fn saved_count(&self) -> usize {
        self.stored.as_ref().map_or_else(
            || {
                self.media
                    .iter()
                    .filter(|item| crate::download::saved_files::exists(item))
                    .count()
            },
            |stored| stored.summary.saved,
        )
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DownloadRecord {
    pub output_dir: String,
    pub total_bytes: u64,
    pub media_filter: String,
    pub earlier: bool,
}

#[derive(Clone, Default)]
pub struct BackendState {
    pub candidates: Vec<MediaCandidate>,
    pub json: String,
    pub profile: Option<ExtractionProfile>,
    pub next_cursor: String,
    pub has_more: bool,
    pub tweet_count: usize,
    pub history_id: String,
    pub fetch_json: Option<String>,
    pub stored_view: Option<std::sync::Arc<crate::history::paged::StoredView>>,
    pub prepared_download: bool,
}

impl BackendState {
    pub fn pagination_json(&self) -> String {
        self.fetch_json.clone().unwrap_or_else(|| self.json.clone())
    }
}

#[derive(Clone, Debug)]
pub struct ExtractionOptions {
    pub url: String,
    pub media_timeline: bool,
    pub media_reposts: bool,
    pub auth_token: String,
    pub max_posts: usize,
    pub page_size: usize,
    pub media_type: String,
    pub retweets: String,
    pub no_videos: bool,
    pub text_tweets: bool,
    pub cursor: String,
    pub previous_json: Option<String>,
    pub refresh_boundary: HashSet<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExtractionProfile {
    pub media_timeline: bool,
    pub media_reposts: bool,
    pub input_url: String,
    pub date_filter_enabled: bool,
    pub start_date: String,
    pub end_date: String,
    pub url: String,
    pub page_size: usize,
    pub media_type: String,
    pub retweets: String,
    pub no_videos: bool,
    pub text_tweets: bool,
}

impl From<&ExtractionOptions> for ExtractionProfile {
    fn from(options: &ExtractionOptions) -> Self {
        Self {
            url: options.url.clone(),
            media_timeline: options.media_timeline,
            media_reposts: options.media_reposts,
            page_size: options.page_size,
            media_type: options.media_type.clone(),
            retweets: options.retweets.clone(),
            no_videos: options.no_videos,
            text_tweets: options.text_tweets,
            ..Default::default()
        }
    }
}

pub struct ExtractionOutcome {
    pub candidates: Vec<MediaCandidate>,
    pub json: String,
    pub tweet_count: usize,
    pub pages: usize,
    pub warning: Option<String>,
    pub next_cursor: String,
    pub has_more: bool,
    pub account_name: String,
    pub username: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    #[serde(default, rename = "dark_mode", skip_serializing)]
    pub legacy_dark_mode: bool,
    pub theme_mode: Option<String>,
    pub theme_preset: String,
    pub language_mode: String,
    pub renderer_mode: String,
    pub auth_token: String,
    pub output_dir: Option<PathBuf>,
    pub auth_token_history: Vec<crate::accounts::tokens::Record>,
    pub auth_token_validation: crate::accounts::tokens::Validation,
    pub media_type_index: i32,
    pub include_photos: Option<bool>,
    pub include_videos: Option<bool>,
    pub include_gifs: Option<bool>,
    pub retweets_index: i32,
    pub no_videos: bool,
    pub text_tweets: bool,
    pub video_quality: String,
    pub image_size: String,
    pub avatar_size: String,
    pub banner_size: String,
    pub convert_gif: bool,
    pub gif_quality: String,
    pub gif_resolution: String,
    pub concurrent_downloads: usize,
    pub retry_attempts: usize,
    pub filename_format: String,
    pub folder_name_format: String,
    pub results_view: String,
    pub results_sort: i32,
    pub separate_media_folders: bool,
    pub embed_metadata: bool,
    pub fetch_timeout_secs: u32,
    pub http_timeout_secs: u32,
    pub download_speed_kib: u32,
    pub download_delay_ms: u32,
    pub download_jitter_ms: u32,
    pub fetch_delay_ms: u32,
    pub fetch_jitter_ms: u32,
    pub rotate_auth_tokens: bool,
    pub proxy_url: String,
    pub notify_updates: bool,
    pub incremental_fetch: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            legacy_dark_mode: false,
            theme_mode: None,
            theme_preset: crate::ui::schemes::DEFAULT.to_owned(),
            language_mode: "system".to_owned(),
            renderer_mode: "gpu".to_owned(),
            auth_token: String::new(),
            output_dir: None,
            auth_token_history: Vec::new(),
            auth_token_validation: Default::default(),
            media_type_index: 0,
            include_photos: None,
            include_videos: None,
            include_gifs: None,
            retweets_index: 0,
            no_videos: false,
            text_tweets: false,
            video_quality: "highest".to_owned(),
            image_size: "original".to_owned(),
            avatar_size: "orig".to_owned(),
            banner_size: "orig".to_owned(),
            convert_gif: false,
            gif_quality: "fast".to_owned(),
            gif_resolution: "original".to_owned(),
            concurrent_downloads: 1,
            retry_attempts: 2,
            filename_format: "{date}_{handle}_{tweet_id}_{index}".to_owned(),
            folder_name_format: "{handle}".to_owned(),
            results_view: "grid".into(),
            results_sort: 0,
            separate_media_folders: false,
            embed_metadata: false,
            fetch_timeout_secs: 330,
            http_timeout_secs: 120,
            download_speed_kib: 0,
            download_delay_ms: 0,
            download_jitter_ms: 0,
            fetch_delay_ms: 0,
            fetch_jitter_ms: 0,
            rotate_auth_tokens: false,
            proxy_url: String::new(),
            notify_updates: true,
            incremental_fetch: true,
        }
    }
}

pub struct DownloadSummary {
    pub message: String,
    pub progress: f32,
}

pub fn load_settings() -> AppSettings {
    settings_path()
        .and_then(|path| fs::read(path).ok())
        .and_then(|data| serde_json::from_slice(&data).ok())
        .unwrap_or_default()
}

pub fn save_settings(settings: AppSettings) -> bool {
    let Some(path) = settings_path() else {
        return false;
    };
    let Some(parent) = path.parent() else {
        return false;
    };
    if fs::create_dir_all(parent).is_err() {
        return false;
    }
    let Ok(bytes) = serde_json::to_vec_pretty(&settings) else {
        return false;
    };
    let temporary = path.with_extension("json.tmp");
    if fs::write(&temporary, bytes).is_err() {
        return false;
    }

    let backup = path.with_extension(format!(
        "json.{}.bak",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let had_previous = path.exists();
    if had_previous && fs::rename(&path, &backup).is_err() {
        let _ = fs::remove_file(&temporary);
        return false;
    }

    match fs::rename(&temporary, &path) {
        Ok(()) => {
            if had_previous {
                let _ = fs::remove_file(backup);
            }
            true
        }
        Err(_) => {
            if had_previous {
                let _ = fs::rename(backup, &path);
            }
            let _ = fs::remove_file(temporary);
            false
        }
    }
}

fn settings_path() -> Option<PathBuf> {
    app_data_dir().map(|dir| dir.join("config.json"))
}

pub fn app_data_dir() -> Option<PathBuf> {
    data_paths::app_data_dir()
}

pub fn to_history_model(
    entries: &[HistoryEntry],
    query: &str,
    saved_only: bool,
) -> ModelRc<HistoryItem> {
    let rows = if saved_only {
        history_rows(
            entries
                .iter()
                .filter(|entry| entry.download.is_some())
                .map(|entry| (entry, 0, String::new())),
            query,
        )
    } else {
        crate::fetch::sessions::groups(entries)
            .into_iter()
            .flat_map(|group| {
                let mut rows = history_rows(
                    std::iter::once((
                        group.sessions[0],
                        group.sessions.len().min(i32::MAX as usize) as i32,
                        crate::fetch::sessions::label(group.sessions[0]),
                    )),
                    query,
                );
                for row in &mut rows {
                    row.media_count = crate::fetch::archives::media_count(&group.sessions)
                        .min(i32::MAX as usize) as i32;
                }
                rows
            })
            .collect()
    };
    model_from_history_rows(rows)
}

pub fn model_from_history_rows(rows: Vec<HistoryItem>) -> ModelRc<HistoryItem> {
    Rc::new(VecModel::from(rows)).into()
}
pub fn history_rows<'a>(
    entries: impl IntoIterator<Item = (&'a HistoryEntry, i32, String)>,
    query: &str,
) -> Vec<HistoryItem> {
    let query = query.trim().to_lowercase();
    entries
        .into_iter()
        .filter_map(|(entry, session_count, group_label)| {
            let saved_count = entry.saved_count();
            let title = if entry.account_name.is_empty() {
                format!("@{}", entry.username)
            } else if entry.username.is_empty() {
                entry.account_name.clone()
            } else {
                entry.account_name.clone()
            };
            let subtitle = format!(
                "{} · {} posts · {} media",
                entry.fetched_at,
                entry.tweet_count,
                entry.media_count()
            );
            let searchable = format!(
                "{} {} {} @{} {}",
                title, subtitle, entry.source_url, entry.username, entry.id
            )
            .to_lowercase();
            if !query.is_empty() && !searchable.contains(&query) {
                return None;
            }
            let profile = account_profile_from_json(&entry.json);
            let avatar_image = profile.avatar_image.clone();
            let has_avatar = avatar_image.size().width > 0;
            let (age_key, age_count) = history_age(entry);
            Some(HistoryItem {
                update_eligible: crate::history::refresh::profile(entry).is_some(),
                account_labels: "".into(),
                account_color: Default::default(),
                session_count,
                group_label,
                id: entry.id.clone(),
                title,
                subtitle,
                source_url: entry.source_url.clone(),
                media_count: entry.media_count().min(i32::MAX as usize) as i32,
                saved_count: saved_count.min(i32::MAX as usize) as i32,
                handle: entry.username.clone(),
                avatar_image,
                avatar_url: profile.avatar_url,
                avatar_square: profile.avatar_square,
                has_avatar,
                age_key,
                age_count,
                fetched_at: entry.fetched_at.clone(),
                output_dir: entry
                    .download
                    .as_ref()
                    .map(|record| record.output_dir.clone())
                    .unwrap_or_default(),
                size_text: entry
                    .download
                    .as_ref()
                    .map(|record| format_bytes(record.total_bytes))
                    .unwrap_or_default(),
                earlier_download: entry.download.as_ref().is_some_and(|record| record.earlier),
            })
        })
        .collect::<Vec<_>>()
}

fn history_age(entry: &HistoryEntry) -> (String, i32) {
    if entry.download.as_ref().is_some_and(|record| record.earlier) {
        return (String::new(), 0);
    }
    let Some(timestamp) = entry
        .id
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 1_000_000_000_000)
    else {
        return (String::new(), 0);
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let elapsed = now.saturating_sub(timestamp / 1000);
    let (key, count) = if elapsed < 60 {
        ("Just now", 0)
    } else if elapsed < 3600 {
        ("{} minutes ago", elapsed / 60)
    } else if elapsed < 86400 {
        ("{} hours ago", elapsed / 3600)
    } else {
        ("{} days ago", elapsed / 86400)
    };
    (key.to_owned(), count.min(i32::MAX as u64) as i32)
}

pub fn history_entry_from_response(json: String) -> Option<HistoryEntry> {
    let value: Value = serde_json::from_str(&json).ok()?;
    value.get("tweets")?.as_array()?;
    let username = account_profile_username(&json);
    if username.is_empty() {
        return None;
    }
    let account_name = account_string(value.get("account").unwrap_or(&Value::Null), &["name"]);
    let media = collect_media(&value);
    Some(HistoryEntry {
        id: new_history_id(),
        fetched_at: current_date_label(),
        source_url: format!("https://x.com/{username}"),
        username,
        account_name,
        tweet_count: value.get("tweets")?.as_array()?.len(),
        media,
        json,
        ..Default::default()
    })
}

pub fn to_ui_model(candidates: &[MediaCandidate]) -> ModelRc<MediaItem> {
    model_from_rows(to_ui_rows(candidates))
}

pub fn empty_account_profile() -> AccountProfile {
    AccountProfile {
        available: false,
        display_name: "".into(),
        username: "".into(),
        avatar_url: "".into(),
        avatar_image: Image::default(),
        has_avatar: false,
        avatar_square: false,
        banner_url: "".into(),
        banner_image: Image::default(),
        has_banner: false,
        verified: false,
        protected: false,
        bio: "".into(),
        location: "".into(),
        website_url: "".into(),
        website_label: "".into(),
        joined: "".into(),
        posts: "0".into(),
        following: "0".into(),
        followers: "0".into(),
        media: "0".into(),
    }
}

pub fn account_profile_from_json(json: &str) -> AccountProfile {
    let value = serde_json::from_str::<Value>(json).unwrap_or(Value::Null);
    let legacy_account = value.get("account").is_none();
    let account = value
        .get("account")
        .or_else(|| value.get("account_info"))
        .unwrap_or(&Value::Null);
    let author = first_tweet_author(&value);
    let account_username = if legacy_account {
        account_string(account, &["name", "username", "screen_name"])
    } else {
        account_string(account, &["username", "screen_name"])
    };
    let username = non_empty_or(
        account_username,
        account_string(author, &["username", "screen_name"]),
    );
    let account_display_name = if legacy_account {
        account_string(account, &["nick", "name"])
    } else {
        account_string(account, &["name", "nick"])
    };
    let display_name = non_empty_or(account_display_name, account_string(author, &["name"]));
    let display_name = if display_name.is_empty() {
        username.clone()
    } else {
        display_name
    };
    let avatar_shape = non_empty_or(
        account_string(account, &["profile_image_shape"]),
        account_string(author, &["profile_image_shape"]),
    );
    let website_url = account_string(account, &["url", "website"]);
    let website_url = normalize_website_url(&website_url);
    let website_label = website_url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_owned();
    let joined = account_string(account, &["created_at", "date"]);
    let avatar_url = non_empty_or(
        account_image_url(account, &["avatar", "profile_image"], "avatar_variants"),
        account_image_url(author, &["avatar", "profile_image"], "avatar_variants"),
    );
    let banner_url = non_empty_or(
        account_image_url(account, &["banner"], "banner_variants"),
        account_image_url(author, &["banner"], "banner_variants"),
    );
    let avatar_square = avatar_shape.eq_ignore_ascii_case("square");
    let avatar_image = remote_images::image(remote_images::avatar_kind(avatar_square), &avatar_url);
    let banner_image = remote_images::image(Kind::Banner, &banner_url);

    AccountProfile {
        available: !username.is_empty() || !display_name.is_empty(),
        display_name,
        username,
        avatar_url,
        has_avatar: avatar_image.size().width > 0,
        avatar_image,
        avatar_square,
        banner_url,
        has_banner: banner_image.size().width > 0,
        banner_image,
        verified: account
            .get("verified")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || account
                .get("blue_verified")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            || author
                .get("verified")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            || author
                .get("blue_verified")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        protected: account
            .get("protected")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || author
                .get("protected")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        bio: account_string(account, &["bio", "description"]),
        location: account_string(account, &["location"]),
        website_url,
        website_label,
        joined: format_joined_date(&joined),
        posts: account_count(account, &["tweets", "statuses_count"]),
        following: account_count(account, &["following", "friends_count"]),
        followers: account_count(account, &["followers", "followers_count"]),
        media: account_count(account, &["media_count"]),
    }
}

pub(crate) fn apply_account_profile(app: &AppWindow, mut profile: AccountProfile) {
    let current = app.get_account_profile();
    if !profile.avatar_url.is_empty()
        && profile.avatar_url == current.avatar_url
        && profile.avatar_square == current.avatar_square
        && current.has_avatar
    {
        profile.avatar_image = current.avatar_image;
        profile.has_avatar = true;
    }
    if !profile.banner_url.is_empty()
        && profile.banner_url == current.banner_url
        && current.has_banner
    {
        profile.banner_image = current.banner_image;
        profile.has_banner = true;
    }
    app.set_account_profile(profile);
}

pub fn account_profile_username(json: &str) -> String {
    let value = serde_json::from_str::<Value>(json).unwrap_or(Value::Null);
    let legacy_account = value.get("account").is_none();
    let account_username = account_string(
        value
            .get("account")
            .or_else(|| value.get("account_info"))
            .unwrap_or(&Value::Null),
        if legacy_account {
            &["name", "username", "screen_name"]
        } else {
            &["username", "screen_name"]
        },
    );
    non_empty_or(
        account_username,
        account_string(first_tweet_author(&value), &["username", "screen_name"]),
    )
}

pub fn profile_image_url(json: &str, kind: &str, size: &str) -> Option<String> {
    let value: Value = serde_json::from_str(json).ok()?;
    let account = value
        .get("account")
        .or_else(|| value.get("account_info"))
        .unwrap_or(&Value::Null);
    let author = first_tweet_author(&value);
    let (keys, variants) = match kind {
        "avatar" => (&["avatar", "profile_image"][..], "avatar_variants"),
        "banner" => (&["banner"][..], "banner_variants"),
        _ => return None,
    };
    for source in [account, author] {
        if let Some(url) = source
            .get(variants)
            .and_then(Value::as_array)
            .and_then(|items| {
                items
                    .iter()
                    .find(|item| item.get("name").and_then(Value::as_str) == Some(size))
            })
            .and_then(|item| item.get("url"))
            .and_then(Value::as_str)
            .filter(|url| !url.is_empty())
        {
            return Some(url.to_owned());
        }
    }
    let fallback = non_empty_or(
        account_image_url(account, keys, variants),
        account_image_url(author, keys, variants),
    );
    if fallback.is_empty() {
        return None;
    }
    let mut url = reqwest::Url::parse(&fallback).ok()?;
    if kind == "avatar" {
        let path = url.path().to_owned();
        if let Some((stem, extension)) = path.rsplit_once('.') {
            let stem = ["_normal", "_bigger", "_mini", "_200x200", "_400x400"]
                .iter()
                .find_map(|suffix| stem.strip_suffix(suffix))
                .unwrap_or(stem);
            let suffix = if size == "orig" {
                String::new()
            } else {
                format!("_{size}")
            };
            url.set_path(&format!("{stem}{suffix}.{extension}"));
        }
    } else {
        let path = url.path().trim_end_matches('/').to_owned();
        let base = ["/1500x500", "/600x200", "/300x100"]
            .iter()
            .find_map(|suffix| path.strip_suffix(suffix))
            .unwrap_or(&path);
        url.set_path(&if size == "orig" {
            base.to_owned()
        } else {
            format!("{base}/{size}")
        });
    }
    Some(url.to_string())
}

pub async fn download_profile_image(
    json: String,
    kind: String,
    size: String,
    output: PathBuf,
    folder_format: String,
    retry_attempts: usize,
    ui: crate::ui::runtime::Weak<AppWindow>,
    cancel: Arc<AtomicBool>,
    policy: crate::download::policy::Options,
) -> Result<PathBuf, String> {
    let url = profile_image_url(&json, &kind, &size)
        .ok_or("This account has no downloadable profile image.")?;
    let username = account_profile_username(&json);
    let value: Value = serde_json::from_str(&json).map_err(|error| error.to_string())?;
    let candidate = MediaCandidate {
        url,
        username,
        author_name: account_string(value.get("account").unwrap_or(&Value::Null), &["name"]),
        media_type: kind,
        ..Default::default()
    };
    let directory = output.join(render_folder_name(&folder_format, &candidate));
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(|error| error.to_string())?;
    let throttle = crate::download::policy::Throttle::new(policy);
    let client = crate::network::proxy::builder()
        .user_agent("Mozilla/5.0 TwitterXMediaBatchDownloader/1.0")
        .connect_timeout(crate::network::timeouts::connect())
        .build()
        .map_err(|error| error.to_string())?;
    download_one_with_policy(
        &client,
        &candidate,
        &directory,
        0,
        "{handle}_{type}",
        ui,
        cancel,
        None,
        retry_attempts,
        throttle,
    )
    .await
    .map_err(|error| match error {
        DownloadError::Cancelled => "Download cancelled.".into(),
        DownloadError::Failed(message) | DownloadError::Transient(message, _) => message,
    })
}

fn account_string(account: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| {
            account
                .get(*key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
        })
        .unwrap_or_default()
}

fn first_tweet_author(response: &Value) -> &Value {
    response
        .get("tweets")
        .and_then(Value::as_array)
        .and_then(|tweets| tweets.first())
        .and_then(|tweet| tweet.get("author"))
        .unwrap_or(&Value::Null)
}

fn non_empty_or(primary: String, fallback: String) -> String {
    if primary.is_empty() {
        fallback
    } else {
        primary
    }
}

fn account_count(account: &Value, keys: &[&str]) -> String {
    for key in keys {
        let Some(value) = account.get(*key) else {
            continue;
        };
        let count = value
            .as_u64()
            .or_else(|| value.as_i64().and_then(|value| u64::try_from(value).ok()))
            .or_else(|| value.as_str().and_then(|value| value.parse().ok()));
        if let Some(count) = count {
            return format_count(count);
        }
    }
    "0".to_owned()
}

fn format_count(count: u64) -> String {
    let digits = count.to_string();
    let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            formatted.push(',');
        }
        formatted.push(digit);
    }
    formatted
}

fn format_preview_date(value: &str) -> String {
    let parts = value.split_whitespace().collect::<Vec<_>>();
    if parts.len() == 6 && parts[0].len() == 3 && parts[3].len() == 8 {
        let day = parts[2].parse::<u32>().ok();
        if let Some(day) = day {
            let zone = if parts[4] == "+0000" {
                "UTC".to_owned()
            } else {
                format!("UTC{}", parts[4])
            };
            return format!(
                "{} {day}, {} · {} {zone}",
                parts[1],
                parts[5],
                parts[3].get(..5).unwrap_or(parts[3])
            );
        }
    }
    value.to_owned()
}

fn normalize_website_url(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        String::new()
    } else if value.starts_with("https://") || value.starts_with("http://") {
        value.to_owned()
    } else {
        format!("https://{value}")
    }
}

fn format_joined_date(value: &str) -> String {
    let value = value.trim();
    let parts = value.split_whitespace().collect::<Vec<_>>();
    if parts.is_empty() {
        return String::new();
    }
    let (month, year) = if parts[0].len() >= 7 && parts[0].as_bytes().get(4) == Some(&b'-') {
        let month = parts[0]
            .get(5..7)
            .and_then(|value| value.parse::<usize>().ok())
            .and_then(month_name);
        let year = parts[0].get(..4).unwrap_or_default();
        (month, year)
    } else if parts.len() >= 3 && parts[0].len() == 3 {
        (Some(parts[1]), parts.last().copied().unwrap_or_default())
    } else if parts.len() == 2 {
        (Some(parts[0]), parts[1])
    } else {
        return value.to_owned();
    };
    match month {
        Some(month) if !year.is_empty() => format!("{month} {year}"),
        _ => value.to_owned(),
    }
}

fn month_name(month: usize) -> Option<&'static str> {
    [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ]
    .get(month.checked_sub(1)?)
    .copied()
}

fn account_image_url(account: &Value, direct_keys: &[&str], variants_key: &str) -> String {
    for key in direct_keys {
        if let Some(url) = account
            .get(*key)
            .and_then(Value::as_str)
            .filter(|url| !url.trim().is_empty())
        {
            return url.to_owned();
        }
    }
    account
        .get(variants_key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find_map(|variant| {
            variant
                .get("url")
                .and_then(Value::as_str)
                .filter(|url| !url.trim().is_empty())
                .map(str::to_owned)
        })
        .unwrap_or_default()
}

pub fn to_ui_rows(candidates: &[MediaCandidate]) -> Vec<MediaItem> {
    candidates
        .iter()
        .map(|item| {
            let dimensions = media_dimensions(item);
            let engagement = format!(
                "{} replies · {} reposts · {} likes · {} views",
                item.replies, item.retweets, item.likes, item.views
            );
            let thumbnail_url = thumbnail_url(item);
            let preview_url = if item.media_type == "photo" {
                photo_preview_url(item)
            } else {
                item.preview_url.clone()
            };
            let preview_image = remote_images::image(Kind::Thumbnail, &thumbnail_url);
            let detail_image = if item.media_type == "photo" {
                remote_images::image(Kind::Preview, &preview_url)
            } else {
                Image::default()
            };
            let missing_saved_file =
                item.status == "Saved" && !crate::download::saved_files::exists(item);
            MediaItem {
                author_name: if item.author_name.is_empty() {
                    item.username.clone()
                } else {
                    item.author_name.clone()
                },
                author_handle: item.username.clone(),
                author_verified: item.author_verified,
                author_avatar_url: item.author_avatar_url.clone(),
                author_avatar_square: item.author_avatar_square,
                author_avatar_image: remote_images::image(
                    remote_images::avatar_kind(item.author_avatar_square),
                    &item.author_avatar_url,
                ),
                replies: format_count(item.replies),
                reposts: format_count(item.retweets),
                likes: format_count(item.likes),
                views: format_count(item.views),
                quotes: format_count(item.quotes),
                bookmarks: format_count(item.bookmarks),
                alt_text: item.alt_text.clone(),
                is_repost: item.is_repost,
                reposted_by: item.reposted_by.clone(),
                preview_date: format_preview_date(&item.created_at),
                file_extension: file_extension(item).to_uppercase(),
                title: if item.author_name.is_empty() {
                    format!("@{} · {}", item.username, item.media_type)
                } else {
                    format!(
                        "{} · @{} · {}",
                        item.author_name, item.username, item.media_type
                    )
                },
                detail: format!("{} · {}", item.created_at, item.tweet_id),
                media_type: item.media_type.clone(),
                status: if missing_saved_file {
                    "Ready".into()
                } else {
                    item.status.clone()
                },
                url: item.url.clone(),
                tweet_url: item.tweet_url.clone(),
                preview_url,
                playback_url: if matches!(item.media_type.as_str(), "video" | "animated_gif") {
                    selected_media_url(item, "highest", "orig")
                } else {
                    String::new()
                },
                thumbnail_url,
                preview_image,
                detail_image,
                tweet_text: item.tweet_text.clone(),
                created_at: item.created_at.clone(),
                source: item.source.clone(),
                dimensions,
                engagement,
                saved_path: if missing_saved_file {
                    "".into()
                } else {
                    item.saved_path.clone()
                },
                media_index: item.media_index.min(i32::MAX as usize) as i32,
                is_new: item.is_new,
                payload_pending: false,
                sort_key: crate::media::identity::post_timestamp(item)
                    .map(|date| date.format("%Y-%m-%dT%H:%M:%S%.3f").to_string())
                    .unwrap_or_default(),
            }
        })
        .collect::<Vec<_>>()
}

pub fn model_from_rows(rows: Vec<MediaItem>) -> ModelRc<MediaItem> {
    Rc::new(VecModel::from(rows)).into()
}

/// Apply status changes to the shown results without replacing the model.
/// Replacing it rebuilds every card, its cached layer and thumbnails; when the
/// same media are shown in the same order, only rows that changed are written.
pub fn refresh_ui_model(app: &AppWindow, candidates: &[MediaCandidate]) {
    let current = app.get_media_items();
    if let Some(model) = current
        .as_any()
        .downcast_ref::<crate::history::paged::PagedModel>()
    {
        model.apply_candidates(candidates);
        crate::media::results::refresh_selection_state(app);
        return;
    }
    let rows = to_ui_rows(candidates);
    let Some(rows) = (match current.as_any().downcast_ref::<VecModel<MediaItem>>() {
        Some(model) => update_rows_in_place(model, rows),
        None => Some(rows),
    }) else {
        // Labels such as "Select filter" depend on statuses but do not track rows.
        crate::media::results::refresh_selection_state(app);
        return;
    };
    app.set_media_items(model_from_rows(rows));
}

/// Writes changed rows into `model` when it holds the same media in the same
/// order. Otherwise returns `rows` so the caller can install a new model.
pub(crate) fn update_rows_in_place(
    model: &impl Model<Data = MediaItem>,
    rows: Vec<MediaItem>,
) -> Option<Vec<MediaItem>> {
    if model.row_count() != rows.len()
        || !model
            .iter()
            .zip(&rows)
            .all(|(shown, row)| shown.media_index == row.media_index)
    {
        return Some(rows);
    }
    for (index, mut row) in rows.into_iter().enumerate() {
        let shown = model.row_data(index).unwrap();
        // Keep images that are already decoded for this row.
        if row.preview_image.size().width == 0 {
            row.preview_image = shown.preview_image.clone();
        }
        if row.detail_image.size().width == 0 {
            row.detail_image = shown.detail_image.clone();
        }
        if row.author_avatar_image.size().width == 0
            && row.author_avatar_url == shown.author_avatar_url
            && row.author_avatar_square == shown.author_avatar_square
        {
            row.author_avatar_image = shown.author_avatar_image.clone();
        }
        if !same_row(&row, &shown) {
            model.set_row_data(index, row);
        }
    }
    None
}

fn same_row(row: &MediaItem, shown: &MediaItem) -> bool {
    row == shown
}

fn publish_account_preview(
    ui: crate::ui::runtime::Weak<AppWindow>,
    generation: i32,
    publish_results: bool,
    account: &Value,
    tweets: &[Value],
    previous: &mut Option<Value>,
) {
    if !publish_results {
        return;
    }
    let author = if account_string(account, &["username", "screen_name"]).is_empty() {
        if previous.as_ref().is_some_and(|payload| {
            !account_string(first_tweet_author(payload), &["username", "screen_name"]).is_empty()
        }) {
            return;
        }
        tweets
            .first()
            .and_then(|tweet| tweet.get("author"))
            .cloned()
            .unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    let payload = json!({"account": account, "tweets": [{"author": author}]});
    if previous.as_ref() == Some(&payload) {
        return;
    }
    *previous = Some(payload.clone());
    let json = payload.to_string();
    let _ = ui.upgrade_in_event_loop(move |app| {
        if !app.get_is_busy()
            || !app.get_is_extracting()
            || app.get_extraction_generation() != generation
        {
            return;
        }
        let profile = account_profile_from_json(&json);
        if !profile.available {
            return;
        }
        apply_account_profile(&app, profile);
    });
}

pub async fn extract(
    options: ExtractionOptions,
    ui: crate::ui::runtime::Weak<AppWindow>,
    cancel: Arc<AtomicBool>,
    extraction_generation: i32,
    publish_results: bool,
) -> Result<ExtractionOutcome, String> {
    let executable = tokio::select! {
        result = crate::updates::xtractor::ensure_installed(Some(ui.clone())) => result?,
        _ = wait_for_cancel(cancel.clone()) => return Err("Extraction cancelled.".into()),
    };
    let is_continuation = options.previous_json.is_some();
    let previous_response = options
        .previous_json
        .as_deref()
        .map(serde_json::from_str::<Value>)
        .transpose()
        .map_err(|error| format!("Could not read the previous xtractor response: {error}"))?
        .unwrap_or(Value::Null);
    let mut cursor = options.cursor.clone();
    let mut seen_cursors = HashSet::new();
    if !cursor.is_empty() {
        seen_cursors.insert(cursor.clone());
    }
    let mut tweets = previous_response
        .get("tweets")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut seen_ids = tweets
        .iter()
        .filter_map(|tweet| {
            tweet
                .get("tweet_id")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .collect::<HashSet<_>>();
    let mut account = previous_response
        .get("account")
        .cloned()
        .unwrap_or_else(|| Value::Object(Default::default()));
    let mut rate_limit = previous_response
        .get("rate_limit")
        .cloned()
        .unwrap_or(Value::Null);
    let mut candidates = collect_media(&json!({
        "account": account.clone(),
        "tweets": tweets.clone(),
    }));
    candidates.retain(|item| eligible_candidate(item, &options));
    for (index, item) in candidates.iter_mut().enumerate() {
        item.media_index = index;
    }
    let mut seen_media_urls = candidates
        .iter()
        .map(|candidate| candidate.url.clone())
        .collect::<HashSet<_>>();
    // A rate-limited page may switch tokens; later pages keep the one that worked.
    let mut token = options.auth_token.clone();
    let mut page_policy = crate::fetch::policy::Pages::default();
    let mut pages = 0usize;
    let mut preview_account = None;
    let mut warning = None;
    let mut response_has_more = previous_response
        .get("has_more")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    while tweets.len() < options.max_posts {
        if cancel.load(Ordering::Relaxed) {
            warning = Some(format!("Extraction cancelled after {pages} page(s)."));
            break;
        }

        let remaining = options.max_posts.saturating_sub(tweets.len());
        let count = options.page_size.min(remaining).clamp(1, 100);
        let page_number = pages + 1;
        set_status(
            ui.clone(),
            format!("Running xtractor · page {page_number}…"),
            if pages == 0 {
                0.0
            } else {
                (tweets.len() as f32 / options.max_posts as f32 * 95.0).min(95.0)
            },
        );

        let page_result =
            crate::fetch::retry::page(ui.clone(), cancel.clone(), &mut token, true, |token| {
                let options = ExtractionOptions {
                    auth_token: token,
                    ..options.clone()
                };
                let (executable, cursor, cancel) = (&executable, &cursor, cancel.clone());
                async move { run_xtractor_page(executable, &options, count, cursor, cancel).await }
            })
            .await;

        let page = match page_result {
            Ok(page) => page,
            Err(error) if pages > 0 => {
                warning = Some(if cancel.load(Ordering::Relaxed) {
                    format!("Extraction cancelled after {pages} page(s).")
                } else {
                    format!("Stopped after {pages} page(s): {error}")
                });
                break;
            }
            Err(error) => return Err(error),
        };

        pages += 1;
        if let Some(value) = page.get("account")
            && value.as_object().is_some_and(|account| !account.is_empty())
        {
            account = value.clone();
        }
        if let Some(value) = page.get("rate_limit")
            && !value.is_null()
        {
            rate_limit = value.clone();
        }

        let page_tweets = page
            .get("tweets")
            .and_then(Value::as_array)
            .ok_or_else(|| "xtractor JSON is missing the tweets array.".to_owned())?;
        publish_account_preview(
            ui.clone(),
            extraction_generation,
            publish_results,
            &account,
            page_tweets,
            &mut preview_account,
        );
        let mut new_tweets = Vec::new();
        for tweet in page_tweets {
            let id = tweet
                .get("tweet_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            if id.is_empty() || !seen_ids.insert(id) {
                continue;
            }
            if tweets.len() < options.max_posts {
                tweets.push(tweet.clone());
                new_tweets.push(tweet.clone());
            }
        }
        let new_tweet_count = new_tweets.len();
        let page_candidates = collect_media(&json!({
            "account": account.clone(),
            "tweets": new_tweets,
        }));
        for mut candidate in page_candidates {
            if !eligible_candidate(&candidate, &options) {
                continue;
            }
            if seen_media_urls.insert(candidate.url.clone()) {
                candidate.media_index = candidates.len();
                candidates.push(candidate);
            }
        }
        let next = page
            .get("next_cursor")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        response_has_more = page
            .get("has_more")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && !next.is_empty();

        if !response_has_more {
            break;
        }
        if !seen_cursors.insert(next.clone()) {
            warning =
                Some("Pagination stopped because xtractor returned a repeated cursor.".into());
            response_has_more = false;
            break;
        }
        if page_policy.overlap(page_tweets, &options.refresh_boundary) {
            response_has_more = false;
            break;
        }
        if page_policy.empty_limit(new_tweet_count) && !options.media_reposts {
            warning = Some(
                "Stopped after five empty pages. Continue is available from the saved cursor."
                    .into(),
            );
            cursor = next;
            break;
        }
        if tweets.len() >= options.max_posts {
            cursor = next;
            break;
        }
        if !is_continuation {
            let ui_candidates = publish_results.then(|| candidates.clone());
            let media_count = candidates.len().min(i32::MAX as usize) as i32;
            let has_more = response_has_more && !next.is_empty();
            let ui_for_update = ui.clone();
            let _ = crate::ui::runtime::invoke_from_event_loop(move || {
                if let Some(app) = ui_for_update.upgrade()
                    && app.get_is_busy()
                    && app.get_extraction_generation() == extraction_generation
                {
                    if let Some(ui_candidates) = ui_candidates {
                        app.set_media_items(to_ui_model(&ui_candidates));
                        app.set_media_count(media_count);
                        app.set_has_more(has_more);
                    } else if !app.get_library().job.is_empty() {
                        app.update_library(|data| {
                            if let Some(job) = data.queue.iter_mut().find(|job| job.key == data.job)
                            {
                                job.media_count = media_count;
                            }
                        });
                    } else {
                        let rows = app.get_batch_fetch_items();
                        for index in 0..rows.row_count() {
                            if let Some(mut row) = rows.row_data(index)
                                && row.status == "Fetching"
                            {
                                row.media_count = media_count;
                                rows.set_row_data(index, row);
                                break;
                            }
                        }
                    }
                }
            });
        }
        cursor = next;
    }

    let has_more = response_has_more && !cursor.is_empty();
    let next_cursor = if has_more { cursor } else { String::new() };
    let mut response = json!({
        "account": account,
        "tweets": tweets,
        "has_more": has_more,
        "fetch_complete": !has_more && warning.is_none(),
    });
    if has_more {
        response["next_cursor"] = Value::String(next_cursor.clone());
    }
    if !rate_limit.is_null() {
        response["rate_limit"] = rate_limit;
    }

    let tweet_count = response["tweets"].as_array().map_or(0, Vec::len);
    let account_name = response
        .get("account")
        .and_then(|value| value.get("name"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let username = response
        .get("account")
        .and_then(|value| value.get("username"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let json = serde_json::to_string_pretty(&response)
        .map_err(|error| format!("Could not encode combined xtractor output: {error}"))?;

    Ok(ExtractionOutcome {
        candidates,
        json,
        tweet_count,
        pages,
        warning,
        next_cursor,
        has_more,
        account_name,
        username,
    })
}

pub(crate) async fn run_xtractor_page(
    executable: &Path,
    options: &ExtractionOptions,
    count: usize,
    cursor: &str,
    cancel: Arc<AtomicBool>,
) -> Result<Value, String> {
    let route = crate::fetch::strategy::request(
        &options.url,
        options.media_timeline,
        options.media_reposts,
        &options.retweets,
        cursor,
    )?;
    crate::updates::worker::request(executable, options, &route, count, cancel)
        .await
        .map(|page| crate::fetch::strategy::finish(page, &route))
}

fn eligible_candidate(item: &MediaCandidate, options: &ExtractionOptions) -> bool {
    if item.media_type == "text" {
        options.text_tweets
    } else {
        crate::media::results::matches_kind(&item.media_type, &options.media_type)
            && !(options.no_videos && item.media_type == "video")
    }
}

pub(crate) fn collect_media(response: &Value) -> Vec<MediaCandidate> {
    let account_name = response
        .get("account")
        .and_then(|value| value.get("username"))
        .and_then(Value::as_str)
        .unwrap_or("account");
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    for tweet in response["tweets"].as_array().into_iter().flatten() {
        let tweet_id = tweet
            .get("tweet_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let tweet_url = tweet.get("url").and_then(Value::as_str).unwrap_or_default();
        let username = tweet
            .get("author")
            .and_then(|value| value.get("username"))
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .unwrap_or(account_name);
        let author_name = tweet
            .get("author")
            .and_then(|value| value.get("name"))
            .and_then(Value::as_str)
            .unwrap_or(username);
        let stats = tweet.get("stats").unwrap_or(&Value::Null);
        let replies = number(stats, "replies");
        let retweets = number(stats, "retweets");
        let likes = number(stats, "likes");
        let views = number(stats, "views");
        let tweet_text = tweet
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let created_at = tweet
            .get("created_at")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let source = tweet
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let post_start = candidates.len();
        if tweet
            .get("media")
            .and_then(Value::as_array)
            .is_none_or(|media| media.is_empty())
            && !tweet_text.trim().is_empty()
            && !tweet_id.is_empty()
        {
            candidates.push(MediaCandidate {
                url: format!("text://post/{tweet_id}"),
                media_type: "text".into(),
                tweet_id: tweet_id.into(),
                tweet_url: tweet_url.into(),
                username: username.into(),
                author_name: author_name.into(),
                tweet_text: tweet_text.into(),
                created_at: created_at.into(),
                source: source.into(),
                replies,
                retweets,
                likes,
                views,
                status: "Ready".into(),
                media_index: candidates.len(),
                ..Default::default()
            });
        }
        for media in tweet
            .get("media")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let variants = video_variants(media);
            let image_variants = image_variants(media);
            let Some(url) = best_media_url(media) else {
                continue;
            };
            if !seen.insert(url.clone()) {
                continue;
            }
            candidates.push(MediaCandidate {
                url,
                media_type: media
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("media")
                    .to_owned(),
                tweet_id: tweet_id.to_owned(),
                tweet_url: tweet_url.to_owned(),
                username: username.to_owned(),
                author_name: author_name.to_owned(),
                author_verified: tweet.get("author").is_some_and(|author| {
                    author
                        .get("blue_verified")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                        || author
                            .get("verified")
                            .and_then(Value::as_bool)
                            .unwrap_or(false)
                }),
                tweet_text: tweet_text.to_owned(),
                created_at: created_at.to_owned(),
                source: source.to_owned(),
                width: media
                    .get("width")
                    .and_then(Value::as_u64)
                    .or_else(|| media.get("sizes")?.get("original")?.get("w")?.as_u64())
                    .unwrap_or_default() as u32,
                height: media
                    .get("height")
                    .and_then(Value::as_u64)
                    .or_else(|| media.get("sizes")?.get("original")?.get("h")?.as_u64())
                    .unwrap_or_default() as u32,
                duration_ms: number(media, "duration_ms"),
                alt_text: media
                    .get("alt_text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .into(),
                replies,
                retweets,
                likes,
                views,
                preview_url: detail_preview_url(media, &image_variants),
                thumb_url: thumb_url(media, &image_variants),
                variants,
                image_variants,
                status: "Ready".into(),
                saved_path: String::new(),
                media_index: candidates.len(),
                is_new: false,
                gif_output_path: String::new(),
                ..Default::default()
            });
        }
        for item in &mut candidates[post_start..] {
            let author = tweet.get("author").unwrap_or(&Value::Null);
            let account = response.get("account").unwrap_or(&Value::Null);
            let profile = if account_string(account, &["username", "screen_name"])
                .eq_ignore_ascii_case(&item.username)
            {
                account
            } else {
                &Value::Null
            };
            item.author_avatar_url = non_empty_or(
                account_image_url(
                    author,
                    &[
                        "avatar",
                        "profile_image",
                        "profile_image_url_https",
                        "profile_image_url",
                    ],
                    "avatar_variants",
                ),
                account_image_url(
                    profile,
                    &[
                        "avatar",
                        "profile_image",
                        "profile_image_url_https",
                        "profile_image_url",
                    ],
                    "avatar_variants",
                ),
            );
            item.author_avatar_square = non_empty_or(
                account_string(author, &["profile_image_shape"]),
                account_string(profile, &["profile_image_shape"]),
            )
            .eq_ignore_ascii_case("square");
            item.quotes = number(stats, "quotes");
            item.bookmarks = number(stats, "bookmarks");
            item.is_repost = tweet
                .get("is_repost")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            item.reposted_by = tweet
                .get("reposted_by")
                .and_then(|value| value.get("username"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .into();
        }
    }
    candidates
}

/// Recover newly exposed fields from the original response without touching saved paths.
pub(crate) fn restore_preview_details(media: &mut [MediaCandidate], response: &str) {
    let Ok(data) = serde_json::from_str::<Value>(response) else {
        return;
    };
    let details = collect_media(&data)
        .into_iter()
        .map(|item| (crate::media::identity::key(&item), item))
        .collect::<std::collections::HashMap<_, _>>();
    for item in media {
        if let Some(detail) = details.get(&crate::media::identity::key(item)) {
            if !detail.author_avatar_url.is_empty() {
                item.author_avatar_url.clone_from(&detail.author_avatar_url);
                item.author_avatar_square = detail.author_avatar_square;
            }
            if !detail.alt_text.is_empty() {
                item.alt_text.clone_from(&detail.alt_text);
            }
            item.quotes = detail.quotes;
            item.bookmarks = detail.bookmarks;
            item.is_repost = detail.is_repost;
            item.reposted_by.clone_from(&detail.reposted_by);
        }
    }
}

fn best_media_url(media: &Value) -> Option<String> {
    let variants = video_variants(media);
    if !variants.is_empty() {
        return choose_video_variant(&variants, "highest").map(|item| item.url.clone());
    }
    choose_image_variant(&image_variants(media), "original")
        .map(|variant| variant.url.clone())
        .or_else(|| {
            media
                .get("thumbnail")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
}

fn number(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or_default()
}

fn video_variants(media: &Value) -> Vec<VideoVariant> {
    media
        .get("variants")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|variant| {
            variant
                .get("content_type")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind == "video/mp4")
        })
        .filter_map(|variant| {
            let url = variant.get("url")?.as_str()?.to_owned();
            let resolution = variant
                .get("resolution")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let height = resolution
                .split_once('x')
                .and_then(|(_, height)| height.parse::<u32>().ok())
                .unwrap_or_default();
            Some(VideoVariant {
                url,
                bitrate: variant
                    .get("bitrate")
                    .and_then(Value::as_u64)
                    .unwrap_or_default(),
                resolution,
                height,
            })
        })
        .collect()
}

fn image_variants(media: &Value) -> Vec<ImageVariant> {
    let mut variants = Vec::new();
    if let Some(thumbs) = media.get("thumbs").and_then(Value::as_array) {
        for thumb in thumbs {
            let Some(url) = thumb.get("url").and_then(Value::as_str) else {
                continue;
            };
            variants.push(ImageVariant {
                name: thumb
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_ascii_lowercase(),
                url: url.to_owned(),
            });
        }
    }
    if let Some(sizes) = media.get("sizes").and_then(Value::as_object) {
        for (name, size) in sizes {
            if let Some(url) = size.get("url").and_then(Value::as_str) {
                variants.push(ImageVariant {
                    name: name.to_ascii_lowercase(),
                    url: url.to_owned(),
                });
            }
        }
    }
    if let Some(base_url) = media.get("thumbnail").and_then(Value::as_str).or_else(|| {
        let is_video = media
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|kind| kind == "video" || kind == "animated_gif" || kind == "gif");
        (!is_video)
            .then(|| media.get("url").and_then(Value::as_str))
            .flatten()
    }) {
        for name in ["360x360", "small", "medium", "large", "orig"] {
            if !variants.iter().any(|variant| variant.name == name) {
                variants.push(ImageVariant {
                    name: name.to_owned(),
                    url: image_url_with_size(base_url, name),
                });
            }
        }
    }
    variants
}

fn image_url_with_size(raw_url: &str, size: &str) -> String {
    let Ok(mut url) = reqwest::Url::parse(raw_url) else {
        return raw_url.to_owned();
    };
    if url.host_str() != Some("pbs.twimg.com") {
        return raw_url.to_owned();
    }
    let pairs = url
        .query_pairs()
        .filter(|(key, _)| key != "name")
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    {
        let mut query = url.query_pairs_mut();
        query.clear();
        for (key, value) in pairs {
            query.append_pair(&key, &value);
        }
        query.append_pair("name", size);
    }
    url.to_string()
}

fn detail_preview_url(media: &Value, variants: &[ImageVariant]) -> String {
    let is_video = media
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind == "video" || kind == "animated_gif");
    if is_video {
        return media
            .get("thumbnail")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
    }
    choose_image_variant(variants, "original")
        .map(|variant| variant.url.clone())
        .or_else(|| {
            media
                .get("thumbnail")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_default()
}

fn photo_preview_url(item: &MediaCandidate) -> String {
    let source = selected_media_url(item, "highest", "original");
    image_url_with_size(
        if source.is_empty() {
            &item.preview_url
        } else {
            &source
        },
        "orig",
    )
}

fn thumb_url(media: &Value, variants: &[ImageVariant]) -> String {
    let thumb = variants
        .iter()
        .find(|variant| variant.name == "360x360")
        .or_else(|| variants.iter().find(|variant| variant.name == "small"));
    thumb
        .map(|variant| variant.url.clone())
        .or_else(|| {
            media
                .get("thumbnail")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .or_else(|| {
            media
                .get("url")
                .and_then(Value::as_str)
                .map(fallback_thumbnail_url)
        })
        .unwrap_or_default()
}

pub fn thumbnail_url(item: &MediaCandidate) -> String {
    item.image_variants
        .iter()
        .find(|variant| variant.name == "360x360")
        .or_else(|| {
            item.image_variants
                .iter()
                .find(|variant| variant.name == "small")
        })
        .map(|variant| variant.url.clone())
        .unwrap_or_else(|| {
            fallback_thumbnail_url(if item.thumb_url.is_empty() {
                &item.url
            } else {
                &item.thumb_url
            })
        })
}

fn fallback_thumbnail_url(raw_url: &str) -> String {
    let Ok(url) = reqwest::Url::parse(raw_url) else {
        return raw_url.to_owned();
    };
    if url.host_str() == Some("video.twimg.com")
        && url.path().starts_with("/tweet_video/")
        && let Some(video_id) = Path::new(url.path())
            .file_stem()
            .and_then(|value| value.to_str())
    {
        return format!(
            "https://pbs.twimg.com/tweet_video_thumb/{video_id}?format=jpg&name=360x360"
        );
    }
    if url.host_str() == Some("pbs.twimg.com") && url.path().starts_with("/media/") {
        return image_url_with_size(raw_url, "360x360");
    }
    raw_url.to_owned()
}

fn choose_image_variant<'a>(variants: &'a [ImageVariant], size: &str) -> Option<&'a ImageVariant> {
    let target = match size {
        "large" => "large",
        "medium" => "medium",
        "small" => "small",
        _ => "orig",
    };
    variants
        .iter()
        .find(|variant| variant.name == target)
        .or_else(|| {
            let fallback_order: &[&str] = match target {
                "small" => &["medium", "large", "orig", "original"],
                "medium" => &["large", "small", "orig", "original"],
                "large" => &["orig", "original", "medium", "small"],
                _ => &["original", "large", "medium", "small"],
            };
            fallback_order
                .iter()
                .find_map(|name| variants.iter().find(|variant| variant.name == *name))
        })
}

fn variant_height(variant: &VideoVariant) -> u32 {
    if variant.height > 0 {
        return variant.height;
    }
    if variant.bitrate >= 2_000_000 {
        1080
    } else if variant.bitrate >= 900_000 {
        720
    } else if variant.bitrate >= 450_000 {
        480
    } else {
        360
    }
}

fn choose_video_variant<'a>(
    variants: &'a [VideoVariant],
    quality: &str,
) -> Option<&'a VideoVariant> {
    let highest = || variants.iter().max_by_key(|item| item.bitrate);
    let lowest = || variants.iter().min_by_key(|item| item.bitrate);
    let cap = match quality {
        "720p" => Some(720),
        "480p" => Some(480),
        "360p" => Some(360),
        "lowest" => return lowest(),
        _ => return highest(),
    }?;
    variants
        .iter()
        .min_by_key(|item| (variant_height(item).abs_diff(cap), item.bitrate))
}

pub fn selected_media_url(item: &MediaCandidate, video_quality: &str, image_size: &str) -> String {
    if item.media_type == "video" || item.media_type == "animated_gif" {
        choose_video_variant(&item.variants, video_quality)
            .map(|variant| variant.url.clone())
            .filter(|url| !url.is_empty())
            .unwrap_or_else(|| item.url.clone())
    } else {
        choose_image_variant(&item.image_variants, image_size)
            .map(|variant| variant.url.clone())
            .filter(|url| !url.is_empty())
            .unwrap_or_else(|| item.url.clone())
    }
}

fn metadata_eligible(item: &MediaCandidate, convert_gif: bool) -> bool {
    !(convert_gif && item.media_type == "animated_gif")
        && matches!(file_extension(item).as_str(), "jpg" | "jpeg" | "mp4")
}

pub async fn download_all(
    mut candidates: Vec<MediaCandidate>,
    media_filter: &str,
    output_dir: PathBuf,
    filename_format: String,
    folder_name_format: String,
    folder_name_override: Option<String>,
    video_quality: String,
    image_size: String,
    concurrent_downloads: usize,
    retry_attempts: usize,
    selected_indices: Vec<usize>,
    convert_gif: bool,
    gif_options: crate::download::gifs::Options,
    backend: Arc<Mutex<BackendState>>,
    ui: crate::ui::runtime::Weak<AppWindow>,
    cancel: Arc<AtomicBool>,
    policy: crate::download::policy::Options,
) -> DownloadSummary {
    crate::download::saved_files::reconcile(&mut candidates);
    backend
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .candidates = candidates.clone();
    if let Err(error) = tokio::fs::create_dir_all(&output_dir).await {
        return DownloadSummary {
            message: format!("Could not create output folder: {error}"),
            progress: 0.0,
        };
    }
    let throttle = crate::download::policy::Throttle::new(policy);
    let client = match crate::network::proxy::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) TwitterXMediaBatchDownloader/1.0")
        .connect_timeout(crate::network::timeouts::connect())
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return DownloadSummary {
                message: format!("Could not initialize the downloader: {error}"),
                progress: 0.0,
            };
        }
    };

    let paged = backend.lock().unwrap().stored_view.is_some();
    let selected = selected_indices.into_iter().collect::<HashSet<_>>();
    let pending = candidates
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            (matches_download_scope(index, item, media_filter, &selected)
                && !crate::download::saved_files::exists(item))
            .then_some(index)
        })
        .collect::<Vec<_>>();
    let total = pending.len();
    if total == 0 {
        return DownloadSummary {
            message: "All selected media files are already saved.".into(),
            progress: 100.0,
        };
    }
    if convert_gif
        && pending
            .iter()
            .any(|index| candidates[*index].media_type == "animated_gif")
        && let Err(error) =
            crate::updates::ffmpeg::ensure_available(ui.clone(), cancel.clone(), true).await
    {
        return DownloadSummary {
            message: format!("Could not prepare FFmpeg: {error}"),
            progress: 0.0,
        };
    }
    let metadata_tool = if policy.embed_metadata
        && pending
            .iter()
            .any(|index| metadata_eligible(&candidates[*index], convert_gif))
    {
        match crate::download::metadata::ensure_available(ui.clone(), cancel.clone()).await {
            Ok(path) => Some(path),
            Err(error) => {
                return DownloadSummary {
                    message: format!("Could not prepare ExifTool: {error}"),
                    progress: 0.0,
                };
            }
        }
    } else {
        None
    };
    let metadata_errors = Arc::new(Mutex::new(Vec::<String>::new()));
    let metrics = Arc::new(Mutex::new(BatchMetrics {
        transfers: vec![TransferAmount::default(); total],
        ..Default::default()
    }));
    let workers = crate::download::policy::worker_count(
        concurrent_downloads,
        pending
            .iter()
            .map(|index| candidates[*index].media_type.as_str()),
    );
    let jobs = pending
        .into_iter()
        .enumerate()
        .map(|(slot, index)| {
            let candidate = candidates[index].clone();
            let client = client.clone();
            let ui = ui.clone();
            let cancel = cancel.clone();
            let transfer = TransferSlot {
                metrics: metrics.clone(),
                slot,
            };
            let folder_name = folder_name_override
                .as_deref()
                .map(|name| sanitize_folder_component(name, &candidate.username))
                .unwrap_or_else(|| render_folder_name(&folder_name_format, &candidate));
            let mut item_output_dir = output_dir.join(folder_name);
            if policy.separate_types {
                item_output_dir = item_output_dir
                    .join(crate::download::policy::media_folder(&candidate.media_type));
            }
            let throttle = throttle.clone();
            let mut download_item = candidate.clone();
            download_item.url = selected_media_url(&candidate, &video_quality, &image_size);
            let template = filename_format.clone();
            let metadata_tool = metadata_tool.clone();
            let metadata_errors = metadata_errors.clone();
            async move {
                let result = async {
                    if cancel.load(Ordering::Relaxed) {
                        return Err(DownloadError::Cancelled);
                    }
                    if convert_gif && download_item.media_type == "animated_gif" {
                        crate::download::gifs::executable().map_err(DownloadError::Failed)?;
                    }
                    tokio::fs::create_dir_all(&item_output_dir)
                        .await
                        .map_err(|error| DownloadError::Failed(error.to_string()))?;
                    let path = download_one_with_policy(
                        &client,
                        &download_item,
                        &item_output_dir,
                        download_item.media_index,
                        &template,
                        ui,
                        cancel.clone(),
                        Some(transfer),
                        retry_attempts,
                        throttle,
                    )
                    .await?;
                    if convert_gif && download_item.media_type == "animated_gif" {
                        crate::download::gifs::convert_download(&path, gif_options, cancel.clone())
                            .await
                            .map_err(|error| {
                                if cancel.load(Ordering::Relaxed) {
                                    DownloadError::Cancelled
                                } else {
                                    DownloadError::Failed(error)
                                }
                            })
                    } else {
                        if let Some(error) = crate::download::metadata::tag_download(
                            metadata_tool.as_deref(),
                            &path,
                            &candidate,
                            cancel.clone(),
                        )
                        .await
                        {
                            metadata_errors.lock().unwrap().push(error);
                        }
                        Ok(path)
                    }
                }
                .await;
                let result = match result {
                    Ok(path) => {
                        let signature =
                            crate::download::integrity::signature(path.clone(), cancel.clone())
                                .await;
                        Ok((path, signature))
                    }
                    Err(error) => Err(error),
                };
                (index, result)
            }
        })
        .collect::<Vec<_>>();
    let mut downloads = futures_util::stream::iter(jobs).buffer_unordered(workers);
    let mut success = 0usize;
    let mut failed = 0usize;
    let mut cancelled = false;
    let mut finished = 0;
    let mut processed = 0;
    let mut row_updates = Vec::new();
    while let Some((index, result)) = downloads.next().await {
        match result {
            Ok((path, signature)) => {
                success += 1;
                candidates[index].status = "Saved".into();
                candidates[index].integrity_error.clear();
                candidates[index].saved_path = path.to_string_lossy().into_owned();
                if let Some(signature) = signature {
                    signature.apply(&mut candidates[index]);
                }
            }
            Err(DownloadError::Cancelled) => {
                candidates[index].status = "Cancelled".into();
                cancelled = true;
            }
            Err(DownloadError::Failed(error) | DownloadError::Transient(error, _)) => {
                failed += 1;
                candidates[index].status = "Failed".into();
                crate::platform::logging::error(format!("Download failed: {error}"));
            }
        }
        finished += 1;
        let detail = {
            let mut metrics = metrics.lock().unwrap_or_else(|error| error.into_inner());
            metrics.saved = success;
            metrics.failed = failed;
            metrics.detail()
        };
        if candidates[index].status != "Cancelled" {
            processed += 1;
        }
        let progress = processed as f32 / total as f32 * 100.0;
        {
            let mut state = backend.lock().unwrap_or_else(|error| error.into_inner());
            if let Some(current) = state
                .candidates
                .iter_mut()
                .find(|item| item.media_index == candidates[index].media_index)
            {
                current.status = candidates[index].status.clone();
                current.saved_path = candidates[index].saved_path.clone();
                current.saved_bytes = candidates[index].saved_bytes;
                current
                    .saved_sha256
                    .clone_from(&candidates[index].saved_sha256);
            }
        }
        set_status(ui.clone(), "Downloading media…".into(), progress);
        set_transfer_detail(ui.clone(), detail);
        if paged {
            row_updates.push(candidates[index].clone());
        }
        if finished % 8 == 0 || finished == total {
            let updated = if paged {
                std::mem::take(&mut row_updates)
            } else {
                candidates.clone()
            };
            let ui = ui.clone();
            let _ = crate::ui::runtime::invoke_from_event_loop(move || {
                if let Some(app) = ui.upgrade() {
                    refresh_ui_model(&app, &updated);
                }
            });
        }
    }
    let progress = if cancelled {
        processed as f32 / total.max(1) as f32 * 100.0
    } else {
        100.0
    };
    {
        let mut state = backend.lock().unwrap_or_else(|e| e.into_inner());
        state.candidates = candidates.clone();
    }
    let updated_candidates = if paged {
        row_updates
    } else {
        candidates.clone()
    };
    let ui = ui.clone();
    let _ = crate::ui::runtime::invoke_from_event_loop(move || {
        if let Some(app) = ui.upgrade() {
            refresh_ui_model(&app, &updated_candidates);
            app.set_progress(progress);
        }
    });
    let message = if cancelled {
        format!("Download cancelled · {success} of {total} files saved.")
    } else if failed == 0 {
        format!(
            "Downloaded {success} media files to {}.",
            output_dir.display()
        )
    } else {
        format!("Downloaded {success} of {total} files · {failed} failed.")
    };
    let errors = metadata_errors.lock().unwrap();
    let message = if errors.is_empty() {
        message
    } else {
        format!(
            "{message} Metadata failed for {} file(s): {}",
            errors.len(),
            errors[0]
        )
    };
    DownloadSummary { message, progress }
}

pub async fn download_selected_file(
    item: MediaCandidate,
    _index: usize,
    output_dir: PathBuf,
    filename_format: String,
    folder_name_format: String,
    folder_name_override: Option<String>,
    video_quality: String,
    image_size: String,
    convert_gif: bool,
    gif_options: crate::download::gifs::Options,
    retry_attempts: usize,
    ui: crate::ui::runtime::Weak<AppWindow>,
    cancel: Arc<AtomicBool>,
    policy: crate::download::policy::Options,
) -> Result<(PathBuf, Option<String>), String> {
    let folder_name = folder_name_override
        .as_deref()
        .map(|name| sanitize_folder_component(name, &item.username))
        .unwrap_or_else(|| render_folder_name(&folder_name_format, &item));
    let mut output_dir = output_dir.join(folder_name);
    if policy.separate_types {
        output_dir = output_dir.join(crate::download::policy::media_folder(&item.media_type));
    }
    tokio::fs::create_dir_all(&output_dir)
        .await
        .map_err(|error| format!("Could not create output folder: {error}"))?;
    let throttle = crate::download::policy::Throttle::new(policy);
    let client = crate::network::proxy::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) TwitterXMediaBatchDownloader/1.0")
        .connect_timeout(crate::network::timeouts::connect())
        .build()
        .map_err(|error| format!("Could not initialize the downloader: {error}"))?;
    if convert_gif && item.media_type == "animated_gif" {
        crate::updates::ffmpeg::ensure_available(ui.clone(), cancel.clone(), true).await?;
    }
    let metadata_tool = if policy.embed_metadata && metadata_eligible(&item, convert_gif) {
        Some(crate::download::metadata::ensure_available(ui.clone(), cancel.clone()).await?)
    } else {
        None
    };
    let mut selected = item.clone();
    selected.url = selected_media_url(&item, &video_quality, &image_size);
    let path = download_one_with_policy(
        &client,
        &selected,
        &output_dir,
        item.media_index,
        &filename_format,
        ui,
        cancel.clone(),
        None,
        retry_attempts,
        throttle,
    )
    .await
    .map_err(|error| match error {
        DownloadError::Cancelled => "Download cancelled.".to_owned(),
        DownloadError::Failed(message) | DownloadError::Transient(message, _) => message,
    })?;
    if convert_gif && item.media_type == "animated_gif" {
        crate::download::gifs::convert_download(&path, gif_options, cancel)
            .await
            .map(|path| (path, None))
    } else {
        let warning =
            crate::download::metadata::tag_download(metadata_tool.as_deref(), &path, &item, cancel)
                .await;
        Ok((path, warning))
    }
}

#[derive(Debug)]
enum DownloadError {
    Cancelled,
    Failed(String),
    Transient(String, Option<Duration>),
}

pub fn matches_download_scope(
    _index: usize,
    item: &MediaCandidate,
    kind: &str,
    selected: &HashSet<usize>,
) -> bool {
    if kind == "selected" {
        selected.contains(&item.media_index)
    } else {
        crate::media::results::matches_kind(&item.media_type, kind)
    }
}

#[derive(Clone)]
struct TransferSlot {
    metrics: Arc<Mutex<BatchMetrics>>,
    slot: usize,
}

#[derive(Clone, Default)]
struct TransferAmount {
    downloaded: u64,
    total: Option<u64>,
}

#[derive(Default)]
struct BatchMetrics {
    transfers: Vec<TransferAmount>,
    saved: usize,
    failed: usize,
}

impl BatchMetrics {
    fn detail(&self) -> String {
        let downloaded = self
            .transfers
            .iter()
            .map(|transfer| transfer.downloaded)
            .sum::<u64>();
        let total = self
            .transfers
            .iter()
            .map(|transfer| transfer.total)
            .collect::<Option<Vec<_>>>()
            .map(|sizes| sizes.into_iter().sum::<u64>());
        let bytes = transfer_detail(downloaded, total);
        let failed = if self.failed > 0 {
            format!(" · {} failed", self.failed)
        } else {
            String::new()
        };
        format!(
            "{} / {} files saved{failed} · {bytes}",
            self.saved,
            self.transfers.len()
        )
    }
}

pub fn format_bytes(bytes: u64) -> String {
    let mut value = bytes as f64;
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut index = 0;
    while value >= 1000.0 && index < units.len() - 1 {
        value /= 1000.0;
        index += 1;
    }
    if index == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", units[index])
    }
}

fn transfer_detail(downloaded: u64, total: Option<u64>) -> String {
    match total {
        Some(total) => format!(
            "{} / {} downloaded",
            format_bytes(downloaded),
            format_bytes(total)
        ),
        None => format!("{} downloaded", format_bytes(downloaded)),
    }
}

#[cfg(test)]
async fn download_one(
    client: &reqwest::Client,
    item: &MediaCandidate,
    output_dir: &Path,
    index: usize,
    filename_format: &str,
    ui: crate::ui::runtime::Weak<AppWindow>,
    cancel: Arc<AtomicBool>,
    aggregate: Option<TransferSlot>,
    retry_attempts: usize,
) -> Result<PathBuf, DownloadError> {
    download_one_with_policy(
        client,
        item,
        output_dir,
        index,
        filename_format,
        ui,
        cancel,
        aggregate,
        retry_attempts,
        crate::download::policy::Throttle::new(Default::default()),
    )
    .await
}

async fn download_one_with_policy(
    client: &reqwest::Client,
    item: &MediaCandidate,
    output_dir: &Path,
    index: usize,
    filename_format: &str,
    ui: crate::ui::runtime::Weak<AppWindow>,
    cancel: Arc<AtomicBool>,
    aggregate: Option<TransferSlot>,
    retry_attempts: usize,
    throttle: Arc<crate::download::policy::Throttle>,
) -> Result<PathBuf, DownloadError> {
    let retries = retry_attempts.min(5);
    for attempt in 0..=retries {
        if cancel.load(Ordering::Relaxed) {
            return Err(DownloadError::Cancelled);
        }
        match download_attempt(
            client,
            item,
            output_dir,
            index,
            filename_format,
            ui.clone(),
            cancel.clone(),
            aggregate.clone(),
            throttle.clone(),
        )
        .await
        {
            Err(DownloadError::Transient(message, retry_after)) if attempt < retries => {
                if let Some(slot) = &aggregate {
                    slot.metrics
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .transfers[slot.slot] = TransferAmount::default();
                }
                let delay = retry_after
                    .unwrap_or_else(|| Duration::from_millis(500 * (1 << attempt)))
                    .min(Duration::from_secs(60));
                crate::platform::logging::warn(format!(
                    "Retrying media {} (attempt {}/{}): {message}",
                    index + 1,
                    attempt + 2,
                    retries + 1
                ));
                let message = format!(
                    "Retrying file {} · attempt {} of {}…",
                    index + 1,
                    attempt + 2,
                    retries + 1
                );
                let _ = ui.upgrade_in_event_loop(move |app| app.set_status(message));
                tokio::select! {
                    _ = sleep(delay) => {},
                    _ = wait_for_cancel(cancel.clone()) => return Err(DownloadError::Cancelled),
                }
            }
            Err(DownloadError::Transient(message, _)) => {
                return Err(DownloadError::Failed(message));
            }
            result => return result,
        }
    }
    unreachable!()
}

async fn download_attempt(
    client: &reqwest::Client,
    item: &MediaCandidate,
    output_dir: &Path,
    index: usize,
    filename_format: &str,
    ui: crate::ui::runtime::Weak<AppWindow>,
    cancel: Arc<AtomicBool>,
    aggregate: Option<TransferSlot>,
    throttle: Arc<crate::download::policy::Throttle>,
) -> Result<PathBuf, DownloadError> {
    if item.media_type == "text" {
        return download_text(item, output_dir, index, filename_format, &cancel, aggregate).await;
    }
    if !throttle.request(&cancel).await {
        return Err(DownloadError::Cancelled);
    }
    let response = tokio::select! {
        result = tokio::time::timeout(crate::network::timeouts::http(),client.get(&item.url).header(reqwest::header::REFERER, "https://x.com/").send()) => match result {
            Ok(result)=>result.map_err(|error| {if error.is_builder(){DownloadError::Failed(error.to_string())}else{DownloadError::Transient(error.to_string(),None)}})?,
            Err(_)=>return Err(DownloadError::Transient("HTTP request timed out.".into(),None)),
        },
        _ = wait_for_cancel(cancel.clone()) => return Err(DownloadError::Cancelled),
    };
    let status = response.status();
    if !status.is_success() {
        if status.as_u16() == 408 || status.as_u16() == 429 || status.is_server_error() {
            let delay = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(retry_after_delay);
            return Err(DownloadError::Transient(format!("HTTP {status}"), delay));
        }
        return Err(DownloadError::Failed(format!("HTTP {status}")));
    }
    let length = response.content_length();
    if let Some(slot) = &aggregate {
        slot.metrics
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .transfers[slot.slot]
            .total = length;
    }
    if let Some(slot) = &aggregate {
        let detail = slot
            .metrics
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .detail();
        set_transfer_detail(ui.clone(), detail);
    }
    // Atomically reserve .part files so parallel downloads with identical
    // filename templates cannot write to the same destination.
    let (output_path, part_path, mut file) = loop {
        let output_path = output_dir.join(media_filename(item, index, output_dir, filename_format));
        let part_path = part_file(&output_path);
        match tokio::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&part_path)
            .await
        {
            Ok(file) => break (output_path, part_path, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(DownloadError::Failed(error.to_string())),
        }
    };
    let mut last_progress = std::time::Instant::now();
    let mut stream = response.bytes_stream();
    let mut downloaded = 0u64;
    while let Some(next) = tokio::select! {
        next = tokio::time::timeout(crate::network::timeouts::http(),stream.next()) => match next {
            Ok(next)=>next,
            Err(_)=>{drop(file);let _=tokio::fs::remove_file(&part_path).await;return Err(DownloadError::Transient("HTTP read timed out.".into(),None));}
        },
        _ = wait_for_cancel(cancel.clone()) => {
            drop(file);
            let _ = tokio::fs::remove_file(&part_path).await;
            return Err(DownloadError::Cancelled);
        }
    } {
        let chunk = match next {
            Ok(chunk) => chunk,
            Err(error) => {
                drop(file);
                let _ = tokio::fs::remove_file(&part_path).await;
                return Err(DownloadError::Transient(error.to_string(), None));
            }
        };
        if !throttle.consume(chunk.len(), &cancel).await {
            drop(file);
            let _ = tokio::fs::remove_file(&part_path).await;
            return Err(DownloadError::Cancelled);
        }
        if let Err(error) = file.write_all(&chunk).await {
            drop(file);
            let _ = tokio::fs::remove_file(&part_path).await;
            return Err(DownloadError::Failed(error.to_string()));
        }
        downloaded += chunk.len() as u64;
        let fraction = length
            .filter(|length| *length > 0)
            .map(|length| (downloaded as f32 / length as f32).min(1.0))
            .unwrap_or(0.0);
        if let Some(slot) = &aggregate {
            slot.metrics
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .transfers[slot.slot]
                .downloaded = downloaded;
        }
        if last_progress.elapsed() >= Duration::from_millis(60) || fraction >= 1.0 {
            if let Some(slot) = &aggregate {
                let detail = slot
                    .metrics
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .detail();
                set_transfer_detail(ui.clone(), detail);
            }
            last_progress = std::time::Instant::now();
        }
    }
    if let Err(error) = file.flush().await {
        drop(file);
        let _ = tokio::fs::remove_file(&part_path).await;
        return Err(DownloadError::Failed(error.to_string()));
    }
    drop(file);
    if let Err(error) = tokio::fs::rename(&part_path, &output_path).await {
        let _ = tokio::fs::remove_file(&part_path).await;
        return Err(DownloadError::Failed(error.to_string()));
    }
    if let Some(slot) = &aggregate {
        let detail = slot
            .metrics
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .detail();
        set_transfer_detail(ui, detail);
    }
    Ok(output_path)
}

async fn download_text(
    item: &MediaCandidate,
    directory: &Path,
    index: usize,
    format: &str,
    cancel: &AtomicBool,
    aggregate: Option<TransferSlot>,
) -> Result<PathBuf, DownloadError> {
    if cancel.load(Ordering::Relaxed) {
        return Err(DownloadError::Cancelled);
    }
    let (output, part, mut file) = loop {
        let output = directory.join(media_filename(item, index, directory, format));
        let part = part_file(&output);
        match tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&part)
            .await
        {
            Ok(file) => break (output, part, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(DownloadError::Failed(error.to_string())),
        }
    };
    let bytes = format!("{}\n", item.tweet_text).into_bytes();
    let result = async {
        file.write_all(&bytes).await?;
        file.flush().await
    }
    .await;
    drop(file);
    if let Err(error) = result {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(DownloadError::Failed(error.to_string()));
    }
    if cancel.load(Ordering::Relaxed) {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(DownloadError::Cancelled);
    }
    if let Err(error) = tokio::fs::rename(&part, &output).await {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(DownloadError::Failed(error.to_string()));
    }
    if let Some(slot) = aggregate {
        slot.metrics
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .transfers[slot.slot] = TransferAmount {
            downloaded: bytes.len() as u64,
            total: Some(bytes.len() as u64),
        };
    }
    Ok(output)
}

fn retry_after_delay(value: &str) -> Option<Duration> {
    let seconds = value.parse::<u64>().ok().or_else(|| {
        let when = chrono::DateTime::parse_from_rfc2822(value).ok()?;
        Some((when.timestamp() - chrono::Utc::now().timestamp()).max(0) as u64)
    })?;
    Some(Duration::from_secs(seconds.min(60)))
}

#[cfg(test)]
mod retry_tests {
    use super::*;
    use std::collections::HashMap;
    use tokio::io::AsyncReadExt;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn retries_transient_responses_and_partial_streams_but_not_404_and_can_cancel_backoff() {
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(HashMap::<String, usize>::new()));
        let counts = requests.clone();
        let waiting = Arc::new(tokio::sync::Notify::new());
        let notification = waiting.clone();
        let server = tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = [0; 2048];
                let length = socket.read(&mut request).await.unwrap();
                let path = String::from_utf8_lossy(&request[..length])
                    .split_whitespace()
                    .nth(1)
                    .unwrap()
                    .to_owned();
                let attempt = {
                    let mut counts = counts.lock().unwrap();
                    let count = counts.entry(path.clone()).or_default();
                    *count += 1;
                    *count
                };
                let response = match (path.as_str(), attempt) {
                    ("/retry.jpg", 1) => b"HTTP/1.1 429 Too Many Requests\r\nRetry-After: 0\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".as_slice(),
                    ("/retry.jpg", 2) => b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\nbad".as_slice(),
                    ("/retry.jpg", _) => b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\ncomplete".as_slice(),
                    ("/not-found.jpg", _) => b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".as_slice(),
                    ("/cancel.jpg", _) => b"HTTP/1.1 429 Too Many Requests\r\nRetry-After: 60\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".as_slice(),
                    _ => b"HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".as_slice(),
                };
                socket.write_all(response).await.unwrap();
                if path == "/cancel.jpg" {
                    notification.notify_one();
                }
            }
        });
        let client = reqwest::Client::new();
        let item = |path: &str| MediaCandidate {
            url: format!("http://{address}/{path}.jpg"),
            media_type: "photo".into(),
            username: path.into(),
            ..Default::default()
        };
        let successful = download_one(
            &client,
            &item("retry"),
            &fixture.0,
            0,
            "{handle}",
            crate::ui::runtime::Weak::default(),
            Arc::new(AtomicBool::new(false)),
            None,
            2,
        )
        .await
        .unwrap();
        assert_eq!(std::fs::read(successful).unwrap(), b"complete");
        assert_eq!(requests.lock().unwrap()["/retry.jpg"], 3);
        assert!(
            download_one(
                &client,
                &item("not-found"),
                &fixture.0,
                0,
                "{handle}",
                crate::ui::runtime::Weak::default(),
                Arc::new(AtomicBool::new(false)),
                None,
                5
            )
            .await
            .is_err()
        );
        assert_eq!(requests.lock().unwrap()["/not-found.jpg"], 1);
        assert!(
            download_one(
                &client,
                &item("no-retry"),
                &fixture.0,
                0,
                "{handle}",
                crate::ui::runtime::Weak::default(),
                Arc::new(AtomicBool::new(false)),
                None,
                0
            )
            .await
            .is_err()
        );
        assert_eq!(requests.lock().unwrap()["/no-retry.jpg"], 1);
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let folder = fixture.0.clone();
        let candidate = item("cancel");
        let task = tokio::spawn(async move {
            download_one(
                &client,
                &candidate,
                &folder,
                0,
                "{handle}",
                crate::ui::runtime::Weak::default(),
                flag,
                None,
                5,
            )
            .await
        });
        waiting.notified().await;
        cancel.store(true, Ordering::Relaxed);
        assert!(matches!(
            tokio::time::timeout(Duration::from_millis(500), task)
                .await
                .unwrap()
                .unwrap(),
            Err(DownloadError::Cancelled)
        ));
        assert_eq!(requests.lock().unwrap()["/cancel.jpg"], 1);
        assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 1);
        server.abort();
    }
}

fn part_file(path: &Path) -> PathBuf {
    path.with_extension(format!(
        "{}.part",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("bin")
    ))
}

fn media_filename(
    item: &MediaCandidate,
    index: usize,
    output_dir: &Path,
    template: &str,
) -> String {
    let extension = file_extension(item);
    let base = render_filename_stem(template, item, index);
    let mut name = format!("{base}.{extension}");
    let mut suffix = 1usize;
    while output_dir.join(&name).exists() || part_file(&output_dir.join(&name)).exists() {
        name = format!("{base} ({suffix}).{extension}");
        suffix += 1;
    }
    name
}

fn render_filename_stem(template: &str, item: &MediaCandidate, index: usize) -> String {
    let post = crate::media::identity::post_timestamp(item);
    let post_value = |format: &str| {
        post.map(|date| date.format(format).to_string())
            .unwrap_or_else(|| "unknown".into())
    };
    let asset = crate::media::identity::asset_id(item);
    let rendered = template
        .replace("{post_date}", &post_value("%Y-%m-%d"))
        .replace("{post_time}", &post_value("%H-%M-%S"))
        .replace("{post_datetime}", &post_value("%Y-%m-%d_%H-%M-%S"))
        .replace(
            "{name}",
            if item.author_name.is_empty() {
                &item.username
            } else {
                &item.author_name
            },
        )
        .replace(
            "{media_id}",
            if asset.is_empty() {
                &item.tweet_id
            } else {
                &asset
            },
        )
        .replace("{date}", &current_utc_date())
        .replace("{handle}", item.username.trim_start_matches('@'))
        .replace("{tweet_id}", &item.tweet_id)
        .replace("{index}", &(index + 1).to_string())
        .replace(
            "{type}",
            if item.media_type == "animated_gif" {
                "gif"
            } else {
                &item.media_type
            },
        )
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let base = sanitize_filename(&rendered);
    let base = base
        .trim_matches(['.', ' '])
        .chars()
        .take(120)
        .collect::<String>();
    if base.is_empty() {
        format!("media_{}", index + 1)
    } else if is_windows_reserved_name(&base) {
        format!("_{base}")
    } else {
        base
    }
}

fn is_windows_reserved_name(value: &str) -> bool {
    let stem = value
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

pub fn filename_preview(template: &str) -> String {
    let sample = MediaCandidate {
        username: "NASA".into(),
        tweet_id: "1989033613824815258".into(),
        created_at: "Fri Jun 20 10:45:56 +0000 2025".into(),
        author_name: "NASA".into(),
        url: "https://pbs.twimg.com/media/Gx1aB2cXkAA7Yz9.jpg".into(),
        media_type: "photo".into(),
        ..MediaCandidate::default()
    };
    format!("{}.jpg", render_filename_stem(template, &sample, 0))
}

pub fn folder_name_preview(template: &str) -> String {
    let sample = MediaCandidate {
        username: "elonmusk".into(),
        author_name: "Elon Musk".into(),
        ..MediaCandidate::default()
    };
    render_folder_name_with_date(template, &sample, "20250620")
}

fn render_folder_name(template: &str, item: &MediaCandidate) -> String {
    let date = current_utc_date().replace('-', "");
    render_folder_name_with_date(template, item, &date)
}

fn render_folder_name_with_date(template: &str, item: &MediaCandidate, date: &str) -> String {
    let template = if template.trim().is_empty() {
        "{handle}"
    } else {
        template
    };
    let handle = item.username.trim().trim_start_matches('@');
    let display_name = if item.author_name.trim().is_empty() {
        handle
    } else {
        item.author_name.trim()
    };
    let rendered = template
        .replace("{handle}", handle)
        .replace("{username}", handle)
        .replace("{name}", display_name)
        .replace("{account_name}", display_name)
        .replace("{date}", date);
    sanitize_folder_component(&rendered, handle)
}

fn sanitize_folder_component(value: &str, fallback: &str) -> String {
    let clean = |value: &str| {
        sanitize_filename(value)
            .trim_matches(['.', ' '])
            .chars()
            .take(120)
            .collect::<String>()
    };
    let name = clean(value);
    let name = if name.is_empty() {
        clean(fallback)
    } else {
        name
    };
    let name = if name.is_empty() {
        "account".to_owned()
    } else {
        name
    };
    if is_windows_reserved_name(&name) {
        format!("_{name}")
    } else {
        name
    }
}

fn current_utc_date() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = (seconds / 86_400) as i64;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

pub fn current_date_label() -> String {
    current_utc_date()
}

pub fn new_history_id() -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("{millis}")
}

fn media_dimensions(item: &MediaCandidate) -> String {
    let size = if item.width > 0 && item.height > 0 {
        format!("{} × {}", item.width, item.height)
    } else {
        String::new()
    };
    if item.duration_ms > 0 {
        let seconds = item.duration_ms as f64 / 1000.0;
        if size.is_empty() {
            format!("{seconds:.1}s")
        } else {
            format!("{size} · {seconds:.1}s")
        }
    } else {
        size
    }
}

fn file_extension(item: &MediaCandidate) -> String {
    if item.media_type == "text" {
        return "txt".into();
    }
    if let Ok(url) = reqwest::Url::parse(&item.url) {
        if let Some(format) = url
            .query_pairs()
            .find(|(key, _)| key == "format")
            .map(|(_, value)| value.to_ascii_lowercase())
            && !format.is_empty()
            && format.len() <= 5
            && format.chars().all(char::is_alphanumeric)
        {
            return format;
        }
        if let Some(extension) = Path::new(url.path())
            .extension()
            .and_then(|ext| ext.to_str())
            .filter(|ext| {
                !ext.is_empty() && ext.len() <= 5 && ext.chars().all(char::is_alphanumeric)
            })
        {
            return extension.to_ascii_lowercase();
        }
    }
    match item.media_type.as_str() {
        "video" | "animated_gif" => "mp4".into(),
        _ => "jpg".into(),
    }
}

fn sanitize_filename(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
                )
            {
                '_'
            } else {
                character
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(['_', ' '])
        .to_owned()
}

pub(crate) fn resolve_xtractor_path() -> Result<PathBuf, String> {
    let directory =
        app_data_dir().ok_or("Could not locate the application data folder for xtractor.")?;
    let destination = directory.join(crate::platform::paths::XTRACTOR_BINARY);
    if destination.is_file() {
        return Ok(destination);
    }
    Err("Xtractor is not installed. Download it in Settings > General.".into())
}

async fn wait_for_cancel(cancel: Arc<AtomicBool>) {
    while !cancel.load(Ordering::Relaxed) {
        sleep(Duration::from_millis(100)).await;
    }
}

fn set_status(ui: crate::ui::runtime::Weak<AppWindow>, status: String, progress: f32) {
    let _ = crate::ui::runtime::invoke_from_event_loop(move || {
        if let Some(app) = ui.upgrade() {
            app.set_status(status);
            app.set_progress(progress.clamp(0.0, 100.0));
        }
    });
}

fn set_transfer_detail(ui: crate::ui::runtime::Weak<AppWindow>, detail: String) {
    let _ = crate::ui::runtime::invoke_from_event_loop(move || {
        if let Some(app) = ui.upgrade() {
            app.set_progress_detail(detail);
        }
    });
}

#[cfg(test)]
mod quality_tests {
    use super::*;
    #[test]
    fn download_details_do_not_present_partial_known_sizes_as_the_batch_total() {
        let mut metrics = BatchMetrics {
            transfers: vec![
                TransferAmount {
                    downloaded: 250_000,
                    total: Some(1_000_000),
                },
                TransferAmount {
                    downloaded: 125_000,
                    total: None,
                },
            ],
            saved: 1,
            failed: 0,
        };
        assert_eq!(metrics.detail(), "1 / 2 files saved · 375.0 KB downloaded");
        metrics.transfers[1].total = Some(2_000_000);
        assert_eq!(
            metrics.detail(),
            "1 / 2 files saved · 375.0 KB / 3.0 MB downloaded"
        );
        metrics.failed = 1;
        assert!(metrics.detail().contains("1 failed"));
        assert_eq!(transfer_detail(4096, None), "4.1 KB downloaded");
        assert_eq!(
            transfer_detail(4096, Some(8192)),
            "4.1 KB / 8.2 KB downloaded"
        );
    }
    #[test]
    fn photo_previews_use_original_sources_without_changing_grid_thumbnails() {
        let item = MediaCandidate {
            media_type: "photo".into(),
            url: "https://pbs.twimg.com/media/demo?format=png&name=small".into(),
            preview_url: "https://pbs.twimg.com/media/demo?format=png&name=large".into(),
            image_variants: vec![
                ImageVariant {
                    name: "360x360".into(),
                    url: "https://pbs.twimg.com/media/demo?format=png&name=360x360".into(),
                },
                ImageVariant {
                    name: "orig".into(),
                    url: "https://cdn.example.test/original.png".into(),
                },
            ],
            ..Default::default()
        };
        let row = to_ui_rows(&[item.clone()]).pop().unwrap();
        assert_eq!(row.preview_url, item.image_variants[1].url);
        assert_eq!(row.thumbnail_url, item.image_variants[0].url);
        let mut legacy = item;
        legacy.image_variants.clear();
        assert_eq!(
            photo_preview_url(&legacy),
            "https://pbs.twimg.com/media/demo?format=png&name=orig"
        );
        legacy.url.clear();
        assert_eq!(
            photo_preview_url(&legacy),
            "https://pbs.twimg.com/media/demo?format=png&name=orig"
        );
        assert_eq!(
            detail_preview_url(
                &serde_json::json!({"type":"photo"}),
                &[
                    ImageVariant {
                        name: "large".into(),
                        url: "large.jpg".into()
                    },
                    ImageVariant {
                        name: "orig".into(),
                        url: "original.jpg".into()
                    },
                ]
            ),
            "original.jpg"
        );
    }

    #[test]
    fn profile_sizes_and_thumbnail_quality_use_the_requested_variants() {
        let json = r#"{"account":{"username":"demo","avatar":"https://pbs.twimg.com/profile_images/1/a_normal.jpg","avatar_variants":[{"name":"orig","url":"https://pbs.twimg.com/profile_images/1/a.jpg"},{"name":"400x400","url":"https://pbs.twimg.com/profile_images/1/a_400x400.jpg"}],"banner":"https://pbs.twimg.com/profile_banners/1/123"}}"#;
        assert!(
            profile_image_url(json, "avatar", "400x400")
                .unwrap()
                .contains("_400x400.jpg")
        );
        assert!(
            profile_image_url(json, "avatar", "orig")
                .unwrap()
                .ends_with("/a.jpg")
        );
        assert!(
            profile_image_url(json, "banner", "600x200")
                .unwrap()
                .ends_with("/600x200")
        );
        let item = MediaCandidate {
            thumb_url: "https://pbs.twimg.com/media/test?format=jpg&name=small".into(),
            ..Default::default()
        };
        assert!(thumbnail_url(&item).contains("name=360x360"));
        let variants = vec![
            ImageVariant {
                name: "360x360".into(),
                url: "https://pbs.twimg.com/media/test?name=360x360".into(),
            },
            ImageVariant {
                name: "large".into(),
                url: "https://pbs.twimg.com/media/test?name=large".into(),
            },
        ];
        assert_eq!(
            thumb_url(&serde_json::json!({}), &variants),
            variants[0].url
        );
        assert_eq!(
            detail_preview_url(&serde_json::json!({"type":"photo"}), &variants),
            variants[1].url
        );
        assert!(
            fallback_thumbnail_url("https://video.twimg.com/tweet_video/abc.mp4")
                .contains("tweet_video_thumb/abc?format=jpg&name=360x360")
        );
        assert_eq!(
            image_url_with_size("https://example.test/image.png", "large"),
            "https://example.test/image.png"
        );
        let old: AppSettings = serde_json::from_str(r#"{"image_size":"large"}"#).unwrap();
        assert_eq!(old.avatar_size, "orig");
        assert_eq!(old.banner_size, "orig");
        assert_eq!(old.concurrent_downloads, 1);
        assert_eq!(old.theme_preset, "cacao");
        assert!(!old.separate_media_folders);
        assert_eq!(
            (
                old.download_speed_kib,
                old.download_delay_ms,
                old.download_jitter_ms
            ),
            (0, 0, 0)
        );
        assert_eq!((old.fetch_delay_ms, old.fetch_jitter_ms), (0, 0));
        assert!(
            !old.rotate_auth_tokens,
            "older settings never switch tokens on their own"
        );
        assert!(old.output_dir.is_none());
        assert!(!old.convert_gif);
        assert_eq!(old.gif_quality, "fast");
        assert_eq!(old.gif_resolution, "original");
    }

    #[tokio::test]
    async fn filtered_downloads_request_all_matching_media_and_preserve_other_results() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let saved = fixture.0.join("already-saved.jpg");
        std::fs::write(&saved, b"existing").unwrap();
        let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = server.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let received = requests.clone();
        let server_task = tokio::spawn(async move {
            loop {
                let (mut socket, _) = server.accept().await.unwrap();
                let mut buffer = [0; 4096];
                let count = socket.read(&mut buffer).await.unwrap();
                let path = String::from_utf8_lossy(&buffer[..count])
                    .split_whitespace()
                    .nth(1)
                    .unwrap()
                    .to_owned();
                received.lock().unwrap().push(path);
                socket
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\nDATA",
                    )
                    .await
                    .unwrap();
            }
        });
        let candidates = (0..40)
            .map(|index| MediaCandidate {
                url: format!("http://{address}/{index}.jpg"),
                media_index: index * 3,
                media_type: if index < 36 {
                    "photo"
                } else if index < 38 {
                    "video"
                } else {
                    "animated_gif"
                }
                .into(),
                username: "filter-test".into(),
                status: if index == 0 {
                    "Saved"
                } else if index == 39 {
                    "Failed"
                } else {
                    "Ready"
                }
                .into(),
                saved_path: if index == 0 {
                    saved.to_string_lossy().into_owned()
                } else {
                    String::new()
                },
                ..Default::default()
            })
            .collect::<Vec<_>>();
        for kind in ["photo", "video", "animated_gif", "all", "selected"] {
            let expected_indices = match kind {
                "photo" => (1..36).collect::<Vec<_>>(),
                "video" => (36..38).collect(),
                "animated_gif" => (38..40).collect(),
                "all" => (1..40).collect(),
                "selected" => vec![2, 34, 36, 39],
                _ => unreachable!(),
            };
            requests.lock().unwrap().clear();
            let backend = Arc::new(Mutex::new(BackendState {
                candidates: candidates.clone(),
                ..Default::default()
            }));
            let summary = download_all(
                candidates.clone(),
                kind,
                fixture.0.join(kind),
                "{handle}_{index}".into(),
                "{handle}".into(),
                None,
                "highest".into(),
                "orig".into(),
                3,
                2,
                if kind == "selected" {
                    [0, 2, 34, 36, 39]
                        .into_iter()
                        .map(|index| candidates[index].media_index)
                        .collect()
                } else {
                    Vec::new()
                },
                false,
                crate::download::gifs::Options::default(),
                backend.clone(),
                crate::ui::runtime::Weak::default(),
                Arc::new(AtomicBool::new(false)),
                crate::download::policy::Options::default(),
            )
            .await;
            let mut expected = expected_indices
                .iter()
                .map(|index| format!("/{index}.jpg"))
                .collect::<Vec<_>>();
            let mut actual = requests.lock().unwrap().clone();
            expected.sort();
            actual.sort();
            assert_eq!(actual, expected, "Wrong files downloaded for {kind}");
            assert_eq!(summary.progress, 100.0);
            let state = backend.lock().unwrap();
            assert_eq!(state.candidates.len(), candidates.len());
            for (index, (item, original)) in state.candidates.iter().zip(&candidates).enumerate() {
                assert_eq!(item.media_index, original.media_index);
                if expected_indices.contains(&index) {
                    assert!(crate::download::saved_files::exists(item));
                    assert_eq!(std::fs::read(&item.saved_path).unwrap(), b"DATA");
                } else {
                    assert_eq!(item.status, original.status);
                    assert_eq!(item.saved_path, original.saved_path);
                }
            }
        }
        server_task.abort();
        assert_eq!(std::fs::read(saved).unwrap(), b"existing");
    }

    #[tokio::test]
    async fn parallel_downloads_preserve_unique_files_with_identical_names() {
        use std::sync::atomic::AtomicUsize;
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };
        let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = server.local_addr().unwrap();
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let seen = peak.clone();
        let task = tokio::spawn(async move {
            let mut tasks = tokio::task::JoinSet::new();
            for _ in 0..12 {
                let (mut socket, _) = server.accept().await.unwrap();
                let active = active.clone();
                let peak = peak.clone();
                tasks.spawn(async move {
                    let mut request = [0; 4096]; assert!(socket.read(&mut request).await.unwrap()>0);
                    let count = active.fetch_add(1, Ordering::AcqRel) + 1; peak.fetch_max(count, Ordering::AcqRel);
                    tokio::time::sleep(Duration::from_millis(80)).await;
                    socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\nDATA").await.unwrap();
                    active.fetch_sub(1, Ordering::AcqRel);
                });
            }
            while tasks.join_next().await.is_some() {}
        });
        let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!(
                "parallel-test-{}-{}",
                std::process::id(),
                new_history_id()
            ));
        let items = (0..12)
            .map(|index| MediaCandidate {
                media_index: index,
                url: format!("http://{address}/{index}.jpg"),
                username: "same".into(),
                status: "Ready".into(),
                media_type: "photo".into(),
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let backend = Arc::new(Mutex::new(BackendState {
            candidates: items.clone(),
            ..Default::default()
        }));
        let result = download_all(
            items,
            "all",
            directory.clone(),
            "{handle}".into(),
            "{handle}".into(),
            None,
            "highest".into(),
            "original".into(),
            8,
            2,
            Vec::new(),
            false,
            crate::download::gifs::Options::default(),
            backend.clone(),
            crate::ui::runtime::Weak::default(),
            Arc::new(AtomicBool::new(false)),
            crate::download::policy::Options::default(),
        )
        .await;
        task.await.unwrap();
        assert_eq!(result.progress, 100.0);
        assert!(seen.load(Ordering::Acquire) > 5 && seen.load(Ordering::Acquire) <= 8);
        let paths = backend
            .lock()
            .unwrap()
            .candidates
            .iter()
            .map(|item| item.saved_path.clone())
            .collect::<HashSet<_>>();
        assert_eq!(paths.len(), 12);
        for path in paths {
            assert_eq!(std::fs::read(path).unwrap(), b"DATA");
        }
    }
}

#[cfg(test)]
mod post_filename_tests {
    use super::*;
    #[test]
    fn post_filename_tokens_preserve_download_date_and_sanitize_names() {
        let item = MediaCandidate {
            created_at: "Sun Aug 31 17:52:02 +0000 2025".into(),
            username: "demo".into(),
            author_name: "A/B:*Artist".into(),
            tweet_id: "123".into(),
            url: "https://pbs.twimg.com/media/ABC.jpg?name=orig".into(),
            ..Default::default()
        };
        assert_eq!(
            render_filename_stem("{post_date}_{post_time}_{media_id}_{name}", &item, 0),
            "2025-08-31_17-52-02_ABC_A_B__Artist"
        );
        assert_eq!(render_filename_stem("{date}", &item, 0), current_utc_date());
        assert_eq!(
            render_filename_stem("{post_datetime}", &MediaCandidate::default(), 0),
            "unknown"
        );
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let first = media_filename(&item, 0, &fixture.0, "{post_date}_{media_id}");
        std::fs::write(fixture.0.join(&first), b"existing").unwrap();
        assert_ne!(
            media_filename(&item, 0, &fixture.0, "{post_date}_{media_id}"),
            first
        );
    }
}

#[cfg(test)]
mod policy_tests {
    use super::*;
    #[tokio::test]
    async fn type_folders_and_shared_pacing_apply_to_real_parallel_requests() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = server.local_addr().unwrap();
        let starts = Arc::new(Mutex::new(Vec::new()));
        let observed = starts.clone();
        let task = tokio::spawn(async move {
            loop {
                let (mut socket, _) = server.accept().await.unwrap();
                let mut request = [0; 2048];
                assert!(socket.read(&mut request).await.unwrap() > 0);
                observed.lock().unwrap().push(tokio::time::Instant::now());
                tokio::spawn(async move {
                    socket
                        .write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 8192\r\nConnection: close\r\n\r\n",
                        )
                        .await
                        .unwrap();
                    socket.write_all(&vec![42; 8192]).await.unwrap();
                });
            }
        });
        let candidates = [("photo", "jpg"), ("video", "mp4"), ("animated_gif", "mp4")]
            .into_iter()
            .enumerate()
            .map(|(index, (kind, ext))| MediaCandidate {
                url: format!("http://{address}/{index}.{ext}"),
                media_type: kind.into(),
                username: "demo".into(),
                media_index: index,
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let backend = Arc::new(Mutex::new(BackendState::default()));
        let started = tokio::time::Instant::now();
        let summary = download_all(
            candidates,
            "all",
            fixture.0.clone(),
            "{type}_{index}".into(),
            "{handle}".into(),
            None,
            "highest".into(),
            "original".into(),
            3,
            0,
            Vec::new(),
            false,
            crate::download::gifs::Options::default(),
            backend.clone(),
            crate::ui::runtime::Weak::default(),
            Arc::new(AtomicBool::new(false)),
            crate::download::policy::Options {
                separate_types: true,
                embed_metadata: false,
                speed_kib: 32,
                delay_ms: 90,
                jitter_ms: 50,
            },
        )
        .await;
        assert!(
            summary.message.contains("Downloaded 3 media"),
            "{}",
            summary.message
        );
        assert!(started.elapsed() >= Duration::from_millis(700));
        let times = starts.lock().unwrap();
        assert_eq!(times.len(), 3);
        for pair in times.windows(2) {
            assert!(pair[1] - pair[0] >= Duration::from_millis(70));
        }
        for item in &backend.lock().unwrap().candidates {
            assert!(
                Path::new(&item.saved_path)
                    .parent()
                    .unwrap()
                    .ends_with(crate::download::policy::media_folder(&item.media_type))
            );
            assert_eq!(std::fs::read(&item.saved_path).unwrap().len(), 8192);
        }
        task.abort();
    }
}

#[cfg(test)]
mod metadata_download_tests {
    use super::*;
    #[tokio::test]
    async fn metadata_runs_in_single_and_batch_downloads_without_losing_failed_tags() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };
        let Some(tool) = crate::download::metadata::use_fixture_tool() else {
            return;
        };
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let source = fixture.0.join("source.jpg");
        image::RgbImage::from_pixel(12, 12, image::Rgb([10, 20, 30]))
            .save(&source)
            .unwrap();
        let jpeg = Arc::new(std::fs::read(source).unwrap());
        let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = server.local_addr().unwrap();
        let task = tokio::spawn(async move {
            loop {
                let (mut socket, _) = server.accept().await.unwrap();
                let bytes = jpeg.clone();
                tokio::spawn(async move {
                    let mut request = [0; 2048];
                    let n = socket.read(&mut request).await.unwrap();
                    let bad = String::from_utf8_lossy(&request[..n]).contains("bad.jpg");
                    let body = if bad {
                        &b"corrupt download"[..]
                    } else {
                        bytes.as_slice()
                    };
                    socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).as_bytes()).await.unwrap();
                    socket.write_all(body).await.unwrap();
                });
            }
        });
        let item = MediaCandidate {
            url: format!("http://{address}/photo.jpg"),
            media_type: "photo".into(),
            username: "demo".into(),
            tweet_url: "https://x.com/demo/status/123".into(),
            tweet_text: "A saved post 世界 🐦".into(),
            ..Default::default()
        };
        let policy = crate::download::policy::Options {
            embed_metadata: true,
            ..Default::default()
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let (single, warning) = download_selected_file(
            item.clone(),
            0,
            fixture.0.clone(),
            "single".into(),
            "{handle}".into(),
            None,
            "highest".into(),
            "original".into(),
            false,
            crate::download::gifs::Options::default(),
            0,
            crate::ui::runtime::Weak::default(),
            cancel.clone(),
            policy,
        )
        .await
        .unwrap();
        assert!(warning.is_none());
        let mut read = tokio::process::Command::new(&tool);
        read.args(["-j", "-Comment"]).arg(&single);
        #[cfg(windows)]
        read.creation_flags(0x08000000);
        let result = read.output().await.unwrap();
        let tags: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(
            tags[0]["Comment"],
            crate::download::metadata::comment(&item)
        );
        let mut bad = item.clone();
        bad.url = format!("http://{address}/bad.jpg");
        bad.media_index = 1;
        let state = Arc::new(Mutex::new(BackendState::default()));
        let summary = download_all(
            vec![item, bad],
            "all",
            fixture.0.clone(),
            "batch_{index}".into(),
            "{handle}".into(),
            None,
            "highest".into(),
            "original".into(),
            2,
            0,
            Vec::new(),
            false,
            crate::download::gifs::Options::default(),
            state.clone(),
            crate::ui::runtime::Weak::default(),
            cancel,
            policy,
        )
        .await;
        assert!(
            summary.message.contains("Metadata failed for 1 file(s)"),
            "{}",
            summary.message
        );
        assert!(
            state
                .lock()
                .unwrap()
                .candidates
                .iter()
                .all(|item| item.status == "Saved" && Path::new(&item.saved_path).is_file())
        );
        task.abort();
    }
}

#[cfg(test)]
mod text_tests {
    use super::*;
    #[tokio::test]
    async fn text_posts_keep_unicode_and_download_without_http_or_overwriting_existing_files() {
        let response = json!({"account":{"username":"demo"},"tweets":[
            {"tweet_id":"1","text":"こんにちは café\nsecond line","author":{"username":"demo"},"media":[]},
            {"tweet_id":"2","text":"caption","media":[{"type":"video","variants":[{"content_type":"video/mp4","url":"https://video.twimg.com/ext_tw_video/2/clip.mp4","bitrate":100}]}]},
            {"tweet_id":"3","text":"   ","media":[]}
        ]});
        let items = collect_media(&response);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].media_type, "text");
        assert_eq!(items[1].media_type, "video");
        assert_eq!(file_extension(&items[0]), "txt");
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let cancel = Arc::new(AtomicBool::new(false));
        let client = reqwest::Client::new();
        let call = || {
            download_one_with_policy(
                &client,
                &items[0],
                &fixture.0,
                0,
                "same",
                crate::ui::runtime::Weak::default(),
                cancel.clone(),
                None,
                0,
                crate::download::policy::Throttle::new(Default::default()),
            )
        };
        let (first, second) = tokio::join!(call(), call());
        let first = first.unwrap();
        let second = second.unwrap();
        assert_ne!(first, second);
        assert_eq!(
            tokio::fs::read_to_string(&first).await.unwrap(),
            "こんにちは café\nsecond line\n"
        );
        assert!(first.extension().is_some_and(|ext| ext == "txt"));
        cancel.store(true, Ordering::Relaxed);
        assert!(matches!(call().await, Err(DownloadError::Cancelled)));
        assert!(std::fs::read_dir(&fixture.0).unwrap().all(|file| {
            !file
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".part")
        }));
    }
}

#[cfg(test)]
mod preview_details_tests {
    use super::*;
    #[test]
    fn preview_details_cover_media_text_reposts_and_legacy_archives() {
        let response = json!({"account":{"username":"demo"},"tweets":[
            {"tweet_id":"1","text":"Photo","author":{"username":"author","avatar":"https://pbs.twimg.com/profile_images/author.jpg","profile_image_shape":"Square"},"is_repost":true,"reposted_by":{"username":"demo"},"stats":{"quotes":7,"bookmarks":12},"media":[{"type":"photo","url":"https://pbs.twimg.com/media/test.jpg","alt_text":"A mountain at sunrise"}]},
            {"tweet_id":"2","text":"Text only","stats":{"quotes":3,"bookmarks":4},"media":[]}
        ]});
        let items = collect_media(&response);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].alt_text, "A mountain at sunrise");
        assert_eq!((items[0].quotes, items[0].bookmarks), (7, 12));
        assert!(items[0].is_repost);
        assert_eq!(
            items[0].author_avatar_url,
            "https://pbs.twimg.com/profile_images/author.jpg"
        );
        assert!(items[0].author_avatar_square);
        assert_eq!(items[0].reposted_by, "demo");
        assert_eq!((items[1].quotes, items[1].bookmarks), (3, 4));
        let mut legacy = items.clone();
        legacy[0].alt_text.clear();
        legacy[0].author_avatar_url.clear();
        legacy[0].quotes = 0;
        legacy[0].bookmarks = 0;
        legacy[0].saved_path = "keep.jpg".into();
        legacy[0].status = "Saved".into();
        legacy[0].media_index = 42;
        restore_preview_details(&mut legacy, &response.to_string());
        assert_eq!(legacy[0].alt_text, items[0].alt_text);
        assert_eq!(legacy[0].author_avatar_url, items[0].author_avatar_url);
        assert_eq!(legacy[0].quotes, 7);
        assert_eq!(legacy[0].saved_path, "keep.jpg");
        assert_eq!(legacy[0].media_index, 42);
        assert_eq!(legacy[0].status, "Saved");
        let rows = to_ui_rows(&legacy);
        assert_eq!(rows[0].quotes, "7");
        assert_eq!(rows[0].bookmarks, "12");
        assert!(rows[0].is_repost);
        assert_eq!(rows[0].author_avatar_url, items[0].author_avatar_url);
        assert!(rows[0].author_avatar_square);
    }

    #[test]
    fn author_avatars_use_the_matching_author_in_mixed_and_legacy_responses() {
        let response = json!({"account":{"username":"owner", "avatar":"https://example.com/owner.jpg"},"tweets":[
            {"tweet_id":"1", "text":"owner post", "media":[]},
            {"tweet_id":"2", "text":"liked post", "author":{"username":"other", "profile_image_url_https":"https://example.com/other.jpg"}, "media":[]},
            {"tweet_id":"3", "text":"no avatar", "author":{"username":"third"}, "media":[]}
        ]});
        let media = collect_media(&response);
        assert_eq!(media[0].author_avatar_url, "https://example.com/owner.jpg");
        assert_eq!(media[1].author_avatar_url, "https://example.com/other.jpg");
        assert!(
            media[2].author_avatar_url.is_empty(),
            "Likes must not inherit the signed-in account's avatar"
        );
        let old: MediaCandidate =
            serde_json::from_str(r#"{"username":"old","media_type":"photo"}"#).unwrap();
        assert!(old.author_avatar_url.is_empty());
    }
}

#[cfg(test)]
mod ui_model_refresh_tests {
    use super::*;
    use std::cell::RefCell;

    /// Records which rows were written, so the test can assert on targeted updates.
    struct Recording {
        rows: VecModel<MediaItem>,
        written: RefCell<Vec<usize>>,
    }
    impl Model for Recording {
        type Data = MediaItem;
        fn row_count(&self) -> usize {
            self.rows.row_count()
        }
        fn row_data(&self, row: usize) -> Option<MediaItem> {
            self.rows.row_data(row)
        }
        fn set_row_data(&self, row: usize, data: MediaItem) {
            self.written.borrow_mut().push(row);
            self.rows.set_row_data(row, data);
        }
    }

    fn row(index: i32, status: &str) -> MediaItem {
        MediaItem {
            media_index: index,
            status: status.into(),
            ..Default::default()
        }
    }

    #[test]
    fn status_refresh_updates_only_changed_rows_and_keeps_decoded_images() {
        let thumbnail = Image::from_rgba8(crate::ui::runtime::SharedPixelBuffer::new(4, 3));
        let mut first = row(0, "Ready");
        first.preview_image = thumbnail.clone();
        let model = Recording {
            rows: VecModel::from(vec![first, row(1, "Ready"), row(2, "Ready")]),
            written: RefCell::default(),
        };

        let rest = update_rows_in_place(
            &model,
            vec![row(0, "Saved"), row(1, "Ready"), row(2, "Failed")],
        );

        assert!(rest.is_none());
        assert_eq!(*model.written.borrow(), vec![0, 2]);
        assert_eq!(model.row_data(0).unwrap().status, "Saved");
        assert_eq!(model.row_data(0).unwrap().preview_image.size().width, 4);
        assert_eq!(model.row_data(2).unwrap().status, "Failed");
    }

    #[test]
    fn different_media_or_order_asks_for_a_new_model() {
        let model = VecModel::from(vec![row(0, "Ready"), row(1, "Ready")]);
        assert!(update_rows_in_place(&model, vec![row(1, "Ready"), row(0, "Ready")]).is_some());
        assert!(update_rows_in_place(&model, vec![row(0, "Ready")]).is_some());
        assert_eq!(model.row_data(0).unwrap().media_index, 0);
    }
}

#[cfg(test)]
mod account_preview_tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    #[test]
    fn first_page_starts_profile_images_before_media_and_preserves_decoded_images() {
        let app = AppWindow::new().unwrap();
        app.set_is_busy(true);
        app.set_is_extracting(true);
        app.set_extraction_generation(7);
        let loads = Rc::new(RefCell::new(Vec::new()));
        app.on_load_profile_images({
            let loads = loads.clone();
            let weak = app.as_weak();
            move || {
                let profile = weak.upgrade().unwrap().get_account_profile();
                loads
                    .borrow_mut()
                    .push((profile.avatar_url, profile.banner_url));
            }
        });
        let mut account = json!({"username":"owner","name":"Early profile","avatar":"https://example.invalid/avatar.png","banner":"https://example.invalid/banner.png","followers":42});
        let mut previous = None;
        publish_account_preview(app.as_weak(), 7, true, &account, &[], &mut previous);
        crate::ui::runtime::pump(&egui::Context::default());
        assert!(app.get_is_extracting());
        assert_eq!(app.get_media_items().row_count(), 0);
        assert_eq!(app.get_account_profile().display_name, "Early profile");
        assert_eq!(
            *loads.borrow(),
            [(
                "https://example.invalid/avatar.png".into(),
                "https://example.invalid/banner.png".into()
            )]
        );
        let mut profile = app.get_account_profile();
        let image = Image::from_rgba8(crate::ui::runtime::SharedPixelBuffer::new(4, 4));
        profile.avatar_image = image.clone();
        profile.has_avatar = true;
        profile.banner_image = image.clone();
        profile.has_banner = true;
        app.set_account_profile(profile);
        account["followers"] = json!(43);
        publish_account_preview(app.as_weak(), 7, true, &account, &[], &mut previous);
        crate::ui::runtime::pump(&egui::Context::default());
        assert_eq!(app.get_account_profile().avatar_image, image);
        assert_eq!(app.get_account_profile().banner_image, image);
        assert_eq!(app.get_account_profile().followers, "43");
        let count = loads.borrow().len();
        publish_account_preview(app.as_weak(), 7, true, &account, &[], &mut previous);
        crate::ui::runtime::pump(&egui::Context::default());
        assert_eq!(loads.borrow().len(), count);
        apply_account_profile(
            &app,
            account_profile_from_json(&json!({"account":account}).to_string()),
        );
        assert_eq!(app.get_account_profile().avatar_image, image);
        assert_eq!(app.get_account_profile().banner_image, image);
        assert_eq!(loads.borrow().len(), count);
    }

    #[test]
    fn background_finished_stale_and_unrelated_pages_cannot_replace_the_visible_account() {
        let app = AppWindow::new().unwrap();
        app.set_account_profile(AccountProfile {
            available: true,
            username: "visible".into(),
            ..Default::default()
        });
        app.set_is_busy(true);
        app.set_is_extracting(true);
        app.set_extraction_generation(9);
        let account = json!({"username":"other","avatar":"https://example.invalid/other.png"});
        for (publish, generation, extracting, busy) in [
            (false, 9, true, true),
            (true, 8, true, true),
            (true, 9, false, true),
            (true, 9, true, false),
        ] {
            app.set_is_extracting(extracting);
            app.set_is_busy(busy);
            publish_account_preview(app.as_weak(), generation, publish, &account, &[], &mut None);
            crate::ui::runtime::pump(&egui::Context::default());
            assert_eq!(app.get_account_profile().username, "visible");
        }
        app.set_is_busy(true);
        app.set_is_extracting(true);
        let mut previous = None;
        for handle in ["first_author", "later_author"] {
            publish_account_preview(
                app.as_weak(),
                9,
                true,
                &json!({}),
                &[json!({"author":{"username":handle}})],
                &mut previous,
            );
            crate::ui::runtime::pump(&egui::Context::default());
            assert_eq!(app.get_account_profile().username, "first_author");
        }
    }
}

#[cfg(test)]
mod overall_progress_tests {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn chunk_updates_report_total_bytes_without_moving_overall_file_progress() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (paused, pause) = tokio::sync::oneshot::channel();
        let (release, resume) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let mut request = [0; 4096];
            let (mut first, _) = listener.accept().await.unwrap();
            first.read(&mut request).await.unwrap();
            first
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\nDATA")
                .await
                .unwrap();
            drop(first);
            let (mut second, _) = listener.accept().await.unwrap();
            second.read(&mut request).await.unwrap();
            second
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\n")
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_millis(90)).await;
            second.write_all(b"DA").await.unwrap();
            paused.send(()).unwrap();
            let _ = resume.await;
            second.write_all(b"TA").await.unwrap();
        });
        let candidates = (0..2)
            .map(|index| MediaCandidate {
                username: "progress".into(),
                tweet_id: index.to_string(),
                media_index: index,
                media_type: "photo".into(),
                url: format!("http://{address}/{index}.jpg"),
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let backend = Arc::new(Mutex::new(BackendState {
            candidates: candidates.clone(),
            ..Default::default()
        }));
        let app = AppWindow::new().unwrap();
        app.set_progress(0.0);
        let task = tokio::spawn(download_all(
            candidates,
            "all",
            fixture.0.clone(),
            "{index}".into(),
            "{handle}".into(),
            None,
            "highest".into(),
            "orig".into(),
            1,
            0,
            Vec::new(),
            false,
            crate::download::gifs::Options::default(),
            backend,
            app.as_weak(),
            Arc::new(AtomicBool::new(false)),
            crate::download::policy::Options::default(),
        ));
        tokio::time::timeout(Duration::from_secs(5), pause)
            .await
            .unwrap()
            .unwrap();
        let ctx = egui::Context::default();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            crate::ui::runtime::pump(&ctx);
            if app.get_progress_detail().contains("6 B / 8 B downloaded") {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "No aggregate byte update: {}",
                app.get_progress_detail()
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(
            app.get_progress(),
            50.0,
            "A partial file moved the overall completion bar"
        );
        assert!(app.get_progress_detail().contains("1 / 2 files saved"));
        release.send(()).unwrap();
        assert_eq!(task.await.unwrap().progress, 100.0);
        server.await.unwrap();
        crate::ui::runtime::pump(&ctx);
        assert_eq!(app.get_progress(), 100.0);
        assert!(app.get_progress_detail().contains("2 / 2 files saved"));
    }
}
