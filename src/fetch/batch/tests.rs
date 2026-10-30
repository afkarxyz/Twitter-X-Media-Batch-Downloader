use super::*;
use crate::ui::runtime::{ModelRc, VecModel};

struct Job {
    options: downloader::ExtractionOptions,
    ui: crate::ui::runtime::Weak<AppWindow>,
    profile: downloader::ExtractionProfile,
    backend: Arc<Mutex<downloader::BackendState>>,
    history: Arc<Mutex<Vec<downloader::HistoryEntry>>>,
    complete: CompletionFn,
}
thread_local! { static JOBS: RefCell<VecDeque<Job>> = const { RefCell::new(VecDeque::new()) }; }

fn runner(
    ui: crate::ui::runtime::Weak<AppWindow>,
    options: downloader::ExtractionOptions,
    profile: downloader::ExtractionProfile,
    backend: Arc<Mutex<downloader::BackendState>>,
    history: Arc<Mutex<Vec<downloader::HistoryEntry>>>,
    _filter: Arc<Mutex<(String, bool)>>,
    _cancel: Arc<AtomicBool>,
    complete: Option<CompletionFn>,
) {
    JOBS.with_borrow_mut(|jobs| {
        jobs.push_back(Job {
            options,
            ui,
            profile,
            backend,
            history,
            complete: complete.unwrap(),
        })
    });
}

fn record(id: &str, profile: downloader::ExtractionProfile) -> downloader::HistoryEntry {
    downloader::HistoryEntry {
        id: id.into(),
        fetched_at: "fixture".into(),
        source_url: profile.url.clone(),
        account_name: id.into(),
        username: id.into(),
        tweet_count: 2,
        media: (0..2)
            .map(|index| downloader::MediaCandidate {
                media_index: index,
                media_type: "photo".into(),
                username: id.into(),
                url: format!("https://example.invalid/{id}/{index}.jpg"),
                ..Default::default()
            })
            .collect(),
        json: serde_json::json!({"account":{"username":id},"tweets":[]}).to_string(),
        profile: Some(profile),
        next_cursor: String::new(),
        has_more: false,
        download: None,
        stored: None,
    }
}

fn finish_next(id: &str, shown: &Arc<Mutex<downloader::BackendState>>) {
    let job = JOBS.with_borrow_mut(|jobs| jobs.pop_front().unwrap());
    assert!(
        !Arc::ptr_eq(shown, &job.backend),
        "Batch work must not use the displayed backend"
    );
    let record = record(id, job.profile);
    job.backend.lock().unwrap().history_id = id.into();
    job.backend.lock().unwrap().candidates = record.media.clone();
    job.history.lock().unwrap().push(record);
    let app = job.ui.upgrade().unwrap();
    app.set_is_busy(false);
    app.set_is_extracting(false);
    (job.complete)(Completion {
        history_id: id.into(),
        media_count: 2,
        has_more: false,
        error: None,
        cancelled: false,
    });
}

fn fixture() -> (AppWindow, Arc<Mutex<downloader::BackendState>>) {
    JOBS.with_borrow_mut(VecDeque::clear);
    let app = AppWindow::new().unwrap();
    app.set_saved_auth_token("a".repeat(40));
    app.set_source_url("https://x.com/single_account".into());
    app.set_media_items(ModelRc::new(VecModel::from(vec![crate::MediaItem {
        media_index: 30,
        title: "Single result".into(),
        media_type: "photo".into(),
        ..Default::default()
    }])));
    app.set_has_output(true);
    crate::media::results::register(&app);
    app.global::<crate::MediaSelection>().invoke_toggle(30);
    let backend = Arc::new(Mutex::new(downloader::BackendState {
        history_id: "single-result".into(),
        ..Default::default()
    }));
    let history = Arc::new(Mutex::new(Vec::<downloader::HistoryEntry>::new()));
    let filter = Arc::new(Mutex::new((String::new(), false)));
    let cancel = Arc::new(AtomicBool::new(false));
    let weak = app.as_weak();
    let shown = backend.clone();
    let records = history.clone();
    app.on_open_history(move |id| {
        let app = weak.upgrade().unwrap();
        let record = records
            .lock()
            .unwrap()
            .iter()
            .find(|record| record.id == id)
            .cloned()
            .unwrap();
        shown.lock().unwrap().history_id = id;
        shown.lock().unwrap().candidates = record.media.clone();
        app.set_source_url(record.source_url);
        app.set_media_items(downloader::to_ui_model(&record.media));
        app.set_media_count(2);
        app.set_has_output(true);
        app.global::<crate::MediaSelection>().invoke_clear();
    });
    register_context(
        &app,
        backend.clone(),
        history,
        filter,
        cancel,
        runner,
        State::default(),
        None,
    );
    crate::fetch::sources::register(&app);
    (app, backend)
}

