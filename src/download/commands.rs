use crate::AppWindow;
use crate::DownloadedFile;
use crate::download::engine::{self as downloader, DownloadRecord, HistoryEntry, MediaCandidate};
use crate::history::db as history_db;
use crate::ui::runtime::{ModelRc, VecModel};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub fn make_entry(
    source: &HistoryEntry,
    media: Vec<MediaCandidate>,
    filter: &str,
    earlier: bool,
) -> Option<HistoryEntry> {
    let media = media
        .into_iter()
        .filter(crate::download::saved_files::exists)
        .collect::<Vec<_>>();
    if media.is_empty() {
        return None;
    }
    let mut folder = Path::new(&media[0].saved_path).parent()?.to_path_buf();
    for item in &media {
        while !Path::new(&item.saved_path).starts_with(&folder) {
            if !folder.pop() {
                break;
            }
        }
    }
    let total_bytes = media
        .iter()
        .filter_map(|item| std::fs::metadata(&item.saved_path).ok())
        .map(|data| data.len())
        .sum();
    Some(HistoryEntry {
        id: if earlier {
            format!("earlier-{}", source.id)
        } else {
            downloader::new_history_id()
        },
        fetched_at: if earlier {
            String::new()
        } else {
            downloader::current_date_label()
        },
        source_url: source.source_url.clone(),
        account_name: source.account_name.clone(),
        username: source.username.clone(),
        json: serde_json::from_str::<serde_json::Value>(&source.json)
            .ok()
            .and_then(|value| value.get("account").cloned())
            .map(|account| serde_json::json!({"account": account}).to_string())
            .unwrap_or_default(),
        media,
        download: Some(DownloadRecord {
            output_dir: folder.to_string_lossy().into_owned(),
            total_bytes,
            media_filter: filter.into(),
            earlier,
        }),
        ..Default::default()
    })
}

pub fn record(
    entries: &mut Vec<HistoryEntry>,
    fetch_id: &str,
    media: Vec<MediaCandidate>,
    filter: &str,
) {
    let Some(source) = entries
        .iter()
        .find(|entry| entry.id == fetch_id && entry.download.is_none())
    else {
        return;
    };
    let Some(mut entry) = make_entry(source, media, filter, false) else {
        return;
    };
    let mut id = entry.id.parse::<u64>().unwrap_or_default();
    while entries.iter().any(|entry| entry.id == id.to_string()) {
        id += 1;
    }
    entry.id = id.to_string();
    history_db::save_history(entry.clone());
    entries.insert(0, entry);
}

pub fn files(entry: &HistoryEntry) -> ModelRc<DownloadedFile> {
    std::rc::Rc::new(VecModel::from(file_rows(entry))).into()
}

pub fn file_rows(entry: &HistoryEntry) -> Vec<DownloadedFile> {
    entry
        .media
        .iter()
        .map(|item| {
            let path = Path::new(&item.saved_path);
            let metadata = std::fs::metadata(path).ok().filter(|data| data.is_file());
            DownloadedFile {
                media_index: item.media_index.min(i32::MAX as usize) as i32,
                path: item.saved_path.clone(),
                folder: path
                    .parent()
                    .unwrap_or_else(|| Path::new(""))
                    .to_string_lossy()
                    .into_owned(),
                filename: if item.saved_path.is_empty() {
                    format!("{} · {}", item.tweet_id, item.media_type)
                } else {
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                },
                kind: match item.media_type.as_str() {
                    "photo" => "image",
                    "video" => "video",
                    "animated_gif" => "film",
                    _ => "file",
                }
                .into(),
                size_text: metadata
                    .as_ref()
                    .map(|data| downloader::format_bytes(data.len()))
                    .unwrap_or_default(),
                present: item.integrity_error.is_empty()
                    && crate::download::integrity::available(&item.saved_path, item.saved_bytes),
                integrity_state: if !item.integrity_error.is_empty() {
                    item.integrity_error.as_str()
                } else if metadata.is_none() {
                    "File missing"
                } else if metadata.as_ref().unwrap().len() == 0 {
                    "Empty file"
                } else {
                    "Size mismatch"
                }
                .into(),
            }
        })
        .collect()
}

pub fn refresh_details(app: &AppWindow, entries: &[HistoryEntry]) {
    let id = app.get_download_batch_id();
    if id.is_empty() {
        return;
    }
    let Some(entry) = entries
        .iter()
        .find(|entry| entry.id == id.as_str() && entry.download.is_some())
    else {
        app.set_download_batch_id("".into());
        return;
    };
    let record = entry.download.as_ref().unwrap();
    app.set_download_batch_title(if entry.account_name.is_empty() {
        format!("@{}", entry.username)
    } else {
        entry.account_name.clone()
    });
    app.set_download_batch_folder(record.output_dir.clone());
    app.set_download_batch_size(downloader::format_bytes(record.total_bytes));
    app.set_download_batch_files(files(entry));
    app.set_archive_gif_count(
        entry
            .media
            .iter()
            .filter(|item| crate::download::conversion::eligible(item))
            .count()
            .min(i32::MAX as usize) as i32,
    );
}

pub fn register(app: &AppWindow, history: Arc<Mutex<Vec<HistoryEntry>>>) {
    let ui = app.as_weak();
    app.on_open_download_batch(move |id| {
        let Some(app) = ui.upgrade() else { return };
        app.set_download_batch_id(id);
        refresh_details(
            &app,
            &history.lock().unwrap_or_else(|error| error.into_inner()),
        );
    });
    let ui = app.as_weak();
    app.on_open_download_path(move |value, folder| {
        let Some(app) = ui.upgrade() else { return };
        let path = PathBuf::from(value.as_str());
        if (folder && path.is_dir()) || (!folder && path.is_file()) {
            if open::that(&path).is_err() {
                crate::app::notifications::show_toast(&app, "Could not open this path.", "error");
            }
        } else {
            crate::app::notifications::show_toast(
                &app,
                "This file or folder is no longer available.",
                "info",
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn batches_include_only_successful_files_and_keep_their_own_snapshot() {
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let path = fixture.0.join("done.gif");
        std::fs::write(&path, b"gif fixture").unwrap();
        let source = HistoryEntry {
            id: "1".into(),
            username: "demo".into(),
            json: r#"{"account":{"username":"demo"},"tweets":[{"text":"fetch payload"}]}"#.into(),
            media: vec![MediaCandidate {
                status: "Ready".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let completed = MediaCandidate {
            media_type: "animated_gif".into(),
            status: "Saved".into(),
            saved_path: path.to_string_lossy().into_owned(),
            media_index: 9,
            ..Default::default()
        };
        let failed = MediaCandidate {
            status: "Failed".into(),
            ..Default::default()
        };
        let batch = make_entry(
            &source,
            vec![completed, failed.clone()],
            "animated_gif",
            false,
        )
        .unwrap();
        assert_eq!(batch.media.len(), 1);
        assert_eq!(batch.media[0].media_index, 9);
        assert_ne!(batch.id, source.id);
        assert_eq!(
            batch.download.as_ref().unwrap().output_dir,
            fixture.0.to_string_lossy()
        );
        assert_eq!(batch.download.as_ref().unwrap().total_bytes, 11);
        assert!(!batch.json.contains("tweets"));
        assert_eq!(source.media[0].status, "Ready");
        assert!(make_entry(&source, vec![failed], "video", false).is_none());
    }
}
