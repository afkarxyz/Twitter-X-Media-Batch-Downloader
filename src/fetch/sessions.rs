use crate::AppWindow;
use crate::download::engine::{self as downloader, HistoryEntry};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Hash, Eq, PartialEq, Serialize)]
struct Key {
    account: String,
    source: String,
    filter: Option<(String, String, bool, bool, bool, String, String)>,
}

fn source(entry: &HistoryEntry) -> String {
    let input = entry.source_url.trim();
    let input = if input.starts_with("http://") || input.starts_with("https://") {
        input.to_owned()
    } else if input.contains('/') || input.starts_with("x.com") || input.starts_with("twitter.com")
    {
        format!("https://{input}")
    } else {
        format!("https://x.com/{}", input.trim_start_matches('@'))
    };
    if let Ok(mut url) = reqwest::Url::parse(&input) {
        if matches!(
            url.host_str(),
            Some("x.com" | "www.x.com" | "twitter.com" | "www.twitter.com" | "mobile.twitter.com")
        ) {
            let _ = url.set_host(Some("x.com"));
            let _ = url.set_scheme("https");
            url.set_path(&url.path().trim_end_matches('/').to_ascii_lowercase());
        }
        url.set_fragment(None);
        let mut query = url
            .query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect::<Vec<_>>();
        query.sort();
        url.set_query(None);
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query);
        }
        url.to_string()
    } else {
        input
    }
}

pub fn key(entry: &HistoryEntry) -> String {
    let filter = entry.profile.as_ref().map(|profile| {
        (
            crate::fetch::request::normalized_mask(&profile.media_type, profile.no_videos),
            if profile.retweets.is_empty() {
                "skip".into()
            } else {
                profile.retweets.clone()
            },
            false,
            profile.text_tweets,
            profile.date_filter_enabled,
            if profile.date_filter_enabled {
                profile.start_date.clone()
            } else {
                String::new()
            },
            if profile.date_filter_enabled {
                profile.end_date.clone()
            } else {
                String::new()
            },
        )
    });
    serde_json::to_string(&Key {
        account: entry.username.trim_start_matches('@').to_ascii_lowercase(),
        source: source(entry),
        filter,
    })
    .unwrap()
}

pub struct Group<'a> {
    pub key: String,
    pub sessions: Vec<&'a HistoryEntry>,
}
pub fn groups(entries: &[HistoryEntry]) -> Vec<Group<'_>> {
    let mut groups: Vec<Group<'_>> = Vec::new();
    let mut indices = HashMap::new();
    let mut records = entries
        .iter()
        .filter(|entry| entry.download.is_none())
        .collect::<Vec<_>>();
    records.sort_by_key(|entry| std::cmp::Reverse(entry.id.parse::<u64>().unwrap_or_default()));
    for entry in records {
        let key = key(entry);
        let index = *indices.entry(key.clone()).or_insert_with(|| {
            groups.push(Group {
                key,
                sessions: Vec::new(),
            });
            groups.len() - 1
        });
        groups[index].sessions.push(entry);
    }
    groups
}

pub fn label(entry: &HistoryEntry) -> String {
    let source = source(entry);
    let kind = if source.contains("/search?") {
        "Search"
    } else if source.contains("/bookmarks") {
        "Bookmarks"
    } else if source.ends_with("/likes") {
        "Likes"
    } else if source.ends_with("/with_replies") {
        "Replies"
    } else if source.ends_with("/media") {
        "Media"
    } else if source.contains("/status/") {
        "Post"
    } else {
        "Timeline"
    };
    let Some(profile) = &entry.profile else {
        return format!("{kind} · Imported filters");
    };
    let media = match profile.media_type.as_str() {
        "photo" => "Images",
        "video" => "Videos",
        "animated_gif" => "GIFs",
        "photo|video" => "Images + Videos",
        "photo|animated_gif" => "Images + GIFs",
        "video|animated_gif" => "Videos + GIFs",
        "none" => "Text only",
        _ => "All media",
    };
    let mut flags = vec![kind, media];
    if profile.retweets == "include" {
        flags.push("Include reposts");
    }
    if profile.retweets == "original" {
        flags.push("Original reposts");
    }
    if profile.no_videos {
        flags.push("Exclude videos");
    }
    if profile.text_tweets {
        flags.push("Include text posts");
    }
    flags.join(" · ")
}

pub fn refresh_details(app: &AppWindow, entries: &[HistoryEntry]) {
    let selected = app.get_fetch_sessions_key();
    if selected.is_empty() {
        return;
    }
    let groups = groups(entries);
    let Some(group) = groups
        .into_iter()
        .find(|group| group.key == selected.as_str())
    else {
        app.set_fetch_sessions_key("".into());
        return;
    };
    let latest = group.sessions[0];
    app.set_fetch_sessions_title(if latest.account_name.is_empty() {
        format!("@{}", latest.username)
    } else {
        latest.account_name.clone()
    });
    app.set_fetch_sessions_label(label(latest));
    app.set_fetch_sessions_items(downloader::model_from_history_rows(
        downloader::history_rows(
            group
                .sessions
                .into_iter()
                .map(|entry| (entry, 0, String::new())),
            "",
        ),
    ));
    crate::media::images::load_history_avatars(app);
}

pub fn open(app: &AppWindow, entries: &[HistoryEntry], id: &str) {
    if let Some(entry) = entries
        .iter()
        .find(|entry| entry.id == id && entry.download.is_none())
    {
        app.set_fetch_sessions_key(key(entry));
        refresh_details(app, entries);
    }
}

pub fn removal_ids(
    entries: &[HistoryEntry],
    id: &str,
    individual: bool,
) -> std::collections::HashSet<String> {
    let group = if individual {
        None
    } else {
        entries
            .iter()
            .find(|entry| entry.id == id && entry.download.is_none())
            .map(key)
    };
    entries
        .iter()
        .filter(|entry| {
            entry.id == id
                || group
                    .as_ref()
                    .is_some_and(|group| entry.download.is_none() && key(entry) == *group)
        })
        .map(|entry| entry.id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn groups_matching_sources_and_filters_but_retains_every_session() {
        let base = HistoryEntry {
            id: "100".into(),
            username: "Demo".into(),
            source_url: "@Demo".into(),
            profile: Some(Default::default()),
            ..Default::default()
        };
        let mut newer = base.clone();
        newer.id = "200".into();
        newer.source_url = "https://twitter.com/demo/".into();
        newer.profile.as_mut().unwrap().page_size = 50;
        let mut media = base.clone();
        media.id = "300".into();
        media.source_url = "https://x.com/demo/media".into();
        let mut images = base.clone();
        images.id = "400".into();
        images.profile.as_mut().unwrap().media_type = "photo".into();
        let mut reposts = base.clone();
        reposts.id = "500".into();
        reposts.profile.as_mut().unwrap().retweets = "include".into();
        let entries = vec![base, newer, media, images, reposts];
        let groups = groups(&entries);
        assert_eq!(groups.len(), 4);
        let timeline = groups
            .iter()
            .find(|group| group.sessions.len() == 2)
            .unwrap();
        assert_eq!(
            timeline
                .sessions
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            ["200", "100"]
        );
        assert_eq!(entries.len(), 5);
        assert_eq!(
            removal_ids(&entries, "200", false),
            ["100".to_string(), "200".to_string()].into_iter().collect()
        );
        assert_eq!(
            removal_ids(&entries, "200", true),
            ["200".to_string()].into_iter().collect()
        );
    }
}
