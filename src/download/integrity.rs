use crate::AppWindow;
use crate::download::engine::{BackendState, HistoryEntry, MediaCandidate};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Debug)]
pub struct Signature {
    pub bytes: u64,
    pub sha256: String,
}
impl Signature {
    pub fn apply(&self, item: &mut MediaCandidate) {
        item.saved_bytes = self.bytes;
        item.saved_sha256.clone_from(&self.sha256);
    }
}
pub fn read_signature(path: &Path, cancel: Option<&AtomicBool>) -> Result<Signature, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let before = file.metadata().map_err(|e| e.to_string())?;
    if !before.is_file() {
        return Err("Not a regular file.".into());
    }
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    let mut bytes = 0;
    loop {
        if cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            return Err("Check cancelled.".into());
        }
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
        bytes += n as u64;
    }
    let after = file.metadata().map_err(|e| e.to_string())?;
    if bytes != before.len()
        || before.len() != after.len()
        || before.modified().ok() != after.modified().ok()
    {
        return Err("File changed during the check.".into());
    }
    Ok(Signature {
        bytes,
        sha256: format!("{:x}", hash.finalize()),
    })
}
pub async fn signature(path: PathBuf, cancel: Arc<AtomicBool>) -> Option<Signature> {
    static READERS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);
    let _permit = loop {
        tokio::select! {
            permit=READERS.acquire()=>break permit.ok()?,
            _=tokio::time::sleep(std::time::Duration::from_millis(50))=>{if cancel.load(Ordering::Relaxed) {return None;}}
        }
    };
    tokio::task::spawn_blocking(move || read_signature(&path, Some(&cancel)))
        .await
        .ok()?
        .ok()
}