#[test]
fn resetting_batch_fetch_options_keeps_accounts_and_resets_their_date_profiles() {
    let (app, _) = fixture();
    app.invoke_switch_home_mode(true);
    let sources = "first_account\nsecond_account";
    assert!(app.invoke_apply_fetch_options(
        true,
        true,
        "2026-01-01".into(),
        "2026-02-01".into(),
        sources.into()
    ));
    crate::app::preferences::register_with_writer(&app, |_| true);
    assert!(app.invoke_reset_settings(5));
    assert!(app.get_batch_mode());
    assert!(!app.get_date_filter_enabled());
    assert!(app.get_fetch_start_date().is_empty());
    assert!(app.get_fetch_end_date().is_empty());
    assert!(app.get_fetch_options_error().is_empty());
    assert_eq!(app.get_batch_sources_text(), sources);
    assert_eq!(app.get_batch_fetch_items().row_count(), 2);
    app.invoke_start_batch();
    JOBS.with_borrow(|jobs| {
        let job = jobs.front().unwrap();
        assert_eq!(job.profile.url, "https://x.com/first_account");
        assert!(job.profile.start_date.is_empty());
        assert!(job.profile.end_date.is_empty());
    });
}

#[test]
fn background_batch_keeps_each_view_stable_and_restores_the_single_workspace() {
    let (app, backend) = fixture();
    let single = app.get_media_items();
    app.invoke_switch_home_mode(true);
    assert!(app.invoke_apply_fetch_options(
        true,
        false,
        "".into(),
        "".into(),
        "first_account\nsecond_account".into()
    ));
    app.invoke_start_batch();
    assert!(app.get_batch_running());
    assert!(single.ptr_eq(&app.get_media_items()));
    assert_eq!(backend.lock().unwrap().history_id, "single-result");
    finish_next("first-result", &backend);
    assert!(app.get_batch_running());
    app.set_history_busy(true);
    app.invoke_open_batch_result(0);
    assert!(!app.get_batch_result_open());
    app.set_history_busy(false);
    app.invoke_open_batch_result(0);
    assert!(app.get_batch_result_open());
    assert!(app.get_batch_mode());
    let first = app.get_media_items();
    assert_eq!(backend.lock().unwrap().history_id, "first-result");
    finish_next("second-result", &backend);
    assert!(!app.get_batch_running());
    assert!(
        first.ptr_eq(&app.get_media_items()),
        "Finishing another account replaced the viewed result"
    );
    assert_eq!(backend.lock().unwrap().history_id, "first-result");
    app.set_batch_result_open(false);
    app.invoke_open_batch_result(0);
    assert!(
        first.ptr_eq(&app.get_media_items()),
        "Reopening the same result discarded its view"
    );
    app.set_batch_result_open(false);
    assert_eq!(app.get_batch_fetch_items().row_count(), 2);
    app.invoke_open_batch_result(1);
    assert_eq!(backend.lock().unwrap().history_id, "second-result");
    app.invoke_switch_home_mode(false);
    assert!(single.ptr_eq(&app.get_media_items()));
    assert_eq!(backend.lock().unwrap().history_id, "single-result");
    assert_eq!(app.get_source_url(), "https://x.com/single_account");
    assert_eq!(
        app.global::<crate::MediaSelection>()
            .get_indices()
            .row_data(0),
        Some(30)
    );
    assert_eq!(app.get_batch_fetch_items().row_count(), 2);
    app.invoke_switch_home_mode(true);
    assert!(
        app.get_batch_fetch_items()
            .iter()
            .all(|row| row.status == "Completed")
    );
}

