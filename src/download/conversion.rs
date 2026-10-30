use crate::AppWindow;
use crate::download::engine::{self as downloader, BackendState, HistoryEntry, MediaCandidate};
use crate::download::gifs as gif_conversion;
use crate::history::db as history_db;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

pub fn eligible(item: &MediaCandidate) -> bool {
    item.media_type == "animated_gif"
        && Path::new(&item.saved_path)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("mp4"))
        && crate::download::saved_files::exists(item)
        && (item.gif_output_path.is_empty() || !Path::new(&item.gif_output_path).is_file())
}

fn same_path(left: &str, right: &Path) -> bool {
    if cfg!(windows) {
        left.eq_ignore_ascii_case(&right.to_string_lossy())
    } else {
        Path::new(left) == right
    }
}

/// Update all references, including independent download snapshots. With Keep
/// original, Downloads lists both files and remembers which MP4 was converted.
pub fn update_entries(
    entries: &mut [HistoryEntry],
    input: &Path,
    output: &Path,
    keep: bool,
) -> Vec<HistoryEntry> {
    let signature = crate::download::integrity::read_signature(output, None).ok();
    let mut changed = Vec::new();
    for entry in entries {
        if let Some(stored) = &entry.stored {
            match crate::history::storage::references_path(stored, input) {
                Ok(false) => continue,
                Ok(true) => match crate::history::storage::resolve(entry) {
                    Ok(loaded) => *entry = loaded,
                    Err(error) => {
                        crate::platform::logging::error(format!(
                            "Could not load GIF file references: {error}"
                        ));
                        changed.push(entry.clone());
                        continue;
                    }
                },
                Err(error) => {
                    crate::platform::logging::error(format!(
                        "Could not locate GIF file references: {error}"
                    ));
                    changed.push(entry.clone());
                    continue;
                }
            }
        }
        let mut originals = Vec::new();
        let mut modified = false;
        for item in &mut entry.media {
            if !same_path(&item.saved_path, input) {
                continue;
            }
            modified = true;
            if keep && entry.download.is_some() {
                let mut original = item.clone();
                original.gif_output_path = output.to_string_lossy().into_owned();
                originals.push(original);
            }
            item.saved_path = output.to_string_lossy().into_owned();
            item.gif_output_path.clear();
            item.status = "Saved".into();
            item.saved_bytes = 0;
            item.saved_sha256.clear();
            if let Some(signature) = &signature {
                signature.apply(item);
            }
        }
        if !modified {
            continue;
        }
        entry.media.extend(originals);
        if let Some(record) = &mut entry.download {
            record.total_bytes = entry
                .media
                .iter()
                .filter_map(|item| std::fs::metadata(&item.saved_path).ok())
                .map(|file| file.len())
                .sum();
        }
        changed.push(entry.clone());
    }
    changed
}

#[derive(Clone)]
struct Context {
    ui: crate::ui::runtime::Weak<AppWindow>,
    history: Arc<Mutex<Vec<HistoryEntry>>>,
    backend: Arc<Mutex<BackendState>>,
    filter: Arc<Mutex<(String, bool)>>,
    cancel: Arc<AtomicBool>,
    persist: fn(Vec<HistoryEntry>) -> Result<(), String>,
}

fn refresh(context: &Context, entries: Vec<HistoryEntry>) {
    let filter = context.filter.lock().unwrap().clone();
    let backend = context.backend.clone();
    let _ = context.ui.upgrade_in_event_loop(move |app| {
        crate::history::view_model::refresh(&app, &entries, &filter.0, filter.1);
        if app.get_selected_tab() != crate::library::TAB {
            crate::download::commands::refresh_details(&app, &entries);
            downloader::refresh_ui_model(&app, &backend.lock().unwrap().candidates);
            if let Some(model) = app
                .get_media_items()
                .as_any()
                .downcast_ref::<crate::history::paged::PagedModel>()
            {
                model.refresh_status();
            }
        }
    });
}

