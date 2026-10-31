use crate::AppWindow;
use crate::download::engine as downloader;
use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[derive(Default)]
struct Usage {
    free: Option<u64>,
    total: Option<u64>,
    used: u64,
    files: u64,
    unreadable: u64,
}

#[derive(Debug, PartialEq)]
struct Capacity {
    used: u64,
    other: u64,
    used_ratio: f32,
    downloads_ratio: f32,
}

impl Usage {
    fn drive_capacity(&self) -> Option<Capacity> {
        let total = self.total.filter(|total| *total > 0)?;
        let used = total.saturating_sub(self.free?);
        // Logical folder sizes can exceed occupied bytes for compressed/sparse
        // files or if drive capacity changed while scanning. Bound the bar to
        // actual capacity and keep the measured folder size in its own metric.
        let downloads = self.used.min(used);
        Some(Capacity {
            used,
            other: used - downloads,
            used_ratio: (used as f64 / total as f64) as f32,
            downloads_ratio: (downloads as f64 / total as f64) as f32,
        })
    }
}

fn inspect(root: &Path, cancel: &AtomicBool) -> Usage {
    let mut usage = Usage::default();
    if let Some(existing) = root.ancestors().find(|path| path.is_dir()) {
        usage.free = fs4::available_space(existing).ok();
        usage.total = fs4::total_space(existing).ok();
    }
    let mut folders = if root.is_dir() {
        vec![root.to_path_buf()]
    } else {
        Vec::new()
    };
    while let Some(folder) = folders.pop() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let Ok(entries) = std::fs::read_dir(folder) else {
            usage.unreadable += 1;
            continue;
        };
        for entry in entries {
            if cancel.load(Ordering::Relaxed) {
                return usage;
            }
            let Ok(entry) = entry else {
                usage.unreadable += 1;
                continue;
            };
            let Ok(kind) = entry.file_type() else {
                usage.unreadable += 1;
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                folders.push(entry.path());
            } else if kind.is_file() {
                let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
                if name.ends_with(".part") || name.ends_with(".tmp") {
                    continue;
                }
                match entry.metadata() {
                    Ok(data) => {
                        usage.used += data.len();
                        usage.files += 1;
                    }
                    Err(_) => usage.unreadable += 1,
                }
            }
        }
    }
    usage
}

