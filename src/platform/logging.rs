//! One portable log per GUI session, written off the UI thread.
use chrono::{Local, NaiveDateTime};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{OnceLock, mpsc},
    thread::{self, JoinHandle},
    time::SystemTime,
};

#[derive(Clone, Copy)]
pub enum Level {
    Info,
    Warn,
    Error,
}
impl Level {
    fn label(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}

#[derive(Clone, Copy)]
struct Limits {
    files: usize,
    bytes: u64,
}
const LIMITS: Limits = Limits {
    files: 20,
    bytes: 50 * 1024 * 1024,
};

enum Command {
    Entry {
        stamp: String,
        level: Level,
        message: String,
    },
    Shutdown,
}
static ACTIVE_NAME: OnceLock<String> = OnceLock::new();

pub fn active_name() -> Option<&'static str> {
    ACTIVE_NAME.get().map(String::as_str)
}

static SENDER: OnceLock<mpsc::Sender<Command>> = OnceLock::new();

pub struct SessionGuard {
    sender: mpsc::Sender<Command>,
    worker: Option<JoinHandle<()>>,
}
impl Drop for SessionGuard {
    fn drop(&mut self) {
        let _ = self.sender.send(Command::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn initialize(data_directory: &Path) -> io::Result<SessionGuard> {
    let stamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let mut session = Session::create(
        &data_directory.join("logs"),
        &stamp,
        std::process::id(),
        LIMITS,
    )?;
    session.append(
        &line_stamp(),
        Level::Info,
        &format!(
            "Application started (version {}, PID {}).",
            env!("CARGO_PKG_VERSION"),
            std::process::id()
        ),
    )?;
    let name = session
        .path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let guard = spawn_writer(session)?;
    SENDER.set(guard.sender.clone()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "Session logger already initialized",
        )
    })?;
    let _ = ACTIVE_NAME.set(name);
    Ok(guard)
}

fn spawn_writer(mut session: Session) -> io::Result<SessionGuard> {
    let (sender, receiver) = mpsc::channel();
    let worker = thread::Builder::new()
        .name("twitterdl-session-log".into())
        .spawn(move || {
            while let Ok(command) = receiver.recv() {
                match command {
                    Command::Entry {
                        stamp,
                        level,
                        message,
                    } => {
                        if let Err(error) = session.append(&stamp, level, &message) {
                            eprintln!("Could not write session log: {error}");
                        }
                    }
                    Command::Shutdown => break,
                }
            }
            let _ = session.file.flush();
        })?;
    Ok(SessionGuard {
        sender,
        worker: Some(worker),
    })
}

fn line_stamp() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S%.3f %:z").to_string()
}

pub fn write(level: Level, message: impl Into<String>) {
    if let Some(sender) = SENDER.get() {
        let _ = sender.send(Command::Entry {
            stamp: line_stamp(),
            level,
            message: message.into(),
        });
    }
}
pub fn info(message: impl Into<String>) {
    write(Level::Info, message);
}
pub fn warn(message: impl Into<String>) {
    write(Level::Warn, message);
}
pub fn error(message: impl Into<String>) {
    write(Level::Error, message);
}

struct Session {
    file: File,
    path: PathBuf,
    limits: Limits,
    written: u64,
    full: bool,
}
impl Session {
    fn create(directory: &Path, stamp: &str, pid: u32, limits: Limits) -> io::Result<Self> {
        fs::create_dir_all(directory)?;
        let mut suffix = 0u64;
        let (file, path) = loop {
            let suffix_text = if suffix == 0 {
                String::new()
            } else {
                format!("-{suffix}")
            };
            let path = directory.join(format!("app-{stamp}-{pid}{suffix_text}.log"));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt;
                // FILE_SHARE_READ permits inspection while blocking replacement/deletion.
                options.share_mode(1);
            }
            match options.open(&path) {
                Ok(file) => break (file, path),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => suffix += 1,
                Err(error) => return Err(error),
            }
        };
        // Held for the whole session; cleanup in other instances skips this file.
        #[cfg(not(windows))]
        file.try_lock()
            .map_err(|error| io::Error::other(error.to_string()))?;
        let session = Self {
            file,
            path,
            limits,
            written: 0,
            full: false,
        };
        prune(directory, &session.path, limits)?;
        Ok(session)
    }

    fn append(&mut self, stamp: &str, level: Level, message: &str) -> io::Result<()> {
        if self.full {
            return Ok(());
        }
        let normalized = message.replace("\r\n", "\n").replace('\r', "\n");
        let entry = normalized
            .split('\n')
            .map(|line| format!("[{stamp}] [{}] {line}\n", level.label()))
            .collect::<String>();
        let notice =
            format!("[{stamp}] [WARN] Session log size limit reached; further entries omitted.\n");
        let remaining = self.limits.bytes.saturating_sub(self.written);
        if entry.len() as u64 + notice.len() as u64 > remaining {
            self.full = true;
            if notice.len() as u64 <= remaining {
                self.file.write_all(notice.as_bytes())?;
                self.written += notice.len() as u64;
            }
        } else {
            self.file.write_all(entry.as_bytes())?;
            self.written += entry.len() as u64;
        }
        self.file.flush()?;
        prune(self.path.parent().unwrap(), &self.path, self.limits)
    }
}