#[test]
fn invalid_batch_edits_keep_the_existing_queue_and_clear_stays_in_batch_mode() {
    let (app, _) = fixture();
    app.invoke_switch_home_mode(true);
    assert!(app.invoke_apply_fetch_options(
        true,
        false,
        "".into(),
        "".into(),
        "first_account\nsecond_account".into()
    ));
    let first = app.get_batch_fetch_items().row_data(0).unwrap();
    assert!(!app.invoke_apply_fetch_options(
        true,
        false,
        "".into(),
        "".into(),
        "https://example.invalid/not-twitter".into()
    ));
    assert_eq!(app.get_batch_fetch_items().row_count(), 2);
    assert_eq!(
        app.get_batch_fetch_items().row_data(0).unwrap().source,
        first.source
    );
    assert!(!app.get_fetch_options_error().is_empty());
    app.invoke_clear_batch();
    assert!(app.get_batch_mode());
    assert_eq!(app.get_batch_fetch_items().row_count(), 0);
}

#[test]
fn private_single_source_survives_a_public_batch_round_trip() {
    let (app, _) = fixture();
    app.invoke_choose_fetch_source(2);
    let private_source = app.get_source_url();
    assert_eq!(app.get_fetch_source_index(), 2);
    app.invoke_switch_home_mode(true);
    assert!(app.get_batch_mode());
    assert_eq!(app.get_fetch_source_index(), 0);
    assert!(app.invoke_apply_fetch_options(
        true,
        false,
        "".into(),
        "".into(),
        "first_account".into()
    ));
    app.invoke_switch_home_mode(false);
    assert_eq!(app.get_source_url(), private_source);
    assert_eq!(app.get_fetch_source_index(), 2);
    assert!(!app.get_batch_mode());
    assert_eq!(app.get_batch_fetch_items().row_count(), 1);
}

#[test]
fn restored_batch_is_paused_and_keeps_its_completed_result_links() {
    let app = AppWindow::new().unwrap();
    let profile = downloader::ExtractionProfile {
        url: "https://x.com/first_account".into(),
        media_type: "all".into(),
        retweets: "skip".into(),
        ..Default::default()
    };
    let history = vec![record("saved-result", profile.clone())];
    let snapshot = crate::fetch::queue::Snapshot {
        version: 1,
        enabled: true,
        from_history: false,
        entries: vec![crate::fetch::queue::Row {
            source: profile.url.clone(),
            status: "Completed".into(),
            history_id: "saved-result".into(),
            media_count: 2,
        }],
        profiles: vec![profile],
    };
    let restored = restore(&app, snapshot, &history);
    assert!(!restored.active);
    assert!(app.get_batch_mode());
    assert!(!app.get_batch_running());
    assert!(!app.get_batch_result_open());
    assert_eq!(
        app.get_batch_fetch_items().row_data(0).unwrap().history_id,
        "saved-result"
    );
    assert_eq!(
        app.get_batch_fetch_items().row_data(0).unwrap().status,
        "Completed"
    );
}

#[test]
fn single_options_do_not_replace_a_dormant_history_batch_profile() {
    JOBS.with_borrow_mut(VecDeque::clear);
    let app = AppWindow::new().unwrap();
    app.set_saved_auth_token("a".repeat(40));
    let profile = downloader::ExtractionProfile {
        url: "https://x.com/history_account".into(),
        media_type: "video".into(),
        retweets: "include".into(),
        ..Default::default()
    };
    let state = State {
        from_history: true,
        entries: vec![Entry {
            source: profile.url.clone(),
            status: "Queued",
            history_id: String::new(),
            media_count: 0,
            error: String::new(),
            avatar_url: String::new(),
            avatar_square: false,
        }],
        profiles: vec![profile],
        ..Default::default()
    };
    register_context(
        &app,
        Arc::new(Mutex::new(Default::default())),
        Arc::new(Mutex::new(Vec::new())),
        Arc::new(Mutex::new((String::new(), false))),
        Arc::new(AtomicBool::new(false)),
        runner,
        state,
        None,
    );
    assert!(app.invoke_apply_fetch_options(false, false, "".into(), "".into(), "".into()));
    app.invoke_switch_home_mode(true);
    assert!(app.get_batch_history_profiles());
    app.invoke_start_batch();
    JOBS.with_borrow(|jobs| {
        let job = jobs.front().unwrap();
        assert_eq!(job.profile.media_type, "video");
        assert_eq!(job.profile.retweets, "include");
    });
}

