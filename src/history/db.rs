use std::{
    sync::{
        OnceLock,
        mpsc::{self, Sender},
    },
    time::Duration,
};

use rusqlite::{Connection, params};

use crate::download::engine::{
    self as downloader, ExtractionProfile, HistoryEntry, MediaCandidate,
};

enum Command {
    Save(HistoryEntry),
    Restore(std::path::PathBuf, mpsc::SyncSender<Result<(), String>>),
    SaveBatch(Vec<HistoryEntry>, mpsc::SyncSender<Result<(), String>>),
    SaveMedia(String, Vec<MediaCandidate>),
    Clear(bool),
    Remove(String),
    Flush(mpsc::SyncSender<()>),
}

fn writer() -> &'static Sender<Command> {
    static WRITER: OnceLock<Sender<Command>> = OnceLock::new();
    WRITER.get_or_init(|| {
        let (sender, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("twitterdl-history-db".into())
            .spawn(move || run_writer(receiver))
            .expect("Could not start history database worker");
        sender
    })
}

fn run_writer(receiver: mpsc::Receiver<Command>) {
    while let Ok(command) = receiver.recv() {
        match command {
            Command::Restore(path, done) => {
                let result =
                    open_database()
                        .map_err(|e| e.to_string())
                        .and_then(|mut connection| {
                            crate::history::backup::restore_on(&mut connection, &path)
                        });
                let _ = done.send(result);
            }
            Command::Save(entry) => {
                if let Err(error) = save_entry(&entry) {
                    crate::platform::logging::error(format!(
                        "Could not save fetch history: {error}"
                    ));
                }
            }
            Command::SaveBatch(entries, done) => {
                let result = (|| {
                    let mut connection = open_database()?;
                    save_batch_on(&mut connection, &entries)
                })()
                .map_err(|error: rusqlite::Error| error.to_string());
                let _ = done.send(result);
            }
            Command::SaveMedia(id, media) => {
                if let Err(error) = update_media_row(&id, &media) {
                    crate::platform::logging::error(format!(
                        "Could not update saved media history: {error}"
                    ));
                }
            }
            Command::Clear(downloads) => {
                if let Err(error) = clear_database(downloads) {
                    crate::platform::logging::error(format!("Could not clear history: {error}"));
                }
            }
            Command::Remove(id) => {
                if let Err(error) = open_database().and_then(|connection| {
                    connection.execute("DELETE FROM history_entries WHERE id = ?1", params![id])
                }) {
                    crate::platform::logging::error(format!(
                        "Could not remove history entry: {error}"
                    ));
                }
            }
            Command::Flush(done) => {
                let _ = done.send(());
            }
        }
    }
}

pub(crate) fn open_database() -> rusqlite::Result<Connection> {
    let directory = downloader::app_data_dir()
        .ok_or_else(|| rusqlite::Error::InvalidPath("no executable data directory".into()))?;
    std::fs::create_dir_all(&directory)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    let path = directory.join("history.sqlite3");
    let connection = Connection::open(path)?;
    initialize_database(&connection)?;
    Ok(connection)
}

pub(crate) fn initialize_database(connection: &Connection) -> rusqlite::Result<()> {
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA foreign_keys=ON;
         CREATE TABLE IF NOT EXISTS history_entries (
            id TEXT PRIMARY KEY NOT NULL,
            fetched_at TEXT NOT NULL,
            source_url TEXT NOT NULL,
            account_name TEXT NOT NULL DEFAULT '',
            username TEXT NOT NULL DEFAULT '',
            tweet_count INTEGER NOT NULL DEFAULT 0,
            media_json TEXT NOT NULL,
            response_json TEXT NOT NULL,
            profile_json TEXT,
            next_cursor TEXT NOT NULL DEFAULT '',
            has_more INTEGER NOT NULL DEFAULT 0,
            download_json TEXT
         );
         CREATE TABLE IF NOT EXISTS account_labels (handle TEXT PRIMARY KEY NOT NULL, group_name TEXT NOT NULL, tags_json TEXT NOT NULL, color TEXT NOT NULL DEFAULT '');
         CREATE TABLE IF NOT EXISTS history_metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
    )?;
    if connection
        .prepare("SELECT download_json FROM history_entries LIMIT 0")
        .is_err()
    {
        connection.execute(
            "ALTER TABLE history_entries ADD COLUMN download_json TEXT",
            [],
        )?;
    }
    if connection
        .prepare("SELECT color FROM account_labels LIMIT 0")
        .is_err()
    {
        connection.execute(
            "ALTER TABLE account_labels ADD COLUMN color TEXT NOT NULL DEFAULT ''",
            [],
        )?;
    }
    super::storage::initialize(connection)
}

