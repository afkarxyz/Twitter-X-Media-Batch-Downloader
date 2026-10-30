use crate::download::engine::{HistoryEntry, MediaCandidate};
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::PathBuf,
    sync::Arc,
};

pub const PAGE_SIZE: usize = 64;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Summary {
    pub count: usize,
    pub saved: usize,
    pub bytes: u64,
    pub kinds: BTreeMap<String, usize>,
}

#[derive(Clone, Debug)]
pub struct StoredEntry {
    pub path: Arc<PathBuf>,
    pub id: String,
    pub summary: Summary,
}

fn data_error(error: impl std::error::Error + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(error))
}

pub fn compress(bytes: &[u8]) -> rusqlite::Result<Vec<u8>> {
    let mut writer = ZlibEncoder::new(Vec::new(), Compression::default());
    writer.write_all(bytes).map_err(data_error)?;
    writer.finish().map_err(data_error)
}

pub fn decompress(bytes: &[u8]) -> rusqlite::Result<Vec<u8>> {
    let mut output = Vec::new();
    ZlibDecoder::new(bytes)
        .read_to_end(&mut output)
        .map_err(data_error)?;
    Ok(output)
}

pub fn initialize(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS history_payloads(
            entry_id TEXT PRIMARY KEY REFERENCES history_entries(id) ON DELETE CASCADE,
            response_zlib BLOB NOT NULL, summary_json TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS history_media_pages(
            entry_id TEXT NOT NULL REFERENCES history_entries(id) ON DELETE CASCADE,
            page INTEGER NOT NULL, payload_zlib BLOB NOT NULL,
            PRIMARY KEY(entry_id,page)
         ) WITHOUT ROWID;
         CREATE TABLE IF NOT EXISTS history_media_index(
            entry_id TEXT NOT NULL REFERENCES history_entries(id) ON DELETE CASCADE,
            ordinal INTEGER NOT NULL, media_index INTEGER NOT NULL,
            identity BLOB NOT NULL, author TEXT NOT NULL, kind TEXT NOT NULL,
            status TEXT NOT NULL, saved_path TEXT NOT NULL,
            saved_bytes INTEGER NOT NULL, is_new INTEGER NOT NULL,
            sort_stamp INTEGER, PRIMARY KEY(entry_id,ordinal)
         ) WITHOUT ROWID;
         CREATE INDEX IF NOT EXISTS history_media_author ON history_media_index(author,entry_id);
         CREATE INDEX IF NOT EXISTS history_media_identity ON history_media_index(identity,entry_id);
         CREATE INDEX IF NOT EXISTS history_media_saved ON history_media_index(entry_id,saved_path,saved_bytes) WHERE saved_path!='';"
    )
}

pub fn account_json(response: &str) -> rusqlite::Result<String> {
    fn first_author<'de, D: serde::Deserializer<'de>>(
        reader: D,
    ) -> Result<Option<serde_json::Value>, D::Error> {
        struct First;
        impl<'de> serde::de::Visitor<'de> for First {
            type Value = Option<serde_json::Value>;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("tweets")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Self::Value, A::Error> {
                let first: Option<serde_json::Value> = sequence.next_element()?;
                while sequence.next_element::<serde::de::IgnoredAny>()?.is_some() {}
                Ok(first
                    .and_then(|value| value.get("author").or_else(|| value.get("user")).cloned()))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                while map
                    .next_entry::<serde::de::IgnoredAny, serde::de::IgnoredAny>()?
                    .is_some()
                {}
                Ok(None)
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(None)
            }
            fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<Self::Value, E> {
                Ok(None)
            }
            fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<Self::Value, E> {
                Ok(None)
            }
            fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<Self::Value, E> {
                Ok(None)
            }
            fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<Self::Value, E> {
                Ok(None)
            }
            fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<Self::Value, E> {
                Ok(None)
            }
        }
        reader.deserialize_any(First)
    }
    #[derive(Deserialize)]
    struct Account {
        account: Option<serde_json::Value>,
        account_info: Option<serde_json::Value>,
        #[serde(default, deserialize_with = "first_author")]
        tweets: Option<serde_json::Value>,
    }
    let data: Account = match serde_json::from_str(response) {
        Ok(data) => data,
        Err(_) => return Ok("{}".into()),
    };
    let value = if let Some(account) = data.account {
        serde_json::json!({"account":account})
    } else if let Some(account) = data.account_info {
        serde_json::json!({"account_info":account})
    } else if let Some(author) = data.tweets {
        serde_json::json!({"tweets":[{"author":author}]})
    } else {
        serde_json::json!({})
    };
    serde_json::to_string(&value).map_err(data_error)
}

