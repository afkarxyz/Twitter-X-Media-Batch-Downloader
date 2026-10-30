use crate::AppWindow;
use crate::BatchFetchItem;
use crate::download::engine as downloader;
use crate::fetch::request::{self as fetch_request, DateRange};
use crate::ui::runtime::Model;
use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone)]
struct Entry {
    source: String,
    status: &'static str,
    history_id: String,
    media_count: usize,
    error: String,
    avatar_url: String,
    avatar_square: bool,
}

fn avatar(record: &downloader::HistoryEntry) -> (String, bool) {
    let json = crate::history::storage::account_json(&record.json).unwrap_or_default();
    let profile = downloader::account_profile_from_json(&json);
    (profile.avatar_url, profile.avatar_square)
}

#[derive(Default)]
struct State {
    entries: Vec<Entry>,
    profiles: Vec<downloader::ExtractionProfile>,
    pending: VecDeque<usize>,
    active: bool,
    enabled: bool,
    from_history: bool,
}

pub struct Completion {
    pub history_id: String,
    pub media_count: usize,
    pub has_more: bool,
    pub error: Option<String>,
    pub cancelled: bool,
}
pub type CompletionFn = Box<dyn FnOnce(Completion) + Send>;
pub(crate) type Runner = fn(
    crate::ui::runtime::Weak<AppWindow>,
    downloader::ExtractionOptions,
    downloader::ExtractionProfile,
    Arc<Mutex<downloader::BackendState>>,
    Arc<Mutex<Vec<downloader::HistoryEntry>>>,
    Arc<Mutex<(String, bool)>>,
    Arc<AtomicBool>,
    Option<CompletionFn>,
);

#[derive(Clone)]
struct Context {
    ui: crate::ui::runtime::Weak<AppWindow>,
    state: Arc<Mutex<State>>,
    backend: Arc<Mutex<downloader::BackendState>>,
    history: Arc<Mutex<Vec<downloader::HistoryEntry>>>,
    filter: Arc<Mutex<(String, bool)>>,
    cancel: Arc<AtomicBool>,
    runner: Runner,
    store: Option<crate::fetch::queue::Sender>,
}

#[derive(Default)]
struct SingleWorkspace {
    backend: downloader::BackendState,
    source: String,
    source_kind: i32,
    dates: bool,
    start: String,
    end: String,
    media: crate::ui::runtime::ModelRc<crate::MediaItem>,
    profile: crate::AccountProfile,
    selected: crate::ui::runtime::ModelRc<i32>,
    has_output: bool,
    has_more: bool,
    hidden: bool,
    status: String,
    options_error: String,
}
impl SingleWorkspace {
    fn capture(app: &AppWindow, backend: &Arc<Mutex<downloader::BackendState>>) -> Self {
        Self {
            backend: backend.lock().unwrap().clone(),
            source: app.get_source_url(),
            source_kind: app.get_fetch_source_index(),
            dates: app.get_date_filter_enabled(),
            start: app.get_fetch_start_date(),
            end: app.get_fetch_end_date(),
            media: app.get_media_items(),
            profile: app.get_account_profile(),
            selected: app.global::<crate::MediaSelection>().get_indices(),
            has_output: app.get_has_output(),
            has_more: app.get_has_more(),
            hidden: app.get_home_results_hidden(),
            status: app.get_status(),
            options_error: app.get_fetch_options_error(),
        }
    }
    fn restore(self, app: &AppWindow, backend: &Arc<Mutex<downloader::BackendState>>) {
        *backend.lock().unwrap() = self.backend;
        app.set_result_source_url(self.source);
        app.set_fetch_source_index(self.source_kind);
        app.set_date_filter_enabled(self.dates);
        app.set_fetch_start_date(self.start);
        app.set_fetch_end_date(self.end);
        app.set_media_count(self.media.row_count().min(i32::MAX as usize) as i32);
        app.set_media_items(self.media);
        app.set_account_profile(self.profile);
        app.set_has_output(self.has_output);
        app.set_has_more(self.has_more);
        app.set_home_results_hidden(self.hidden);
        app.set_status(self.status);
        app.set_fetch_options_error(self.options_error);
        app.global::<crate::MediaSelection>()
            .set_indices(self.selected);
        crate::media::results::refresh_selection_state(app);
    }
}