pub fn load_summaries() -> rusqlite::Result<Vec<HistoryEntry>> {
    let connection = open_database()?;
    let root = downloader::app_data_dir().ok_or(rusqlite::Error::InvalidQuery)?;
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let legacy_json = root.join("history.json");
    if version == 0 && legacy_json.is_file() {
        let backup_root = root.join("backups");
        std::fs::create_dir_all(&backup_root)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        std::fs::copy(
            &legacy_json,
            backup_root.join(format!(
                "history-before-json-migration-{}.json",
                downloader::new_history_id()
            )),
        )
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        drop(connection);
        migrate_json_history();
        if legacy_json.exists() {
            return Err(rusqlite::Error::InvalidQuery);
        }
        return load_summaries();
    }
    let legacy: i64 = connection.query_row("SELECT COUNT(*) FROM history_entries WHERE id NOT IN(SELECT entry_id FROM history_payloads)", [], |row|row.get(0))?;
    if legacy > 0 {
        let root = downloader::app_data_dir().ok_or(rusqlite::Error::InvalidQuery)?;
        let backup_root = root.join("backups");
        std::fs::create_dir_all(&backup_root)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        let backup = backup_root.join(format!(
            "history-before-storage-v2-{}.sqlite3",
            downloader::new_history_id()
        ));
        let mut target = Connection::open(&backup)?;
        let copy = rusqlite::backup::Backup::new(&connection, &mut target)?;
        copy.run_to_completion(512, Duration::from_millis(5), None)?;
        drop(copy);
        drop(target);
        let imported: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM history_metadata WHERE key='downloads-v1')",
            [],
            |row| row.get(0),
        )?;
        if !imported {
            let mut entries = read_entries(&connection)?;
            migrate_downloads(&connection, &mut entries)?;
        }
        super::storage::migrate(&connection)?;
        connection.pragma_update(None, "user_version", 2)?;
        connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); VACUUM;")?;
    }
    connection.pragma_update(None, "user_version", 2)?;
    connection.execute("INSERT INTO history_metadata(key,value) VALUES('downloads-v1','1') ON CONFLICT(key) DO NOTHING",[])?;
    let path = std::sync::Arc::new(
        downloader::app_data_dir()
            .ok_or(rusqlite::Error::InvalidQuery)?
            .join("history.sqlite3"),
    );
    read_summaries(&connection, path)
}

pub(crate) fn read_summaries(
    connection: &Connection,
    path: std::sync::Arc<std::path::PathBuf>,
) -> rusqlite::Result<Vec<HistoryEntry>> {
    let mut statement = connection.prepare("SELECT h.id,h.fetched_at,h.source_url,h.account_name,h.username,h.tweet_count,h.response_json,h.profile_json,h.next_cursor,h.has_more,h.download_json,p.summary_json FROM history_entries h JOIN history_payloads p ON p.entry_id=h.id ORDER BY CAST(h.id AS INTEGER) DESC")?;
    statement
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let text: String = row.get(11)?;
            let summary: super::storage::Summary = serde_json::from_str(&text)
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
            let profile: Option<String> = row.get(7)?;
            let download: Option<String> = row.get(10)?;
            Ok(HistoryEntry {
                id: id.clone(),
                fetched_at: row.get(1)?,
                source_url: row.get(2)?,
                account_name: row.get(3)?,
                username: row.get(4)?,
                tweet_count: row.get::<_, i64>(5)?.max(0) as usize,
                json: row.get(6)?,
                profile: profile.and_then(|value| serde_json::from_str(&value).ok()),
                next_cursor: row.get(8)?,
                has_more: row.get::<_, i64>(9)? != 0,
                download: download.and_then(|value| serde_json::from_str(&value).ok()),
                stored: Some(super::storage::StoredEntry {
                    path: path.clone(),
                    id,
                    summary,
                }),
                ..Default::default()
            })
        })?
        .collect()
}