async fn convert(
    context: Context,
    paths: Vec<PathBuf>,
    options: gif_conversion::Options,
    keep: bool,
) {
    let mut completed = 0;
    let mut errors = Vec::new();
    let total = paths.len();
    if let Err(error) =
        crate::updates::ffmpeg::ensure_available(context.ui.clone(), context.cancel.clone(), true)
            .await
    {
        errors.push(error);
    } else {
        for (index, input) in paths.into_iter().enumerate() {
            if context.cancel.load(Ordering::Relaxed) {
                break;
            }
            let label = input
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let _ = context.ui.upgrade_in_event_loop(move |app| {
                app.set_status(format!("Converting GIF {}/{}…", index + 1, total));
                app.set_progress(index as f32 / total as f32 * 100.0);
                app.set_progress_detail(label);
            });
            let output =
                match gif_conversion::convert_file(&input, options, context.cancel.clone()).await {
                    Ok(output) => output,
                    Err(error) => {
                        errors.push(error);
                        continue;
                    }
                };
            let task = context.clone();
            let source = input.clone();
            let destination = output.clone();
            let saved = tokio::task::spawn_blocking(move || {
                let mut entries = task.history.lock().unwrap().clone();
                let updates = update_entries(&mut entries, &source, &destination, keep);
                if updates.is_empty() {
                    return Err("The download record is no longer available.".to_owned());
                }
                (task.persist)(updates)?;
                if entries.iter().any(|entry| entry.stored.is_some()) {
                    entries =
                        crate::history::db::load_summaries().map_err(|error| error.to_string())?;
                }
                *task.history.lock().unwrap() = entries.clone();
                {
                    let mut state = task.backend.lock().unwrap();
                    for item in &mut state.candidates {
                        if same_path(&item.saved_path, &source) {
                            item.saved_path = destination.to_string_lossy().into_owned();
                            item.gif_output_path.clear();
                            item.status = "Saved".into();
                        }
                    }
                }
                Ok(entries)
            })
            .await;
            match saved {
                Ok(Ok(entries)) => {
                    completed += 1;
                    refresh(&context, entries);
                    if !keep && let Err(error) = tokio::fs::remove_file(&input).await {
                        errors.push(format!(
                            "GIF saved; could not remove {}: {error}",
                            input.display()
                        ));
                    }
                }
                result => {
                    // Preserve both files if persistence failed; never delete the source.
                    errors.push(match result {
                        Ok(Err(error)) => {
                            format!("GIF created but history could not be saved: {error}")
                        }
                        Err(error) => error.to_string(),
                        _ => unreachable!(),
                    });
                }
            }
        }
    }
    let stopped = context.cancel.load(Ordering::Relaxed);
    let _ = context.ui.upgrade_in_event_loop(move |app| {
        app.set_is_busy(false);
        app.set_archive_conversion_busy(false);
        if app.get_selected_tab() == crate::library::TAB {
            app.invoke_library_refresh();
        }
        app.set_progress(if stopped {
            completed as f32 / total as f32 * 100.0
        } else {
            100.0
        });
        let message = format!(
            "GIF conversion {} · {completed}/{total} converted · {} errors.",
            if stopped { "stopped" } else { "finished" },
            errors.len()
        );
        app.set_status(message.clone());
        app.set_progress_detail(errors.first().cloned().unwrap_or_default());
        if !errors.is_empty() {
            crate::platform::logging::error(errors.join("\n"));
        }
        crate::app::notifications::show_toast(
            &app,
            if let Some(error) = errors.first() {
                format!("{message}\n{error}")
            } else {
                message
            },
            if errors.is_empty() { "info" } else { "error" },
        );
        app.update_library(|data| data.feedback_active = false);
    });
}

pub fn register(
    app: &AppWindow,
    history: Arc<Mutex<Vec<HistoryEntry>>>,
    backend: Arc<Mutex<BackendState>>,
    filter: Arc<Mutex<(String, bool)>>,
    cancel: Arc<AtomicBool>,
) {
    register_with_persistence(
        app,
        history,
        backend,
        filter,
        cancel,
        history_db::save_batch,
    );
}