fn checkpoint(context: &Context, state: &State) {
    if let Some(store) = &context.store {
        let snapshot = crate::fetch::queue::Snapshot {
            version: 1,
            enabled: state.enabled,
            from_history: state.from_history,
            entries: state
                .entries
                .iter()
                .map(|entry| crate::fetch::queue::Row {
                    source: entry.source.clone(),
                    status: entry.status.into(),
                    history_id: entry.history_id.clone(),
                    media_count: entry.media_count,
                })
                .collect(),
            profiles: state.profiles.clone(),
        };
        let _ = store.send(Some(snapshot));
    }
}

fn restore(
    app: &AppWindow,
    saved: crate::fetch::queue::Snapshot,
    history: &[downloader::HistoryEntry],
) -> State {
    let mut state = State {
        enabled: saved.enabled,
        from_history: saved.from_history,
        profiles: saved.profiles,
        ..Default::default()
    };
    state.entries = saved
        .entries
        .into_iter()
        .enumerate()
        .map(|(index, row)| {
            let record = history.iter().find(|record| {
                record.id == row.history_id
                    && record.download.is_none()
                    && record.profile.as_ref().is_some_and(|profile| {
                        crate::app::fetch::same_extraction_filters(profile, &state.profiles[index])
                    })
            });
            let status = match row.status.as_str() {
                "Completed" if app.get_history_busy() && record.is_none() => "Completed",
                "Completed" if record.is_some_and(|record| !record.has_more) => "Completed",
                "Completed" if record.is_some() => "Incomplete",
                "Fetching" => "Cancelled",
                "Incomplete" => "Incomplete",
                "Failed" => "Failed",
                "Cancelled" => "Cancelled",
                _ => "Queued",
            };
            let (avatar_url, avatar_square) = record.map(avatar).unwrap_or_default();
            Entry {
                source: row.source,
                status,
                history_id: record.map(|record| record.id.clone()).unwrap_or_else(|| {
                    if app.get_history_busy() {
                        row.history_id
                    } else {
                        String::new()
                    }
                }),
                media_count: record.map_or_else(
                    || {
                        if app.get_history_busy() {
                            row.media_count
                        } else {
                            0
                        }
                    },
                    |record| {
                        let key = crate::fetch::sessions::key(record);
                        let records = history
                            .iter()
                            .filter(|entry| {
                                entry.download.is_none()
                                    && crate::fetch::sessions::key(entry) == key
                            })
                            .collect::<Vec<_>>();
                        crate::fetch::archives::media_count(&records)
                    },
                ),
                error: String::new(),
                avatar_url,
                avatar_square,
            }
        })
        .collect();
    app.set_batch_sources_text(
        state
            .entries
            .iter()
            .map(|entry| entry.source.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
    );
    app.set_batch_history_profiles(state.from_history);
    app.set_batch_mode(state.enabled);
    if app.get_batch_mode()
        && let Some(profile) = state.profiles.first()
    {
        app.set_source_url(state.entries[0].source.clone());
        app.set_date_filter_enabled(!state.from_history && profile.date_filter_enabled);
        app.set_fetch_start_date(profile.start_date.clone());
        app.set_fetch_end_date(profile.end_date.clone());
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
        app.set_no_videos(profile.no_videos);
        app.set_text_tweets(profile.text_tweets);
    }
    refresh(app, &state);
    state
}

fn refresh(app: &AppWindow, state: &State) {
    let rows = state
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let kind = crate::media::images::avatar_kind(entry.avatar_square);
            let avatar_image = crate::media::images::image(kind, &entry.avatar_url);
            BatchFetchItem {
                index: index as i32,
                source: entry.source.clone(),
                status: entry.status.into(),
                history_id: entry.history_id.clone(),
                media_count: entry.media_count.min(i32::MAX as usize) as i32,
                error: entry.error.chars().take(160).collect::<String>(),
                has_avatar: avatar_image.size().width > 0,
                avatar_image,
                avatar_url: entry.avatar_url.clone(),
                avatar_square: entry.avatar_square,
            }
        })
        .collect::<Vec<_>>();
    for row in rows.iter().filter(|row| !row.has_avatar) {
        crate::media::images::request(
            app,
            crate::media::images::avatar_kind(row.avatar_square),
            row.avatar_url.clone(),
        );
    }
    app.set_batch_fetch_items(crate::ui::runtime::ModelRc::new(
        crate::ui::runtime::VecModel::from(rows),
    ));
    app.set_batch_running(state.active);
    app.set_batch_pending_count(
        state
            .entries
            .iter()
            .filter(|entry| entry.status != "Completed")
            .count() as i32,
    );
}