pub fn write_payload(
    connection: &Connection,
    id: &str,
    username: &str,
    media: &[MediaCandidate],
    response: &str,
) -> rusqlite::Result<Summary> {
    let response = compress(response.as_bytes())?;
    let summary = write_media(connection, id, username, media)?;
    connection.execute(
        "INSERT INTO history_payloads(entry_id,response_zlib,summary_json) VALUES(?1,?2,?3)
         ON CONFLICT(entry_id) DO UPDATE SET response_zlib=excluded.response_zlib,summary_json=excluded.summary_json",
        params![id, response, serde_json::to_string(&summary).map_err(data_error)?],
    )?;
    Ok(summary)
}

pub fn write_media(
    connection: &Connection,
    id: &str,
    username: &str,
    media: &[MediaCandidate],
) -> rusqlite::Result<Summary> {
    connection.execute("DELETE FROM history_media_pages WHERE entry_id=?1", [id])?;
    connection.execute("DELETE FROM history_media_index WHERE entry_id=?1", [id])?;
    let mut index = connection.prepare_cached(
        "INSERT INTO history_media_index(entry_id,ordinal,media_index,identity,author,kind,status,saved_path,saved_bytes,is_new,sort_stamp)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)"
    )?;
    let mut summary = Summary::default();
    for (ordinal, item) in media.iter().enumerate() {
        let author = if item.username.is_empty() {
            username
        } else {
            &item.username
        };
        index.execute(params![
            id,
            ordinal as i64,
            item.media_index as i64,
            Sha256::digest(crate::media::identity::key(item).as_bytes()).as_slice(),
            author.trim_start_matches('@').to_ascii_lowercase(),
            item.media_type,
            item.status,
            item.saved_path,
            item.saved_bytes as i64,
            item.is_new,
            crate::media::identity::post_timestamp(item).map(|date| date.timestamp_millis())
        ])?;
        summary.count += 1;
        *summary.kinds.entry(item.media_type.clone()).or_default() += 1;
        if item.status == "Saved" {
            summary.saved += 1;
            summary.bytes += item.saved_bytes;
        }
    }
    let mut pages = connection.prepare_cached(
        "INSERT INTO history_media_pages(entry_id,page,payload_zlib) VALUES(?1,?2,?3)",
    )?;
    for (page, items) in media.chunks(PAGE_SIZE).enumerate() {
        pages.execute(params![
            id,
            page as i64,
            compress(&serde_json::to_vec(items).map_err(data_error)?)?
        ])?;
    }
    connection.execute(
        "UPDATE history_payloads SET summary_json=?2 WHERE entry_id=?1",
        params![id, serde_json::to_string(&summary).map_err(data_error)?],
    )?;
    Ok(summary)
}

pub fn read_page(
    connection: &Connection,
    id: &str,
    page: usize,
) -> rusqlite::Result<Vec<MediaCandidate>> {
    let data: Vec<u8> = connection.query_row(
        "SELECT payload_zlib FROM history_media_pages WHERE entry_id=?1 AND page=?2",
        params![id, page as i64],
        |row| row.get(0),
    )?;
    serde_json::from_slice(&decompress(&data)?).map_err(data_error)
}

pub fn read_media(connection: &Connection, id: &str) -> rusqlite::Result<Vec<MediaCandidate>> {
    let mut statement = connection
        .prepare("SELECT payload_zlib FROM history_media_pages WHERE entry_id=?1 ORDER BY page")?;
    let mut rows = statement.query([id])?;
    let mut result = Vec::new();
    while let Some(row) = rows.next()? {
        let data: Vec<u8> = row.get(0)?;
        result.extend(
            serde_json::from_slice::<Vec<MediaCandidate>>(&decompress(&data)?)
                .map_err(data_error)?,
        );
    }
    Ok(result)
}

pub fn read_response(connection: &Connection, id: &str) -> rusqlite::Result<String> {
    let bytes: Vec<u8> = connection.query_row(
        "SELECT response_zlib FROM history_payloads WHERE entry_id=?1",
        [id],
        |row| row.get(0),
    )?;
    String::from_utf8(decompress(&bytes)?).map_err(data_error)
}

