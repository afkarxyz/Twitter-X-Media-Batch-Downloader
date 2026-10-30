use super::*;
use crate::download::engine::{DownloadRecord, MediaCandidate};
use crate::ui::runtime::{self, VecModel};
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::{Duration, Instant},
};

struct Job {
    source: String,
    backend: Arc<Mutex<BackendState>>,
    complete: crate::fetch::batch::CompletionFn,
}
thread_local! { static JOB: RefCell<Option<Job>> = const { RefCell::new(None) }; }

fn runner(
    _ui: runtime::Weak<AppWindow>,
    options: downloader::ExtractionOptions,
    profile: downloader::ExtractionProfile,
    backend: Arc<Mutex<BackendState>>,
    _history: Arc<Mutex<Vec<HistoryEntry>>>,
    _filter: Arc<Mutex<(String, bool)>>,
    _cancel: Arc<AtomicBool>,
    complete: Option<crate::fetch::batch::CompletionFn>,
) {
    assert!(options.url.starts_with("https://x.com/"));
    assert!(options.media_timeline);
    assert_eq!(options.media_timeline, profile.media_timeline);
    JOB.with_borrow_mut(|job| {
        *job = Some(Job {
            source: options.url,
            backend,
            complete: complete.unwrap(),
        })
    });
}

fn entry(item: MediaCandidate) -> HistoryEntry {
    HistoryEntry {
        id: "1".into(),
        username: "demo".into(),
        account_name: "Demo".into(),
        source_url: "https://x.com/demo".into(),
        json: r#"{"account":{"username":"demo","name":"Demo"}}"#.into(),
        media: vec![item],
        download: Some(DownloadRecord::default()),
        ..Default::default()
    }
}

