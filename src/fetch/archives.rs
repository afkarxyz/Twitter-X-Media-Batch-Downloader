//! One derived archive per source/filter group; SQLite keeps the original sessions.
use crate::download::engine::{HistoryEntry, MediaCandidate};
use crate::download::saved_files;
use crate::fetch::sessions as fetch_sessions;
use crate::media::identity as media_identity;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

fn matching<'a>(current: &HistoryEntry, history: &'a [HistoryEntry]) -> Vec<&'a HistoryEntry> {
    let key = fetch_sessions::key(current);
    let mut entries = history
        .iter()
        .filter(|entry| {
            entry.id != current.id && entry.download.is_none() && fetch_sessions::key(entry) == key
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.id.parse::<u64>().unwrap_or_default()));
    entries
}

fn media(current: &HistoryEntry, previous: &[&HistoryEntry]) -> Vec<MediaCandidate> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for (latest, entry) in
        std::iter::once((true, current)).chain(previous.iter().map(|entry| (false, *entry)))
    {
        for original in &entry.media {
            if seen.insert(media_identity::key(original)) {
                let mut item = original.clone();
                item.is_new &= latest;
                item.media_index = result.len();
                result.push(item);
            }
        }
    }
    saved_files::reconcile(&mut result);
    result
}

fn post_key(post: &Value) -> String {
    match post.get("tweet_id") {
        Some(Value::String(id)) if !id.is_empty() => format!("id:{id}"),
        Some(Value::Number(id)) => format!("id:{id}"),
        _ => format!("json:{post}"),
    }
}

fn merge_json(current: &HistoryEntry, previous: &[&HistoryEntry]) -> (String, usize) {
    let mut response = None;
    let mut posts: Vec<Value> = Vec::new();
    let mut indices = HashMap::new();
    let mut parsed = false;
    for entry in std::iter::once(current).chain(previous.iter().copied()) {
        let Ok(value) = serde_json::from_str::<Value>(&entry.json) else {
            continue;
        };
        if !value.is_object() {
            continue;
        }
        if response.is_none() {
            response = Some(value.clone());
        }
        let Some(tweets) = value.get("tweets").and_then(Value::as_array) else {
            continue;
        };
        parsed = true;
        for post in tweets {
            let key = post_key(post);
            if let Some(index) = indices.get(&key).copied() {
                // Keep the newest post details, but retain older assets omitted by a later response.
                let newest: &mut Value = &mut posts[index];
                if newest.get("media").and_then(Value::as_array).is_none()
                    && let Some(old) = post.get("media").filter(|value| value.is_array())
                {
                    newest["media"] = old.clone();
                }
                if let (Some(new), Some(old)) = (
                    newest.get_mut("media").and_then(Value::as_array_mut),
                    post.get("media").and_then(Value::as_array),
                ) {
                    for asset in old {
                        let identity = json_media_key(post, asset);
                        if !new
                            .iter()
                            .any(|item| json_media_key(post, item) == identity)
                        {
                            new.push(asset.clone());
                        }
                    }
                }
            } else {
                indices.insert(key, posts.len());
                posts.push(post.clone());
            }
        }
    }
    if !parsed {
        return (current.json.clone(), current.tweet_count);
    }
    let count = posts.len();
    let mut response = response.unwrap_or_else(|| json!({}));
    response["tweets"] = Value::Array(posts);
    (response.to_string(), count)
}

fn json_media_key(post: &Value, asset: &Value) -> String {
    // Reuse the extractor's URL/variant selection and the same stable media identity.
    let wrapper = json!({"tweets": [{"tweet_id": post.get("tweet_id").cloned().unwrap_or_default(), "media": [asset]}]});
    crate::download::engine::collect_media(&wrapper)
        .first()
        .map(media_identity::key)
        .unwrap_or_else(|| asset.to_string())
}

pub fn restore_text(entry: &mut HistoryEntry) -> bool {
    if entry.download.is_some()
        || !entry.profile.as_ref().is_some_and(|profile| {
            profile.text_tweets
                && !profile.no_videos
                && matches!(profile.media_type.as_str(), "all" | "")
        })
    {
        return false;
    }
    let Ok(value) = serde_json::from_str::<Value>(&entry.json) else {
        return false;
    };
    let mut seen = entry
        .media
        .iter()
        .map(media_identity::key)
        .collect::<HashSet<_>>();
    let mut next = entry
        .media
        .iter()
        .map(|item| item.media_index)
        .max()
        .map_or(0, |index| index + 1);
    let mut changed = false;
    for mut item in crate::download::engine::collect_media(&value)
        .into_iter()
        .filter(|item| item.media_type == "text")
    {
        if seen.insert(media_identity::key(&item)) {
            item.media_index = next;
            next += 1;
            entry.media.push(item);
            changed = true;
        }
    }
    changed
}