pub fn available(path: &str, expected: u64) -> bool {
    std::fs::metadata(path).is_ok_and(|data| {
        data.is_file() && data.len() > 0 && (expected == 0 || data.len() == expected)
    })
}
#[derive(Debug, PartialEq)]
pub enum Check {
    Verified,
    Unverified,
    Missing,
    Empty,
    SizeMismatch,
    Modified,
    Unreadable,
}
pub fn check(item: &MediaCandidate, deep: bool, cancel: &AtomicBool) -> Check {
    let Ok(data) = std::fs::metadata(&item.saved_path) else {
        return Check::Missing;
    };
    if !data.is_file() {
        return Check::Missing;
    }
    if data.len() == 0 {
        return Check::Empty;
    }
    if item.saved_bytes > 0 && data.len() != item.saved_bytes {
        return Check::SizeMismatch;
    }
    if !deep && !item.integrity_error.is_empty() {
        return Check::Modified;
    }
    if deep && !item.saved_sha256.is_empty() {
        return match read_signature(Path::new(&item.saved_path), Some(cancel)) {
            Ok(signature) if signature.sha256 == item.saved_sha256 => Check::Verified,
            Ok(_) => Check::Modified,
            Err(_) => Check::Unreadable,
        };
    }
    if !deep && item.saved_bytes > 0 {
        Check::Verified
    } else {
        Check::Unverified
    }
}
type Observed = Option<(u64, Option<std::time::SystemTime>)>;
fn invalidate(
    items: &mut [MediaCandidate],
    invalid: &BTreeMap<String, (Observed, String)>,
    preserve: bool,
) -> bool {
    let mut changed = false;
    for item in items {
        if let Some((observed, reason)) = invalid.get(&item.saved_path) {
            let current = std::fs::metadata(&item.saved_path)
                .ok()
                .map(|data| (data.len(), data.modified().ok()));
            if &current == observed {
                if preserve {
                    item.status = "Invalid".into();
                    item.integrity_error.clone_from(reason);
                } else {
                    item.status = "Ready".into();
                    item.saved_path.clear();
                    item.saved_bytes = 0;
                    item.saved_sha256.clear();
                    item.integrity_error.clear();
                }
                changed = true;
            }
        }
    }
    changed
}
pub fn register(
    app: &AppWindow,
    history: Arc<Mutex<Vec<HistoryEntry>>>,
    backend: Arc<Mutex<BackendState>>,
    filter: Arc<Mutex<(String, bool)>>,
    cancel: Arc<AtomicBool>,
) {
    let ui = app.as_weak();
    app.on_check_downloads(move |deep| {
        let Some(app) = ui.upgrade() else {
            return;
        };
        if app.get_is_busy()
            || app.get_history_busy()
            || app.get_tool_update_busy()
            || app.get_xtractor_busy()
            || app.get_ffmpeg_busy()
        {
            return;
        }
        let batch = app.get_download_batch_id().to_string();
        let snapshots = history.lock().unwrap().clone();
        cancel.store(false, Ordering::Relaxed);
        if app.get_selected_tab() == crate::library::TAB {
            app.update_library(|data| {
                data.feedback_active = true;
                data.error.clear();
            });
        }
        app.set_integrity_open(true);
        app.set_integrity_busy(true);
        app.set_is_busy(true);
        app.set_integrity_report("".into());
        app.set_integrity_status("Checking downloaded files…".into());
        let ui = ui.clone();
        let cancel = cancel.clone();
        let history = history.clone();
        let backend = backend.clone();
        let filter = filter.clone();
        tokio::spawn(async move {
            let worker_ui = ui.clone();
            let worker_cancel = cancel.clone();
            let worker_history = history.clone();
            let worker_backend = backend.clone();
            let result = tokio::task::spawn_blocking(move || {
                let mut files = BTreeMap::new();
                for entry in snapshots
                    .iter()
                    .filter(|entry| batch.is_empty() || entry.id == batch)
                {
                    for item in crate::history::storage::saved_media(entry)
                        .map_err(|error| error.to_string())?
                    {
                        files.entry(item.saved_path.clone()).or_insert(item);
                    }
                }

                let total = files.len();
                let mut rows = Vec::new();
                let mut issues = 0;
                let mut verified = 0;
                let mut invalid = BTreeMap::new();
                for (index, (path, item)) in files.into_iter().enumerate() {
                    if worker_cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let result = check(&item, deep, &worker_cancel);
                    if worker_cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let label = match result {
                        Check::Verified => {
                            verified += 1;
                            "Verified"
                        }
                        Check::Unverified => "Available; no recorded checksum",
                        Check::Missing => "Missing",
                        Check::Empty => "Empty file",
                        Check::SizeMismatch => "Size mismatch",
                        Check::Modified => "Checksum mismatch",
                        Check::Unreadable => "Could not read file",
                    };
                    if !matches!(result, Check::Verified | Check::Unverified) {
                        issues += 1;
                    }
                    if matches!(
                        result,
                        Check::Missing | Check::Empty | Check::SizeMismatch | Check::Modified
                    ) {
                        let observed = std::fs::metadata(&path)
                            .ok()
                            .map(|data| (data.len(), data.modified().ok()));
                        invalid.insert(path.clone(), (observed, label.to_owned()));
                    }
                    rows.push(format!("{label} · {path}"));
                    let text = format!("Checking files · {}/{}", index + 1, total);
                    let _ =
                        worker_ui.upgrade_in_event_loop(move |app| app.set_integrity_status(text));
                }
                let status = format!(
                    "{} checked · {verified} verified · {issues} issues{}",
                    rows.len(),
                    if worker_cancel.load(Ordering::Relaxed) {
                        " · Cancelled"
                    } else {
                        ""
                    }
                );
                let (key, mut candidates) = {
                    let state = worker_backend.lock().unwrap();
                    (
                        (state.history_id.clone(), state.profile.clone()),
                        state.candidates.clone(),
                    )
                };
                let dirty = invalidate(&mut candidates, &invalid, false)
                    | crate::download::saved_files::reconcile(&mut candidates);
                if dirty {
                    let mut state = worker_backend.lock().unwrap();
                    if (state.history_id.clone(), state.profile.clone()) == key {
                        state.candidates = candidates;
                    }
                }
                let mut entries = worker_history.lock().unwrap().clone();
                for entry in &mut entries {
                    if let Some(stored) = &mut entry.stored {
                        let mut items = crate::history::storage::saved_media(&HistoryEntry {
                            stored: Some(stored.clone()),
                            ..Default::default()
                        })
                        .map_err(|error| error.to_string())?;
                        let changed = invalidate(&mut items, &invalid, entry.download.is_some())
                            | (entry.download.is_none()
                                && crate::download::saved_files::reconcile(&mut items));
                        if changed
                            && let Some(summary) =
                                crate::history::storage::patch_saved_checked(stored, &items, true)
                                    .map_err(|error| error.to_string())?
                        {
                            stored.summary = summary;
                        }
                    } else if invalidate(&mut entry.media, &invalid, entry.download.is_some())
                        | (entry.download.is_none()
                            && crate::download::saved_files::reconcile(&mut entry.media))
                    {
                        crate::history::db::save_media(entry.id.clone(), entry.media.clone());
                    }
                }
                *worker_history.lock().unwrap() = entries;
                Ok::<_, String>((status, rows.join("\n"), invalid))
            })
            .await
            .map_err(|error| error.to_string())
            .and_then(|result| result);
            let _ = ui.upgrade_in_event_loop(move |app| {
                app.set_integrity_busy(false);
                app.set_is_busy(false);
                let _invalid = match result {
                    Ok((status, report, invalid)) => {
                        app.set_integrity_status(status);
                        app.set_integrity_report(report);
                        invalid
                    }
                    Err(error) => {
                        app.set_integrity_status(format!("Check failed: {error}"));
                        BTreeMap::new()
                    }
                };
                crate::download::engine::refresh_ui_model(
                    &app,
                    &backend.lock().unwrap().candidates,
                );
                if let Some(model) = app
                    .get_media_items()
                    .as_any()
                    .downcast_ref::<crate::history::paged::PagedModel>()
                {
                    model.refresh_status();
                }
                let entries = history.lock().unwrap();
                let filter = filter.lock().unwrap();
                crate::history::view_model::refresh(&app, &entries, &filter.0, filter.1);
                app.update_library(|data| data.feedback_active = false);
            });
        });
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signatures_detect_truncation_and_same_size_changes_without_rejecting_converted_files() {
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let path = fixture.0.join("converted.gif");
        std::fs::write(&path, b"final GIF").unwrap();
        let mut item = MediaCandidate {
            saved_path: path.to_string_lossy().into_owned(),
            ..Default::default()
        };
        read_signature(&path, None).unwrap().apply(&mut item);
        let cancel = AtomicBool::new(false);
        assert_eq!(check(&item, true, &cancel), Check::Verified);
        std::fs::write(&path, b"alter GIF").unwrap();
        assert_eq!(check(&item, false, &cancel), Check::Verified);
        assert_eq!(check(&item, true, &cancel), Check::Modified);
        std::fs::write(&path, b"part").unwrap();
        assert_eq!(check(&item, false, &cancel), Check::SizeMismatch);
        assert!(!available(&item.saved_path, item.saved_bytes));
        std::fs::write(&path, b"").unwrap();
        assert_eq!(check(&item, true, &cancel), Check::Empty);
        std::fs::remove_file(&path).unwrap();
        assert_eq!(check(&item, true, &cancel), Check::Missing);
    }
}

#[cfg(test)]
mod invalidation_tests {
    use super::*;
    #[test]
    fn corrupt_download_snapshots_cannot_restore_saved_status_and_preserve_file_references() {
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let path = fixture.0.join("file.jpg");
        std::fs::write(&path, b"modified").unwrap();
        let mut item = MediaCandidate {
            status: "Saved".into(),
            saved_path: path.to_string_lossy().into_owned(),
            saved_bytes: 8,
            ..Default::default()
        };
        let metadata = std::fs::metadata(&path).unwrap();
        let failures = BTreeMap::from([(
            item.saved_path.clone(),
            (
                Some((8, metadata.modified().ok())),
                "Checksum mismatch".into(),
            ),
        )]);
        assert!(invalidate(std::slice::from_mut(&mut item), &failures, true));
        assert_eq!(item.status, "Invalid");
        assert_eq!(item.saved_path, path.to_string_lossy());
        assert!(!crate::download::saved_files::exists(&item));
        let serialized = serde_json::to_string(&item).unwrap();
        let reloaded: MediaCandidate = serde_json::from_str(&serialized).unwrap();
        assert_eq!(reloaded.integrity_error, "Checksum mismatch");
        assert!(invalidate(
            std::slice::from_mut(&mut item),
            &failures,
            false
        ));
        assert_eq!(item.status, "Ready");
        assert!(item.saved_path.is_empty());
        assert!(path.exists());
    }
}
