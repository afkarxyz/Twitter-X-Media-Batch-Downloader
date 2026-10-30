use super::notifications::show_toast;
use crate::AppWindow;
use crate::I18n;
use crate::MediaItem;
use crate::download::engine as downloader;
use crate::download::saved_files;
use crate::fetch::archives as fetch_archives;
use crate::fetch::batch as batch_fetch;
use crate::fetch::policy as fetch_policy;
use crate::fetch::request as fetch_request;
use crate::history::db as history_db;
use crate::history::view_model as history_ui;
use crate::i18n;
use crate::media::changes as media_changes;
use crate::platform::logging as session_log;
use crate::ui::runtime::ModelRc;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

pub(crate) fn empty_media_model() -> ModelRc<MediaItem> {
    downloader::model_from_rows(Vec::<MediaItem>::new())
}

pub(crate) fn current_profile_from_ui(app: &AppWindow) -> downloader::ExtractionProfile {
    if let Ok(profile) = fetch_request::profile_from_app(
        app,
        app.get_source_url().as_str(),
        &fetch_request::range_from_app(app),
    ) {
        return profile;
    }
    let media_type = fetch_request::media_mask(app);
    let retweet_modes = ["skip", "include"];
    let retweets = retweet_modes
        .get(app.get_retweets_index().clamp(0, 1) as usize)
        .copied()
        .unwrap_or("skip")
        .to_owned();
    downloader::ExtractionProfile {
        url: app.get_source_url().trim().to_owned(),
        page_size: downloader::POSTS_PER_REQUEST,
        media_type: media_type.clone(),
        retweets,
        no_videos: app.get_no_videos() && media_type != "video",
        text_tweets: app.get_text_tweets(),
        ..Default::default()
    }
}

pub(crate) fn same_extraction_filters(
    current: &downloader::ExtractionProfile,
    previous: &downloader::ExtractionProfile,
) -> bool {
    current.url == previous.url
        && fetch_request::normalized_mask(&current.media_type, current.no_videos)
            == fetch_request::normalized_mask(&previous.media_type, previous.no_videos)
        && current.retweets == previous.retweets
        && current.text_tweets == previous.text_tweets
}

pub(crate) fn spawn_extraction(
    ui: crate::ui::runtime::Weak<AppWindow>,
    options: downloader::ExtractionOptions,
    profile: downloader::ExtractionProfile,
    backend: Arc<Mutex<downloader::BackendState>>,
    history: Arc<Mutex<Vec<downloader::HistoryEntry>>>,
    history_filter: Arc<Mutex<(String, bool)>>,
    cancel: Arc<AtomicBool>,
) {
    spawn_extraction_job(
        ui,
        options,
        profile,
        backend,
        history,
        history_filter,
        cancel,
        None,
    );
}