pub fn merge(current: &HistoryEntry, history: &[HistoryEntry]) -> HistoryEntry {
    if current.stored.is_some()
        || history.iter().any(|entry| {
            entry.stored.is_some()
                && (entry.download.is_some()
                    || fetch_sessions::key(entry) == fetch_sessions::key(current))
        })
    {
        let key = fetch_sessions::key(current);
        let resolved = history
            .iter()
            .filter(|entry| entry.download.is_some() || fetch_sessions::key(entry) == key)
            .map(crate::history::storage::resolve)
            .collect::<rusqlite::Result<Vec<_>>>();
        match (crate::history::storage::resolve(current), resolved) {
            (Ok(current), Ok(history)) => return merge_loaded(&current, &history),
            (Err(error), _) | (_, Err(error)) => {
                crate::platform::logging::error(format!(
                    "Could not restore fetch archive: {error}"
                ));
                return current.clone();
            }
        }
    }
    merge_loaded(current, history)
}
fn merge_loaded(current: &HistoryEntry, history: &[HistoryEntry]) -> HistoryEntry {
    if current.download.is_some() {
        return current.clone();
    }
    let previous = matching(current, history);
    let mut archive = current.clone();
    archive.media = media(current, &previous);
    // Prefer existing files, including converted GIF paths, without replacing current metadata.
    let mut saved = HashMap::new();
    let mut checked = HashMap::new();
    for entry in std::iter::once(current).chain(history.iter()) {
        for item in &entry.media {
            if item.status != "Saved"
                || !item.integrity_error.is_empty()
                || item.saved_path.is_empty()
            {
                continue;
            }
            let key = media_identity::key(item);
            if saved.contains_key(&key) {
                continue;
            }
            let exists = *checked.entry(&item.saved_path).or_insert_with(|| {
                crate::download::integrity::available(&item.saved_path, item.saved_bytes)
            });
            if exists {
                saved.insert(key, item);
            }
        }
    }
    for item in &mut archive.media {
        if let Some(existing) = saved.get(&media_identity::key(item)) {
            item.saved_path.clone_from(&existing.saved_path);
            item.gif_output_path.clone_from(&existing.gif_output_path);
            item.saved_bytes = existing.saved_bytes;
            item.saved_sha256.clone_from(&existing.saved_sha256);
            item.status = "Saved".into();
        }
    }
    let (json, count) = merge_json(current, &previous);
    archive.json = json;
    archive.tweet_count = count.max(
        archive
            .media
            .iter()
            .filter(|item| !item.tweet_id.is_empty())
            .map(|item| &item.tweet_id)
            .collect::<HashSet<_>>()
            .len(),
    );
    archive
}

pub fn try_open(
    entries: &[HistoryEntry],
    id: &str,
    individual: bool,
) -> Result<Option<HistoryEntry>, String> {
    let Some(entry) = entries.iter().find(|entry| entry.id == id) else {
        return Ok(None);
    };
    if individual || entry.download.is_some() {
        return crate::history::storage::resolve(entry)
            .map(Some)
            .map_err(|error| error.to_string());
    }
    let key = fetch_sessions::key(entry);
    let resolved = entries
        .iter()
        .filter(|entry| entry.download.is_some() || fetch_sessions::key(entry) == key)
        .map(crate::history::storage::resolve)
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| error.to_string())?;
    let latest = fetch_sessions::groups(&resolved)
        .into_iter()
        .find(|group| group.key == key)
        .map(|group| group.sessions[0]);
    Ok(latest.map(|entry| merge_loaded(entry, &resolved)))
}
#[cfg(test)]
pub fn open(entries: &[HistoryEntry], id: &str, individual: bool) -> Option<HistoryEntry> {
    match try_open(entries, id, individual) {
        Ok(entry) => entry,
        Err(error) => {
            crate::platform::logging::error(format!("Could not open fetch archive: {error}"));
            None
        }
    }
}