pub fn migrate(connection: &Connection) -> rusqlite::Result<usize> {
    initialize(connection)?;
    let ids = connection
        .prepare(
            "SELECT id FROM history_entries WHERE id NOT IN(SELECT entry_id FROM history_payloads)",
        )?
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if ids.is_empty() {
        return Ok(0);
    }
    let transaction = connection.unchecked_transaction()?;
    for id in &ids {
        let (username, raw_media, response): (String, String, String) = transaction.query_row(
            "SELECT username,media_json,response_json FROM history_entries WHERE id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let mut media: Vec<MediaCandidate> =
            serde_json::from_str(&raw_media).map_err(data_error)?;
        crate::download::engine::restore_preview_details(&mut media, &response);
        write_payload(&transaction, id, &username, &media, &response)?;
        transaction.execute(
            "UPDATE history_entries SET media_json='[]',response_json=?2 WHERE id=?1",
            params![id, account_json(&response)?],
        )?;
    }
    transaction.execute("INSERT INTO history_metadata(key,value) VALUES('storage-v2','2') ON CONFLICT(key) DO UPDATE SET value='2'",[])?;
    transaction.commit()?;
    Ok(ids.len())
}

pub fn summary(connection: &Connection, id: &str) -> rusqlite::Result<Option<Summary>> {
    let value: Option<String> = connection
        .query_row(
            "SELECT summary_json FROM history_payloads WHERE entry_id=?1",
            [id],
            |row| row.get(0),
        )
        .optional()?;
    value
        .map(|text| serde_json::from_str(&text).map_err(data_error))
        .transpose()
}

pub fn validate(connection: &Connection) -> rusqlite::Result<()> {
    fn invalid() -> rusqlite::Error {
        data_error(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Saved media database is inconsistent. Restore a backup.",
        ))
    }
    if connection
        .prepare("PRAGMA foreign_key_check")?
        .query([])?
        .next()?
        .is_some()
    {
        return Err(invalid());
    }
    let ids = connection
        .prepare("SELECT id FROM history_entries")?
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for id in ids {
        if let Some(summary) = summary(connection, &id)? {
            let (count,minimum,maximum):(i64,Option<i64>,Option<i64>)=connection.query_row("SELECT COUNT(*),MIN(ordinal),MAX(ordinal) FROM history_media_index WHERE entry_id=?1",[&id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?)))?;
            if count as usize != summary.count
                || (count > 0 && (minimum != Some(0) || maximum != Some(count - 1)))
            {
                return Err(invalid());
            }
            let pages: i64 = connection.query_row(
                "SELECT COUNT(*) FROM history_media_pages WHERE entry_id=?1",
                [&id],
                |row| row.get(0),
            )?;
            if pages as usize != summary.count.div_ceil(PAGE_SIZE) {
                return Err(invalid());
            }
            for page in 0..pages as usize {
                let items = read_page(connection, &id, page)?;
                if items.len() != PAGE_SIZE.min(summary.count - page * PAGE_SIZE) {
                    return Err(invalid());
                }
            }
            let response = read_response(connection, &id)?;
            if !response.is_empty() {
                serde_json::from_str::<serde::de::IgnoredAny>(&response).map_err(data_error)?;
            }
        } else {
            let (media, response): (String, String) = connection.query_row(
                "SELECT media_json,response_json FROM history_entries WHERE id=?1",
                [&id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            serde_json::from_str::<Vec<MediaCandidate>>(&media).map_err(data_error)?;
            if !response.is_empty() {
                serde_json::from_str::<serde::de::IgnoredAny>(&response).map_err(data_error)?;
            }
        }
    }
    Ok(())
}

pub fn resolve(entry: &HistoryEntry) -> rusqlite::Result<HistoryEntry> {
    let Some(stored) = &entry.stored else {
        return Ok(entry.clone());
    };
    let connection = Connection::open_with_flags(
        stored.path.as_ref(),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    let mut entry = entry.clone();
    entry.media = read_media(&connection, &stored.id)?;
    entry.json = read_response(&connection, &stored.id)?;
    entry.stored = None;
    Ok(entry)
}

pub fn resolve_media(entry: &HistoryEntry) -> rusqlite::Result<HistoryEntry> {
    let Some(stored) = &entry.stored else {
        return Ok(entry.clone());
    };
    let connection = Connection::open_with_flags(
        stored.path.as_ref(),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    let mut resolved = entry.clone();
    resolved.media = read_media(&connection, &stored.id)?;
    resolved.stored = None;
    Ok(resolved)
}

pub fn library_entries(entries: &[HistoryEntry]) -> rusqlite::Result<Vec<HistoryEntry>> {
    let mut loaded = Vec::new();
    let mut members = std::collections::HashSet::new();
    for entry in entries.iter().filter(|entry| entry.download.is_some()) {
        let entry = resolve_media(entry)?;
        for item in &entry.media {
            members.insert(if item.username.is_empty() {
                entry.username.to_ascii_lowercase()
            } else {
                item.username.to_ascii_lowercase()
            });
        }
        loaded.push(entry);
    }
    if members.is_empty() {
        return Ok(loaded);
    }
    let authors = serde_json::to_string(&members).map_err(data_error)?;
    for entry in entries.iter().filter(|entry| entry.download.is_none()) {
        let Some(stored) = &entry.stored else {
            loaded.push(entry.clone());
            continue;
        };
        let connection = Connection::open_with_flags(
            stored.path.as_ref(),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        let ordinals=connection.prepare("SELECT ordinal FROM history_media_index WHERE entry_id=?1 AND author IN(SELECT value FROM json_each(?2)) ORDER BY ordinal")?
            .query_map(params![stored.id,authors],|row|row.get::<_,i64>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let mut media = Vec::new();
        let mut page = None;
        for ordinal in ordinals {
            let ordinal = ordinal as usize;
            let number = ordinal / PAGE_SIZE;
            if page
                .as_ref()
                .is_none_or(|(previous, _)| *previous != number)
            {
                page = Some((number, read_page(&connection, &stored.id, number)?));
            }
            media.push(page.as_ref().unwrap().1[ordinal % PAGE_SIZE].clone());
        }
        let mut resolved = entry.clone();
        resolved.media = media;
        resolved.stored = None;
        loaded.push(resolved);
    }
    let order = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.id.as_str(), index))
        .collect::<std::collections::HashMap<_, _>>();
    loaded.sort_by_key(|entry| order.get(entry.id.as_str()).copied().unwrap_or(usize::MAX));
    Ok(loaded)
}

pub fn saved_media(entry: &HistoryEntry) -> rusqlite::Result<Vec<MediaCandidate>> {
    let Some(stored) = &entry.stored else {
        return Ok(entry
            .media
            .iter()
            .filter(|item| !item.saved_path.is_empty())
            .cloned()
            .collect());
    };
    let connection = Connection::open_with_flags(
        stored.path.as_ref(),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    let ordinals=connection.prepare("SELECT ordinal FROM history_media_index WHERE entry_id=?1 AND saved_path!='' ORDER BY ordinal")?
        .query_map([&stored.id],|row|row.get::<_,i64>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let mut result = Vec::new();
    let mut cached = None;
    for ordinal in ordinals {
        let ordinal = ordinal as usize;
        let page = ordinal / PAGE_SIZE;
        if cached
            .as_ref()
            .is_none_or(|(previous, _)| *previous != page)
        {
            cached = Some((page, read_page(&connection, &stored.id, page)?));
        }
        result.push(cached.as_ref().unwrap().1[ordinal % PAGE_SIZE].clone());
    }
    Ok(result)
}

pub fn saved_paths(
    entries: &[HistoryEntry],
) -> rusqlite::Result<std::collections::HashSet<(String, u64)>> {
    let mut result = std::collections::HashSet::new();
    for entry in entries {
        if let Some(stored) = &entry.stored {
            let connection = Connection::open_with_flags(
                stored.path.as_ref(),
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )?;
            let mut statement=connection.prepare("SELECT saved_path,saved_bytes FROM history_media_index WHERE entry_id=?1 AND saved_path!=''")?;
            result.extend(
                statement
                    .query_map([&stored.id], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, i64>(1)?.max(0) as u64,
                        ))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?,
            );
        } else {
            result.extend(
                entry
                    .media
                    .iter()
                    .filter(|item| !item.saved_path.is_empty())
                    .map(|item| (item.saved_path.clone(), item.saved_bytes)),
            );
        }
    }
    Ok(result)
}

pub fn missing_media(
    entry: &HistoryEntry,
    missing: &std::collections::HashSet<String>,
) -> rusqlite::Result<Vec<MediaCandidate>> {
    let Some(stored) = &entry.stored else {
        return Ok(entry
            .media
            .iter()
            .filter(|item| item.status == "Saved" && missing.contains(&item.saved_path))
            .cloned()
            .collect());
    };
    let connection = Connection::open_with_flags(
        stored.path.as_ref(),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    connection.execute_batch("BEGIN")?;
    let mut ordinals = Vec::new();
    let mut statement=connection.prepare("SELECT ordinal,saved_path FROM history_media_index WHERE entry_id=?1 AND saved_path!='' AND status='Saved' ORDER BY ordinal")?;
    let mut rows = statement.query([&stored.id])?;
    while let Some(row) = rows.next()? {
        let path: String = row.get(1)?;
        if missing.contains(&path) {
            ordinals.push(row.get::<_, i64>(0)? as usize);
        }
    }
    drop(rows);
    drop(statement);
    let mut cached = None;
    let mut result = Vec::new();
    for ordinal in ordinals {
        let page = ordinal / PAGE_SIZE;
        if cached
            .as_ref()
            .is_none_or(|(previous, _)| *previous != page)
        {
            cached = Some((page, read_page(&connection, &stored.id, page)?));
        }
        result.push(cached.as_ref().unwrap().1[ordinal % PAGE_SIZE].clone());
    }
    Ok(result)
}

pub fn identities(entry: &HistoryEntry) -> rusqlite::Result<std::collections::HashSet<Vec<u8>>> {
    if let Some(stored) = &entry.stored {
        let connection = Connection::open_with_flags(
            stored.path.as_ref(),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        return connection
            .prepare("SELECT identity FROM history_media_index WHERE entry_id=?1")?
            .query_map([&stored.id], |row| row.get::<_, Vec<u8>>(0))?
            .collect();
    }
    Ok(entry
        .media
        .iter()
        .map(|item| Sha256::digest(crate::media::identity::key(item).as_bytes()).to_vec())
        .collect())
}

pub fn references_path(stored: &StoredEntry, path: &std::path::Path) -> rusqlite::Result<bool> {
    let connection = Connection::open_with_flags(
        stored.path.as_ref(),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    let sql = if cfg!(windows) {
        "SELECT 1 FROM history_media_index WHERE entry_id=?1 AND saved_path!='' AND saved_path=?2 COLLATE NOCASE LIMIT 1"
    } else {
        "SELECT 1 FROM history_media_index WHERE entry_id=?1 AND saved_path!='' AND saved_path=?2 LIMIT 1"
    };
    Ok(connection
        .query_row(sql, params![stored.id, path.to_string_lossy()], |row| {
            row.get::<_, i64>(0)
        })
        .optional()?
        .is_some())
}

pub fn patch_saved(
    stored: &StoredEntry,
    updated: &[MediaCandidate],
) -> rusqlite::Result<Option<Summary>> {
    patch_saved_checked(stored, updated, false)
}

pub fn patch_saved_checked(
    stored: &StoredEntry,
    updated: &[MediaCandidate],
    force: bool,
) -> rusqlite::Result<Option<Summary>> {
    let connection = Connection::open_with_flags(
        stored.path.as_ref(),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE,
    )?;
    connection.busy_timeout(std::time::Duration::from_secs(5))?;
    let transaction = connection.unchecked_transaction()?;
    let download: bool = transaction.query_row(
        "SELECT download_json IS NOT NULL FROM history_entries WHERE id=?1",
        [&stored.id],
        |row| row.get(0),
    )?;
    let states = updated
        .iter()
        .map(|item| {
            (
                Sha256::digest(crate::media::identity::key(item).as_bytes()).to_vec(),
                item,
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    let mut pages: BTreeMap<usize, Vec<(usize, &MediaCandidate)>> = BTreeMap::new();
    if download {
        let mut statement=transaction.prepare("SELECT ordinal FROM history_media_index WHERE entry_id=?1 AND identity=?2 AND saved_path=?3")?;
        for state in updated {
            let identity = Sha256::digest(crate::media::identity::key(state).as_bytes());
            for ordinal in statement.query_map(
                params![stored.id, identity.as_slice(), state.saved_path],
                |row| row.get::<_, i64>(0),
            )? {
                let ordinal = ordinal? as usize;
                pages
                    .entry(ordinal / PAGE_SIZE)
                    .or_default()
                    .push((ordinal % PAGE_SIZE, state));
            }
        }
    } else if states.len() <= 64 {
        let mut statement = transaction
            .prepare("SELECT ordinal FROM history_media_index WHERE entry_id=?1 AND identity=?2")?;
        for (identity, state) in &states {
            for ordinal in
                statement.query_map(params![stored.id, identity], |row| row.get::<_, i64>(0))?
            {
                let ordinal = ordinal? as usize;
                pages
                    .entry(ordinal / PAGE_SIZE)
                    .or_default()
                    .push((ordinal % PAGE_SIZE, *state));
            }
        }
    } else {
        let mut statement = transaction
            .prepare("SELECT ordinal,identity FROM history_media_index WHERE entry_id=?1")?;
        let mut cursor = statement.query([&stored.id])?;
        while let Some(row) = cursor.next()? {
            let identity: Vec<u8> = row.get(1)?;
            if let Some(state) = states.get(&identity) {
                let ordinal = row.get::<_, i64>(0)? as usize;
                pages
                    .entry(ordinal / PAGE_SIZE)
                    .or_default()
                    .push((ordinal % PAGE_SIZE, *state));
            }
        }
    }
    let mut changed = false;
    for (page, updates) in pages {
        let mut items = read_page(&transaction, &stored.id, page)?;
        let mut dirty = false;
        for (slot, state) in updates {
            let item = &mut items[slot];
            if !force
                && crate::download::saved_files::exists(item)
                && !crate::download::saved_files::exists(state)
            {
                continue;
            }
            if item.status == state.status
                && item.saved_path == state.saved_path
                && item.saved_bytes == state.saved_bytes
                && item.saved_sha256 == state.saved_sha256
                && item.gif_output_path == state.gif_output_path
            {
                continue;
            }
            item.status.clone_from(&state.status);
            item.saved_path.clone_from(&state.saved_path);
            item.saved_bytes = state.saved_bytes;
            item.saved_sha256.clone_from(&state.saved_sha256);
            item.gif_output_path.clone_from(&state.gif_output_path);
            item.integrity_error.clone_from(&state.integrity_error);
            transaction.execute("UPDATE history_media_index SET status=?3,saved_path=?4,saved_bytes=?5 WHERE entry_id=?1 AND ordinal=?2",params![stored.id,(page*PAGE_SIZE+slot) as i64,item.status,item.saved_path,item.saved_bytes as i64])?;
            dirty = true;
        }
        if dirty {
            transaction.execute(
                "UPDATE history_media_pages SET payload_zlib=?3 WHERE entry_id=?1 AND page=?2",
                params![
                    stored.id,
                    page as i64,
                    compress(&serde_json::to_vec(&items).map_err(data_error)?)?
                ],
            )?;
            changed = true;
        }
    }
    if !changed {
        return Ok(None);
    }
    let mut summary = stored.summary.clone();
    let (saved,bytes):(i64,i64)=transaction.query_row("SELECT COUNT(*),COALESCE(SUM(saved_bytes),0) FROM history_media_index WHERE entry_id=?1 AND status='Saved'",[&stored.id],|row|Ok((row.get(0)?,row.get(1)?)))?;
    summary.saved = saved as usize;
    summary.bytes = bytes as u64;
    transaction.execute(
        "UPDATE history_payloads SET summary_json=?2 WHERE entry_id=?1",
        params![
            stored.id,
            serde_json::to_string(&summary).map_err(data_error)?
        ],
    )?;
    transaction.commit()?;
    Ok(Some(summary))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::runtime::Model;
    #[test]
    fn compressed_pages_preserve_payloads_and_invalid_legacy_rows_roll_back() {
        let connection = Connection::open_in_memory().unwrap();
        crate::history::db::initialize_database(&connection).unwrap();
        let media = (0..150)
            .map(|index| MediaCandidate {
                media_index: index,
                username: "demo".into(),
                tweet_id: index.to_string(),
                url: format!("https://pbs.twimg.com/media/{index}.jpg?name=orig"),
                media_type: "photo".into(),
                tweet_text: "日本語 café".into(),
                saved_path: if index == 0 {
                    "missing.jpg".into()
                } else {
                    String::new()
                },
                status: if index == 0 {
                    "Saved".into()
                } else {
                    "Ready".into()
                },
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let raw = serde_json::to_string(&media).unwrap();
        let response =
            "{\"account\":{\"username\":\"demo\"},\"tweets\":[],\"extra\":\"unchanged\"}";
        connection.execute("INSERT INTO history_entries(id,fetched_at,source_url,media_json,response_json) VALUES('1','today','demo',?1,?2)",params![raw,response]).unwrap();
        assert_eq!(migrate(&connection).unwrap(), 1);
        assert_eq!(migrate(&connection).unwrap(), 0);
        assert_eq!(read_response(&connection, "1").unwrap(), response);
        assert_eq!(read_media(&connection, "1").unwrap(), media);
        assert_eq!(read_page(&connection, "1", 1).unwrap(), media[64..128]);
        assert_eq!(summary(&connection, "1").unwrap().unwrap().count, 150);
        connection.execute("INSERT INTO history_entries(id,fetched_at,source_url,media_json,response_json) VALUES('2','','','[]','{}'),('3','','','broken','{}')",[]).unwrap();
        assert!(migrate(&connection).is_err());
        assert!(summary(&connection, "2").unwrap().is_none());
        assert_eq!(read_response(&connection, "1").unwrap(), response);
    }

    #[test]
    fn stored_references_survive_conversion_missing_files_and_atomic_write_failures() {
        use crate::download::engine::{DownloadRecord, ExtractionProfile};
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let path = fixture.0.join("references.sqlite3");
        let connection = Connection::open(&path).unwrap();
        crate::history::db::initialize_database(&connection).unwrap();
        let input = fixture.0.join("original.mp4");
        let output = fixture.0.join("converted.gif");
        std::fs::write(&input, b"original").unwrap();
        std::fs::write(&output, b"converted").unwrap();
        let raw = r#"{"account":{"username":"demo"},"tweets":[],"opaque":"preserved"}"#;
        let candidate = MediaCandidate {
            media_index: 42,
            username: "demo".into(),
            tweet_id: "10".into(),
            media_type: "animated_gif".into(),
            url: "https://example.invalid/loop.mp4".into(),
            saved_path: input.to_string_lossy().into_owned(),
            saved_bytes: 8,
            status: "Saved".into(),
            ..Default::default()
        };
        let fetch = HistoryEntry {
            id: "100".into(),
            username: "demo".into(),
            source_url: "https://x.com/demo".into(),
            json: raw.into(),
            media: vec![candidate],
            profile: Some(ExtractionProfile {
                url: "https://x.com/demo".into(),
                media_type: "all".into(),
                retweets: "skip".into(),
                ..Default::default()
            }),
            next_cursor: "retained-cursor".into(),
            has_more: true,
            ..Default::default()
        };
        crate::history::db::save_entry_on(&connection, &fetch).unwrap();
        let mut download = fetch.clone();
        download.id = "200".into();
        download.download = Some(DownloadRecord {
            output_dir: fixture.0.to_string_lossy().into_owned(),
            ..Default::default()
        });
        crate::history::db::save_entry_on(&connection, &download).unwrap();
        let mut entries =
            crate::history::db::read_summaries(&connection, Arc::new(path.clone())).unwrap();
        let changes =
            crate::download::conversion::update_entries(&mut entries, &input, &output, true);
        assert_eq!(changes.len(), 2);
        let transaction = connection.unchecked_transaction().unwrap();
        for entry in &changes {
            crate::history::db::save_entry_on(&transaction, entry).unwrap();
        }
        transaction.commit().unwrap();
        assert_eq!(read_response(&connection, "100").unwrap(), raw);
        let snapshot = read_media(&connection, "200").unwrap();
        assert_eq!(snapshot.len(), 2);
        assert!(
            snapshot
                .iter()
                .any(|item| item.saved_path == input.to_string_lossy())
        );
        assert!(
            snapshot
                .iter()
                .any(|item| item.saved_path == output.to_string_lossy())
        );
        entries = crate::history::db::read_summaries(&connection, Arc::new(path.clone())).unwrap();
        std::fs::remove_file(&input).unwrap();
        std::fs::remove_file(&output).unwrap();
        let missing = std::collections::HashSet::from([
            input.to_string_lossy().into_owned(),
            output.to_string_lossy().into_owned(),
        ]);
        for entry in &entries {
            let mut items = missing_media(entry, &missing).unwrap();
            for item in &mut items {
                if entry.download.is_some() {
                    item.status = "Missing".into();
                } else {
                    item.status = "Ready".into();
                    item.saved_path.clear();
                    item.saved_bytes = 0;
                    item.saved_sha256.clear();
                }
            }
            patch_saved(entry.stored.as_ref().unwrap(), &items).unwrap();
        }
        let snapshot = read_media(&connection, "200").unwrap();
        assert_eq!(snapshot.len(), 2);
        assert!(snapshot.iter().all(|item| item.status == "Missing"));
        assert_ne!(
            snapshot[0].saved_path, snapshot[1].saved_path,
            "Independent MP4/GIF paths were overwritten"
        );
        assert!(
            read_media(&connection, "100").unwrap()[0]
                .saved_path
                .is_empty()
        );
        assert_eq!(summary(&connection, "100").unwrap().unwrap().saved, 0);
        assert_eq!(read_response(&connection, "100").unwrap(), raw);
        assert!(
            references_path(
                entries
                    .iter()
                    .find(|entry| entry.id == "200")
                    .unwrap()
                    .stored
                    .as_ref()
                    .unwrap(),
                &input
            )
            .unwrap()
        );
        assert!(
            crate::fetch::archives::try_open(&entries, "100", true)
                .unwrap()
                .is_some()
        );
        validate(&connection).unwrap();
        let before = read_response(&connection, "100").unwrap();
        let transaction = connection.unchecked_transaction().unwrap();
        let mut rewritten = fetch.clone();
        rewritten.json = r#"{"changed":true}"#.into();
        crate::history::db::save_entry_on(&transaction, &rewritten).unwrap();
        assert!(crate::history::db::save_entry_on(&transaction, &entries[0]).is_err());
        drop(transaction);
        assert_eq!(
            read_response(&connection, "100").unwrap(),
            before,
            "Failed writes committed a partial update"
        );
        connection.execute("UPDATE history_media_pages SET payload_zlib=x'000102' WHERE entry_id='100' AND page=0",[]).unwrap();
        assert!(validate(&connection).is_err());
        assert!(crate::fetch::archives::try_open(&entries, "100", true).is_err());
    }

    #[test]
    #[ignore = "Migrates and audits an explicitly supplied database snapshot without modifying its source"]
    fn audit_large_snapshot_migration() {
        use std::time::{Duration, Instant};
        let directory = std::path::PathBuf::from(
            std::env::var("LAZY_HISTORY_REVIEW_DIR")
                .expect("Set a review directory containing history-original.sqlite3"),
        );
        let source = directory.join("history-original.sqlite3");
        let target = directory.join(format!(
            "migrated-review-{}.sqlite3",
            crate::download::engine::new_history_id()
        ));
        std::fs::copy(&source, &target).unwrap();
        let connection = Connection::open(&target).unwrap();
        crate::history::db::initialize_database(&connection).unwrap();
        let ids = connection
            .prepare("SELECT id FROM history_entries ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        fn fingerprints(
            connection: &Connection,
            ids: &[String],
            compact: bool,
        ) -> Vec<([u8; 32], [u8; 32], usize)> {
            ids.iter()
                .map(|id| {
                    let (media, response): (String, String) = connection
                        .query_row(
                            "SELECT media_json,response_json FROM history_entries WHERE id=?1",
                            [id],
                            |row| Ok((row.get(0)?, row.get(1)?)),
                        )
                        .unwrap();
                    let mut media: Vec<MediaCandidate> = if compact {
                        read_media(connection, id).unwrap()
                    } else {
                        serde_json::from_str(&media).unwrap()
                    };
                    let response = if compact {
                        read_response(connection, id).unwrap()
                    } else {
                        response
                    };
                    if !compact {
                        crate::download::engine::restore_preview_details(&mut media, &response);
                    }
                    (
                        Sha256::digest(serde_json::to_vec(&media).unwrap()).into(),
                        Sha256::digest(response.as_bytes()).into(),
                        media.len(),
                    )
                })
                .collect()
        }
        let before = fingerprints(&connection, &ids, false);
        fn headers(connection: &Connection, ids: &[String]) -> Vec<[u8; 32]> {
            ids.iter().map(|id| {
                let row=connection.query_row("SELECT fetched_at,source_url,account_name,username,tweet_count,profile_json,next_cursor,has_more,download_json FROM history_entries WHERE id=?1",[id],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?,row.get::<_,i64>(4)?,row.get::<_,Option<String>>(5)?,row.get::<_,String>(6)?,row.get::<_,i64>(7)?,row.get::<_,Option<String>>(8)?))).unwrap();
                Sha256::digest(format!("{row:?}").as_bytes()).into()
            }).collect()
        }
        let original_headers = headers(&connection, &ids);
        let original_labels: Vec<(String, String, String, String)> = connection
            .prepare("SELECT handle,group_name,tags_json,color FROM account_labels ORDER BY handle")
            .unwrap()
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        let start = Instant::now();
        assert_eq!(migrate(&connection).unwrap(), ids.len());
        let migration_ms = start.elapsed().as_millis();
        assert_eq!(
            headers(&connection, &ids),
            original_headers,
            "Fetch cursors, filters, account or download metadata changed"
        );
        validate(&connection).unwrap();
        assert_eq!(
            before,
            fingerprints(&connection, &ids, true),
            "Migrated media or raw response changed"
        );
        let labels: Vec<(String, String, String, String)> = connection
            .prepare("SELECT handle,group_name,tags_json,color FROM account_labels ORDER BY handle")
            .unwrap()
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(labels, original_labels);
        assert_eq!(migrate(&connection).unwrap(), 0);
        connection.pragma_update(None, "user_version", 2).unwrap();
        connection
            .execute_batch(
                "PRAGMA wal_checkpoint(TRUNCATE);VACUUM;PRAGMA wal_checkpoint(TRUNCATE);",
            )
            .unwrap();
        let integrity: String = connection
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .unwrap();
        assert_eq!(integrity, "ok");
        assert!(
            connection
                .prepare("PRAGMA foreign_key_check")
                .unwrap()
                .query([])
                .unwrap()
                .next()
                .unwrap()
                .is_none()
        );
        let start = Instant::now();
        let headers =
            crate::history::db::read_summaries(&connection, Arc::new(target.clone())).unwrap();
        let header_ms = start.elapsed().as_millis();
        assert_eq!(
            headers.iter().map(HistoryEntry::media_count).sum::<usize>(),
            before.iter().map(|row| row.2).sum::<usize>()
        );
        assert!(
            headers
                .iter()
                .all(|entry| entry.media.is_empty() && entry.json.len() < 100000)
        );
        let largest = headers
            .iter()
            .filter(|entry| entry.download.is_none())
            .max_by_key(|entry| entry.media_count())
            .unwrap();
        let start = Instant::now();
        let view = crate::history::paged::StoredView::prepare(&headers, &largest.id, true)
            .unwrap()
            .unwrap();
        let index_ms = start.elapsed().as_millis();
        let model = crate::history::paged::PagedModel::new(view);
        let first = model.ordered("all", 30, 0);
        assert_eq!(
            model
                .decoded_pages
                .load(std::sync::atomic::Ordering::Relaxed),
            0
        );
        let start = Instant::now();
        for position in 0..first.row_count() {
            first.row_data(position).unwrap();
        }
        let ctx = egui::Context::default();
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            crate::ui::runtime::pump(&ctx);
            if (0..first.row_count())
                .all(|position| !first.row_data(position).unwrap().payload_pending)
            {
                break;
            }
            assert!(Instant::now() < deadline, "First page did not load");
            std::thread::sleep(Duration::from_millis(5));
        }
        let first_ms = start.elapsed().as_millis();
        let pages = model
            .decoded_pages
            .load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(pages, 1);
        drop(connection);
        let report = serde_json::json!({"source_bytes":source.metadata().unwrap().len(),"compact_bytes":target.metadata().unwrap().len(),"records":ids.len(),"media":before.iter().map(|row|row.2).sum::<usize>(),"migration_ms":migration_ms,"header_ms":header_ms,"index_ms":index_ms,"first_30_ms":first_ms,"decoded_pages_for_30":pages,"integrity":"ok","media_and_response_hashes":"matched","headers":"matched","labels":"matched"});
        std::fs::write(
            directory.join("migration-report.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        std::fs::write(
            directory.join("review-db-path.txt"),
            target.to_string_lossy().as_bytes(),
        )
        .unwrap();
        eprintln!("{report}");
    }
}
