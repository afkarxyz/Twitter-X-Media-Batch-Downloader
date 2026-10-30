//! Runtime data belongs to the executable, including after the app is moved.
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use rusqlite::{
    Connection, OpenFlags,
    backup::{Backup, StepResult},
};

pub const APP_DIR: &str = ".twitterxmediabatchdownloader";
pub const XTRACTOR_BINARY: &str = if cfg!(windows) {
    "xtractor.exe"
} else {
    "xtractor"
};
const MIGRATION_MARKER: &str = ".portable-migration-v1";

/// Folder name for a macOS app installed in Applications, under Application Support and Downloads.
const INSTALLED_NAME: &str = "TwitterXMediaBatchDownloader";

pub fn app_data_dir() -> Option<PathBuf> {
    portable_dir_for(&std::env::current_exe().ok()?)
}

/// Where the app keeps its data and, unless the user picks another folder, its downloads.
struct Layout {
    /// Relative output folders from older settings resolve against this folder.
    root: PathBuf,
    data: PathBuf,
    downloads: PathBuf,
}

fn current_layout(executable: &Path) -> Option<Layout> {
    layout_for(
        executable,
        cfg!(target_os = "macos"),
        appimage_for(executable).as_deref(),
        dirs::home_dir().as_deref(),
    )
}

/// Data and downloads normally sit beside the app so the folder can be moved as a whole.
/// An AppImage runs its executable from a temporary read-only mount and a macOS executable
/// lives in `Name.app/Contents/MacOS`, so the `.AppImage` file or the bundle anchors them
/// instead. A macOS app installed in Applications uses the user's Library and Downloads.
fn layout_for(
    executable: &Path,
    bundles: bool,
    appimage: Option<&Path>,
    home: Option<&Path>,
) -> Option<Layout> {
    let beside = |root: &Path| Layout {
        root: root.to_path_buf(),
        data: root.join(APP_DIR),
        downloads: root.join("Downloads"),
    };
    if let Some(image) = appimage {
        return Some(beside(image.parent()?));
    }
    let directory = executable.parent()?;
    let Some(folder) = bundles.then(|| bundle_folder(directory)).flatten() else {
        return Some(beside(directory));
    };
    if let Some(home) = home
        && (folder.starts_with("/Applications") || folder.starts_with(home.join("Applications")))
    {
        return Some(Layout {
            root: home.to_path_buf(),
            data: home
                .join("Library/Application Support")
                .join(INSTALLED_NAME),
            downloads: home.join("Downloads").join(INSTALLED_NAME),
        });
    }
    Some(beside(folder))
}

/// The folder holding `Name.app` when `directory` is its `Contents/MacOS`.
fn bundle_folder(directory: &Path) -> Option<&Path> {
    if directory.file_name() == Some("MacOS".as_ref())
        && let Some(contents) = directory.parent()
        && contents.file_name() == Some("Contents".as_ref())
        && let Some(bundle) = contents.parent()
        && bundle
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
    {
        return bundle.parent();
    }
    None
}

/// The `.AppImage` file this process was started from, if any.
pub fn appimage() -> Option<PathBuf> {
    appimage_for(&std::env::current_exe().ok()?)
}

fn appimage_for(executable: &Path) -> Option<PathBuf> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    // Child processes inherit these variables, so require the executable to be in the mount.
    let mount = PathBuf::from(std::env::var_os("APPDIR")?);
    let image = PathBuf::from(std::env::var_os("APPIMAGE")?);
    executable.starts_with(&mount).then_some(image)
}

fn portable_dir_for(executable: &Path) -> Option<PathBuf> {
    current_layout(executable).map(|layout| layout.data)
}

pub fn output_dir(configured: Option<&Path>) -> io::Result<PathBuf> {
    output_dir_for(&std::env::current_exe()?, configured)
}

fn output_dir_for(executable: &Path, configured: Option<&Path>) -> io::Result<PathBuf> {
    let layout = current_layout(executable)
        .ok_or_else(|| io::Error::other("Could not locate the executable directory"))?;
    let configured = configured.filter(|path| !path.as_os_str().is_empty());
    let output = match configured {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => layout.root.join(path),
        None => layout.downloads.clone(),
    };
    if configured.is_none()
        || stored_output_dir_for(Some(&layout.downloads), output.clone()).is_none()
    {
        fs::create_dir_all(&output)?;
    }
    Ok(output)
}

