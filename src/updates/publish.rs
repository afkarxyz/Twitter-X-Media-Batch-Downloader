//! Validated portable files/directories replace existing tools with a recoverable rollback.
use std::{
    fs, io,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

const ERROR_SHARING_VIOLATION: i32 = 32;

/// Antivirus scanners hold freshly unpacked files for a few seconds, and Windows
/// refuses to rename their folder meanwhile. Retry until the files are released.
pub async fn replace_when_released(
    staging: &Path,
    target: &Path,
    cancel: &AtomicBool,
) -> io::Result<()> {
    let started = Instant::now();
    loop {
        match replace(staging, target) {
            Err(error)
                if (error.kind() == io::ErrorKind::PermissionDenied
                    || error.raw_os_error() == Some(ERROR_SHARING_VIOLATION))
                    && started.elapsed() < Duration::from_secs(60)
                    && !cancel.load(Ordering::Relaxed) =>
            {
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            result => return result,
        }
    }
}

pub fn replace(staging: &Path, target: &Path) -> io::Result<()> {
    let root = target
        .parent()
        .ok_or_else(|| io::Error::other("Missing tool directory"))?
        .canonicalize()?;
    if staging
        .parent()
        .ok_or_else(|| io::Error::other("Missing staging directory"))?
        .canonicalize()?
        != root
    {
        return Err(io::Error::other(
            "Tool staging must be inside the destination directory",
        ));
    }
    if fs::symlink_metadata(staging)?.file_type().is_symlink()
        || fs::symlink_metadata(target).is_ok_and(|data| data.file_type().is_symlink())
    {
        return Err(io::Error::other(
            "Tool publication does not replace symlinks",
        ));
    }
    let backup = root.join(format!(
        ".previous-{}-{}",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    if backup.exists() {
        return Err(io::Error::other("Tool backup already exists"));
    }
    let had_previous = target.exists();
    if had_previous {
        fs::rename(target, &backup)?;
    }
    if let Err(error) = fs::rename(staging, target) {
        if had_previous {
            fs::rename(&backup, target).map_err(|restore| {
                io::Error::other(format!("{error}; rollback failed: {restore}"))
            })?;
        }
        return Err(error);
    }
    if had_previous {
        // backup is an absolute child of the checked canonical portable directory.
        let result = if backup.is_dir() {
            fs::remove_dir_all(&backup)
        } else {
            fs::remove_file(&backup)
        };
        if let Err(error) = result {
            crate::platform::logging::warn(format!("Previous tool backup retained: {error}"));
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_publication_restores_the_previous_tool() {
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let target = fixture.0.join("tool");
        let staging = fixture.0.join("missing");
        fs::write(&target, b"previous").unwrap();
        assert!(replace(&staging, &target).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"previous");
        fs::write(&staging, b"new").unwrap();
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            let held = fs::OpenOptions::new()
                .read(true)
                .share_mode(1)
                .open(&staging)
                .unwrap();
            assert!(replace(&staging, &target).is_err());
            assert_eq!(fs::read(&target).unwrap(), b"previous");
            drop(held);
        }
        replace(&staging, &target).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"new");
        let staged = fixture.0.join("staged");
        fs::create_dir(&staged).unwrap();
        fs::write(staged.join("support"), b"complete").unwrap();
        let directory = fixture.0.join("directory");
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("old"), b"old").unwrap();
        replace(&staged, &directory).unwrap();
        assert!(directory.join("support").is_file());
        assert!(!directory.join("old").exists());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn publication_waits_for_a_scanner_to_release_unpacked_files() {
        use std::os::windows::fs::OpenOptionsExt;
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let staged = fixture.0.join("staged");
        let target = fixture.0.join("v1");
        fs::create_dir(&staged).unwrap();
        fs::write(staged.join("tool.exe"), b"new").unwrap();
        let held = fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(staged.join("tool.exe"))
            .unwrap();
        assert!(replace(&staged, &target).is_err());
        let cancelled = AtomicBool::new(true);
        assert!(
            replace_when_released(&staged, &target, &cancelled)
                .await
                .is_err()
        );
        let release = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(400));
            drop(held);
        });
        replace_when_released(&staged, &target, &AtomicBool::new(false))
            .await
            .unwrap();
        release.join().unwrap();
        assert_eq!(fs::read(target.join("tool.exe")).unwrap(), b"new");
        assert!(!staged.exists());
    }
}