async fn wait(app: &AppWindow, ready: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(12);
    let ctx = egui::Context::default();
    loop {
        runtime::pump(&ctx);
        if ready() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Library operation timed out: busy={}, job={}, loading={}",
            app.get_is_busy(),
            app.get_library().job,
            app.get_library().loading
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

#[test]
fn catalog_groups_authors_and_quality_variants_without_losing_missing_paths() {
    let fixture = crate::download::saved_files::tests::Fixture::new();
    let path = fixture.0.join("photo.jpg");
    std::fs::write(&path, b"saved image").unwrap();
    let item = MediaCandidate {
        username: "Demo".into(),
        tweet_id: "100".into(),
        url: "https://pbs.twimg.com/media/ABC.jpg?name=small".into(),
        media_type: "photo".into(),
        status: "Saved".into(),
        saved_path: path.to_string_lossy().into_owned(),
        saved_bytes: 11,
        ..Default::default()
    };
    let old = entry(item.clone());
    let mut newer = old.clone();
    newer.id = "2".into();
    newer.download = None;
    newer.media[0].url = "https://pbs.twimg.com/media/ABC?format=png&name=orig".into();
    newer.media[0].saved_path.clear();
    newer.media.push(MediaCandidate {
        username: "unrelated".into(),
        url: "https://example.invalid/other.jpg".into(),
        ..Default::default()
    });
    let mut accounts = catalog::accounts(&[newer.clone(), old.clone()]);
    assert_eq!(accounts.len(), 1);
    let account = accounts.pop().unwrap();
    assert_eq!(
        (
            account.available,
            account.missing,
            account.entry.media.len()
        ),
        (1, 0, 1)
    );
    assert_eq!(account.entry.media[0].saved_path, item.saved_path);
    assert_eq!(account.entry.media[0].url, newer.media[0].url);
    std::fs::remove_file(path).unwrap();
    let account = catalog::accounts(&[newer, old]).pop().unwrap();
    assert_eq!((account.available, account.missing), (0, 1));
    assert_eq!(account.entry.media[0].status, "Missing");
    assert_eq!(account.entry.media[0].saved_path, item.saved_path);
}

#[tokio::test(flavor = "current_thread")]
async fn library_refetch_and_redownload_stay_out_of_home_and_survive_cleared_fetch_history() {
    let fixture = crate::download::saved_files::tests::Fixture::new();
    let folder = fixture.0.join("demo");
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("original.png");
    let bytes = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/logo/logo.png"));
    std::fs::write(&path, bytes).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/photo.png", listener.local_addr().unwrap());
    let saved_item = MediaCandidate {
        username: "demo".into(),
        author_name: "Demo".into(),
        tweet_id: "100".into(),
        created_at: "2026-10-15T00:00:00Z".into(),
        url: url.clone(),
        media_type: "photo".into(),
        status: "Saved".into(),
        saved_path: path.to_string_lossy().into_owned(),
        saved_bytes: bytes.len() as u64,
        ..Default::default()
    };
    let mut saved_entry = entry(saved_item);
    saved_entry.download.as_mut().unwrap().output_dir = folder.to_string_lossy().into_owned();
    let history = Arc::new(Mutex::new(vec![saved_entry]));
    let app = AppWindow::new().unwrap();
    crate::media::results::register(&app);
    app.set_source_url("single_home".into());
    app.set_status("Home original status".into());
    app.set_progress(17.0);
    app.set_progress_detail("Home original progress".into());
    app.set_home_results_hidden(true);
    app.set_saved_auth_token("a".repeat(40));
    let home = ModelRc::new(VecModel::from(vec![MediaItem {
        title: "Home result".into(),
        media_index: 99,
        ..Default::default()
    }]));
    app.set_media_items(home.clone());
    let backend = Arc::new(Mutex::new(BackendState {
        history_id: "home".into(),
        ..Default::default()
    }));
    let context = Context {
        ui: app.as_weak(),
        backend: backend.clone(),
        history: history.clone(),
        filter: Arc::new(Mutex::new((String::new(), false))),
        cancel: Arc::new(AtomicBool::new(false)),
        catalog: Arc::new(Mutex::new(Arc::new(Vec::new()))),
    };
    register_context(&app, context.clone());
    transfers::register_refetch(&app, context, runner);
    app.set_selected_tab(TAB);
    wait(&app, || !app.get_library().loading).await;
    assert_eq!(app.get_library().items.row_count(), 1);
    app.invoke_library_open("demo".into());
    wait(&app, || {
        app.get_library().view_ready && !app.get_library().loading
    })
    .await;
    assert!(
        app.get_media_items()
            .row_data(0)
            .unwrap()
            .preview_url
            .starts_with("file:")
    );
    app.invoke_library_refetch("demo".into());
    assert!(
        app.get_home_results_hidden(),
        "Refetch exposed the hidden Home result"
    );
    let job = JOB.with_borrow_mut(Option::take).unwrap();
    assert!(!Arc::ptr_eq(&job.backend, &backend));
    let mut fetched = entry(MediaCandidate {
        username: "demo".into(),
        tweet_id: "101".into(),
        url: "https://example.invalid/new.jpg".into(),
        media_type: "photo".into(),
        ..Default::default()
    });
    fetched.download = None;
    fetched.id = "3".into();
    history.lock().unwrap().insert(0, fetched);
    app.set_is_busy(false);
    app.set_is_extracting(false);
    (job.complete)(crate::fetch::batch::Completion {
        history_id: "3".into(),
        media_count: 2,
        has_more: false,
        error: None,
        cancelled: false,
    });
    wait(&app, || !app.get_library().loading).await;
    assert_eq!(app.get_selected_tab(), TAB);
    assert_eq!(app.get_library().pending, 1);
    history
        .lock()
        .unwrap()
        .retain(|entry| entry.download.is_some());
    app.invoke_library_refresh();
    wait(&app, || !app.get_library().loading).await;
    assert_eq!(
        app.get_library().pending,
        1,
        "Clearing fetch history lost the Library update"
    );
    std::fs::remove_file(&path).unwrap();
    app.invoke_library_refresh();
    wait(&app, || !app.get_library().loading).await;
    assert_eq!(app.get_library().missing, 1);
    listener.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Ok((mut stream, _)) = listener.accept() {
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut request = [0; 4096];
                stream.read(&mut request).unwrap();
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", bytes.len()).unwrap();
                stream.write_all(bytes).unwrap();
                return;
            }
            assert!(Instant::now() < deadline, "No re-download request arrived");
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    let index = app
        .get_media_items()
        .iter()
        .find(|item| item.url == url)
        .unwrap()
        .media_index;
    app.invoke_download_selected_media(index);
    assert!(app.get_is_busy());
    app.set_selected_tab(0);
    assert!(home.ptr_eq(&app.get_media_items()));
    wait(&app, || {
        !app.get_is_busy() && app.get_library().job.is_empty()
    })
    .await;
    server.join().unwrap();
    assert!(
        app.get_library().error.is_empty(),
        "{}",
        app.get_library().error
    );
    assert!(
        home.ptr_eq(&app.get_media_items()),
        "Background download replaced Home results"
    );
    assert_eq!(backend.lock().unwrap().history_id, "home");
    assert_eq!(app.get_source_url(), "single_home");
    assert_eq!(app.get_status(), "Home original status");
    assert_eq!(app.get_progress(), 17.0);
    assert_eq!(app.get_progress_detail(), "Home original progress");
    assert!(app.get_last_error().is_empty());
    assert!(!app.get_library().feedback_active);
    app.set_selected_tab(TAB);
    wait(&app, || {
        app.get_library().view_ready && !app.get_library().loading
    })
    .await;
    assert_eq!(
        (app.get_library().available, app.get_library().missing),
        (1, 0)
    );
    let restored = app
        .get_media_items()
        .iter()
        .find(|item| item.url == url)
        .unwrap();
    assert_eq!(std::fs::read(restored.saved_path).unwrap(), bytes);
    app.invoke_library_refetch("demo".into());
    let job = JOB.with_borrow_mut(Option::take).unwrap();
    app.set_is_busy(false);
    app.set_is_extracting(false);
    (job.complete)(crate::fetch::batch::Completion {
        history_id: String::new(),
        media_count: 0,
        has_more: false,
        error: Some("Fixture extraction failure".into()),
        cancelled: false,
    });
    wait(&app, || !app.get_library().loading).await;
    assert!(app.get_library().job.is_empty());
    assert_eq!(app.get_library().error, "Fixture extraction failure");
    assert!(!app.get_library().feedback_active);
    assert!(app.get_last_error().is_empty());
    assert_eq!(app.get_status(), "Home original status");
    assert_eq!(app.get_library().available, 1);
}

#[tokio::test(flavor = "current_thread")]
async fn selected_library_updates_are_sequential_survive_navigation_and_stop_pending_accounts() {
    let app = AppWindow::new().unwrap();
    app.set_saved_auth_token("a".repeat(40));
    app.set_source_url("untouched_home".into());
    app.set_home_results_hidden(true);
    let home = ModelRc::new(VecModel::from(vec![MediaItem {
        title: "Home".into(),
        ..Default::default()
    }]));
    app.set_media_items(home.clone());
    let mut entries = Vec::new();
    for (index, key) in ["demo", "other", "third", "fetch_only"]
        .into_iter()
        .enumerate()
    {
        let mut record = entry(MediaCandidate {
            username: key.into(),
            tweet_id: index.to_string(),
            url: format!("https://example.invalid/{key}.jpg"),
            media_type: "photo".into(),
            ..Default::default()
        });
        record.id = index.to_string();
        record.username = key.into();
        record.account_name = key.into();
        if key == "fetch_only" {
            record.download = None;
        }
        entries.push(record);
    }
    let context = Context {
        ui: app.as_weak(),
        backend: Arc::new(Mutex::new(BackendState::default())),
        history: Arc::new(Mutex::new(entries)),
        filter: Arc::new(Mutex::new(Default::default())),
        cancel: Arc::new(AtomicBool::new(false)),
        catalog: Arc::new(Mutex::new(Arc::new(Vec::new()))),
    };
    register_context(&app, context.clone());
    transfers::register_refetch(&app, context.clone(), runner);
    app.set_selected_tab(TAB);
    wait(&app, || !app.get_library().loading).await;
    app.update_library(|data| {
        data.select_mode = true;
        data.selection = vec![
            "demo".into(),
            "other".into(),
            "third".into(),
            "demo".into(),
            "fetch_only".into(),
        ];
    });
    app.invoke_library_refetch_selected();
    assert_eq!(app.get_library().queue.len(), 3);
    let first = JOB.with_borrow_mut(Option::take).unwrap();
    assert_eq!(first.source, "https://x.com/demo");
    app.invoke_library_refetch("third".into());
    assert!(
        JOB.with_borrow(|job| job.is_none()),
        "A conflicting job started"
    );
    app.set_selected_tab(0);
    app.set_is_busy(false);
    app.set_is_extracting(false);
    (first.complete)(crate::fetch::batch::Completion {
        history_id: String::new(),
        media_count: 0,
        has_more: false,
        error: Some("Failed first account".into()),
        cancelled: false,
    });
    wait(&app, || JOB.with_borrow(|job| job.is_some())).await;
    assert_eq!(app.get_selected_tab(), 0);
    assert!(app.get_home_results_hidden());
    assert!(home.ptr_eq(&app.get_media_items()));
    assert_eq!(app.get_source_url(), "untouched_home");
    assert_eq!(app.get_library().queue[0].status, "Failed");
    let second = JOB.with_borrow_mut(Option::take).unwrap();
    assert_eq!(second.source, "https://x.com/other");
    assert!(!Arc::ptr_eq(&first.backend, &second.backend));
    context.cancel.store(true, Ordering::Relaxed);
    app.set_is_busy(false);
    app.set_is_extracting(false);
    (second.complete)(crate::fetch::batch::Completion {
        history_id: String::new(),
        media_count: 8,
        has_more: true,
        error: None,
        cancelled: true,
    });
    wait(&app, || app.get_library().job.is_empty()).await;
    assert!(!app.get_is_busy());
    assert!(JOB.with_borrow(|job| job.is_none()));
    assert_eq!(
        app.get_library()
            .queue
            .iter()
            .map(|job| job.status.as_str())
            .collect::<Vec<_>>(),
        ["Failed", "Cancelled", "Cancelled"]
    );
    assert_eq!(app.get_library().queue[1].media_count, 8);
    assert_eq!(app.get_library().selection.len(), 5);
    app.set_selected_tab(TAB);
    wait(&app, || !app.get_library().loading).await;
    assert_eq!(app.get_library().items.row_count(), 3);
    assert_eq!(
        app.get_library().selection,
        ["demo", "other", "third", "demo"]
    );
    app.update_library(|data| data.selection = vec!["third".into()]);
    app.invoke_library_refetch_selected();
    let retry = JOB.with_borrow_mut(Option::take).unwrap();
    assert_eq!(retry.source, "https://x.com/third");
    assert!(!context.cancel.load(Ordering::Relaxed));
    app.set_is_busy(false);
    app.set_is_extracting(false);
    (retry.complete)(crate::fetch::batch::Completion {
        history_id: String::new(),
        media_count: 4,
        has_more: false,
        error: None,
        cancelled: false,
    });
    wait(&app, || {
        app.get_library().job.is_empty() && !app.get_library().loading
    })
    .await;
    assert_eq!(app.get_library().queue[0].status, "Completed");
    assert_eq!(app.get_selected_tab(), TAB);
    assert!(home.ptr_eq(&app.get_media_items()));
}

#[tokio::test(flavor = "current_thread")]
async fn library_download_progress_tracks_the_queue_and_does_not_reset_for_the_next_file() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    for cancelled in [false, true] {
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (paused, pause) = tokio::sync::oneshot::channel();
        let (release, resume) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut first, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            first.read(&mut request).await.unwrap();
            first
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\nDATA")
                .await
                .unwrap();
            drop(first);
            let (mut second, _) = listener.accept().await.unwrap();
            second.read(&mut request).await.unwrap();
            second
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\nDA")
                .await
                .unwrap();
            paused.send(()).unwrap();
            let _ = resume.await;
            let _ = second.write_all(b"TA").await;
        });
        let mut record = entry(MediaCandidate {
            username: "demo".into(),
            tweet_id: "1".into(),
            url: format!("http://{address}/1.jpg"),
            media_type: "photo".into(),
            ..Default::default()
        });
        record.media.push(MediaCandidate {
            username: "demo".into(),
            tweet_id: "2".into(),
            media_index: 1,
            url: format!("http://{address}/2.jpg"),
            media_type: "photo".into(),
            ..Default::default()
        });
        record.download.as_mut().unwrap().output_dir =
            fixture.0.join("demo").to_string_lossy().into_owned();
        let app = AppWindow::new().unwrap();
        let context = Context {
            ui: app.as_weak(),
            backend: Arc::new(Mutex::new(BackendState::default())),
            history: Arc::new(Mutex::new(vec![record])),
            filter: Arc::new(Mutex::new(Default::default())),
            cancel: Arc::new(AtomicBool::new(false)),
            catalog: Arc::new(Mutex::new(Arc::new(Vec::new()))),
        };
        register_context(&app, context.clone());
        app.set_selected_tab(TAB);
        wait(&app, || !app.get_library().loading).await;
        app.invoke_library_open("demo".into());
        wait(&app, || {
            app.get_library().view_ready && !app.get_library().loading
        })
        .await;
        app.invoke_library_download(-1);
        tokio::time::timeout(Duration::from_secs(5), pause)
            .await
            .unwrap()
            .unwrap();
        wait(&app, || {
            app.get_library()
                .progress_detail
                .contains("1 / 2 files saved")
        })
        .await;
        let ctx = egui::Context::default();
        for _ in 0..20 {
            runtime::pump(&ctx);
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(
            app.get_library().progress,
            50.0,
            "The second transfer reset or moved overall progress"
        );
        let model = app.get_media_items();
        let profile = app.get_account_profile();
        app.global::<MediaSelection>()
            .set_indices(ModelRc::new(VecModel::from(vec![1])));
        app.update_library(|data| data.files_view = true);
        app.set_selected_tab(2);
        app.set_selected_tab(0);
        app.set_selected_tab(TAB);
        wait(&app, || {
            app.get_library().workspace_active && !app.get_library().loading
        })
        .await;
        assert!(app.get_library().view_ready);
        assert!(app.get_library().files_view);
        assert!(
            model.ptr_eq(&app.get_media_items()),
            "Navigation rebuilt the active download view"
        );
        assert_eq!(profile, app.get_account_profile());
        assert_eq!(
            app.global::<MediaSelection>()
                .get_indices()
                .iter()
                .collect::<Vec<_>>(),
            [1]
        );
        assert_eq!(app.get_library().progress, 50.0);

        assert!(
            app.get_library()
                .progress_detail
                .contains("1 / 2 files saved")
        );
        if cancelled {
            context.cancel.store(true, Ordering::Relaxed);
            wait(&app, || {
                !app.get_is_busy() && app.get_library().job.is_empty()
            })
            .await;
            assert_eq!(app.get_library().progress, 50.0);
            assert!(
                app.get_library().error.is_empty(),
                "Stopping a transfer became an error"
            );
            release.send(()).unwrap();
        } else {
            release.send(()).unwrap();
            wait(&app, || {
                !app.get_is_busy() && app.get_library().job.is_empty()
            })
            .await;
            assert_eq!(app.get_library().progress, 100.0);
            assert!(
                app.get_library()
                    .progress_detail
                    .contains("2 / 2 files saved")
            );
        }
        server.await.unwrap();
        wait(&app, || !app.get_library().loading).await;
        assert_eq!(app.get_library().available, if cancelled { 1 } else { 2 });
    }
}