pub(crate) fn spawn_extraction_job(
    ui: crate::ui::runtime::Weak<AppWindow>,
    mut options: downloader::ExtractionOptions,
    profile: downloader::ExtractionProfile,
    backend: Arc<Mutex<downloader::BackendState>>,
    history: Arc<Mutex<Vec<downloader::HistoryEntry>>>,
    history_filter: Arc<Mutex<(String, bool)>>,
    cancel: Arc<AtomicBool>,
    completion: Option<batch_fetch::CompletionFn>,
) {
    let Some(app) = ui.upgrade() else { return };
    let incremental = app.get_incremental_fetch();
    let generation = app.get_extraction_generation().wrapping_add(1);
    app.set_extraction_generation(generation);
    let publish_results = completion.is_none();
    tokio::spawn(async move {
        if incremental && options.cursor.is_empty() && options.previous_json.is_none() {
            let profile = profile.clone();
            let entries = history.lock().unwrap().clone();
            options.refresh_boundary =
                tokio::task::spawn_blocking(move || fetch_policy::boundary(&profile, &entries))
                    .await
                    .unwrap_or_default();
        }

        let result = downloader::extract(
            options,
            ui.clone(),
            cancel.clone(),
            generation,
            publish_results,
        )
        .await;
        let cancelled = cancel.load(Ordering::Relaxed);
        match result {
            Ok(mut outcome) => {
                let profile_json =
                    crate::history::storage::account_json(&outcome.json).unwrap_or_default();
                let (history_id, saved) = {
                    let mut data = backend.lock().unwrap_or_else(|e| e.into_inner());
                    let saved = data
                        .candidates
                        .iter()
                        .filter(|candidate| saved_files::exists(candidate))
                        .map(|candidate| (candidate.url.clone(), candidate.clone()))
                        .collect::<std::collections::HashMap<_, _>>();
                    if data.history_id.is_empty() {
                        data.history_id = downloader::new_history_id();
                    }
                    (data.history_id.clone(), saved)
                };
                for candidate in &mut outcome.candidates {
                    if let Some(saved) = saved.get(&candidate.url) {
                        candidate.status.clone_from(&saved.status);
                        candidate.saved_path.clone_from(&saved.saved_path);
                        candidate.saved_bytes = saved.saved_bytes;
                        candidate.saved_sha256.clone_from(&saved.saved_sha256);
                    }
                }
                let fetched_media_count = outcome.candidates.len();
                let status = if let Some(warning) = outcome.warning.as_deref() {
                    format!(
                        "{} posts · {} media files · {} page(s). {}",
                        outcome.tweet_count, fetched_media_count, outcome.pages, warning
                    )
                } else {
                    format!(
                        "{} posts · {} media files · {} page(s) extracted.",
                        outcome.tweet_count, fetched_media_count, outcome.pages
                    )
                };
                let status = if outcome.has_more {
                    format!("{status} More pages are available.")
                } else {
                    status
                };
                let mut entry = downloader::HistoryEntry {
                    id: history_id.clone(),
                    fetched_at: downloader::current_date_label(),
                    source_url: profile.url.clone(),
                    account_name: outcome.account_name.clone(),
                    username: if outcome.username.is_empty() {
                        outcome
                            .candidates
                            .first()
                            .map(|item| item.username.clone())
                            .unwrap_or_default()
                    } else {
                        outcome.username.clone()
                    },
                    tweet_count: outcome.tweet_count,
                    media: outcome.candidates.clone(),
                    json: outcome.json.clone(),
                    profile: Some(profile.clone()),
                    next_cursor: outcome.next_cursor.clone(),
                    has_more: outcome.has_more,
                    download: None,
                    stored: None,
                };
                let previous = history.lock().unwrap().clone();
                media_changes::annotate(&mut entry, &previous);
                let archive = fetch_archives::merge(&entry, &previous);
                let media_count = archive.media.len();
                let status = format!("{status} {media_count} media in archive.");
                outcome.candidates = archive.media;
                {
                    let mut data = backend.lock().unwrap_or_else(|e| e.into_inner());
                    data.candidates = outcome.candidates.clone();
                    data.stored_view = None;
                    data.prepared_download = false;
                    data.fetch_json = Some(entry.json.clone());
                    data.json = archive.json;
                    data.profile = Some(profile.clone());
                    data.next_cursor = outcome.next_cursor.clone();
                    data.has_more = outcome.has_more;
                    data.tweet_count = archive.tweet_count;
                }
                let persisted = entry.clone();
                let saved = tokio::task::spawn_blocking(move || {
                    history_db::save_batch(vec![persisted]).map_err(|error| error.to_string())?;
                    history_db::load_summaries().map_err(|error| error.to_string())
                })
                .await;
                let mut persistence_error = None;
                let history_snapshot = match saved {
                    Ok(Ok(headers)) => {
                        *history.lock().unwrap() = headers.clone();
                        headers
                    }
                    result => {
                        persistence_error = Some(format!("Could not save history: {result:?}"));
                        let mut entries = history.lock().unwrap();
                        entries.retain(|item| item.id != history_id);
                        entries.insert(0, entry);
                        entries.clone()
                    }
                };
                let view = if persistence_error.is_none() {
                    crate::history::paged::StoredView::prepare(
                        &history_snapshot,
                        &history_id,
                        false,
                    )
                    .ok()
                    .flatten()
                } else {
                    None
                };
                if let Some(view) = &view {
                    let mut data = backend.lock().unwrap();
                    data.candidates.clear();
                    data.json = view.header.json.clone();
                    data.fetch_json = None;
                    data.stored_view = Some(view.clone());
                }
                let filter = history_filter
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                let has_more = outcome.has_more;
                let warning_toast = persistence_error.or_else(|| outcome.warning.clone());
                let ui_candidates = if publish_results && view.is_none() {
                    outcome.candidates.clone()
                } else {
                    Vec::new()
                };
                let ui_profile_json = profile_json.clone();
                let ui_for_completion = ui.clone();
                let _ = crate::ui::runtime::invoke_from_event_loop(move || {
                    if let Some(app) = ui_for_completion.upgrade() {
                        if publish_results {
                            downloader::apply_account_profile(
                                &app,
                                downloader::account_profile_from_json(&ui_profile_json),
                            );
                            app.set_media_items(if let Some(view) = view {
                                crate::ui::runtime::ModelRc::new(
                                    crate::history::paged::PagedModel::for_app(&app, view),
                                )
                            } else {
                                downloader::to_ui_model(&ui_candidates)
                            });
                            app.set_media_count(media_count as i32);
                            app.set_has_output(true);
                            app.set_has_more(has_more);
                        }
                        history_ui::refresh(&app, &history_snapshot, &filter.0, filter.1);
                        if media_count == 0 {
                            let ready_status = i18n::translate(
                                &app.global::<I18n>().get_language(),
                                "Ready for an X/Twitter URL.",
                            );
                            app.set_status(ready_status);
                        } else {
                            app.set_status(status);
                        }
                        app.set_progress(100.0);
                        app.set_is_extracting(false);
                        app.set_is_busy(false);
                        if let Some(complete) = completion {
                            complete(batch_fetch::Completion {
                                history_id,
                                media_count,
                                has_more,
                                error: warning_toast,
                                cancelled,
                            });
                        } else if let Some(warning) = warning_toast {
                            let lowered = warning.to_ascii_lowercase();
                            let kind = if lowered.contains("rate limit")
                                || lowered.contains("exited")
                                || lowered.contains("failed")
                                || lowered.contains("error")
                            {
                                "error"
                            } else {
                                "info"
                            };
                            show_toast(&app, warning, kind);
                        } else if media_count == 0 {
                            show_toast(&app, "No media found for this request.", "info");
                        }
                    }
                });
            }
            Err(error) => {
                let _ = crate::ui::runtime::invoke_from_event_loop(move || {
                    if let Some(app) = ui.upgrade() {
                        let toast_kind = if error == "Extraction cancelled." {
                            "info"
                        } else {
                            "error"
                        };
                        if completion.is_none() {
                            show_toast(&app, error.clone(), toast_kind);
                        }
                        let ready_status = i18n::translate(
                            &app.global::<I18n>().get_language(),
                            "Ready for an X/Twitter URL.",
                        );
                        app.set_status(ready_status);
                        app.set_is_extracting(false);
                        app.set_is_busy(false);
                        if let Some(complete) = completion {
                            session_log::error(format!("Background extraction failed: {error}"));
                            complete(batch_fetch::Completion {
                                history_id: String::new(),
                                media_count: 0,
                                has_more: false,
                                error: Some(error),
                                cancelled,
                            });
                        }
                    }
                });
            }
        }
    });
}
