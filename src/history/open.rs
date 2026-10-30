use crate::app::notifications::show_toast;
use crate::download::engine::{self as downloader, BackendState, HistoryEntry};
use crate::fetch::request as fetch_request;
use crate::{AppWindow, MediaSelection};
use std::sync::{Arc, Mutex};

pub fn register(
    app: &AppWindow,
    history: Arc<Mutex<Vec<HistoryEntry>>>,
    backend: Arc<Mutex<BackendState>>,
) {
    let weak = app.as_weak();
    app.on_open_history(move |id| {
        let Some(app) = weak.upgrade() else {
            return;
        };
        let from_batch = app.get_batch_result_open();
        if (app.get_is_busy() && !(from_batch && app.get_batch_running())) || app.get_history_busy()
        {
            return;
        }
        let entries = history.lock().unwrap().clone();
        let individual = app.get_selected_tab() == 1 && !app.get_fetch_sessions_key().is_empty();
        let origin = app.get_selected_tab();
        app.set_history_busy(true);
        let weak = weak.clone();
        let backend = backend.clone();
        tokio::task::spawn_blocking(move || {
            crate::history::db::flush();
            let result = (|| -> Result<_, String> {
                let stored = crate::history::paged::StoredView::prepare(&entries, &id, individual)
                    .map_err(|e| e.to_string())?;
                let entry = if let Some(view) = &stored {
                    Some(view.header.clone())
                } else {
                    crate::fetch::archives::try_open(&entries, &id, individual)?
                };
                let Some(entry) = entry.filter(|entry| entry.download.is_none()) else {
                    return Err("This fetch is no longer available.".into());
                };
                let rows = if stored.is_some() {
                    Vec::new()
                } else {
                    downloader::to_ui_rows(&entry.media)
                };
                let profile_json = crate::history::storage::account_json(&entry.json)
                    .map_err(|e| e.to_string())?;
                let fetch_json = if stored.is_some() {
                    None
                } else {
                    entries
                        .iter()
                        .find(|raw| raw.id == entry.id)
                        .map(|raw| raw.json.clone())
                };
                Ok((entry, stored, rows, profile_json, fetch_json))
            })();
            let _ = weak.upgrade_in_event_loop(move |app| {
                app.set_history_busy(false);
                if app.get_selected_tab() != origin {
                    return;
                }
                match result {
                    Ok((mut entry, stored, rows, profile_json, fetch_json)) => {
                        {
                            let mut state = backend.lock().unwrap();
                            state.candidates = std::mem::take(&mut entry.media);
                            state.json = entry.json.clone();
                            state.fetch_json = fetch_json;
                            state.profile = entry.profile.clone();
                            state.next_cursor = entry.next_cursor.clone();
                            state.has_more = entry.has_more;
                            state.tweet_count = entry.tweet_count;
                            state.history_id = entry.id.clone();
                            state.stored_view = stored.clone();
                        }
                        app.set_selected_tab(0);
                        app.set_selected_media_index(-1);
                        if !from_batch {
                            app.set_batch_mode(false);
                        }
                        let restored_profile = entry.profile.as_ref();
                        if !from_batch {
                            app.set_date_filter_enabled(
                                restored_profile.is_some_and(|profile| profile.date_filter_enabled),
                            );
                            app.set_fetch_start_date(
                                restored_profile
                                    .map(|profile| profile.start_date.clone())
                                    .unwrap_or_default(),
                            );
                            app.set_fetch_end_date(
                                restored_profile
                                    .map(|profile| profile.end_date.clone())
                                    .unwrap_or_default(),
                            );
                        }
                        let result_source = restored_profile
                            .filter(|profile| !profile.input_url.is_empty())
                            .map(|profile| profile.input_url.clone())
                            .unwrap_or(entry.source_url);
                        if from_batch {
                            app.set_result_source_url(result_source);
                        } else {
                            app.set_source_url(result_source);
                        }
                        app.global::<MediaSelection>().invoke_clear();
                        let count = stored
                            .as_ref()
                            .map_or(rows.len(), |view| view.rows.read().unwrap().len());
                        app.set_media_items(if let Some(view) = stored {
                            crate::ui::runtime::ModelRc::new(
                                crate::history::paged::PagedModel::for_app(&app, view),
                            )
                        } else {
                            downloader::model_from_rows(rows)
                        });
                        app.set_media_count(count.min(i32::MAX as usize) as i32);
                        app.set_has_output(!entry.json.is_empty());
                        app.set_has_more(entry.has_more);
                        app.set_account_profile(downloader::account_profile_from_json(
                            &profile_json,
                        ));
                        let message = format!("Loaded fetch from {}.", entry.fetched_at);
                        if !from_batch {
                            app.set_status(message.clone());
                            show_toast(&app, message, "info");
                            app.set_progress(100.0);
                        }
                        if !from_batch && let Some(profile) = entry.profile {
                            fetch_request::restore_filters(
                                &app,
                                &profile.media_type,
                                profile.no_videos,
                            );
                            app.set_media_type_index(match profile.media_type.as_str() {
                                "photo" => 1,
                                "video" => 2,
                                "animated_gif" => 3,
                                _ => 0,
                            });
                            app.set_retweets_index(match profile.retweets.as_str() {
                                "include" | "original" => 1,
                                _ => 0,
                            });
                            app.set_no_videos(false);
                            app.set_text_tweets(profile.text_tweets);
                        }
                    }
                    Err(error) => show_toast(
                        &app,
                        format!("Could not open saved fetch: {error}"),
                        "error",
                    ),
                }
            });
        });
    });
}