pub fn media_count(sessions: &[&HistoryEntry]) -> usize {
    if let [entry] = sessions {
        return entry.media_count();
    }
    let mut identities = HashSet::new();
    for entry in sessions {
        if let Some(stored) = &entry.stored {
            if let Ok(connection) = rusqlite::Connection::open_with_flags(
                stored.path.as_ref(),
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            ) && let Ok(mut statement) =
                connection.prepare("SELECT identity FROM history_media_index WHERE entry_id=?1")
                && let Ok(rows) = statement.query_map([&stored.id], |row| row.get::<_, Vec<u8>>(0))
            {
                identities.extend(rows.filter_map(Result::ok));
            }
        } else {
            use sha2::Digest;
            identities.extend(
                entry.media.iter().map(|item| {
                    sha2::Sha256::digest(media_identity::key(item).as_bytes()).to_vec()
                }),
            );
        }
    }
    identities.len()
}

/// Persist saved state into the sessions that own each asset, preserving snapshot membership.
pub fn update_saved(
    entries: &mut [HistoryEntry],
    updated: &[MediaCandidate],
) -> Vec<(String, Vec<MediaCandidate>)> {
    let states = updated
        .iter()
        .map(|item| (media_identity::key(item), item))
        .collect::<HashMap<_, _>>();
    let mut changed = Vec::new();
    for entry in entries.iter_mut().filter(|entry| entry.download.is_none()) {
        if let Some(stored) = &mut entry.stored {
            match crate::history::storage::patch_saved(stored, updated) {
                Ok(Some(summary)) => stored.summary = summary,
                Ok(None) => {}
                Err(error) => crate::platform::logging::error(format!(
                    "Could not update saved media metadata: {error}"
                )),
            }
            continue;
        }
        let mut dirty = false;
        for item in &mut entry.media {
            let Some(state) = states.get(&media_identity::key(item)) else {
                continue;
            };
            if saved_files::exists(item) && !saved_files::exists(state) {
                continue;
            }
            if item.status != state.status
                || item.saved_path != state.saved_path
                || item.gif_output_path != state.gif_output_path
                || item.saved_bytes != state.saved_bytes
                || item.saved_sha256 != state.saved_sha256
            {
                item.status.clone_from(&state.status);
                item.saved_path.clone_from(&state.saved_path);
                item.gif_output_path.clone_from(&state.gif_output_path);
                item.saved_bytes = state.saved_bytes;
                item.saved_sha256.clone_from(&state.saved_sha256);
                dirty = true;
            }
        }
        if dirty {
            changed.push((entry.id.clone(), entry.media.clone()));
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(id: &str, assets: &[(&str, &str)]) -> HistoryEntry {
        HistoryEntry {
            id:id.into(), username:"demo".into(), source_url:"https://x.com/demo".into(), profile:Some(Default::default()),
            media:assets.iter().enumerate().map(|(index,(tweet,asset))| MediaCandidate { tweet_id:(*tweet).into(), url:format!("https://pbs.twimg.com/media/{asset}.jpg?name=small"), media_type:"photo".into(), media_index:index, status:"Ready".into(), ..Default::default() }).collect(),
            json:json!({"account":{"username":"demo"},"tweets":assets.iter().map(|(tweet,asset)| json!({"tweet_id":tweet,"text":format!("session {id}"),"media":[{"type":"photo","thumbnail":format!("https://pbs.twimg.com/media/{asset}.jpg")}]})).collect::<Vec<_>>()} ).to_string(),
            ..Default::default()
        }
    }
    #[test]
    fn refetch_merges_unique_assets_with_current_details_and_preserves_sessions() {
        let mut old = entry("100", &[("1", "A"), ("2", "B")]);
        old.media[0].is_new = true;
        let mut latest = entry("200", &[("3", "C"), ("1", "A")]);
        latest.media[0].is_new = true;
        latest.media[1].url = "https://pbs.twimg.com/media/A?format=jpg&name=orig".into();
        latest.next_cursor = "fresh-cursor".into();
        latest.has_more = true;
        let entries = vec![latest.clone(), old.clone()];
        let merged = open(&entries, "100", false).unwrap();
        assert_eq!(merged.id, "200");
        assert_eq!(
            merged
                .media
                .iter()
                .map(|item| item.tweet_id.as_str())
                .collect::<Vec<_>>(),
            ["3", "1", "2"]
        );
        assert_eq!(
            merged
                .media
                .iter()
                .map(|item| item.media_index)
                .collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert_eq!(merged.media.iter().filter(|item| item.is_new).count(), 1);
        assert_eq!(merged.media[1].url, latest.media[1].url);
        assert_eq!(
            (merged.next_cursor.as_str(), merged.has_more),
            ("fresh-cursor", true)
        );
        assert_eq!(merged.tweet_count, 3);
        let json: Value = serde_json::from_str(&merged.json).unwrap();
        assert_eq!(json["tweets"][1]["text"], "session 200");
        assert_eq!(entries[0].media.len(), 2);
        assert_eq!(open(&entries, "100", true).unwrap().media.len(), 2);
        assert_eq!(merge(&merged, &entries).media.len(), 3);
        let backend = crate::download::engine::BackendState {
            json: merged.json.clone(),
            fetch_json: Some(latest.json.clone()),
            ..Default::default()
        };
        assert_eq!(backend.pagination_json(), latest.json);
        let baseline: Value = serde_json::from_str(&backend.pagination_json()).unwrap();
        assert_eq!(baseline["tweets"].as_array().unwrap().len(), 2);
        let rows = crate::download::engine::to_history_model(&entries, "", false);
        use crate::ui::runtime::Model;
        assert_eq!(rows.row_data(0).unwrap().media_count, 3);
    }
    #[test]
    fn source_filters_dates_and_downloads_stay_separate_and_saved_paths_survive() {
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let path = fixture.0.join("saved.jpg");
        std::fs::write(&path, b"photo").unwrap();
        let mut old = entry("100", &[("1", "A"), ("2", "B")]);
        old.media[1].saved_path = path.to_string_lossy().into_owned();
        old.media[1].status = "Saved".into();
        let latest = entry("200", &[("1", "A")]);
        let mut filtered = entry("300", &[("3", "C")]);
        filtered.profile.as_mut().unwrap().media_type = "video".into();
        let mut dates = entry("400", &[("4", "D")]);
        dates.profile.as_mut().unwrap().date_filter_enabled = true;
        dates.profile.as_mut().unwrap().start_date = "2025-01-01".into();
        let mut download = entry("500", &[("5", "E")]);
        download.download = Some(Default::default());
        let mut entries = vec![latest.clone(), old, filtered, dates, download];
        let merged = merge(&latest, &entries);
        assert_eq!(merged.media.len(), 2);
        assert_eq!(merged.media[1].saved_path, path.to_string_lossy());
        let mut updated = merged.media.clone();
        updated[0].saved_path = path.to_string_lossy().into_owned();
        updated[0].status = "Saved".into();
        let changes = update_saved(&mut entries, &updated);
        assert_eq!(changes.len(), 2);
        assert_eq!(entries[0].media.len(), 1);
        assert_eq!(entries[1].media.len(), 2);
        std::fs::remove_file(path).unwrap();
        let merged = merge(&latest, &entries);
        assert!(
            merged
                .media
                .iter()
                .all(|item| item.saved_path.is_empty() && item.status == "Ready")
        );
    }

    #[test]
    fn json_export_retains_older_assets_from_a_post_updated_by_the_latest_fetch() {
        let old = entry("100", &[("1", "A"), ("1", "B")]);
        let latest = entry("200", &[("1", "A")]);
        let merged = merge(&latest, &[old]);
        let json: Value = serde_json::from_str(&merged.json).unwrap();
        assert_eq!(json["tweets"].as_array().unwrap().len(), 1);
        assert_eq!(json["tweets"][0]["text"], "session 200");
        assert_eq!(crate::download::engine::collect_media(&json).len(), 2);
        assert_eq!(merged.media.len(), 2);
        let imported: HistoryEntry =
            serde_json::from_str(&serde_json::to_string(&merged).unwrap()).unwrap();
        assert_eq!(imported.media.len(), 2);
        assert_eq!(imported.json, merged.json);
    }
    #[test]
    fn restores_text_from_legacy_json_once_without_changing_existing_media_indices() {
        let mut legacy = entry("100", &[("1", "A")]);
        legacy.profile.as_mut().unwrap().text_tweets = true;
        legacy.media[0].media_index = 7;
        legacy.json=json!({"account":{"username":"demo"},"tweets":[{"tweet_id":"2","text":"旧いテキスト", "media":[]}]}).to_string();
        assert!(restore_text(&mut legacy));
        assert_eq!(legacy.media[1].media_index, 8);
        assert_eq!(legacy.media[1].media_type, "text");
        assert!(!restore_text(&mut legacy));
    }
}