pub fn load_history() -> Vec<HistoryEntry> {
    let Ok(connection) = open_database() else {
        return Vec::new();
    };
    let version = connection
        .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
        .unwrap_or(1);
    if version == 0 {
        drop(connection);
        let _ = migrate_json_history();
        return load_history();
    }
    let mut entries = read_entries(&connection).unwrap_or_default();
    if let Err(error) = migrate_downloads(&connection, &mut entries) {
        crate::platform::logging::error(format!("Could not migrate download history: {error}"));
    }
    drop(connection);
    for entry in &mut entries {
        let restored = crate::fetch::archives::restore_text(entry);
        if entry.download.is_none()
            && (crate::download::saved_files::reconcile(&mut entry.media) || restored)
        {
            save_media(entry.id.clone(), entry.media.clone());
        }
    }
    entries
}

pub(crate) fn read_entries(connection: &Connection) -> rusqlite::Result<Vec<HistoryEntry>> {
    let mut statement = connection.prepare(
        "SELECT id, fetched_at, source_url, account_name, username, tweet_count,
                media_json, response_json, profile_json, next_cursor, has_more, download_json
         FROM history_entries
         ORDER BY CAST(id AS INTEGER) DESC",
    )?;
    let rows = statement.query_map([], |row| {
        let media_json: String = row.get(6)?;
        let profile_json: Option<String> = row.get(8)?;
        let mut media: Vec<MediaCandidate> = serde_json::from_str(&media_json).unwrap_or_default();
        let id: String = row.get(0)?;
        let compact = super::storage::summary(connection, &id)?.is_some();
        let response: String = if compact {
            super::storage::read_response(connection, &id)?
        } else {
            row.get(7)?
        };
        if compact {
            media = super::storage::read_media(connection, &id)?;
        } else {
            downloader::restore_preview_details(&mut media, &response);
        }
        let profile: Option<ExtractionProfile> =
            profile_json.and_then(|value| serde_json::from_str(&value).ok());
        Ok(HistoryEntry {
            id: row.get(0)?,
            fetched_at: row.get(1)?,
            source_url: row.get(2)?,
            account_name: row.get(3)?,
            username: row.get(4)?,
            tweet_count: row.get::<_, i64>(5)?.max(0) as usize,
            media,
            json: response,
            stored: None,
            profile,
            next_cursor: row.get(9)?,
            has_more: row.get::<_, i64>(10)? != 0,
            download: row
                .get::<_, Option<String>>(11)?
                .and_then(|value| serde_json::from_str(&value).ok()),
        })
    })?;
    rows.collect()
}

fn migrate_downloads(
    connection: &Connection,
    entries: &mut Vec<HistoryEntry>,
) -> rusqlite::Result<()> {
    let migrated = connection.query_row(
        "SELECT COUNT(*) FROM history_metadata WHERE key='downloads-v1'",
        [],
        |row| row.get::<_, i64>(0),
    )?;
    if migrated > 0 {
        return Ok(());
    }
    let imports = entries
        .iter()
        .filter(|entry| entry.download.is_none())
        .filter_map(|entry| {
            crate::download::commands::make_entry(entry, entry.media.clone(), "all", true)
        })
        .filter(|entry| !entries.iter().any(|previous| previous.id == entry.id))
        .collect::<Vec<_>>();
    let transaction = connection.unchecked_transaction()?;
    for entry in &imports {
        save_entry_on(&transaction, entry)?;
    }
    transaction.execute(
        "INSERT INTO history_metadata(key,value) VALUES('downloads-v1','1')",
        [],
    )?;
    transaction.commit()?;
    entries.extend(imports);
    Ok(())
}

