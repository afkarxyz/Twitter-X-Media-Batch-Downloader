//! Read portable session logs without blocking the GUI or loading unbounded text.
use crate::AppWindow;
use crate::platform::logging as session_log;
use crate::ui::runtime::{Model, ModelRc, VecModel};
use copypasta::{ClipboardContext, ClipboardProvider};
use std::{
    fs::{self, File},
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

const VIEW_BYTES: u64 = 256 * 1024;

fn sessions(directory: &Path) -> io::Result<Vec<String>> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut rows = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if session_log::managed_name(&name) && metadata.file_type().is_file() {
            rows.push((metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH), name));
        }
    }
    rows.sort_by(|a, b| b.cmp(a));
    Ok(rows.into_iter().map(|(_, name)| name).collect())
}

fn session_path(directory: &Path, name: &str) -> io::Result<PathBuf> {
    if Path::new(name).components().count() != 1 || !session_log::managed_name(name) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid session log name",
        ));
    }
    let path = directory.join(name);
    if !fs::symlink_metadata(&path)?.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Session log is not a regular file",
        ));
    }
    Ok(path)
}

fn read_log(directory: &Path, name: &str) -> io::Result<(String, bool)> {
    let mut file = File::open(session_path(directory, name)?)?;
    let length = file.metadata()?.len();
    let offset = length.saturating_sub(VIEW_BYTES);
    file.seek(SeekFrom::Start(offset))?;
    let mut bytes = Vec::new();
    // Bound the read even while another process is appending to the session.
    file.take(VIEW_BYTES).read_to_end(&mut bytes)?;
    if offset > 0 {
        // Drop the first partial line, including any truncated UTF-8 character.
        if let Some(end) = bytes.iter().position(|byte| *byte == b'\n') {
            bytes.drain(..=end);
        } else {
            let first = (0..bytes.len())
                .find(|index| bytes[*index] & 0xc0 != 0x80)
                .unwrap_or(bytes.len());
            bytes.drain(..first);
        }
    }
    Ok((String::from_utf8_lossy(&bytes).into_owned(), offset > 0))
}

fn export_to(directory: &Path, name: &str, destination: &Path) -> io::Result<()> {
    let source = session_path(directory, name)?;
    // Never overwrite the original log, even through another path spelling.
    if destination.exists() && fs::canonicalize(&source)? == fs::canonicalize(destination)? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Choose a destination outside the original session log",
        ));
    }
    fs::copy(source, destination).map(|_| ())
}

fn export_errors_to(directory: &Path, name: &str, destination: &Path) -> io::Result<()> {
    let source = session_path(directory, name)?;
    if destination.exists() && fs::canonicalize(&source)? == fs::canonicalize(destination)? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Choose a destination outside the original session log",
        ));
    }
    let reader = BufReader::new(File::open(source)?.take(50 * 1024 * 1024));
    let mut output = File::create(destination)?;
    for line in reader.lines() {
        let line = line?;
        let severity = line.strip_prefix("[ERROR] ").is_some()
            || line
                .split_once("] ")
                .is_some_and(|(_, tail)| tail.starts_with("[ERROR] "));
        if severity {
            writeln!(output, "{line}")?;
        }
    }
    output.flush()
}

fn selected_name(app: &AppWindow) -> Option<String> {
    let index = usize::try_from(app.get_log_session_index()).ok()?;
    app.get_log_sessions()
        .row_data(index)
        .map(|name| name.to_string())
}