/// Leave the default unset in config so moving the app moves its download root.
pub fn stored_output_dir(path: PathBuf) -> Option<PathBuf> {
    let default = std::env::current_exe()
        .ok()
        .and_then(|exe| current_layout(&exe))
        .map(|layout| layout.downloads);
    stored_output_dir_for(default.as_deref(), path)
}

fn stored_output_dir_for(default: Option<&Path>, path: PathBuf) -> Option<PathBuf> {
    if path.as_os_str().is_empty() {
        return None;
    }
    let is_default = default.is_some_and(|default| {
        path == default
            || (cfg!(windows)
                && path
                    .to_string_lossy()
                    .replace('/', "\\")
                    .eq_ignore_ascii_case(&default.to_string_lossy().replace('/', "\\")))
    });
    (!is_default).then_some(path)
}

pub fn prepare() -> io::Result<PathBuf> {
    let directory =
        app_data_dir().ok_or_else(|| io::Error::other("Could not locate the executable"))?;
    prepare_directory(&directory)?;
    Ok(directory)
}

fn prepare_directory(directory: &Path) -> io::Result<()> {
    // A missing migration marker can mean the user intentionally reset the app.
    // Normal startup must never restore data from another installation.
    fs::create_dir_all(directory)
}

pub fn migrate_legacy_data() -> io::Result<PathBuf> {
    let directory = prepare()?;
    let legacy_home = dirs::home_dir().map(|directory| directory.join(APP_DIR));
    let legacy_local = dirs::data_local_dir()
        .or_else(dirs::config_dir)
        .map(|directory| directory.join(crate::APP_ID));
    migrate(&directory, legacy_home.as_deref(), legacy_local.as_deref())?;
    Ok(directory)
}

fn migrate(
    directory: &Path,
    legacy_home: Option<&Path>,
    legacy_local: Option<&Path>,
) -> io::Result<()> {
    fs::create_dir_all(directory)?;
    if directory.join(MIGRATION_MARKER).is_file() {
        return Ok(());
    }
    // Existing portable files always win. Keep the old folder as a backup.
    if let Some(source) = legacy_home.filter(|source| *source != directory && source.is_dir()) {
        copy_missing(source, directory)?;
    }
    if let Some(source) = legacy_local {
        for (old, new) in [
            ("settings.json", "config.json"),
            ("history.json", "history.json"),
        ] {
            let old = source.join(old);
            if old.is_file() {
                copy_file_missing(&old, &directory.join(new))?;
            }
        }
    }
    fs::write(
        directory.join(MIGRATION_MARKER),
        b"Legacy data imported. Runtime data stays beside the executable.\n",
    )
}

fn copy_missing(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let name = entry.file_name();
        let name_string = name.to_string_lossy();
        if name_string == MIGRATION_MARKER
            || name_string == "previews"
            || ["-wal", "-shm", "-journal", ".part", ".tmp"]
                .iter()
                .any(|suffix| name_string.ends_with(suffix))
        {
            continue;
        }
        let target = destination.join(&name);
        if kind.is_dir() && !target.is_file() {
            copy_missing(&entry.path(), &target)?;
        } else if kind.is_file() {
            copy_file_missing(&entry.path(), &target)?;
        }
    }
    Ok(())
}

