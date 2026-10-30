use crate::AppWindow;
use crate::download::engine::{BackendState, HistoryEntry};
use rusqlite::{
    Connection, OpenFlags,
    backup::{Backup, StepResult},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
const MAX_DB: u64 = 256 * 1024 * 1024;
#[derive(Clone, Serialize, Deserialize)]
struct Manifest {
    format: String,
    version: u32,
    schema: u32,
    created_at: String,
    sha256: String,
}
struct Work(PathBuf);
impl Work {
    fn new(root: &Path) -> Result<Self, String> {
        let path = root.join(format!(
            ".backup-work-{}",
            crate::download::engine::new_history_id()
        ));
        fs::create_dir(&path).map_err(|e| e.to_string())?;
        Ok(Self(path))
    }
}
impl Drop for Work {
    fn drop(&mut self) {
        for name in [
            "snapshot.sqlite3",
            "snapshot.sqlite3-wal",
            "snapshot.sqlite3-shm",
        ] {
            let _ = fs::remove_file(self.0.join(name));
        }
        let _ = fs::remove_dir(&self.0);
    }
}
fn copy_database(source: &Connection, target: &mut Connection) -> Result<(), String> {
    let copy = Backup::new(source, target).map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match copy.step(256).map_err(|e| e.to_string())? {
            StepResult::Done => return Ok(()),
            _ if Instant::now() >= deadline => {
                return Err("Database remained busy during backup.".into());
            }
            _ => std::thread::sleep(Duration::from_millis(5)),
        }
    }
}
fn validate(connection: &Connection) -> Result<(), String> {
    let check: String = connection
        .query_row("PRAGMA integrity_check(1)", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if check != "ok" {
        return Err("Backup database is damaged.".into());
    }
    connection.prepare("SELECT id,fetched_at,source_url,media_json,response_json,profile_json,next_cursor,has_more FROM history_entries LIMIT 0").map_err(|_|"Backup is not a TwitterDL history database.".to_string())?;
    connection
        .prepare("SELECT handle,group_name,tags_json FROM account_labels LIMIT 0")
        .map_err(|_| "Backup is missing account labels.".to_string())?;
    let schema: u32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if schema > 2 {
        return Err("Backup needs a newer application version.".into());
    }
    if connection
        .prepare("SELECT entry_id FROM history_payloads LIMIT 0")
        .is_ok()
    {
        crate::history::storage::validate(connection).map_err(|error| error.to_string())?;
    }
    Ok(())
}
fn write_package(source: &Connection, destination: &Path, root: &Path) -> Result<(), String> {
    write_package_checked(source, destination, root, true)
}
fn write_package_checked(
    source: &Connection,
    destination: &Path,
    root: &Path,
    check: bool,
) -> Result<(), String> {
    if destination.extension().and_then(|value| value.to_str()) != Some("tdbackup") {
        return Err("Choose a .tdbackup file for the database backup.".into());
    }
    let work = Work::new(root)?;
    let snapshot = work.0.join("snapshot.sqlite3");
    let mut target = Connection::open(&snapshot).map_err(|e| e.to_string())?;
    copy_database(source, &mut target)?;
    if check {
        validate(&target)?;
    }
    drop(target);
    let bytes = fs::read(&snapshot).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_DB {
        return Err("Database backup exceeds 256 MiB.".into());
    }
    let manifest = Manifest {
        format: "twitterdl-database-backup".into(),
        version: 1,
        schema: source
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|e| e.to_string())?,
        created_at: chrono::Utc::now().to_rfc3339(),
        sha256: format!("{:x}", Sha256::digest(&bytes)),
    };
    let part = destination.with_extension(format!(
        "tdbackup-{}.part",
        crate::download::engine::new_history_id()
    ));
    let result = (|| {
        let file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&part)
            .map_err(|e| e.to_string())?;
        let mut archive = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        archive
            .start_file("manifest.json", options)
            .map_err(|e| e.to_string())?;
        archive
            .write_all(&serde_json::to_vec(&manifest).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        archive
            .start_file("history.sqlite3", options)
            .map_err(|e| e.to_string())?;
        archive.write_all(&bytes).map_err(|e| e.to_string())?;
        archive
            .finish()
            .map_err(|e| e.to_string())?
            .sync_all()
            .map_err(|e| e.to_string())?;
        fs::rename(&part, destination).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(part);
    }
    result
}
fn read_package(path: &Path, root: &Path) -> Result<(Work, Manifest), String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > MAX_DB + 65536 {
        return Err("Backup file is too large.".into());
    }
    let mut archive =
        zip::ZipArchive::new(file).map_err(|_| "Invalid database backup.".to_string())?;
    if archive.len() != 2 {
        return Err("Backup must contain one database and its manifest.".into());
    }
    let manifest: Manifest = {
        let entry = archive
            .by_name("manifest.json")
            .map_err(|e| e.to_string())?;
        if entry.size() > 16384 {
            return Err("Invalid backup manifest.".into());
        }
        serde_json::from_reader(entry.take(16385)).map_err(|e| e.to_string())?
    };
    if manifest.format != "twitterdl-database-backup"
        || manifest.version != 1
        || manifest.schema > 2
    {
        return Err("Unsupported backup format or schema.".into());
    }
    let mut bytes = Vec::new();
    let entry = archive
        .by_name("history.sqlite3")
        .map_err(|e| e.to_string())?;
    if entry.size() > MAX_DB {
        return Err("Backup database is too large.".into());
    }
    entry
        .take(MAX_DB + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_DB || format!("{:x}", Sha256::digest(&bytes)) != manifest.sha256 {
        return Err("Backup checksum mismatch.".into());
    }
    let work = Work::new(root)?;
    let snapshot = work.0.join("snapshot.sqlite3");
    fs::write(&snapshot, bytes).map_err(|e| e.to_string())?;
    let connection = Connection::open_with_flags(&snapshot, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| e.to_string())?;
    validate(&connection)?;
    let schema: u32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if schema != manifest.schema {
        return Err("Backup schema does not match its manifest.".into());
    }
    drop(connection);
    Ok((work, manifest))
}
pub(crate) fn restore_on(target: &mut Connection, source: &Path) -> Result<(), String> {
    let source = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| e.to_string())?;
    validate(&source)?;
    copy_database(&source, target)
}
pub fn register(
    app: &AppWindow,
    root: PathBuf,
    history: Arc<Mutex<Vec<HistoryEntry>>>,
    backend: Arc<Mutex<BackendState>>,
    filter: Arc<Mutex<(String, bool)>>,
) {
    let pending = Arc::new(Mutex::new(None::<(PathBuf, String)>));
    let ui = app.as_weak();
    let directory = root.clone();
    app.on_backup_database(move || {
        let Some(app) = ui.upgrade() else {
            return;
        };
        if busy(&app) {
            return;
        }
        app.set_history_busy(true);
        app.set_backup_busy(true);
        app.set_backup_error("".into());
        let ui = ui.clone();
        let directory = directory.clone();
        tokio::spawn(async move {
            let target = rfd::AsyncFileDialog::new()
                .set_file_name("twitterdl-history.tdbackup")
                .add_filter("TwitterDL backup", &["tdbackup"])
                .save_file()
                .await;
            let result = if let Some(target) = target {
                tokio::task::spawn_blocking(move || {
                    crate::history::db::flush();
                    let source = crate::history::db::open_database().map_err(|e| e.to_string())?;
                    write_package(&source, target.path(), &directory).map(|_| true)
                })
                .await
                .map_err(|e| e.to_string())
                .and_then(|result| result)
            } else {
                Ok(false)
            };
            let _ = ui.upgrade_in_event_loop(move |app| {
                app.set_history_busy(false);
                app.set_backup_busy(false);
                match result {
                    Ok(true) => crate::app::notifications::show_toast(
                        &app,
                        "Database backup created.",
                        "info",
                    ),
                    Ok(false) => {}
                    Err(error) => app.set_backup_error(error),
                }
            });
        });
    });
    let ui = app.as_weak();
    let directory = root.clone();
    let preview = pending.clone();
    app.on_choose_database_backup(move || {
        let Some(app) = ui.upgrade() else {
            return;
        };
        if busy(&app) {
            return;
        }
        app.set_history_busy(true);
        app.set_backup_busy(true);
        app.set_backup_error("".into());
        let ui = ui.clone();
        let directory = directory.clone();
        let pending = preview.clone();
        tokio::spawn(async move {
            let source = rfd::AsyncFileDialog::new()
                .add_filter("TwitterDL backup", &["tdbackup"])
                .pick_file()
                .await;
            let result = if let Some(source) = source {
                tokio::task::spawn_blocking(move || {
                    let (work, manifest) = read_package(source.path(), &directory)?;
                    let database = Connection::open(work.0.join("snapshot.sqlite3"))
                        .map_err(|e| e.to_string())?;
                    let records: i64 = database
                        .query_row("SELECT COUNT(*) FROM history_entries", [], |row| row.get(0))
                        .map_err(|e| e.to_string())?;
                    let labels: i64 = database
                        .query_row("SELECT COUNT(*) FROM account_labels", [], |row| row.get(0))
                        .map_err(|e| e.to_string())?;
                    drop(database);
                    *pending.lock().unwrap() = Some((source.path().to_path_buf(), manifest.sha256));
                    Ok::<_, String>(Some(format!(
                        "{}\n{} history/download records · {} account labels\nCreated {}",
                        source.path().display(),
                        records,
                        labels,
                        manifest.created_at
                    )))
                })
                .await
                .map_err(|e| e.to_string())
                .and_then(|result| result)
            } else {
                Ok(None)
            };
            let _ = ui.upgrade_in_event_loop(move |app| {
                app.set_history_busy(false);
                app.set_backup_busy(false);
                match result {
                    Ok(Some(summary)) => {
                        app.set_backup_summary(summary);
                        app.set_backup_restore_open(true);
                    }
                    Ok(None) => {}
                    Err(error) => app.set_backup_error(error),
                }
            });
        });
    });
    let ui = app.as_weak();
    app.on_restore_database(move|| {
        let Some(app)=ui.upgrade() else {return;};if busy(&app) {return;}let Some((path,digest))=pending.lock().unwrap().take() else {return;};
        app.set_backup_restore_open(false);app.set_backup_busy(true);app.set_history_busy(true);app.set_backup_error("".into());
        let directory=root.clone();let ui=ui.clone();let history=history.clone();let backend=backend.clone();let filter=filter.clone();
        tokio::spawn(async move {
            let result=tokio::task::spawn_blocking(move|| {
                let (work,manifest)=read_package(&path,&directory)?;if manifest.sha256!=digest {return Err("Backup changed after the preview. Select it again.".into());}
                crate::history::db::flush();let source=crate::history::db::open_database().map_err(|e|e.to_string())?;
                let recovery=directory.join("backups");fs::create_dir_all(&recovery).map_err(|e|e.to_string())?;
                write_package_checked(&source,&recovery.join(format!("before-restore-{}.tdbackup",crate::download::engine::new_history_id())),&directory,false)?;drop(source);
                crate::history::db::restore_database(work.0.join("snapshot.sqlite3"))?;
                Ok::<_,String>((crate::history::db::load_summaries().map_err(|error|error.to_string())?,crate::history::db::load_account_labels()?))
            }).await.map_err(|e|e.to_string()).and_then(|result|result);
            let _=ui.upgrade_in_event_loop(move|app| {
                app.set_backup_busy(false);app.set_history_busy(false);
                match result {Ok((entries,labels))=>{
                    crate::accounts::labels::replace_catalog(labels);*history.lock().unwrap()=entries.clone();*backend.lock().unwrap()=BackendState::default();
                    app.set_has_output(false);app.set_has_more(false);app.set_selected_media_index(-1);app.set_media_items(crate::download::engine::model_from_rows(Vec::new()));app.set_media_count(0);app.set_account_profile(crate::download::engine::empty_account_profile());app.set_last_error(String::new());app.invoke_history_loaded();
                    app.set_download_batch_id("".into());app.set_fetch_sessions_key("".into());app.set_history_selection(crate::ui::runtime::ModelRc::default());
                    let filter=filter.lock().unwrap();crate::history::view_model::refresh(&app,&entries,&filter.0,filter.1);crate::app::notifications::show_toast(&app,"Database restored. A recovery backup was saved in the portable backups folder.","info");
                },Err(error)=>app.set_backup_error(error)}
            });
        });
    });
}
fn busy(app: &AppWindow) -> bool {
    app.get_is_busy()
        || app.get_history_busy()
        || app.get_batch_running()
        || app.get_tool_update_busy()
        || app.get_ffmpeg_busy()
        || app.get_xtractor_busy()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backup_restores_wal_records_and_labels_and_rejects_bad_archives() {
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let source = Connection::open(fixture.0.join("live.sqlite3")).unwrap();
        crate::history::db::initialize_database(&source).unwrap();
        source.pragma_update(None, "user_version", 1).unwrap();
        source
            .execute(
                "INSERT INTO account_labels VALUES('demo','Artists','[]','#f59e0b')",
                [],
            )
            .unwrap();
        source.execute("INSERT INTO history_entries(id,fetched_at,source_url,media_json,response_json) VALUES('1','now','https://x.com/demo','[]','{}')",[]).unwrap();
        let archive = fixture.0.join("history.tdbackup");
        write_package(&source, &archive, &fixture.0).unwrap();
        let (work, _) = read_package(&archive, &fixture.0).unwrap();
        let mut restored = Connection::open_in_memory().unwrap();
        restore_on(&mut restored, &work.0.join("snapshot.sqlite3")).unwrap();
        assert_eq!(
            restored
                .query_row(
                    "SELECT color FROM account_labels WHERE handle='demo'",
                    [],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            "#f59e0b"
        );
        assert_eq!(
            restored
                .query_row("SELECT COUNT(*) FROM history_entries", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        // A syntactically valid archive with altered database bytes must fail before restore.
        let bad = fixture.0.join("tampered.tdbackup");
        let mut file = zip::ZipWriter::new(File::create(&bad).unwrap());
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        let manifest = Manifest {
            format: "twitterdl-database-backup".into(),
            version: 1,
            schema: 1,
            created_at: "fixture".into(),
            sha256: "0".repeat(64),
        };
        file.start_file("manifest.json", options).unwrap();
        file.write_all(&serde_json::to_vec(&manifest).unwrap())
            .unwrap();
        file.start_file("history.sqlite3", options).unwrap();
        file.write_all(b"tampered").unwrap();
        file.finish().unwrap();
        assert!(
            read_package(&bad, &fixture.0)
                .err()
                .unwrap()
                .contains("checksum")
        );
        fs::write(&archive, b"not a backup").unwrap();
        assert!(read_package(&archive, &fixture.0).is_err());
        assert_eq!(
            restored
                .query_row("SELECT COUNT(*) FROM account_labels", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
}