pub fn register(app: &AppWindow) -> crate::ui::runtime::Timer {
    let active = Arc::new(Mutex::new((
        Arc::new(AtomicBool::new(false)),
        String::new(),
    )));
    let ui = app.as_weak();
    app.on_refresh_storage(move || {
        let Some(app) = ui.upgrade() else { return };
        if app.get_selected_tab() != 2 || app.get_settings_category() != 3 {
            return;
        }
        let path = app.get_output_dir().to_string();
        if path.is_empty() {
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut previous = active.lock().unwrap();
            previous.0.store(true, Ordering::Relaxed);
            if previous.1 != path {
                app.set_storage_capacity_known(false);
                app.set_storage_ready(false);
            }
            *previous = (cancel.clone(), path.clone());
        }
        app.set_storage_busy(true);
        let ui = ui.clone();
        tokio::task::spawn_blocking(move || {
            let usage = inspect(Path::new(&path), &cancel);
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            let _ = ui.upgrade_in_event_loop(move |app| {
                if cancel.load(Ordering::Relaxed) || app.get_output_dir().as_str() != path {
                    return;
                }
                app.set_storage_busy(false);
                app.set_storage_ready(true);
                let capacity = usage.drive_capacity();
                app.set_storage_capacity_known(capacity.is_some());
                app.set_storage_used_ratio(capacity.as_ref().map_or(0.0, |value| value.used_ratio));
                app.set_storage_downloads_ratio(
                    capacity.as_ref().map_or(0.0, |value| value.downloads_ratio),
                );
                app.set_storage_other_used(
                    capacity
                        .map(|value| downloader::format_bytes(value.other))
                        .unwrap_or_else(|| "—".into()),
                );
                app.set_storage_free(
                    usage
                        .free
                        .map(downloader::format_bytes)
                        .unwrap_or_else(|| "—".into()),
                );
                app.set_storage_total(
                    usage
                        .total
                        .map(downloader::format_bytes)
                        .unwrap_or_else(|| "—".into()),
                );
                app.set_storage_used(downloader::format_bytes(usage.used));
                app.set_storage_files(usage.files.min(i32::MAX as u64) as i32);
                app.set_storage_partial(usage.unreadable > 0);
            });
        });
    });
    let timer = crate::ui::runtime::Timer::default();
    let ui = app.as_weak();
    timer.start(
        crate::ui::runtime::TimerMode::Repeated,
        Duration::from_secs(10),
        move || {
            if let Some(app) = ui.upgrade()
                && !app.get_storage_busy()
                && app.get_selected_tab() == 2
                && app.get_settings_category() == 3
            {
                app.invoke_refresh_storage();
            }
        },
    );
    timer
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capacity_uses_drive_bytes_and_handles_missing_or_invalid_totals() {
        let mut usage = Usage {
            total: Some(1000),
            free: Some(250),
            used: 12,
            ..Usage::default()
        };
        assert_eq!(
            usage.drive_capacity(),
            Some(Capacity {
                used: 750,
                other: 738,
                used_ratio: 0.75,
                downloads_ratio: 0.012
            })
        );
        usage.free = Some(0);
        assert_eq!(
            usage.drive_capacity(),
            Some(Capacity {
                used: 1000,
                other: 988,
                used_ratio: 1.0,
                downloads_ratio: 0.012
            })
        );
        usage.free = Some(1001);
        assert_eq!(
            usage.drive_capacity(),
            Some(Capacity {
                used: 0,
                other: 0,
                used_ratio: 0.0,
                downloads_ratio: 0.0
            })
        );
        usage.free = None;
        assert_eq!(usage.drive_capacity(), None);
        usage.free = Some(250);
        usage.total = Some(0);
        assert_eq!(usage.drive_capacity(), None);
        usage.total = None;
        assert_eq!(usage.drive_capacity(), None);
    }

    #[test]
    fn partition_counts_downloads_once_and_bounds_large_or_empty_folders() {
        let mut usage = Usage {
            total: Some(1000),
            free: Some(250),
            used: 300,
            ..Default::default()
        };
        let capacity = usage.drive_capacity().unwrap();
        assert_eq!((capacity.used, capacity.other), (750, 450));
        assert_eq!(capacity.downloads_ratio, 0.3);
        assert_eq!(capacity.used_ratio, 0.75);
        assert_eq!(
            300 + capacity.other + usage.free.unwrap(),
            usage.total.unwrap()
        );
        usage.used = 1500;
        let capacity = usage.drive_capacity().unwrap();
        assert_eq!((capacity.downloads_ratio, capacity.other), (0.75, 0));
        assert_eq!(usage.used, 1500);
        usage.used = 0;
        let capacity = usage.drive_capacity().unwrap();
        assert_eq!((capacity.downloads_ratio, capacity.other), (0.0, 750));
    }

    #[test]
    fn usage_reads_nested_files_and_excludes_incomplete_transfers() {
        let fixture = crate::download::saved_files::tests::Fixture::new();
        std::fs::create_dir(fixture.0.join("demo")).unwrap();
        std::fs::write(fixture.0.join("avatar.jpg"), b"12345").unwrap();
        std::fs::write(fixture.0.join("demo/video.mp4"), b"123456789").unwrap();
        std::fs::write(fixture.0.join("demo/incomplete.mp4.part"), b"ignored").unwrap();
        let usage = inspect(&fixture.0, &AtomicBool::new(false));
        assert_eq!((usage.files, usage.used), (2, 14));
        assert!(usage.free.is_some_and(|free| free <= usage.total.unwrap()));
        assert_eq!(usage.unreadable, 0);
        std::fs::remove_file(fixture.0.join("demo/video.mp4")).unwrap();
        assert_eq!(inspect(&fixture.0, &AtomicBool::new(false)).used, 5);
        let missing = inspect(&fixture.0.join("not-created"), &AtomicBool::new(false));
        assert_eq!(missing.files, 0);
        assert!(missing.free.is_some());
    }
}