fn copy_file_missing(source: &Path, destination: &Path) -> io::Result<()> {
    if destination.exists() {
        return Ok(());
    }
    let temporary = destination.with_extension(format!("portable-import-{}", std::process::id()));
    let result = (|| {
        if source
            .extension()
            .is_some_and(|extension| extension == "db" || extension == "sqlite3")
        {
            // Include committed WAL data, even while an older app is still open.
            let source = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(io::Error::other)?;
            let mut target = Connection::open(&temporary).map_err(io::Error::other)?;
            {
                let backup = Backup::new(&source, &mut target).map_err(io::Error::other)?;
                let deadline = Instant::now() + Duration::from_secs(20);
                loop {
                    match backup.step(256).map_err(io::Error::other)? {
                        StepResult::Done => break,
                        _ if Instant::now() >= deadline => {
                            return Err(io::Error::new(
                                io::ErrorKind::TimedOut,
                                "Legacy database stayed busy during portable migration",
                            ));
                        }
                        StepResult::More => (),
                        _ => std::thread::sleep(Duration::from_millis(10)),
                    }
                }
            }
            // A standalone snapshot must not depend on its own WAL sidecars.
            target
                .pragma_update(None, "journal_mode", "DELETE")
                .map_err(io::Error::other)?;
            target
                .close()
                .map_err(|(_, error)| io::Error::other(error))?;
        } else {
            fs::copy(source, &temporary)?;
        }
        if destination.exists() {
            fs::remove_file(&temporary)?;
        } else {
            fs::rename(&temporary, destination)?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT_FIXTURE: std::sync::atomic::AtomicUsize =
                std::sync::atomic::AtomicUsize::new(0);
            let name = format!(
                "twitterdl-portable-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT_FIXTURE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            );
            let root = std::env::temp_dir().join(name);
            fs::create_dir_all(&root).unwrap();
            Self(root)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            if self.0.is_absolute()
                && self.0.parent() == Some(std::env::temp_dir().as_path())
                && self
                    .0
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("twitterdl-portable-"))
            {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
    }

    #[test]
    fn runtime_directory_follows_the_executable() {
        let fixture = Fixture::new();
        let original = portable_dir_for(&fixture.0.join("original/app.exe")).unwrap();
        let moved = portable_dir_for(&fixture.0.join("moved/app.exe")).unwrap();
        assert_eq!(original, fixture.0.join("original").join(APP_DIR));
        assert_eq!(moved, fixture.0.join("moved").join(APP_DIR));
        assert_ne!(original, moved);
    }

    #[test]
    fn macos_bundles_keep_portable_data_beside_the_app() {
        const BUNDLED: &str =
            "TwitterXMediaBatchDownloader.app/Contents/MacOS/TwitterXMediaBatchDownloader";
        let home = Path::new("/Users/demo");
        let folder = home.join("Documents/Portable");
        let bundled = folder.join(BUNDLED);
        let layout = layout_for(&bundled, true, None, Some(home)).unwrap();
        assert_eq!(layout.data, folder.join(APP_DIR));
        assert_eq!(layout.downloads, folder.join("Downloads"));
        assert_eq!(
            layout_for(&bundled, false, None, Some(home)).unwrap().data,
            bundled.parent().unwrap().join(APP_DIR),
            "Other platforms keep data beside the executable"
        );
        let loose = folder.join("Contents/MacOS/app");
        assert_eq!(
            layout_for(&loose, true, None, Some(home)).unwrap().data,
            loose.parent().unwrap().join(APP_DIR)
        );
        for installed in [Path::new("/Applications"), &home.join("Applications")] {
            let layout = layout_for(&installed.join(BUNDLED), true, None, Some(home)).unwrap();
            assert_eq!(
                layout.data,
                home.join("Library/Application Support/TwitterXMediaBatchDownloader")
            );
            assert_eq!(
                layout.downloads,
                home.join("Downloads/TwitterXMediaBatchDownloader")
            );
        }
    }

    #[test]
    fn appimages_keep_portable_data_beside_the_image_file() {
        let mounted = Path::new("/tmp/.mount_TwitteAbc123/usr/bin/TwitterXMediaBatchDownloader");
        let home = Path::new("/home/demo");
        // ~/Applications is a common AppImage folder and stays portable on Linux.
        for folder in [home.join("Apps"), home.join("Applications")] {
            let image = folder.join("TwitterXMediaBatchDownloader.AppImage");
            let layout = layout_for(mounted, false, Some(&image), Some(home)).unwrap();
            assert_eq!(layout.data, folder.join(APP_DIR));
            assert_eq!(layout.downloads, folder.join("Downloads"));
        }
    }

    #[test]
    fn download_default_is_created_beside_exe_and_follows_relocation_after_save() {
        let fixture = Fixture::new();
        let original_exe = fixture.0.join("original/app.exe");
        let moved_exe = fixture.0.join("moved/app.exe");
        let original = output_dir_for(&original_exe, None).unwrap();
        assert!(original.is_dir());
        let default = original_exe.parent().unwrap().join("Downloads");
        let stored = stored_output_dir_for(Some(&default), original.clone());
        assert!(stored.is_none());
        let moved = output_dir_for(&moved_exe, stored.as_deref()).unwrap();
        assert_eq!(moved, fixture.0.join("moved/Downloads"));
        assert!(moved.is_dir());
        assert_ne!(original, moved);
        let custom = fixture.0.join("custom");
        let stored = stored_output_dir_for(Some(&default), custom.clone());
        assert_eq!(
            output_dir_for(&moved_exe, stored.as_deref()).unwrap(),
            custom
        );
        assert!(!custom.exists());
        assert_eq!(
            output_dir_for(&moved_exe, Some(Path::new("Downloads"))).unwrap(),
            moved
        );
        let relative_default = output_dir_for(
            &fixture.0.join("relative/app.exe"),
            Some(Path::new("Downloads")),
        )
        .unwrap();
        assert!(relative_default.is_dir());
    }

    #[test]
    fn startup_with_only_config_and_extractor_does_not_restore_deleted_data() {
        let fixture = Fixture::new();
        let portable = fixture.0.join(APP_DIR);
        fs::create_dir_all(&portable).unwrap();
        fs::write(portable.join("config.json"), b"portable settings").unwrap();
        fs::write(portable.join("xtractor.exe"), b"portable extractor").unwrap();
        // Match a portable folder after the user removed the old data and marker.
        prepare_directory(&portable).unwrap();
        prepare_directory(&portable).unwrap();
        let mut files = fs::read_dir(&portable)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        files.sort();
        assert_eq!(
            files,
            ["config.json", "xtractor.exe"].map(std::ffi::OsString::from)
        );
        assert_eq!(
            fs::read(portable.join("config.json")).unwrap(),
            b"portable settings"
        );
        assert_eq!(
            fs::read(portable.join("xtractor.exe")).unwrap(),
            b"portable extractor"
        );
    }

    #[test]
    fn normal_startup_does_not_reimport_after_an_explicit_migration_is_reset() {
        let fixture = Fixture::new();
        let old = fixture.0.join("old");
        let portable = fixture.0.join(APP_DIR);
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("app.log"), b"old log").unwrap();
        fs::write(old.join("ffmpeg.exe"), b"old tool").unwrap();
        migrate(&portable, Some(&old), None).unwrap();
        fs::remove_file(portable.join("app.log")).unwrap();
        fs::remove_file(portable.join("ffmpeg.exe")).unwrap();
        fs::remove_file(portable.join(MIGRATION_MARKER)).unwrap();
        prepare_directory(&portable).unwrap();
        assert_eq!(fs::read_dir(&portable).unwrap().count(), 0);
        assert!(old.join("app.log").is_file() && old.join("ffmpeg.exe").is_file());
    }

    #[test]
    fn migration_preserves_portable_files_and_snapshots_live_wal_once() {
        let fixture = Fixture::new();
        let old = fixture.0.join("old");
        let portable = fixture.0.join(APP_DIR);
        fs::create_dir_all(old.join("previews")).unwrap();
        fs::create_dir_all(&portable).unwrap();
        fs::write(old.join("config.json"), b"old settings").unwrap();
        fs::write(portable.join("config.json"), b"portable settings").unwrap();
        fs::write(old.join("previews/thumb.png"), b"thumbnail").unwrap();
        fs::write(old.join("previews/incomplete.png.part"), b"unfinished").unwrap();
        let source = Connection::open(old.join("history.sqlite3")).unwrap();
        source.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; PRAGMA user_version=1; CREATE TABLE history_entries(id INTEGER); INSERT INTO history_entries VALUES(42);").unwrap();
        assert!(old.join("history.sqlite3-wal").is_file());
        let old_bytes = fs::read(old.join("history.sqlite3")).unwrap();
        migrate(&portable, Some(&old), None).unwrap();
        assert_eq!(
            fs::read(portable.join("config.json")).unwrap(),
            b"portable settings"
        );
        assert!(!portable.join("previews").exists());
        assert!(!portable.join("previews/incomplete.png.part").exists());
        assert!(!portable.join("history.sqlite3-wal").exists());
        assert_eq!(fs::read(old.join("history.sqlite3")).unwrap(), old_bytes);
        let target = Connection::open(portable.join("history.sqlite3")).unwrap();
        assert_eq!(
            target
                .query_row("SELECT id FROM history_entries", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            42
        );
        assert_eq!(
            target
                .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
        target.execute("DELETE FROM history_entries", []).unwrap();
        migrate(&portable, Some(&old), None).unwrap();
        assert_eq!(
            target
                .query_row("SELECT COUNT(*) FROM history_entries", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn imports_legacy_json_without_removing_originals() {
        let fixture = Fixture::new();
        let old = fixture.0.join("local");
        let portable = fixture.0.join(APP_DIR);
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("settings.json"), b"{}").unwrap();
        fs::write(old.join("history.json"), b"[]").unwrap();
        migrate(&portable, None, Some(&old)).unwrap();
        assert_eq!(fs::read(portable.join("config.json")).unwrap(), b"{}");
        assert_eq!(fs::read(portable.join("history.json")).unwrap(), b"[]");
        assert!(old.join("settings.json").exists() && old.join("history.json").exists());
    }
}
