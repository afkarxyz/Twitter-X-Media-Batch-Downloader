use crate::AppWindow;
use crate::download::engine::{self as downloader, HistoryEntry};
use crate::history::db as history_db;
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

pub fn refresh(app: &AppWindow, entries: &[HistoryEntry], query: &str, saved_only: bool) {
    if app.get_selected_tab() == crate::library::TAB {
        if app.get_library().job.is_empty() {
            let _ = app
                .as_weak()
                .upgrade_in_event_loop(|app| app.invoke_library_refresh());
        }
        return;
    }
    crate::history::refresh::reconcile(app, entries);
    app.set_history_fetch_count(
        crate::fetch::sessions::groups(entries)
            .len()
            .min(i32::MAX as usize) as i32,
    );
    app.set_history_saved_count(
        entries
            .iter()
            .filter(|entry| entry.download.is_some())
            .count()
            .min(i32::MAX as usize) as i32,
    );
    let rows = downloader::to_history_model(entries, "", saved_only)
        .iter()
        .collect();
    let query = query.trim().to_lowercase();
    let rows = crate::accounts::labels::decorate(app, rows)
        .into_iter()
        .filter(|row| {
            query.is_empty()
                || [
                    row.title.as_str(),
                    row.handle.as_str(),
                    row.source_url.as_str(),
                    row.account_labels.as_str(),
                ]
                .iter()
                .any(|value| value.to_lowercase().contains(&query))
        })
        .collect();
    app.set_history_items(downloader::model_from_history_rows(rows));
    crate::media::images::load_history_avatars(app);
    crate::download::commands::refresh_details(app, entries);
    crate::fetch::sessions::refresh_details(app, entries);
}

fn export_payload(entry: &HistoryEntry) -> String {
    serde_json::to_string_pretty(
        &json!({ "type": "twitterdl-desktop-history", "version": 1, "entry": entry }),
    )
    .unwrap_or_default()
}

fn parse_import(text: &str) -> Result<Vec<HistoryEntry>, String> {
    let value: Value = serde_json::from_str(text).map_err(|_| "Could not read history JSON.")?;
    let records = value
        .as_array()
        .map(|items| items.as_slice())
        .unwrap_or(std::slice::from_ref(&value));
    let mut entries = Vec::new();
    let mut ids = HashSet::new();
    for record in records {
        let raw = record.get("tweets").is_some();
        let entry = if raw {
            downloader::history_entry_from_response(record.to_string())
        } else if record.get("type").and_then(Value::as_str) == Some("twitterdl-desktop-history") {
            if record.get("version").and_then(Value::as_u64) != Some(1) {
                continue;
            }
            record
                .get("entry")
                .and_then(|entry| serde_json::from_value::<HistoryEntry>(entry.clone()).ok())
        } else if record.get("source_url").is_some() && record.get("media").is_some() {
            serde_json::from_value::<HistoryEntry>(record.clone()).ok()
        } else {
            None
        };
        let Some(mut entry) = entry.filter(|entry| {
            !entry.username.trim().is_empty() && !entry.source_url.trim().is_empty()
        }) else {
            continue;
        };
        if raw || entry.id.is_empty() {
            let mut id = downloader::new_history_id()
                .parse::<u64>()
                .unwrap_or_default();
            while ids.contains(&id.to_string()) {
                id += 1;
            }
            entry.id = id.to_string();
        }
        ids.insert(entry.id.clone());
        downloader::restore_preview_details(&mut entry.media, &entry.json);
        if entry.download.is_none() {
            crate::fetch::archives::restore_text(&mut entry);
            crate::download::saved_files::reconcile(&mut entry.media);
        }
        entries.push(entry);
    }
    if entries.is_empty() {
        Err("No valid history entries found in this JSON.".into())
    } else {
        Ok(entries)
    }
}