fn load(app: &AppWindow, directory: PathBuf, name: Option<String>, enumerate: bool, quiet: bool) {
    if app.get_logs_busy() || (quiet && app.get_logs_polling()) {
        return;
    }
    if quiet {
        app.set_logs_polling(true);
    }
    if !quiet {
        app.set_logs_generation(app.get_logs_generation().wrapping_add(1));
        app.set_logs_busy(true);
        app.set_logs_error("".into());
        app.set_log_text("".into());
        app.set_log_truncated(false);
    }
    let generation = app.get_logs_generation();
    let expected_name = name.clone();
    let ui = app.as_weak();
    tokio::spawn(async move {
        let result = tokio::task::spawn_blocking(move || -> io::Result<_> {
            let names = if enumerate {
                Some(sessions(&directory)?)
            } else {
                None
            };
            let selected = if let Some(names) = &names {
                name.filter(|name| names.contains(name))
                    .or_else(|| {
                        session_log::active_name()
                            .filter(|name| names.iter().any(|row| row == name))
                            .map(str::to_owned)
                    })
                    .or_else(|| names.first().cloned())
            } else {
                name
            };
            let (text, truncated) = match &selected {
                Some(name) => read_log(&directory, name)?,
                None => (String::new(), false),
            };
            Ok((names, selected, text, truncated))
        })
        .await;
        let _ = ui.upgrade_in_event_loop(move |app| {
            if quiet {
                app.set_logs_polling(false);
            }
            if app.get_logs_generation() != generation
                || (quiet && selected_name(&app) != expected_name)
            {
                return;
            }
            if !quiet {
                app.set_logs_busy(false);
            }
            match result {
                Ok(Ok((names, selected, text, truncated))) => {
                    if let Some(names) = names {
                        let index = selected
                            .as_ref()
                            .and_then(|name| names.iter().position(|row| row == name))
                            .map(|index| index as i32)
                            .unwrap_or(-1);
                        app.set_log_sessions(ModelRc::new(VecModel::from(
                            names
                                .into_iter()
                                .collect::<Vec<crate::ui::runtime::SharedString>>(),
                        )));
                        app.set_log_session_index(index);
                    }
                    if app.get_log_text().as_str() != text {
                        app.set_log_text(text);
                    }
                    app.set_log_truncated(truncated);
                    app.set_logs_error("".into());
                }
                Ok(Err(error)) => {
                    app.set_logs_error(format!("Could not read session logs: {error}"))
                }
                Err(error) => app.set_logs_error(format!("Could not load session logs: {error}")),
            }
        });
    });
}

pub fn register(app: &AppWindow, directory: PathBuf) -> crate::ui::runtime::Timer {
    let ui = app.as_weak();
    let folder = directory.clone();
    app.on_view_logs(move || {
        let Some(app) = ui.upgrade() else { return };
        app.set_log_viewer_open(true);
        load(&app, folder.clone(), selected_name(&app), true, false);
    });
    let ui = app.as_weak();
    let folder = directory.clone();
    app.on_refresh_logs(move || {
        if let Some(app) = ui.upgrade() {
            load(&app, folder.clone(), selected_name(&app), true, false);
        }
    });
    let ui = app.as_weak();
    let folder = directory.clone();
    app.on_load_log(move |index| {
        let Some(app) = ui.upgrade() else { return };
        if app.get_logs_busy() || index < 0 || index as usize >= app.get_log_sessions().row_count()
        {
            return;
        }
        app.set_log_session_index(index);
        load(&app, folder.clone(), selected_name(&app), false, false);
    });
    let ui = app.as_weak();
    app.on_copy_log(move || {
        let Some(app) = ui.upgrade() else { return };
        if app.get_logs_busy() || app.get_log_text().is_empty() {
            return;
        }
        let copied = ClipboardContext::new()
            .and_then(|mut clipboard| clipboard.set_contents(app.get_log_text().to_string()))
            .is_ok();
        if copied {
            app.set_logs_error("".into());
            crate::app::notifications::show_toast(&app, "Displayed log copied.", "info");
        } else {
            app.set_logs_error("Could not copy the displayed log.".into());
        }
    });
    register_export(app, directory.clone(), false);
    register_export(app, directory.clone(), true);
    let timer = crate::ui::runtime::Timer::default();
    let ui = app.as_weak();
    timer.start(
        crate::ui::runtime::TimerMode::Repeated,
        Duration::from_secs(1),
        move || {
            if let Some(app) = ui.upgrade()
                && app.get_log_viewer_open()
                && app.get_logs_live()
                && !app.get_logs_busy()
            {
                load(&app, directory.clone(), selected_name(&app), false, true);
            }
        },
    );
    timer
}