fn migrate_json_history() -> Vec<HistoryEntry> {
    let Some(path) = downloader::app_data_dir().map(|directory| directory.join("history.json"))
    else {
        mark_history_database_version();
        return Vec::new();
    };
    let Some(entries) = std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Vec<HistoryEntry>>(&bytes).ok())
    else {
        mark_history_database_version();
        return Vec::new();
    };
    for entry in &entries {
        let mut entry = entry.clone();
        downloader::restore_preview_details(&mut entry.media, &entry.json);
        save_history(entry);
    }
    flush();
    let count = open_database()
        .and_then(|connection| {
            connection.query_row("SELECT COUNT(*) FROM history_entries", [], |row| {
                row.get::<_, i64>(0).map(|count| count.max(0) as usize)
            })
        })
        .unwrap_or_default();
    if count >= entries.len() {
        mark_history_database_version();
        let _ = std::fs::remove_file(path);
        load_history()
    } else {
        entries
    }
}

fn mark_history_database_version() {
    if let Ok(connection) = open_database() {
        let _ = connection.pragma_update(None, "user_version", 1);
    }
}

pub fn save_history(entry: HistoryEntry) {
    let _ = writer().send(Command::Save(entry));
}

/// Acknowledged transaction used before removing converted original files.
pub fn save_batch(entries: Vec<HistoryEntry>) -> Result<(), String> {
    let (done, receiver) = mpsc::sync_channel(1);
    writer()
        .send(Command::SaveBatch(entries, done))
        .map_err(|error| error.to_string())?;
    receiver
        .recv_timeout(Duration::from_secs(30))
        .map_err(|error| error.to_string())?
}

pub fn save_media(id: String, media: Vec<MediaCandidate>) {
    let _ = writer().send(Command::SaveMedia(id, media));
}

pub fn clear_history(downloads: bool) {
    let _ = writer().send(Command::Clear(downloads));
}

pub fn remove_history(id: String) {
    let _ = writer().send(Command::Remove(id));
}

pub fn flush() {
    let (done, receiver) = mpsc::sync_channel(0);
    if writer().send(Command::Flush(done)).is_ok() {
        let _ = receiver.recv_timeout(Duration::from_secs(10));
    }
}

fn save_entry(entry: &HistoryEntry) -> rusqlite::Result<()> {
    let connection = open_database()?;
    save_entry_on(&connection, entry)
}

fn save_batch_on(connection: &mut Connection, entries: &[HistoryEntry]) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    for entry in entries {
        save_entry_on(&transaction, entry)?;
    }
    transaction.commit()
}

pub(crate) fn save_entry_on(connection: &Connection, entry: &HistoryEntry) -> rusqlite::Result<()> {
    if connection.is_autocommit() {
        let transaction = connection.unchecked_transaction()?;
        save_entry_data(&transaction, entry)?;
        return transaction.commit();
    }
    save_entry_data(connection, entry)
}