pub fn prepare_download(
    app: &AppWindow,
    backend: Arc<Mutex<BackendState>>,
    cancel: Arc<std::sync::atomic::AtomicBool>,
    kind: String,
    single: Option<i32>,
) {
    use std::sync::atomic::Ordering;
    let selected = if let Some(index) = single {
        std::collections::HashSet::from([index.max(0) as usize])
    } else {
        app.global::<MediaSelection>()
            .get_indices()
            .iter()
            .filter(|index| *index >= 0)
            .map(|index| index as usize)
            .collect()
    };
    let (view, candidates) = {
        let mut state = backend.lock().unwrap();
        (
            state.stored_view.clone(),
            std::mem::take(&mut state.candidates),
        )
    };
    cancel.store(false, Ordering::Relaxed);
    app.set_is_busy(true);
    app.set_is_extracting(false);
    app.set_progress(0.0);
    app.set_progress_detail(String::new());
    app.set_status(crate::i18n::translate(
        &app.global::<crate::I18n>().get_language(),
        "Preparing downloads…",
    ));
    let weak = app.as_weak();
    tokio::task::spawn_blocking(move || {
        let loaded = if let Some(view) = view {
            if let Some(index) = single {
                view.candidate(index)
                    .map(|item| vec![item])
                    .map_err(|error| error.to_string())
            } else {
                view.media(&kind, &selected)
                    .map_err(|error| error.to_string())
            }
        } else {
            Ok(candidates)
        };
        let loaded = loaded.map(|mut candidates| {
            for item in &mut candidates {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                crate::download::saved_files::reconcile(std::slice::from_mut(item));
            }
            candidates
        });
        let _ = weak.upgrade_in_event_loop(move |app| {
            app.set_is_busy(false);
            match loaded {
                Ok(candidates) => {
                    let mut state = backend.lock().unwrap();
                    state.candidates = candidates;
                    state.prepared_download = true;
                    drop(state);
                    if cancel.load(Ordering::Relaxed) {
                        backend.lock().unwrap().prepared_download = false;
                        show_toast(&app, "Download cancelled.", "info");
                        return;
                    }
                    if let Some(index) = single {
                        app.invoke_home_download_selected_media(index);
                    } else {
                        app.invoke_start_downloads(kind);
                    }
                }
                Err(error) => show_toast(
                    &app,
                    format!("Could not prepare downloads: {error}"),
                    "error",
                ),
            }
        });
    });
}

