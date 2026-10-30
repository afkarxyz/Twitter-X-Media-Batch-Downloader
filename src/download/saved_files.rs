use crate::AppWindow;
use crate::download::engine::{self as downloader, BackendState, HistoryEntry, MediaCandidate};
use crate::history::db as history_db;
use crate::history::view_model as history_ui;
use std::{
    collections::HashSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

pub fn exists(item: &MediaCandidate) -> bool {
    item.status == "Saved"
        && !item.saved_path.is_empty()
        && crate::download::integrity::available(&item.saved_path, item.saved_bytes)
}

pub fn reconcile(items: &mut [MediaCandidate]) -> bool {
    let mut changed = false;
    for item in items {
        if item.status == "Saved" && !exists(item) {
            item.status = "Ready".into();
            item.saved_path.clear();
            item.saved_bytes = 0;
            item.saved_sha256.clear();
            changed = true;
        }
    }
    changed
}

fn reconcile_missing(items: &mut [MediaCandidate], missing: &HashSet<String>) -> bool {
    let mut changed = false;
    for item in items {
        // Check again before changing state: a new download may have recreated
        // the same path while the background scan was running.
        if item.status == "Saved" && missing.contains(&item.saved_path) {
            changed |= reconcile(std::slice::from_mut(item));
        }
    }
    changed
}

pub fn watch(
    app: &AppWindow,
    backend: Arc<Mutex<BackendState>>,
    history: Arc<Mutex<Vec<HistoryEntry>>>,
    filter: Arc<Mutex<(String, bool)>>,
) -> crate::ui::runtime::Timer {
    let timer = crate::ui::runtime::Timer::default();
    let ui = app.as_weak();
    let scanning = Arc::new(AtomicBool::new(false));
    let last_missing = Arc::new(Mutex::new(HashSet::<String>::new()));
    let refresh_pending = Arc::new(AtomicBool::new(false));
    timer.start(
        crate::ui::runtime::TimerMode::Repeated,
        Duration::from_secs(2),
        move || {
            let Some(app) = ui.upgrade() else {
                return;
            };
            if app.get_is_busy() || app.get_history_busy() || scanning.swap(true, Ordering::AcqRel)
            {
                return;
            }
            let ui = ui.clone();
            let backend = backend.clone();
            let history = history.clone();
            let filter = filter.clone();
            let scanning = scanning.clone();
            let previous = last_missing.clone();
            let refresh_pending = refresh_pending.clone();
            std::thread::spawn(move || {
                let snapshots = history.lock().unwrap().clone();
                let mut paths = match crate::history::storage::saved_paths(&snapshots) {
                    Ok(paths) => paths,
                    Err(error) => {
                        crate::platform::logging::error(format!(
                            "Could not scan saved paths: {error}"
                        ));
                        scanning.store(false, Ordering::Release);
                        return;
                    }
                };
                paths.extend(
                    backend
                        .lock()
                        .unwrap()
                        .candidates
                        .iter()
                        .filter(|item| !item.saved_path.is_empty())
                        .map(|item| (item.saved_path.clone(), item.saved_bytes)),
                );
                let missing = paths
                    .into_iter()
                    .filter(|(path, bytes)| !crate::download::integrity::available(path, *bytes))
                    .map(|(path, _)| path)
                    .collect::<HashSet<_>>();
                let flag = scanning.clone();
                let failed = scanning.clone();
                if ui
                    .upgrade_in_event_loop(move |app| {
                        if app.get_is_busy() || app.get_history_busy() {
                            flag.store(false, Ordering::Release);
                            return;
                        }
                        let ui = app.as_weak();
                        std::thread::spawn(move || {
                            let mut summaries = Vec::new();
                            let mut legacy = Vec::new();
                            for mut entry in snapshots {
                                if let Some(stored) = &entry.stored {
                                    let updates =
                                        crate::history::storage::missing_media(&entry, &missing)
                                            .map(|items| {
                                                items
                                                    .into_iter()
                                                    .filter_map(|mut item| {
                                                        if crate::download::integrity::available(
                                                            &item.saved_path,
                                                            item.saved_bytes,
                                                        ) {
                                                            return None;
                                                        }
                                                        if entry.download.is_some() {
                                                            item.status = "Missing".into();
                                                        } else {
                                                            item.status = "Ready".into();
                                                            item.saved_path.clear();
                                                            item.saved_bytes = 0;
                                                            item.saved_sha256.clear();
                                                            item.integrity_error.clear();
                                                        }
                                                        Some(item)
                                                    })
                                                    .collect::<Vec<_>>()
                                            });
                                    match updates.and_then(|items| {
                                        if items.is_empty() {
                                            Ok(None)
                                        } else {
                                            crate::history::storage::patch_saved(stored, &items)
                                        }
                                    }) {
                                        Ok(Some(summary)) => summaries.push((entry.id, summary)),
                                        Ok(None) => {}
                                        Err(error) => crate::platform::logging::error(format!(
                                            "Could not reconcile missing files: {error}"
                                        )),
                                    }
                                } else if entry.download.is_none()
                                    && reconcile_missing(&mut entry.media, &missing)
                                {
                                    history_db::save_media(entry.id.clone(), entry.media.clone());
                                    legacy.push(entry);
                                }
                            }
                            let changed = {
                                let mut previous = previous.lock().unwrap();
                                let changed = *previous != missing;
                                *previous = missing.clone();
                                changed
                            };
                            let flag = scanning.clone();
                            if ui
                                .upgrade_in_event_loop(move |app| {
                                    flag.store(false, Ordering::Release);
                                    let mut entries = history.lock().unwrap();
                                    let dirty =
                                        changed || !summaries.is_empty() || !legacy.is_empty();
                                    for (id, summary) in summaries {
                                        if let Some(entry) =
                                            entries.iter_mut().find(|entry| entry.id == id)
                                            && let Some(stored) = &mut entry.stored
                                        {
                                            stored.summary = summary;
                                        }
                                    }
                                    if app.get_is_busy() || app.get_history_busy() {
                                        if dirty {
                                            refresh_pending.store(true, Ordering::Release);
                                        }
                                        return;
                                    }
                                    let dirty =
                                        dirty || refresh_pending.swap(false, Ordering::AcqRel);
                                    for updated in legacy {
                                        if let Some(entry) = entries.iter_mut().find(|entry| {
                                            entry.id == updated.id && entry.stored.is_none()
                                        }) {
                                            *entry = updated;
                                        }
                                    }
                                    let mut state = backend.lock().unwrap();
                                    if reconcile_missing(&mut state.candidates, &missing) {
                                        downloader::refresh_ui_model(&app, &state.candidates);
                                    }
                                    drop(state);
                                    if dirty {
                                        if let Some(model) =
                                            app.get_media_items()
                                                .as_any()
                                                .downcast_ref::<crate::history::paged::PagedModel>()
                                        {
                                            model.refresh_status();
                                        }
                                        let filter = filter.lock().unwrap();
                                        history_ui::refresh(&app, &entries, &filter.0, filter.1);
                                    }
                                })
                                .is_err()
                            {
                                scanning.store(false, Ordering::Release);
                            }
                        });
                    })
                    .is_err()
                {
                    failed.store(false, Ordering::Release);
                }
            });
        },
    );
    timer
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    pub(crate) struct Fixture(pub PathBuf);
    impl Fixture {
        pub(crate) fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let root = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("target")
                .join(format!(
                    "saved-files-test-{}-{}-{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
            std::fs::create_dir_all(&root).unwrap();
            Self(root)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
            if self.0.is_absolute()
                && self.0.parent() == Some(target.as_path())
                && self
                    .0
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("saved-files-test-"))
            {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    #[test]
    fn existing_saved_files_are_kept_and_deleted_or_invalid_paths_are_reset() {
        let fixture = Fixture::new();
        let file = fixture.0.join("saved.jpg");
        std::fs::write(&file, b"saved").unwrap();
        let mut items = vec![MediaCandidate {
            status: "Saved".into(),
            saved_path: file.to_string_lossy().into_owned(),
            ..Default::default()
        }];
        assert!(exists(&items[0]));
        assert!(!reconcile(&mut items));
        std::fs::remove_file(&file).unwrap();
        assert!(reconcile(&mut items));
        assert_eq!(items[0].status, "Ready");
        assert!(items[0].saved_path.is_empty());
        for path in [String::new(), fixture.0.to_string_lossy().into_owned()] {
            let mut invalid = [MediaCandidate {
                status: "Saved".into(),
                saved_path: path,
                ..Default::default()
            }];
            assert!(reconcile(&mut invalid));
        }
    }

    #[test]
    fn a_stale_scan_does_not_reset_a_file_recreated_before_it_is_applied() {
        let fixture = Fixture::new();
        let path = fixture.0.join("recreated.mp4");
        let value = path.to_string_lossy().into_owned();
        let missing = HashSet::from([value.clone()]);
        std::fs::write(&path, b"new download").unwrap();
        let mut items = [MediaCandidate {
            status: "Saved".into(),
            saved_path: value,
            ..Default::default()
        }];
        assert!(!reconcile_missing(&mut items, &missing));
        assert_eq!(items[0].status, "Saved");
    }

    #[tokio::test]
    async fn batch_redownloads_missing_files_and_skips_existing_saved_files() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };
        let fixture = Fixture::new();
        let existing = fixture.0.join("existing.jpg");
        std::fs::write(&existing, b"keep existing").unwrap();
        let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = server.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let mut requests = Vec::new();
            for _ in 0..2 {
                let (mut stream, _) = server.accept().await.unwrap();
                let mut buffer = [0; 4096];
                let size = stream.read(&mut buffer).await.unwrap();
                requests.push(
                    String::from_utf8_lossy(&buffer[..size])
                        .split_whitespace()
                        .nth(1)
                        .unwrap()
                        .to_owned(),
                );
                stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\nRESTORED").await.unwrap();
            }
            requests
        });
        let candidates = (0..3)
            .map(|index| MediaCandidate {
                url: format!("http://{address}/{index}.jpg"),
                media_type: "photo".into(),
                username: "demo".into(),
                media_index: index,
                status: "Saved".into(),
                saved_path: if index == 0 {
                    existing.to_string_lossy().into_owned()
                } else {
                    fixture
                        .0
                        .join(format!("missing-{index}.jpg"))
                        .to_string_lossy()
                        .into_owned()
                },
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let backend = Arc::new(Mutex::new(BackendState {
            candidates: candidates.clone(),
            ..Default::default()
        }));
        let summary = downloader::download_all(
            candidates,
            "all",
            fixture.0.clone(),
            "{handle}_{index}".into(),
            "{handle}".into(),
            None,
            "highest".into(),
            "orig".into(),
            2,
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
        let requests = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("Missing-file downloads never reached the server")
            .unwrap();
        assert_eq!(requests.len(), 2);
        assert!(!requests.contains(&"/0.jpg".to_owned()));
        assert_eq!(summary.progress, 100.0);
        assert_eq!(std::fs::read(existing).unwrap(), b"keep existing");
        let state = backend.lock().unwrap();
        for item in &state.candidates[1..] {
            assert!(exists(item));
            assert_eq!(std::fs::read(&item.saved_path).unwrap(), b"RESTORED");
        }
    }
}