pub(crate) fn managed_name(name: &str) -> bool {
    let Some(stem) = name
        .strip_prefix("app-")
        .and_then(|name| name.strip_suffix(".log"))
    else {
        return false;
    };
    let Some(stamp) = stem.get(..19) else {
        return false;
    };
    if NaiveDateTime::parse_from_str(stamp, "%Y-%m-%d_%H-%M-%S").is_err() {
        return false;
    }
    let Some(tail) = stem.get(19..).and_then(|tail| tail.strip_prefix('-')) else {
        return false;
    };
    let mut pieces = tail.split('-');
    let numeric = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    pieces.next().is_some_and(numeric) && pieces.all(numeric)
}

fn prune(directory: &Path, active_path: &Path, limits: Limits) -> io::Result<()> {
    let mut files = Vec::new();
    for entry in fs::read_dir(directory)? {
        // Another instance can remove an old log between enumeration and stat.
        let Ok(entry) = entry else { continue };
        let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if !metadata.file_type().is_file() || !managed_name(&entry.file_name().to_string_lossy()) {
            continue;
        }
        files.push((
            metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            entry.path(),
            metadata.len(),
        ));
    }
    files.sort_by(|first, second| first.0.cmp(&second.0).then(first.1.cmp(&second.1)));
    let mut count = files.len();
    let mut bytes = files.iter().map(|file| file.2).sum::<u64>();
    for (_, path, size) in files {
        if count <= limits.files && bytes <= limits.bytes {
            break;
        }
        if path == active_path {
            continue;
        }
        let file = match OpenOptions::new().write(true).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                count -= 1;
                bytes = bytes.saturating_sub(size);
                continue;
            }
            Err(_) => continue,
        };
        if file.try_lock().is_err() {
            continue;
        }
        if fs::remove_file(&path).is_ok() {
            count -= 1;
            bytes = bytes.saturating_sub(size);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const STAMP: &str = "2026-10-01_00-30-45";

    fn fixture() -> (crate::download::saved_files::tests::Fixture, PathBuf) {
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let directory = fixture.0.join("logs");
        fs::create_dir_all(&directory).unwrap();
        (fixture, directory)
    }
    fn old_file(directory: &Path, index: u64, size: u64) -> PathBuf {
        let path = directory.join(format!("app-2026-09-30_12-00-00-{index}.log"));
        let file = File::create(&path).unwrap();
        file.set_len(size).unwrap();
        file.set_modified(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(index))
            .unwrap();
        path
    }

    #[test]
    fn sessions_are_unique_and_every_line_has_readable_time_and_level() {
        let (_fixture, directory) = fixture();
        let mut first = Session::create(&directory, STAMP, 1234, LIMITS).unwrap();
        let mut second = Session::create(&directory, STAMP, 1234, LIMITS).unwrap();
        assert_ne!(first.path, second.path);
        assert_eq!(
            first.path.file_name().unwrap(),
            "app-2026-10-01_00-30-45-1234.log"
        );
        let stamp = "2026-10-01 00:30:45.123 +07:00";
        first
            .append(stamp, Level::Info, "first session\r\ncontinued\rnext line")
            .unwrap();
        second.append(stamp, Level::Warn, "warning").unwrap();
        second.append(stamp, Level::Error, "error").unwrap();
        let first_text = fs::read_to_string(&first.path).unwrap();
        assert_eq!(first_text.lines().count(), 3);
        assert!(
            first_text
                .lines()
                .all(|line| line.starts_with(&format!("[{stamp}] [INFO]")))
        );
        assert!(!first_text.contains("warning"));
        let second_text = fs::read_to_string(&second.path).unwrap();
        assert!(second_text.contains("[WARN] warning"));
        assert!(second_text.contains("[ERROR] error"));
    }

    #[test]
    fn retention_limits_count_and_bytes_without_touching_other_files() {
        let (_fixture, directory) = fixture();
        let files = (0..5)
            .map(|index| old_file(&directory, index, 40))
            .collect::<Vec<_>>();
        let unrelated = directory.join("app-custom.log");
        fs::write(&unrelated, b"user data").unwrap();
        let subdirectory = directory.join("app-2026-09-30_12-00-00-999.log");
        fs::create_dir(&subdirectory).unwrap();
        let session = Session::create(
            &directory,
            STAMP,
            1234,
            Limits {
                files: 3,
                bytes: 10_000,
            },
        )
        .unwrap();
        assert!(files.iter().take(3).all(|path| !path.exists()));
        assert!(files.iter().skip(3).all(|path| path.is_file()));
        assert!(unrelated.is_file() && subdirectory.is_dir());
        drop(session);
        let session = Session::create(
            &directory,
            STAMP,
            2345,
            Limits {
                files: 20,
                bytes: 50,
            },
        )
        .unwrap();
        assert!(!files[3].exists());
        assert!(files[4].is_file());
        assert!(session.path.is_file());
        assert_eq!(fs::read(unrelated).unwrap(), b"user data");
    }

    #[test]
    fn cleanup_preserves_logs_locked_by_other_running_instances() {
        let (_fixture, directory) = fixture();
        let first = Session::create(&directory, STAMP, 1234, LIMITS).unwrap();
        let old = old_file(&directory, 1, 10);
        let second = Session::create(
            &directory,
            STAMP,
            2345,
            Limits {
                files: 2,
                bytes: 1000,
            },
        )
        .unwrap();
        assert!(first.path.is_file() && second.path.is_file());
        assert!(!old.exists());
        // A closed session becomes eligible for pruning on the next launch.
        let first_path = first.path.clone();
        drop(first);
        let third = Session::create(
            &directory,
            STAMP,
            3456,
            Limits {
                files: 2,
                bytes: 1000,
            },
        )
        .unwrap();
        assert!(!first_path.exists());
        assert!(second.path.is_file() && third.path.is_file());
    }

    #[test]
    fn an_active_session_cannot_grow_past_the_byte_limit() {
        let (_fixture, directory) = fixture();
        let mut session = Session::create(
            &directory,
            STAMP,
            1234,
            Limits {
                files: 20,
                bytes: 512,
            },
        )
        .unwrap();
        session
            .append(
                "2026-10-01 00:30:45.123 +07:00",
                Level::Info,
                "before limit",
            )
            .unwrap();
        session
            .append(
                "2026-10-01 00:30:45.123 +07:00",
                Level::Error,
                &"large diagnostic".repeat(100),
            )
            .unwrap();
        session
            .append("2026-10-01 00:30:45.123 +07:00", Level::Info, "after limit")
            .unwrap();
        let text = fs::read_to_string(&session.path).unwrap();
        assert!(text.contains("before limit"));
        assert!(text.contains("[WARN] Session log size limit reached"));
        assert!(!text.contains("after limit"));
        assert!(fs::metadata(&session.path).unwrap().len() <= 512);
    }

    #[test]
    fn writer_serializes_concurrent_events_and_flushes_before_shutdown() {
        let (_fixture, directory) = fixture();
        let session = Session::create(&directory, STAMP, 1234, LIMITS).unwrap();
        let path = session.path.clone();
        let guard = spawn_writer(session).unwrap();
        let workers = (0..4)
            .map(|worker| {
                let sender = guard.sender.clone();
                thread::spawn(move || {
                    for index in 0..25 {
                        sender
                            .send(Command::Entry {
                                stamp: "2026-10-01 00:30:45.123 +07:00".into(),
                                level: Level::Info,
                                message: format!("worker {worker} event {index}"),
                            })
                            .unwrap();
                    }
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            worker.join().unwrap();
        }
        drop(guard);
        let text = fs::read_to_string(path).unwrap();
        assert_eq!(text.lines().count(), 100);
        for worker in 0..4 {
            for index in 0..25 {
                assert_eq!(
                    text.lines()
                        .filter(|line| line.ends_with(&format!("worker {worker} event {index}")))
                        .count(),
                    1
                );
            }
        }
    }
}