fn register_export(app: &AppWindow, directory: PathBuf, errors_only: bool) {
    let ui = app.as_weak();
    let callback = move || {
        let Some(app) = ui.upgrade() else { return };
        if app.get_logs_busy() {
            return;
        }
        let Some(name) = selected_name(&app) else {
            return;
        };
        app.set_logs_busy(true);
        app.set_logs_error("".into());
        let folder = directory.clone();
        let ui = ui.clone();
        tokio::spawn(async move {
            let target = rfd::AsyncFileDialog::new()
                .set_file_name(if errors_only {
                    format!("{}-errors.log", name.trim_end_matches(".log"))
                } else {
                    name.clone()
                })
                .add_filter("Log", &["log"])
                .save_file()
                .await;
            let result = if let Some(target) = target {
                tokio::task::spawn_blocking(move || {
                    if errors_only {
                        export_errors_to(&folder, &name, target.path()).map(|_| true)
                    } else {
                        export_to(&folder, &name, target.path()).map(|_| true)
                    }
                })
                .await
                .map_err(|error| io::Error::other(error.to_string()))
                .and_then(|result| result)
            } else {
                Ok(false)
            };
            let _ = ui.upgrade_in_event_loop(move |app| {
                app.set_logs_busy(false);
                match result {
                    Ok(true) => {
                        crate::app::notifications::show_toast(&app, "Session log exported.", "info")
                    }
                    Ok(false) => {}
                    Err(error) => {
                        app.set_logs_error(format!("Could not export session log: {error}"))
                    }
                }
            });
        });
    };
    if errors_only {
        app.on_export_log_errors(callback);
    } else {
        app.on_export_log(callback);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn error_export_filters_severity_and_preserves_original() {
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let name = "app-2026-10-01_00-00-00-1.log";
        let content = "[2026-10-01 12:00:00 +07:00] [INFO] Payload contains [ERROR] but is info.\n[2026-10-01 12:00:01 +07:00] [ERROR] First error café.\n[ERROR] Second error.\n[WARN] Warning.\n";
        fs::write(fixture.0.join(name), content).unwrap();
        let output = fixture.0.join("errors.log");
        export_errors_to(&fixture.0, name, &output).unwrap();
        assert_eq!(
            fs::read_to_string(output).unwrap(),
            "[2026-10-01 12:00:01 +07:00] [ERROR] First error café.\n[ERROR] Second error.\n"
        );
        assert!(export_errors_to(&fixture.0, name, &fixture.0.join(name)).is_err());
        assert_eq!(fs::read_to_string(fixture.0.join(name)).unwrap(), content);
    }
    #[test]
    fn session_browser_orders_logs_and_rejects_unrelated_paths() {
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let directory = &fixture.0;
        let first = "app-2026-10-01_00-00-00-1.log";
        let second = "app-2026-10-01_00-00-01-2.log";
        fs::write(directory.join(first), "[INFO] first\n").unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(directory.join(first))
            .unwrap()
            .set_modified(SystemTime::UNIX_EPOCH)
            .unwrap();
        fs::write(directory.join(second), "[ERROR] second\n").unwrap();
        fs::write(directory.join("config.json"), "unrelated").unwrap();
        fs::create_dir(directory.join("app-2026-10-01_00-00-02-3.log")).unwrap();
        assert_eq!(sessions(directory).unwrap(), vec![second, first]);
        assert_eq!(
            read_log(directory, second).unwrap(),
            ("[ERROR] second\n".into(), false)
        );
        for invalid in [
            "../config.json",
            "config.json",
            "app-2026-10-01_00-00-02-3.log",
        ] {
            assert!(read_log(directory, invalid).is_err());
        }
        fs::remove_file(directory.join(second)).unwrap();
        assert!(read_log(directory, second).is_err());
        assert!(sessions(&directory.join("missing")).unwrap().is_empty());
    }
    #[test]
    fn large_logs_show_only_the_tail_at_complete_unicode_lines() {
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let name = "app-2026-10-01_00-00-00-1.log";
        let text = format!(
            "{}[ERROR] final café 日本\n",
            "[INFO] café 日本\n".repeat(40_000)
        );
        fs::write(fixture.0.join(name), &text).unwrap();
        let (tail, truncated) = read_log(&fixture.0, name).unwrap();
        assert!(truncated && tail.len() <= VIEW_BYTES as usize);
        assert!(tail.starts_with("[INFO]"));
        assert!(tail.ends_with("[ERROR] final café 日本\n"));
        assert!(!tail.contains('\u{fffd}'));
        let destination = fixture.0.join("export.log");
        export_to(&fixture.0, name, &destination).unwrap();
        assert_eq!(fs::read_to_string(destination).unwrap(), text);
        assert!(export_to(&fixture.0, name, &fixture.0.join(name)).is_err());
        assert_eq!(fs::read_to_string(fixture.0.join(name)).unwrap(), text);
    }
}
