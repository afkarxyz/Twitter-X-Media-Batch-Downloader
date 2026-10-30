use crate::download::engine::{ExtractionProfile, HistoryEntry};
use serde_json::Value;
use std::collections::HashSet;

pub const EMPTY_PAGE_LIMIT: usize = 5;

pub fn boundary(profile: &ExtractionProfile, entries: &[HistoryEntry]) -> HashSet<String> {
    // Reposts/private lists can change order independently of post creation time.
    if profile.media_reposts
        || profile.retweets != "skip"
        || profile.url.contains("/likes")
        || profile.url.contains("/bookmarks")
    {
        return HashSet::new();
    }
    let Some(entry) = entries.iter().find(|entry| {
        entry.download.is_none()
            && !entry.has_more
            && entry.profile.as_ref().is_some_and(|previous| {
                crate::app::fetch::same_extraction_filters(profile, previous)
                    && previous.media_timeline == profile.media_timeline
                    && previous.media_reposts == profile.media_reposts
            })
    }) else {
        return HashSet::new();
    };
    let response = if let Some(stored) = &entry.stored {
        rusqlite::Connection::open_with_flags(
            stored.path.as_ref(),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .and_then(|connection| crate::history::storage::read_response(&connection, &stored.id))
        .ok()
    } else {
        None
    };
    let json = response.as_deref().unwrap_or(&entry.json);
    serde_json::from_str::<Value>(json)
        .ok()
        .and_then(|data| {
            if data["fetch_complete"].as_bool() == Some(false) {
                return None;
            }
            data.get("tweets").and_then(Value::as_array).cloned()
        })
        .unwrap_or_default()
        .iter()
        .filter_map(|tweet| tweet.get("tweet_id").and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
}

#[derive(Default)]
pub struct Pages {
    empty: usize,
    overlap: usize,
}
impl Pages {
    pub fn empty_limit(&mut self, new_count: usize) -> bool {
        self.empty = if new_count == 0 { self.empty + 1 } else { 0 };
        self.empty >= EMPTY_PAGE_LIMIT
    }
    pub fn overlap(&mut self, page: &[Value], known: &HashSet<String>) -> bool {
        let ids = page
            .iter()
            .filter_map(|tweet| tweet.get("tweet_id").and_then(Value::as_str))
            .collect::<Vec<_>>();
        let all_known =
            !known.is_empty() && !ids.is_empty() && ids.iter().all(|id| known.contains(*id));
        self.overlap = if all_known { self.overlap + 1 } else { 0 };
        // Do not stop on a single pinned/overlapping page. Consume the whole page first.
        self.overlap >= 2
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn empty_pages_are_bounded_and_pinned_overlap_does_not_hide_new_posts() {
        let mut pages = Pages::default();
        for _ in 0..4 {
            assert!(!pages.empty_limit(0));
        }
        assert!(pages.empty_limit(0));
        assert!(!pages.empty_limit(1));
        let known = HashSet::from(["1".to_owned()]);
        assert!(!pages.overlap(&[json!({"tweet_id":"1"})], &known));
        assert!(!pages.overlap(&[json!({"tweet_id":"2"}), json!({"tweet_id":"1"})], &known));
        assert!(!pages.overlap(&[json!({"tweet_id":"1"})], &known));
        assert!(pages.overlap(&[json!({"tweet_id":"1"})], &known));
    }
    #[test]
    fn only_complete_matching_scopes_supply_an_incremental_boundary() {
        let profile = ExtractionProfile {
            url: "https://x.com/demo".into(),
            media_type: "all".into(),
            retweets: "skip".into(),
            media_timeline: true,
            ..Default::default()
        };
        let mut entry = HistoryEntry {
            profile: Some(profile.clone()),
            json: r#"{"tweets":[{"tweet_id":"1"}]}"#.into(),
            ..Default::default()
        };
        assert!(boundary(&profile, &[entry.clone()]).contains("1"));
        entry.json = r#"{"fetch_complete":false,"tweets":[{"tweet_id":"1"}]}"#.into();
        assert!(boundary(&profile, &[entry.clone()]).is_empty());
        entry.json = r#"{"tweets":[{"tweet_id":"1"}]}"#.into();
        entry.has_more = true;
        assert!(boundary(&profile, &[entry.clone()]).is_empty());
        entry.has_more = false;
        entry.profile.as_mut().unwrap().media_timeline = false;
        assert!(boundary(&profile, &[entry]).is_empty());
    }
}