fn halt(context: &Context, app: &AppWindow, stopped: bool) {
    let mut state = context.state.lock().unwrap();
    state.active = false;
    state.pending.clear();
    refresh(app, &state);
    checkpoint(context, &state);
    app.set_is_busy(false);
    app.set_is_extracting(false);
    let completed = state
        .entries
        .iter()
        .filter(|entry| entry.status == "Completed")
        .count();
    let failed = state
        .entries
        .iter()
        .filter(|entry| entry.status == "Failed")
        .count();
    let incomplete = state
        .entries
        .iter()
        .filter(|entry| matches!(entry.status, "Incomplete" | "Cancelled" | "Queued"))
        .count();
    let message = if stopped {
        format!("Batch stopped · {completed} completed · {failed} failed · {incomplete} pending.")
    } else {
        format!(
            "Batch finished · {completed} completed · {failed} failed · {incomplete} incomplete."
        )
    };
    let error = state
        .entries
        .iter()
        .find(|entry| entry.status != "Cancelled" && !entry.error.is_empty());
    let message = error
        .map(|entry| format!("{message}\n{}: {}", entry.source, entry.error))
        .unwrap_or(message);
    app.set_status(message.clone());
    crate::app::notifications::show_toast(
        app,
        message,
        if error.is_some() { "error" } else { "info" },
    );
}

fn step(context: Context) {
    let Some(app) = context.ui.upgrade() else {
        return;
    };
    if context.cancel.load(Ordering::Relaxed) {
        halt(&context, &app, true);
        return;
    }
    let next = {
        let mut state = context.state.lock().unwrap();
        if !state.active {
            return;
        }
        state.pending.pop_front().map(|index| {
            state.entries[index].status = "Fetching";
            state.entries[index].error.clear();
            refresh(&app, &state);
            checkpoint(&context, &state);
            (
                index,
                state.entries[index].clone(),
                state.profiles[index].clone(),
                // The active token now: a rate-limited account may have switched it.
                app.get_saved_auth_token().trim().to_owned(),
            )
        })
    };
    let Some((index, entry, profile, auth_token)) = next else {
        halt(&context, &app, false);
        return;
    };
    let previous = context
        .history
        .lock()
        .unwrap()
        .iter()
        .find(|record| {
            record.id == entry.history_id
                && record.has_more
                && record.profile.as_ref().is_some_and(|previous| {
                    crate::app::fetch::same_extraction_filters(&profile, previous)
                })
        })
        .cloned();
    if previous
        .as_ref()
        .is_some_and(|entry| entry.stored.is_some())
    {
        app.set_is_busy(true);
        app.set_is_extracting(true);
        app.set_status("Preparing xtractor…".into());
        let ui = context.ui.clone();
        tokio::task::spawn_blocking(move || {
            let previous = crate::history::storage::resolve(previous.as_ref().unwrap());
            let _ = ui.upgrade_in_event_loop(move |app| {
                if context.cancel.load(Ordering::Relaxed) {
                    context.state.lock().unwrap().entries[index].status = "Cancelled";
                    halt(&context, &app, true);
                    return;
                }
                match previous {
                    Ok(previous) => run_item(context, index, profile, auth_token, Some(previous)),
                    Err(error) => {
                        let mut state = context.state.lock().unwrap();
                        state.entries[index].status = "Failed";
                        state.entries[index].error = error.to_string();
                        drop(state);
                        halt(&context, &app, true);
                    }
                }
            });
        });
        return;
    }
    run_item(context, index, profile, auth_token, previous);
}
fn run_item(
    context: Context,
    index: usize,
    mut profile: downloader::ExtractionProfile,
    auth_token: String,
    previous: Option<downloader::HistoryEntry>,
) {
    let Some(app) = context.ui.upgrade() else {
        return;
    };
    let work_backend = Arc::new(Mutex::new(downloader::BackendState::default()));
    let (cursor, previous_json) = if let Some(record) = previous {
        // A cursor belongs to the endpoint that originally produced it.
        profile.media_timeline = record
            .profile
            .as_ref()
            .is_some_and(|profile| profile.media_timeline);
        profile.media_reposts = record
            .profile
            .as_ref()
            .is_some_and(|profile| profile.media_reposts);
        *work_backend.lock().unwrap() = downloader::BackendState {
            candidates: record.media,
            fetch_json: Some(record.json.clone()),
            json: record.json.clone(),
            profile: record.profile,
            next_cursor: record.next_cursor.clone(),
            has_more: record.has_more,
            tweet_count: record.tweet_count,
            history_id: record.id,
            stored_view: None,
            prepared_download: false,
        };
        (record.next_cursor, Some(record.json))
    } else {
        profile.media_reposts =
            crate::fetch::strategy::combined(&profile.url, &profile.retweets, profile.text_tweets);
        profile.media_timeline = crate::fetch::request::uses_media_timeline(
            &profile.url,
            &profile.retweets,
            profile.text_tweets,
        );
        (String::new(), None)
    };
    let options = downloader::ExtractionOptions {
        url: profile.url.clone(),
        media_timeline: profile.media_timeline,
        media_reposts: profile.media_reposts,
        auth_token,
        max_posts: usize::MAX,
        page_size: downloader::POSTS_PER_REQUEST,
        media_type: profile.media_type.clone(),
        retweets: profile.retweets.clone(),
        no_videos: profile.no_videos,
        text_tweets: profile.text_tweets,
        cursor,
        previous_json,
        refresh_boundary: Default::default(),
    };
    app.set_is_busy(true);
    app.set_is_extracting(true);
    app.set_status("Preparing xtractor…".into());
    app.set_progress(0.0);
    app.set_progress_detail("".into());
    let next_context = context.clone();
    (context.runner)(
        context.ui.clone(),
        options,
        profile,
        work_backend,
        context.history.clone(),
        context.filter.clone(),
        context.cancel.clone(),
        Some(Box::new(move |result| {
            let rate_limited = result.error.as_ref().is_some_and(|error| {
                let error = error.to_ascii_lowercase();
                error.contains("rate limit") || error.contains("429 too many")
            });
            let fetched_avatar = next_context
                .history
                .lock()
                .unwrap()
                .iter()
                .find(|record| record.id == result.history_id)
                .map(avatar)
                .filter(|(url, _)| !url.is_empty());
            {
                let mut state = next_context.state.lock().unwrap();
                let entry = &mut state.entries[index];
                let have_result = !result.history_id.is_empty();
                if !result.history_id.is_empty() {
                    entry.history_id = result.history_id;
                    entry.media_count = result.media_count;
                }
                if let Some((url, square)) = fetched_avatar {
                    entry.avatar_url = url;
                    entry.avatar_square = square;
                }
                entry.status = if result.cancelled {
                    "Cancelled"
                } else if result.error.is_some() && !have_result {
                    "Failed"
                } else if result.has_more || result.error.is_some() {
                    "Incomplete"
                } else {
                    "Completed"
                };
                entry.error = result.error.unwrap_or_default();
                if let Some(app) = next_context.ui.upgrade() {
                    refresh(&app, &state);
                    checkpoint(&next_context, &state);
                }
            }
            if rate_limited {
                if let Some(app) = next_context.ui.upgrade() {
                    halt(&next_context, &app, true);
                }
            } else {
                step(next_context);
            }
        })),
    );
}

