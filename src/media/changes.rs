use crate::download::engine::HistoryEntry;
use crate::download::saved_files;
use crate::media::identity as media_identity;
use std::collections::{HashMap, HashSet};

/// Compare only matching source/filter sessions, retaining each fetch snapshot.
pub fn annotate(current: &mut HistoryEntry, history: &[HistoryEntry]) {
    let group = crate::fetch::sessions::key(current);
    let previous = history
        .iter()
        .filter(|entry| {
            entry.id != current.id
                && entry.download.is_none()
                && crate::fetch::sessions::key(entry) == group
        })
        .collect::<Vec<_>>();
    let known = previous
        .iter()
        .flat_map(|entry| crate::history::storage::identities(entry).unwrap_or_default())
        .collect::<HashSet<_>>();
    let mut saved = HashMap::new();
    for item in history
        .iter()
        .flat_map(|entry| crate::history::storage::saved_media(entry).unwrap_or_default())
        .filter(saved_files::exists)
    {
        saved.entry(media_identity::key(&item)).or_insert(item);
    }
    for item in &mut current.media {
        let key = media_identity::key(item);
        use sha2::Digest;
        item.is_new = !previous.is_empty()
            && !known.contains(sha2::Sha256::digest(key.as_bytes()).as_slice());
        if let Some(existing) = saved.get(&key) {
            item.saved_path.clone_from(&existing.saved_path);
            item.status = "Saved".into();
            item.saved_bytes = existing.saved_bytes;
            item.saved_sha256.clone_from(&existing.saved_sha256);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::download::engine::{ExtractionProfile, MediaCandidate};
    fn entry(id: &str, assets: &[&str]) -> HistoryEntry {
        HistoryEntry {
            id: id.into(),
            username: "demo".into(),
            source_url: "https://x.com/demo".into(),
            profile: Some(ExtractionProfile {
                url: "https://x.com/demo".into(),
                media_type: "all".into(),
                retweets: "skip".into(),
                ..Default::default()
            }),
            media: assets
                .iter()
                .enumerate()
                .map(|(index, asset)| MediaCandidate {
                    url: format!("https://pbs.twimg.com/media/{asset}.jpg?name=small"),
                    tweet_id: "123".into(),
                    media_type: "photo".into(),
                    media_index: index,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }
    #[test]
    fn compares_matching_sessions_and_preserves_saved_files_across_refetches() {
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let path = fixture.0.join("existing.jpg");
        std::fs::write(&path, b"photo").unwrap();
        let mut old = entry("1", &["A", "B"]);
        old.media[0].saved_path = path.to_string_lossy().into_owned();
        old.media[0].status = "Saved".into();
        let mut current = entry("2", &["C", "A"]);
        current.media[1].url = "https://pbs.twimg.com/media/A?format=png&name=orig".into();
        annotate(&mut current, &[old.clone()]);
        assert!(current.media[0].is_new);
        assert!(!current.media[1].is_new);
        assert_eq!(current.media[1].saved_path, old.media[0].saved_path);
        let mut continuation = current.clone();
        continuation
            .media
            .push(entry("unused", &["D"]).media.remove(0));
        annotate(&mut continuation, &[current.clone(), old.clone()]);
        assert!(continuation.media[0].is_new && continuation.media[2].is_new);
        std::fs::remove_file(path).unwrap();
        let mut fresh = entry("3", &["A"]);
        annotate(&mut fresh, &[old.clone()]);
        assert!(fresh.media[0].saved_path.is_empty());
        old.profile.as_mut().unwrap().media_type = "photo".into();
        let mut unrelated = entry("4", &["C"]);
        annotate(&mut unrelated, &[old]);
        assert!(!unrelated.media[0].is_new);
        annotate(&mut unrelated, &[]);
        assert!(!unrelated.media[0].is_new);
    }
}