pub(crate) fn register_with_persistence(
    app: &AppWindow,
    history: Arc<Mutex<Vec<HistoryEntry>>>,
    backend: Arc<Mutex<BackendState>>,
    filter: Arc<Mutex<(String, bool)>>,
    cancel: Arc<AtomicBool>,
    persist: fn(Vec<HistoryEntry>) -> Result<(), String>,
) {
    let context = Context {
        ui: app.as_weak(),
        history,
        backend,
        filter,
        cancel,
        persist,
    };
    app.on_convert_archived_gifs(move |keep, quality, resolution| {
        let Some(app) = context.ui.upgrade() else {
            return;
        };
        if app.get_is_busy()
            || app.get_history_busy()
            || app.get_ffmpeg_busy()
            || app.get_xtractor_busy()
        {
            return;
        }
        let id = app.get_download_batch_id();
        let entries = context.history.lock().unwrap();
        let Some(entry) = entries
            .iter()
            .find(|entry| entry.id == id.as_str() && entry.download.is_some())
        else {
            return;
        };
        let mut seen = HashSet::new();
        let candidates =
            if app.get_selected_tab() == crate::library::TAB && app.get_library().view_ready {
                context.backend.lock().unwrap().candidates.clone()
            } else {
                entry.media.clone()
            };
        let paths = candidates
            .iter()
            .filter(|item| eligible(item))
            .filter(|item| {
                seen.insert(if cfg!(windows) {
                    item.saved_path.to_ascii_lowercase()
                } else {
                    item.saved_path.clone()
                })
            })
            .map(|item| PathBuf::from(&item.saved_path))
            .collect::<Vec<_>>();
        drop(entries);
        if paths.is_empty() {
            crate::app::notifications::show_toast(
                &app,
                "No downloaded GIF MP4 files need conversion in this batch.",
                "info",
            );
            return;
        }
        context.cancel.store(false, Ordering::Relaxed);
        app.set_selected_media_index(-1);
        if app.get_selected_tab() == crate::library::TAB {
            app.update_library(|data| {
                data.feedback_active = true;
                data.error.clear();
            });
        }
        app.set_is_busy(true);
        app.set_archive_conversion_busy(true);
        app.set_status("Preparing GIF conversion…".into());
        app.set_progress(0.0);
        app.set_progress_detail("".into());
        tokio::spawn(convert(
            context.clone(),
            paths,
            gif_conversion::Options::from_indices(quality, resolution),
            keep,
        ));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_x_gif_mp4s_are_eligible_and_all_references_follow_the_conversion() {
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let input = fixture.0.join("animation.mp4");
        let output = fixture.0.join("animation.gif");
        std::fs::write(&input, b"mp4").unwrap();
        std::fs::write(&output, b"animated GIF").unwrap();
        let item = MediaCandidate {
            media_type: "animated_gif".into(),
            saved_path: input.to_string_lossy().into_owned(),
            status: "Saved".into(),
            ..Default::default()
        };
        assert!(eligible(&item));
        let mut video = item.clone();
        video.media_type = "video".into();
        assert!(!eligible(&video));
        let entries = vec![
            HistoryEntry {
                id: "fetch".into(),
                media: vec![item.clone()],
                ..Default::default()
            },
            HistoryEntry {
                id: "download".into(),
                media: vec![item],
                download: Some(Default::default()),
                ..Default::default()
            },
        ];
        let mut keep = entries.clone();
        assert_eq!(update_entries(&mut keep, &input, &output, true).len(), 2);
        assert_eq!(keep[0].media.len(), 1);
        assert_eq!(keep[1].media.len(), 2);
        assert_eq!(keep[1].download.as_ref().unwrap().total_bytes, 15);
        assert!(!eligible(&keep[1].media[1]));
        let mut replace = entries;
        update_entries(&mut replace, &input, &output, false);
        assert_eq!(replace[1].media.len(), 1);
        assert_eq!(replace[1].download.as_ref().unwrap().total_bytes, 12);
        assert_eq!(replace[0].media[0].saved_path, output.to_string_lossy());
        assert!(input.exists());
    }
}