fn save_entry_data(connection: &Connection, entry: &HistoryEntry) -> rusqlite::Result<()> {
    if entry.stored.is_some() {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let profile_json = entry
        .profile
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    connection.execute(
        "INSERT INTO history_entries
            (id, fetched_at, source_url, account_name, username, tweet_count,
             media_json, response_json, profile_json, next_cursor, has_more, download_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(id) DO UPDATE SET
            fetched_at=excluded.fetched_at,
            source_url=excluded.source_url,
            account_name=excluded.account_name,
            username=excluded.username,
            tweet_count=excluded.tweet_count,
            media_json=excluded.media_json,
            response_json=excluded.response_json,
            profile_json=excluded.profile_json,
            next_cursor=excluded.next_cursor,
            has_more=excluded.has_more,
            download_json=excluded.download_json",
        params![
            entry.id,
            entry.fetched_at,
            entry.source_url,
            entry.account_name,
            entry.username,
            entry.tweet_count as i64,
            "[]",
            super::storage::account_json(&entry.json)?,
            profile_json,
            entry.next_cursor,
            entry.has_more as i64,
            entry
                .download
                .as_ref()
                .map(serde_json::to_string)
                .transpose()
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?,
        ],
    )?;
    super::storage::write_payload(
        connection,
        &entry.id,
        &entry.username,
        &entry.media,
        &entry.json,
    )?;
    Ok(())
}

fn update_media_row(id: &str, media: &[MediaCandidate]) -> rusqlite::Result<()> {
    let connection = open_database()?;
    let transaction = connection.unchecked_transaction()?;
    let username: String = transaction.query_row(
        "SELECT username FROM history_entries WHERE id=?1",
        [id],
        |row| row.get(0),
    )?;
    super::storage::write_media(&transaction, id, &username, media)?;
    transaction.commit()?;
    Ok(())
}

fn clear_database(downloads: bool) -> rusqlite::Result<()> {
    clear_database_on(&open_database()?, downloads)
}

fn clear_database_on(connection: &Connection, downloads: bool) -> rusqlite::Result<()> {
    connection.execute(
        "DELETE FROM history_entries WHERE (download_json IS NOT NULL) = ?1",
        params![downloads],
    )?;
    Ok(())
}

#[cfg(test)]
fn serialize_media(media: &[MediaCandidate]) -> rusqlite::Result<String> {
    serde_json::to_string(media)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn existing_group_labels_migrate_with_colors_without_losing_tags() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("CREATE TABLE account_labels(handle TEXT PRIMARY KEY NOT NULL,group_name TEXT NOT NULL,tags_json TEXT NOT NULL); INSERT INTO account_labels VALUES('demo','Artists','[\"art\"]');").unwrap();
        initialize_database(&connection).unwrap();
        initialize_database(&connection).unwrap();
        let (tags, color): (String, String) = connection
            .query_row(
                "SELECT tags_json,color FROM account_labels WHERE handle='demo'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(tags, "[\"art\"]");
        assert!(color.is_empty());
        save_labels_on(
            &mut connection,
            &[(
                "demo".into(),
                crate::accounts::labels::Label {
                    group: "Artists".into(),
                    tags: vec!["art".into()],
                    color: "#34d399".into(),
                },
            )],
        )
        .unwrap();
        assert_eq!(
            connection
                .query_row(
                    "SELECT color FROM account_labels WHERE handle='demo'",
                    [],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            "#34d399"
        );
    }
    #[test]
    fn account_labels_survive_history_removal_and_upsert_transactionally() {
        let mut connection = Connection::open_in_memory().unwrap();
        initialize_database(&connection).unwrap();
        let label = crate::accounts::labels::Label {
            color: "#f59e0b".into(),
            group: "Artists".into(),
            tags: vec!["art".into()],
        };
        save_labels_on(&mut connection, &[("demo".into(), label)]).unwrap();
        save_entry_on(
            &connection,
            &HistoryEntry {
                id: "1".into(),
                username: "demo".into(),
                ..Default::default()
            },
        )
        .unwrap();
        clear_database_on(&connection, false).unwrap();
        assert_eq!(
            connection
                .query_row(
                    "SELECT group_name FROM account_labels WHERE handle='demo'",
                    [],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            "Artists"
        );
        save_labels_on(
            &mut connection,
            &[("demo".into(), crate::accounts::labels::Label::default())],
        )
        .unwrap();
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM account_labels", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
    #[test]
    fn conversion_history_transaction_rolls_back_all_references_on_error() {
        let mut connection = Connection::open_in_memory().unwrap();
        initialize_database(&connection).unwrap();
        connection.execute_batch("CREATE TRIGGER reject_second BEFORE INSERT ON history_entries WHEN NEW.id = '2' BEGIN SELECT RAISE(ABORT, 'test failure'); END;").unwrap();
        let entries = [
            HistoryEntry {
                id: "1".into(),
                ..Default::default()
            },
            HistoryEntry {
                id: "2".into(),
                ..Default::default()
            },
        ];
        assert!(save_batch_on(&mut connection, &entries).is_err());
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM history_entries", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        connection
            .execute_batch("DROP TRIGGER reject_second")
            .unwrap();
        save_batch_on(&mut connection, &entries).unwrap();
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM history_entries", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            2
        );
    }
    #[test]
    fn batch_fetch_history_keeps_more_than_thirty_results() {
        let connection = Connection::open_in_memory().unwrap();
        initialize_database(&connection).unwrap();
        for index in 0..50 {
            let entry = HistoryEntry {
                id: (1000 + index).to_string(),
                username: format!("account{index}"),
                source_url: format!("https://x.com/account{index}"),
                ..Default::default()
            };
            save_entry_on(&connection, &entry).unwrap();
        }
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM history_entries", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            50
        );
    }
    #[test]
    fn downloads_migrate_once_and_survive_fetch_removal_and_missing_files() {
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let path = fixture.0.join("saved.jpg");
        std::fs::write(&path, b"download").unwrap();
        let connection = Connection::open_in_memory().unwrap();
        initialize_database(&connection).unwrap();
        connection
            .execute("ALTER TABLE history_entries DROP COLUMN download_json", [])
            .unwrap();
        initialize_database(&connection).unwrap();
        let fetch = HistoryEntry {
            id: "123".into(),
            username: "demo".into(),
            source_url: "https://x.com/demo".into(),
            media: vec![MediaCandidate {
                status: "Saved".into(),
                saved_path: path.to_string_lossy().into_owned(),
                ..Default::default()
            }],
            ..Default::default()
        };
        save_entry_on(&connection, &fetch).unwrap();
        let mut entries = read_entries(&connection).unwrap();
        migrate_downloads(&connection, &mut entries).unwrap();
        assert_eq!(entries.len(), 2);
        let legacy = entries
            .iter()
            .find(|entry| entry.download.is_some())
            .unwrap();
        assert!(legacy.download.as_ref().unwrap().earlier);
        assert!(legacy.fetched_at.is_empty());
        assert_eq!(legacy.download.as_ref().unwrap().total_bytes, 8);
        clear_database_on(&connection, true).unwrap();
        let mut entries = read_entries(&connection).unwrap();
        assert_eq!(entries.len(), 1);
        migrate_downloads(&connection, &mut entries).unwrap();
        assert_eq!(entries.len(), 1, "Cleared downloads were imported again");
        let batch =
            crate::download::commands::make_entry(&fetch, fetch.media.clone(), "photo", false)
                .unwrap();
        save_entry_on(&connection, &batch).unwrap();
        clear_database_on(&connection, false).unwrap();
        let entries = read_entries(&connection).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, batch.id);
        assert!(!entries[0].download.as_ref().unwrap().earlier);
        assert_eq!(entries[0].download.as_ref().unwrap().media_filter, "photo");
        assert!(path.is_file(), "Removing history deleted a downloaded file");
        std::fs::remove_file(&path).unwrap();
        let restored = read_entries(&connection).unwrap();
        assert_eq!(restored[0].media[0].saved_path, path.to_string_lossy());
        use crate::ui::runtime::Model;
        assert!(
            !crate::download::commands::files(&restored[0])
                .row_data(0)
                .unwrap()
                .present
        );
    }
    #[test]
    fn history_preserves_urls_and_downloads_without_thumbnail_cache_paths() {
        let media: Vec<MediaCandidate> = serde_json::from_str(r#"[{"url":"https://example.test/video.mp4","thumb_url":"https://example.test/thumb.jpg","preview_path":"C:/old/previews/thumb.png","detail_preview_path":"C:/old/previews/detail.png","saved_path":"D:/Downloads/video.mp4"}]"#).unwrap();
        let stored = serialize_media(&media).unwrap();
        assert!(!stored.contains("preview_path"));
        let restored: Vec<MediaCandidate> = serde_json::from_str(&stored).unwrap();
        assert_eq!(restored[0].thumb_url, media[0].thumb_url);
        assert_eq!(restored[0].url, media[0].url);
        assert_eq!(restored[0].saved_path, media[0].saved_path);
    }
}

pub fn load_account_labels()
-> Result<std::collections::BTreeMap<String, crate::accounts::labels::Label>, String> {
    let connection = open_database().map_err(|e| e.to_string())?;
    let mut statement = connection
        .prepare("SELECT handle,group_name,tags_json,color FROM account_labels")
        .map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut labels = std::collections::BTreeMap::new();
    for row in rows {
        let (handle, group, tags, color) = row.map_err(|e| e.to_string())?;
        labels.insert(
            handle,
            crate::accounts::labels::Label {
                group,
                color,
                tags: serde_json::from_str(&tags).map_err(|e| e.to_string())?,
            },
        );
    }
    Ok(labels)
}
pub fn save_account_labels(
    values: Vec<(String, crate::accounts::labels::Label)>,
) -> Result<(), String> {
    let mut connection = open_database().map_err(|e| e.to_string())?;
    save_labels_on(&mut connection, &values).map_err(|e| e.to_string())
}
fn save_labels_on(
    connection: &mut Connection,
    values: &[(String, crate::accounts::labels::Label)],
) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    for (handle, label) in values {
        transaction.execute("INSERT INTO account_labels(handle,group_name,tags_json,color) VALUES(?1,?2,?3,?4) ON CONFLICT(handle) DO UPDATE SET group_name=excluded.group_name,tags_json=excluded.tags_json,color=excluded.color",params![handle,label.group,serde_json::to_string(&label.tags).unwrap(),label.color])?;
    }
    transaction.commit()
}

#[cfg(test)]
mod archive_tests {
    use super::*;
    #[test]
    fn merged_archive_survives_reload_without_rewriting_or_removing_raw_sessions() {
        let mut connection = Connection::open_in_memory().unwrap();
        initialize_database(&connection).unwrap();
        let make = |id: &str, assets: &[&str]| HistoryEntry {
            id: id.into(),
            username: "demo".into(),
            source_url: "https://x.com/demo".into(),
            profile: Some(Default::default()),
            media: assets
                .iter()
                .map(|asset| MediaCandidate {
                    tweet_id: (*asset).into(),
                    url: format!("https://pbs.twimg.com/media/{asset}.jpg"),
                    media_type: "photo".into(),
                    status: "Ready".into(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        save_batch_on(
            &mut connection,
            &[make("100", &["A", "B"]), make("200", &["B", "C"])],
        )
        .unwrap();
        let mut entries = read_entries(&connection).unwrap();
        let archive = crate::fetch::archives::open(&entries, "200", false).unwrap();
        assert_eq!(archive.media.len(), 3);
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let path = fixture.0.join("A.jpg");
        std::fs::write(&path, b"photo").unwrap();
        let mut updated = archive.media;
        let saved = updated
            .iter_mut()
            .find(|item| item.tweet_id == "A")
            .unwrap();
        saved.saved_path = path.to_string_lossy().into_owned();
        saved.status = "Saved".into();
        let changed = crate::fetch::archives::update_saved(&mut entries, &updated);
        for (id, media) in changed {
            let entry = entries.iter().find(|entry| entry.id == id).unwrap();
            let mut entry = entry.clone();
            entry.media = media;
            save_entry_on(&connection, &entry).unwrap();
        }
        let reloaded = read_entries(&connection).unwrap();
        assert_eq!(reloaded.len(), 2);
        assert!(reloaded.iter().all(|entry| entry.media.len() == 2));
        let archive = crate::fetch::archives::open(&reloaded, "200", false).unwrap();
        assert_eq!(archive.media.len(), 3);
        assert_eq!(
            archive
                .media
                .iter()
                .filter(|item| item.status == "Saved")
                .count(),
            1
        );
        // Removing one session removes only its unique assets from the derived archive.
        connection
            .execute("DELETE FROM history_entries WHERE id='100'", [])
            .unwrap();
        let remaining = read_entries(&connection).unwrap();
        assert_eq!(
            crate::fetch::archives::open(&remaining, "200", false)
                .unwrap()
                .media
                .len(),
            2
        );
        assert!(path.is_file());
    }
}

pub fn restore_database(path: std::path::PathBuf) -> Result<(), String> {
    let (done, receiver) = mpsc::sync_channel(1);
    writer()
        .send(Command::Restore(path, done))
        .map_err(|e| e.to_string())?;
    receiver.recv().map_err(|e| e.to_string())?
}