#[test]
fn deferred_history_loading_preserves_saved_batch_links_and_uses_summary_counts() {
    let app = AppWindow::new().unwrap();
    app.set_history_busy(true);
    let profile = downloader::ExtractionProfile {
        url: "https://x.com/demo".into(),
        media_type: "all".into(),
        retweets: "skip".into(),
        page_size: 100,
        ..Default::default()
    };
    let saved = crate::fetch::queue::Snapshot {
        version: 1,
        enabled: true,
        from_history: false,
        profiles: vec![profile.clone()],
        entries: vec![crate::fetch::queue::Row {
            source: "https://x.com/demo".into(),
            status: "Completed".into(),
            history_id: "100".into(),
            media_count: 60000,
        }],
    };
    let initial = restore(&app, saved, &[]);
    assert_eq!(initial.entries[0].history_id, "100");
    assert_eq!(initial.entries[0].status, "Completed");
    let header = downloader::HistoryEntry {
        id: "100".into(),
        username: "demo".into(),
        source_url: profile.url.clone(),
        profile: Some(profile),
        stored: Some(crate::history::storage::StoredEntry {
            path: Arc::new(std::path::PathBuf::from("does-not-exist.sqlite3")),
            id: "100".into(),
            summary: crate::history::storage::Summary {
                count: 60000,
                ..Default::default()
            },
        }),
        ..Default::default()
    };
    register_context(
        &app,
        Arc::new(Mutex::new(downloader::BackendState::default())),
        Arc::new(Mutex::new(vec![header])),
        Arc::new(Mutex::new((String::new(), false))),
        Arc::new(AtomicBool::new(false)),
        runner,
        initial,
        None,
    );
    app.set_history_busy(false);
    app.invoke_history_loaded();
    let row = app.get_batch_fetch_items().row_data(0).unwrap();
    assert_eq!(row.history_id, "100");
    assert_eq!(row.status, "Completed");
    assert_eq!(row.media_count, 60000);
}

#[tokio::test(flavor = "current_thread")]
async fn resuming_a_stored_batch_loads_its_real_cursor_response_before_starting_the_worker() {
    let fixture = crate::download::saved_files::tests::Fixture::new();
    let path = fixture.0.join("batch-resume.sqlite3");
    let connection = rusqlite::Connection::open(&path).unwrap();
    crate::history::db::initialize_database(&connection).unwrap();
    let profile = downloader::ExtractionProfile {
        url: "https://x.com/demo".into(),
        media_type: "all".into(),
        retweets: "skip".into(),
        media_timeline: true,
        page_size: 100,
        ..Default::default()
    };
    let mut entry = record("100", profile.clone());
    entry.has_more = true;
    entry.next_cursor = "cursor-next".into();
    entry.json = r#"{"account":{"username":"demo"},"tweets":[],"opaque":"preserved"}"#.into();
    crate::history::db::save_entry_on(&connection, &entry).unwrap();
    let headers = crate::history::db::read_summaries(&connection, Arc::new(path)).unwrap();
    let app = AppWindow::new().unwrap();
    app.set_saved_auth_token("a".repeat(40));
    let saved = crate::fetch::queue::Snapshot {
        version: 1,
        enabled: true,
        from_history: false,
        profiles: vec![profile],
        entries: vec![crate::fetch::queue::Row {
            source: "https://x.com/demo".into(),
            status: "Incomplete".into(),
            history_id: "100".into(),
            media_count: 2,
        }],
    };
    let initial = restore(&app, saved, &headers);
    JOBS.with_borrow_mut(VecDeque::clear);
    register_context(
        &app,
        Arc::new(Mutex::new(downloader::BackendState::default())),
        Arc::new(Mutex::new(headers)),
        Arc::new(Mutex::new((String::new(), false))),
        Arc::new(AtomicBool::new(false)),
        runner,
        initial,
        None,
    );
    app.invoke_start_batch();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    let ctx = egui::Context::default();
    loop {
        crate::ui::runtime::pump(&ctx);
        if JOBS.with_borrow(|jobs| !jobs.is_empty()) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Stored batch did not resume"
        );
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    let job = JOBS.with_borrow_mut(VecDeque::pop_front).unwrap();
    assert_eq!(job.options.cursor, "cursor-next");
    assert_eq!(
        job.options.previous_json.as_deref(),
        Some(entry.json.as_str())
    );
    assert_eq!(job.backend.lock().unwrap().candidates.len(), 2);
}