pub fn register(
    app: &AppWindow,
    backend: Arc<Mutex<downloader::BackendState>>,
    history: Arc<Mutex<Vec<downloader::HistoryEntry>>>,
    filter: Arc<Mutex<(String, bool)>>,
    cancel: Arc<AtomicBool>,
) -> Option<crate::fetch::queue::Guard> {
    let path = downloader::app_data_dir().map(|directory| directory.join("batch-queue.json"));
    let saved = path
        .as_ref()
        .and_then(|path| match crate::fetch::queue::load(path) {
            Ok(saved) => saved,
            Err(error) => {
                crate::platform::logging::warn(format!("Could not restore batch queue: {error}"));
                crate::app::notifications::show_toast(
                    app,
                    "The saved batch queue could not be restored. Its file has been preserved.",
                    "error",
                );
                None
            }
        });
    let writer =
        path.and_then(
            |path| match crate::fetch::queue::writer(path, Some(app.as_weak())) {
                Ok(writer) => Some(writer),
                Err(error) => {
                    crate::app::notifications::show_toast(
                        app,
                        format!("Could not start batch queue storage: {error}"),
                        "error",
                    );
                    None
                }
            },
        );
    let (sender, guard) = match writer {
        Some((sender, guard)) => (Some(sender), Some(guard)),
        None => (None, None),
    };
    let initial = saved
        .map(|saved| restore(app, saved, &history.lock().unwrap()))
        .unwrap_or_default();
    register_context(
        app,
        backend,
        history,
        filter,
        cancel,
        crate::app::fetch::spawn_extraction_job,
        initial,
        sender,
    );
    guard
}