pub async fn settle_history(
    history: &Arc<Mutex<Vec<HistoryEntry>>>,
    fallback: Option<Vec<HistoryEntry>>,
) -> Option<Vec<HistoryEntry>> {
    let fallback = fallback?;
    let loaded = tokio::task::spawn_blocking(|| {
        crate::history::db::flush();
        crate::history::db::load_summaries()
    })
    .await;
    let entries = match loaded {
        Ok(Ok(entries)) => entries,
        result => {
            crate::platform::logging::error(format!(
                "Could not reload saved history summaries: {result:?}"
            ));
            fallback
        }
    };
    *history.lock().unwrap() = entries.clone();
    Some(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::download::engine::{ExtractionProfile, MediaCandidate};
    use crate::ui::runtime::{self, Model};
    use std::{
        cell::Cell,
        rc::Rc,
        time::{Duration, Instant},
    };
    async fn until(ready: impl Fn() -> bool) {
        let ctx = egui::Context::default();
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            runtime::pump(&ctx);
            if ready() {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "Stored history operation timed out"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
    #[tokio::test(flavor = "current_thread")]
    async fn opening_stored_history_and_downloading_one_row_do_not_materialize_the_full_entry() {
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let path = fixture.0.join("open.sqlite3");
        let connection = rusqlite::Connection::open(&path).unwrap();
        crate::history::db::initialize_database(&connection).unwrap();
        let raw = r#"{"account":{"username":"demo","name":"Demo"},"tweets":[],"opaque":"keep"}"#;
        let entry = HistoryEntry {
            id: "100".into(),
            username: "demo".into(),
            account_name: "Demo".into(),
            source_url: "https://x.com/demo".into(),
            json: raw.into(),
            next_cursor: "cursor-kept".into(),
            has_more: true,
            profile: Some(ExtractionProfile {
                url: "https://x.com/demo".into(),
                media_type: "all".into(),
                retweets: "skip".into(),
                ..Default::default()
            }),
            media: (0..640)
                .map(|index| MediaCandidate {
                    media_index: index * 3,
                    username: "demo".into(),
                    tweet_id: index.to_string(),
                    media_type: "photo".into(),
                    status: "Ready".into(),
                    url: format!("https://example.invalid/{index}.jpg"),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        crate::history::db::save_entry_on(&connection, &entry).unwrap();
        let headers = crate::history::db::read_summaries(&connection, Arc::new(path)).unwrap();
        let history = Arc::new(Mutex::new(headers));
        let backend = Arc::new(Mutex::new(BackendState::default()));
        let app = AppWindow::new().unwrap();
        crate::media::results::register(&app);
        app.set_selected_tab(1);
        register(&app, history, backend.clone());
        app.invoke_open_history("100".into());
        assert!(app.get_history_busy());
        until(|| !app.get_history_busy()).await;
        assert_eq!(app.get_selected_tab(), 0);
        assert_eq!(app.get_media_count(), 640);
        assert!(app.get_has_more());
        assert_eq!(backend.lock().unwrap().next_cursor, "cursor-kept");
        assert!(backend.lock().unwrap().candidates.is_empty());
        assert!(backend.lock().unwrap().json.len() < 1000);
        let items = app.get_media_items();
        let model = items
            .as_any()
            .downcast_ref::<crate::history::paged::PagedModel>()
            .unwrap();
        assert_eq!(
            app.global::<crate::MediaResults>()
                .invoke_count(items.clone(), "photo".into()),
            640
        );
        assert_eq!(
            model
                .decoded_pages
                .load(std::sync::atomic::Ordering::Relaxed),
            0
        );
        items.row_data(42).unwrap();
        until(|| !items.row_data(42).unwrap().payload_pending).await;
        assert_eq!(
            items.row_data(42).unwrap().url,
            "https://example.invalid/42.jpg"
        );
        assert_eq!(
            model
                .decoded_pages
                .load(std::sync::atomic::Ordering::Relaxed),
            1
        );
        assert_eq!(
            backend
                .lock()
                .unwrap()
                .stored_view
                .as_ref()
                .unwrap()
                .response(true)
                .unwrap(),
            raw
        );
        let calls = Rc::new(Cell::new(0));
        let called = calls.clone();
        let data = backend.clone();
        app.on_download_selected_media(move |index| {
            assert_eq!(index, 42);
            let data = data.lock().unwrap();
            assert_eq!(data.candidates.len(), 1);
            assert_eq!(data.candidates[0].media_index, 42);
            assert_eq!(data.candidates[0].url, "https://example.invalid/42.jpg");
            called.set(called.get() + 1);
        });
        app.on_library_download(|_| panic!("A Home download was redirected to Library"));
        prepare_download(
            &app,
            backend,
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
            "selected".into(),
            Some(42),
        );
        app.set_selected_tab(crate::library::TAB);
        until(|| calls.get() == 1).await;
    }
}