pub fn register(
    app: &AppWindow,
    history: Arc<Mutex<Vec<HistoryEntry>>>,
    filter: Arc<Mutex<(String, bool)>>,
) {
    let ui = app.as_weak();
    let entries = history.clone();
    app.on_open_fetch_sessions(move |id| {
        let Some(app) = ui.upgrade() else { return };
        let entries = entries.lock().unwrap_or_else(|error| error.into_inner());
        crate::fetch::sessions::open(&app, &entries, id.as_str());
    });
    let ui = app.as_weak();
    let entries = history.clone();
    let active_filter = filter.clone();
    app.on_remove_history(move |id| {
        let Some(app) = ui.upgrade() else { return };
        if app.get_is_busy() || app.get_history_busy() {
            return;
        }
        let active_filter = active_filter
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut entries = entries.lock().unwrap_or_else(|error| error.into_inner());
        let removed = crate::fetch::sessions::removal_ids(
            &entries,
            id.as_str(),
            !app.get_fetch_sessions_key().is_empty(),
        );
        entries.retain(|entry| !removed.contains(&entry.id));
        for id in removed {
            history_db::remove_history(id);
        }
        refresh(&app, &entries, &active_filter.0, active_filter.1);
    });
    let ui = app.as_weak();
    let entries = history.clone();
    app.on_export_history(move |id| {
        let Some(app) = ui.upgrade() else { return };
        if app.get_history_busy() {
            return;
        }
        let individual = app.get_selected_tab() == 1 && !app.get_fetch_sessions_key().is_empty();
        let snapshots = entries.lock().unwrap().clone();
        let Some(entry) = snapshots.iter().find(|entry| entry.id == id).cloned() else {
            return;
        };
        let handle = entry
            .username
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
            .collect::<String>();
        let id = entry
            .id
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
            .take(64)
            .collect::<String>();
        let filename = format!("twitterdl-{handle}-{id}.json");
        let Some(path) = rfd::FileDialog::new()
            .set_file_name(filename)
            .add_filter("JSON", &["json"])
            .save_file()
        else {
            return;
        };
        let ui = ui.clone();
        app.set_history_busy(true);
        tokio::spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                let entry = crate::fetch::archives::try_open(&snapshots, &entry.id, individual)?
                    .ok_or_else(|| "This history entry is no longer available.".to_owned())?;
                std::fs::write(path, export_payload(&entry)).map_err(|error| error.to_string())
            })
            .await;

            let _ = crate::ui::runtime::invoke_from_event_loop(move || {
                if let Some(app) = ui.upgrade() {
                    app.set_history_busy(false);
                    match result {
                        Ok(Ok(())) => crate::app::notifications::show_toast(
                            &app,
                            "History JSON exported.",
                            "info",
                        ),
                        _ => crate::app::notifications::show_toast(
                            &app,
                            "Could not export history JSON.",
                            "error",
                        ),
                    }
                }
            });
        });
    });
    let ui = app.as_weak();
    app.on_import_history(move || {
        let Some(app) = ui.upgrade() else { return };
        if app.get_history_busy() || app.get_is_busy() {
            return;
        }
        let Some(paths) = rfd::FileDialog::new()
            .add_filter("History / xtractor JSON", &["json"])
            .pick_files()
        else {
            return;
        };
        app.set_history_busy(true);
        let ui = ui.clone();
        let history = history.clone();
        let filter = filter.clone();
        tokio::spawn(async move {
            let mut imported = Vec::new();
            for path in paths {
                if tokio::fs::metadata(&path)
                    .await
                    .is_ok_and(|data| data.len() <= 1_073_741_824)
                    && let Ok(text) = tokio::fs::read_to_string(path).await
                    && let Ok(mut entries) = parse_import(&text)
                {
                    imported.append(&mut entries);
                }
            }
            let count = imported.len();
            {
                let mut history = history.lock().unwrap_or_else(|error| error.into_inner());
                for entry in imported {
                    if let Some(previous) =
                        history.iter_mut().find(|previous| previous.id == entry.id)
                    {
                        *previous = entry.clone();
                    } else {
                        history.insert(0, entry.clone());
                    }
                    history_db::save_history(entry);
                }
            }
            let restored = tokio::task::spawn_blocking(|| {
                history_db::flush();
                history_db::load_summaries()
            })
            .await;
            if let Ok(Ok(entries)) = restored {
                *history.lock().unwrap() = entries;
            }
            let _ = crate::ui::runtime::invoke_from_event_loop(move || {
                if let Some(app) = ui.upgrade() {
                    app.set_history_busy(false);
                    let filter = filter.lock().unwrap_or_else(|error| error.into_inner());
                    let entries = history.lock().unwrap_or_else(|error| error.into_inner());
                    refresh(&app, &entries, &filter.0, filter.1);
                    if count > 0 {
                        crate::app::notifications::show_toast(
                            &app,
                            format!("Imported {count} history entries."),
                            "info",
                        );
                    } else {
                        crate::app::notifications::show_toast(
                            &app,
                            "No valid history entries found in this JSON.",
                            "error",
                        );
                    }
                }
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_json_roundtrip_keeps_source_and_media_and_rejects_unknown_records() {
        let entry = HistoryEntry {
            id: "1".into(),
            username: "demo".into(),
            source_url: "https://x.com/demo".into(),
            media: vec![downloader::MediaCandidate {
                url: "https://video.twimg.com/example.mp4".into(),
                media_index: 7,
                media_type: "video".into(),
                status: "Saved".into(),
                saved_path: "missing-test-file.mp4".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let imported = parse_import(&export_payload(&entry)).unwrap();
        assert_eq!(imported[0].source_url, entry.source_url);
        assert_eq!(imported[0].media[0].media_index, 7);
        assert_eq!(imported[0].media[0].status, "Ready");
        assert!(parse_import("{\"settings\":{}}").is_err());
        let raw = r#"{"account":{"username":"demo","name":"Demo"},"tweets":[]}"#;
        assert_eq!(parse_import(raw).unwrap()[0].username, "demo");
    }
}