fn register_context(
    app: &AppWindow,
    backend: Arc<Mutex<downloader::BackendState>>,
    history: Arc<Mutex<Vec<downloader::HistoryEntry>>>,
    filter: Arc<Mutex<(String, bool)>>,
    cancel: Arc<AtomicBool>,
    runner: Runner,
    initial: State,
    store: Option<crate::fetch::queue::Sender>,
) {
    let context = Context {
        ui: app.as_weak(),
        state: Arc::new(Mutex::new(initial)),
        backend,
        history,
        filter,
        cancel,
        runner,
        store,
    };
    let single_workspace = Rc::new(RefCell::new(None::<SingleWorkspace>));
    let restored = context.clone();
    let restored_single = single_workspace.clone();
    app.on_history_loaded(move || {
        let Some(app) = restored.ui.upgrade() else {
            return;
        };
        let snapshot = {
            let state = restored.state.lock().unwrap();
            if state.active {
                return;
            }
            crate::fetch::queue::Snapshot {
                version: 1,
                enabled: state.enabled,
                from_history: state.from_history,
                profiles: state.profiles.clone(),
                entries: state
                    .entries
                    .iter()
                    .map(|entry| crate::fetch::queue::Row {
                        source: entry.source.clone(),
                        status: entry.status.into(),
                        history_id: entry.history_id.clone(),
                        media_count: entry.media_count,
                    })
                    .collect(),
            }
        };
        *restored_single.borrow_mut() = None;
        app.set_batch_result_open(false);
        app.set_batch_result_index(-1);
        let next = restore(&app, snapshot, &restored.history.lock().unwrap());
        *restored.state.lock().unwrap() = next;
    });
    let owner = context.clone();
    let saved_single = single_workspace.clone();
    app.on_switch_home_mode(move |batch| {
        let Some(app) = owner.ui.upgrade() else {
            return;
        };
        if app.get_is_busy() {
            return;
        }
        if batch && !app.get_batch_mode() {
            *saved_single.borrow_mut() = Some(SingleWorkspace::capture(&app, &owner.backend));
            app.set_fetch_source_index(0);
            if let Some(profile) = owner.state.lock().unwrap().profiles.first() {
                app.set_date_filter_enabled(profile.date_filter_enabled);
                app.set_fetch_start_date(profile.start_date.clone());
                app.set_fetch_end_date(profile.end_date.clone());
                app.set_fetch_options_error(String::new());
            }
        } else if !batch && app.get_batch_mode() {
            saved_single
                .borrow_mut()
                .take()
                .unwrap_or_default()
                .restore(&app, &owner.backend);
        }
        app.set_batch_mode(batch);
        app.set_batch_result_open(false);
        app.set_batch_result_index(-1);
        app.set_selected_media_index(-1);
        let mut state = owner.state.lock().unwrap();
        state.enabled = batch;
        app.set_batch_history_profiles(state.from_history);
        checkpoint(&owner, &state);
    });
    let owner = context.clone();
    app.on_apply_fetch_options(move |batch, dates, start, end, text| {
        let Some(app) = owner.ui.upgrade() else {
            return false;
        };
        if app.get_is_busy() {
            return false;
        }
        let range = DateRange {
            enabled: dates,
            start: start.trim().into(),
            end: end.trim().into(),
        };
        // Keep partially typed options as well. Validation protects the fetch action,
        // while the dialog can stay open and save each edit without an Apply button.
        app.set_batch_mode(batch);
        app.set_date_filter_enabled(dates);
        app.set_fetch_start_date(range.start.clone());
        app.set_fetch_end_date(range.end.clone());
        app.set_batch_sources_text(text.clone());
        if batch {
            app.set_batch_history_profiles(false);
        }
        let result = (|| {
            range.validate()?;
            if batch {
                let sources = fetch_request::batch_sources(text.as_str())?;
                let profiles = sources
                    .iter()
                    .map(|source| fetch_request::profile_from_app(&app, source, &range))
                    .collect::<Result<Vec<_>, _>>()?;
                let mut state = owner.state.lock().unwrap();
                let changed = profiles != state.profiles;
                if changed {
                    state.entries = sources
                        .into_iter()
                        .map(|source| Entry {
                            source,
                            status: "Queued",
                            history_id: String::new(),
                            media_count: 0,
                            error: String::new(),
                            avatar_url: String::new(),
                            avatar_square: false,
                        })
                        .collect();
                    state.profiles = profiles;
                }
                state.enabled = batch;
                state.pending.clear();
                state.active = false;
                refresh(&app, &state);
                drop(state);
            }
            Ok::<_, String>(())
        })();
        if let Err(error) = result {
            let mut state = owner.state.lock().unwrap();
            state.enabled = batch;
            refresh(&app, &state);
            checkpoint(&owner, &state);
            app.set_fetch_options_error(error);
            return false;
        }
        app.set_batch_mode(batch);
        app.set_date_filter_enabled(dates);
        app.set_fetch_start_date(range.start);
        app.set_fetch_end_date(range.end);
        app.set_batch_sources_text(text);
        app.set_fetch_options_error("".into());
        let mut state = owner.state.lock().unwrap();
        state.enabled = batch;
        if batch {
            state.from_history = false;
            app.set_batch_history_profiles(false);
        }
        checkpoint(&owner, &state);
        true
    });
    let owner = context.clone();
    app.on_start_batch(move || {
        let Some(app) = owner.ui.upgrade() else {
            return;
        };
        if app.get_is_busy()
            || app.get_auth_check_busy()
            || app.get_xtractor_busy()
            || app.get_tool_update_busy()
        {
            return;
        }
        if app.get_saved_auth_token().trim().is_empty() {
            crate::app::notifications::show_toast(
                &app,
                "An X auth_token is required before extraction.",
                "error",
            );
            return;
        }
        if !app.get_fetch_options_error().is_empty() {
            crate::app::notifications::show_toast(&app, app.get_fetch_options_error(), "error");
            return;
        }
        let mut state = owner.state.lock().unwrap();
        if state.active || state.entries.is_empty() {
            return;
        }
        state.pending = state
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.status != "Completed")
            .map(|(index, _)| index)
            .collect();
        if state.pending.is_empty() {
            return;
        }
        state.active = true;
        owner.cancel.store(false, Ordering::Relaxed);
        refresh(&app, &state);
        drop(state);
        step(owner.clone());
    });
    let owner = context.clone();
    app.on_clear_batch(move || {
        let Some(app) = owner.ui.upgrade() else {
            return;
        };
        if app.get_is_busy() {
            return;
        }
        *owner.state.lock().unwrap() = State {
            enabled: true,
            ..Default::default()
        };
        refresh(&app, &owner.state.lock().unwrap());
        app.set_batch_sources_text("".into());
        app.set_batch_mode(true);
        app.set_batch_result_open(false);
        app.set_batch_result_index(-1);
        app.set_batch_history_profiles(false);
        checkpoint(&owner, &owner.state.lock().unwrap());
    });
    let owner = context.clone();
    app.on_reset_batch(move || {
        let Some(app) = owner.ui.upgrade() else {
            return;
        };
        if app.get_is_busy() {
            return;
        }
        let mut state = owner.state.lock().unwrap();
        for entry in &mut state.entries {
            entry.status = "Queued";
            entry.history_id.clear();
            entry.media_count = 0;
            entry.error.clear();
        }
        state.pending.clear();
        state.active = false;
        refresh(&app, &state);
        checkpoint(&owner, &state);
    });
    let owner = context;
    app.on_open_batch_result(move |index| {
        let Some(app) = owner.ui.upgrade() else {
            return;
        };
        if app.get_history_busy() || (app.get_is_busy() && !app.get_batch_running()) || index < 0 {
            return;
        }
        let id = owner
            .state
            .lock()
            .unwrap()
            .entries
            .get(index as usize)
            .map(|entry| (entry.history_id.clone(), entry.media_count));
        if let Some((id, count)) = id.filter(|(id, _)| !id.is_empty()) {
            if !owner
                .history
                .lock()
                .unwrap()
                .iter()
                .any(|record| record.id == id && record.download.is_none())
            {
                crate::app::notifications::show_toast(
                    &app,
                    "Saved batch result is no longer available. Reset the queue to fetch it again.",
                    "error",
                );
                return;
            }
            let cached = {
                let shown = owner.backend.lock().unwrap();
                shown.history_id == id
                    && shown.candidates.len() == count
                    && app.get_batch_result_index() == index
            };
            if cached {
                app.set_batch_result_open(true);
            } else {
                app.open_batch_history_result(index, id);
            }
        }
    });
}

#[cfg(test)]
mod tests;
