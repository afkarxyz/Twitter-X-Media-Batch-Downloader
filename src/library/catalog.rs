use crate::download::engine::{DownloadRecord, ExtractionProfile, HistoryEntry, MediaCandidate};
use crate::media::identity;
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
pub struct Account {
    pub key: String,
    pub entry: HistoryEntry,
    pub available: usize,
    pub missing: usize,
    pub pending: usize,
}

fn handle(item: &MediaCandidate, entry: &HistoryEntry) -> String {
    let value = if item.username.is_empty() {
        &entry.username
    } else {
        &item.username
    };
    value.trim_start_matches('@').to_ascii_lowercase()
}

fn valid(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 50
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

pub fn accounts(entries: &[HistoryEntry]) -> Vec<Account> {
    match crate::history::storage::library_entries(entries) {
        Ok(entries) => accounts_loaded(&entries),
        Err(error) => {
            crate::platform::logging::error(format!("Could not load Library catalog: {error}"));
            Vec::new()
        }
    }
}
fn accounts_loaded(entries: &[HistoryEntry]) -> Vec<Account> {
    let mut folders = HashMap::new();
    let mut sources = HashMap::new();
    for entry in entries {
        sources
            .entry(entry.username.to_ascii_lowercase())
            .or_insert(entry);
        if let Some(record) = &entry.download {
            for item in &entry.media {
                let key = handle(item, entry);
                if valid(&key) {
                    folders
                        .entry(key)
                        .or_insert_with(|| record.output_dir.clone());
                }
            }
        }
    }
    let members: HashSet<_> = folders.keys().cloned().collect();
    let mut media: HashMap<String, Vec<MediaCandidate>> = HashMap::new();
    let mut seen = HashSet::new();
    let mut saved: HashMap<(String, String), (MediaCandidate, bool)> = HashMap::new();
    let mut presence = HashMap::new();
    for entry in entries {
        for item in &entry.media {
            let key = handle(item, entry);
            if !members.contains(&key) {
                continue;
            }
            let identity = identity::key(item);
            if !item.saved_path.is_empty() {
                let (present, bytes) = *presence
                    .entry((item.saved_path.clone(), item.saved_bytes))
                    .or_insert_with(|| {
                        std::fs::metadata(&item.saved_path)
                            .map(|data| {
                                (
                                    data.is_file()
                                        && data.len() > 0
                                        && (item.saved_bytes == 0
                                            || data.len() == item.saved_bytes),
                                    data.len(),
                                )
                            })
                            .unwrap_or_default()
                    });
                let available = present && item.integrity_error.is_empty();
                let index = (key.clone(), identity.clone());
                if saved
                    .get(&index)
                    .is_none_or(|(_, previous)| !*previous && available)
                {
                    let mut local = item.clone();
                    if available && local.saved_bytes == 0 {
                        local.saved_bytes = bytes;
                    }
                    saved.insert(index, (local, available));
                }
            }
            if seen.insert((key.clone(), identity)) {
                media.entry(key).or_default().push(item.clone());
            }
        }
    }
    let mut result = Vec::new();
    for key in members {
        let mut items = media.remove(&key).unwrap_or_default();
        let mut available = 0;
        let mut missing = 0;
        let mut pending = 0;
        for (index, item) in items.iter_mut().enumerate() {
            item.media_index = index;
            if let Some((local, present)) = saved.get(&(key.clone(), identity::key(item))) {
                item.saved_path.clone_from(&local.saved_path);
                item.saved_bytes = local.saved_bytes;
                item.saved_sha256.clone_from(&local.saved_sha256);
                item.gif_output_path.clone_from(&local.gif_output_path);
                item.integrity_error.clone_from(&local.integrity_error);
                item.status = if *present {
                    available += 1;
                    "Saved"
                } else {
                    missing += 1;
                    "Missing"
                }
                .into();
            } else {
                item.status = "Ready".into();
                pending += 1;
            }
        }
        let source = sources.get(&key).copied();
        let author = items.iter().find(|item| !item.author_name.is_empty());
        let title = source
            .filter(|entry| !entry.account_name.is_empty())
            .map(|entry| entry.account_name.clone())
            .or_else(|| author.map(|item| item.author_name.clone()))
            .unwrap_or_else(|| format!("@{key}"));
        let folder = folders.remove(&key).unwrap_or_default();
        let json = source.and_then(|entry| serde_json::from_str::<serde_json::Value>(&entry.json).ok())
            .and_then(|value| value.get("account").or_else(|| value.get("account_info")).cloned())
            .map(|account| serde_json::json!({"account":account}).to_string()).unwrap_or_else(|| {
            serde_json::json!({"account":{"username":key,"name":title,"avatar":items.iter().find(|item| !item.author_avatar_url.is_empty()).map(|item| &item.author_avatar_url)}}).to_string()
        });
        let total_bytes = items
            .iter()
            .filter(|item| item.status == "Saved")
            .map(|item| item.saved_bytes)
            .sum();
        let text_tweets = items.iter().any(|item| item.media_type == "text");
        result.push(Account {
            entry: HistoryEntry {
                id: format!("library-{key}"),
                username: key.clone(),
                account_name: title,
                source_url: format!("https://x.com/{key}"),
                fetched_at: source
                    .map(|entry| entry.fetched_at.clone())
                    .unwrap_or_default(),
                media: items,
                json,
                profile: Some(ExtractionProfile {
                    url: format!("https://x.com/{key}"),
                    media_type: "all".into(),
                    retweets: "skip".into(),
                    text_tweets,
                    page_size: crate::download::engine::POSTS_PER_REQUEST,
                    ..Default::default()
                }),
                download: Some(DownloadRecord {
                    output_dir: folder,
                    total_bytes,
                    media_filter: "library".into(),
                    earlier: false,
                }),
                ..Default::default()
            },
            key,
            available,
            missing,
            pending,
        });
    }
    result.sort_by(|left, right| left.key.cmp(&right.key));
    result
}

pub fn persist(entries: &mut Vec<HistoryEntry>, account: &Account) {
    // Library manifests survive clearing fetch sessions, including newly fetched media.
    entries.retain(|entry| entry.id != account.entry.id);
    crate::history::db::save_history(account.entry.clone());
    entries.insert(0, account.entry.clone());
}
