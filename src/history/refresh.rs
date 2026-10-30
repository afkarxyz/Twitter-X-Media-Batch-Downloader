use crate::AppWindow;
use crate::download::engine::{self as downloader, ExtractionProfile, HistoryEntry};
use crate::fetch::request::{self as fetch_request, DateRange};
use crate::ui::runtime::Model;
use std::sync::{Arc, Mutex};

pub fn profile(entry: &HistoryEntry) -> Option<(String, ExtractionProfile)> {
    if entry.download.is_some() {
        return None;
    }
    let mut profile = entry.profile.clone().unwrap_or_else(|| ExtractionProfile {
        media_type: "all".into(),
        retweets: "skip".into(),
        ..Default::default()
    });
    let source = fetch_request::account_source(if !profile.input_url.is_empty() {
        &profile.input_url
    } else {
        &entry.source_url
    })
    .ok()?;
    if profile.media_type.is_empty() {
        profile.media_type = "all".into();
    }
    if profile.retweets.is_empty() {
        profile.retweets = "skip".into();
    }
    // The removed Original mode returned the same posts as Include through a slower route.
    if profile.retweets == "original" {
        profile.retweets = "include".into();
    }
    if !matches!(
        profile.media_type.as_str(),
        "all"
            | "photo"
            | "video"
            | "animated_gif"
            | "photo|video"
            | "photo|animated_gif"
            | "video|animated_gif"
            | "none"
    ) || !matches!(profile.retweets.as_str(), "skip" | "include")
    {
        return None;
    }
    let dates = DateRange {
        enabled: profile.date_filter_enabled,
        start: profile.start_date.clone(),
        end: profile.end_date.clone(),
    };
    profile.url = dates
        .url(
            &source,
            &profile.media_type,
            &profile.retweets,
            profile.text_tweets,
        )
        .ok()?;
    profile.media_reposts =
        crate::fetch::strategy::combined(&profile.url, &profile.retweets, profile.text_tweets);
    profile.media_timeline =
        fetch_request::uses_media_timeline(&profile.url, &profile.retweets, profile.text_tweets);
    profile.input_url = if dates.enabled {
        source.clone()
    } else {
        String::new()
    };
    profile.page_size = downloader::POSTS_PER_REQUEST;
    Some((source, profile))
}

pub fn reconcile(app: &AppWindow, entries: &[HistoryEntry]) {
    let ids = app
        .get_history_selection()
        .iter()
        .filter(|id| {
            entries
                .iter()
                .any(|entry| entry.id == id.as_str() && profile(entry).is_some())
        })
        .collect::<Vec<_>>();
    if ids.len() != app.get_history_selection().row_count() {
        app.set_history_selection(crate::ui::runtime::ModelRc::new(
            crate::ui::runtime::VecModel::from(ids),
        ));
    }
}

pub fn register(app: &AppWindow, history: Arc<Mutex<Vec<HistoryEntry>>>) {
    app.global::<crate::HistorySelection>()
        .on_is_selected(|ids, id| ids.iter().any(|value| value == id));
    let ui = app.as_weak();
    app.on_toggle_history_selection(move |id| {
        let Some(app) = ui.upgrade() else {
            return;
        };
        if app.get_is_busy()
            || app.get_history_busy()
            || !history
                .lock()
                .unwrap()
                .iter()
                .any(|entry| entry.id == id.as_str() && profile(entry).is_some())
        {
            return;
        }
        let mut ids = app.get_history_selection().iter().collect::<Vec<_>>();
        if let Some(index) = ids.iter().position(|value| *value == id) {
            ids.remove(index);
        } else {
            ids.push(id);
        }
        app.set_history_selection(crate::ui::runtime::ModelRc::new(
            crate::ui::runtime::VecModel::from(ids),
        ));
    });
    let ui = app.as_weak();
    app.on_select_visible_history(move || {
        let Some(app) = ui.upgrade() else {
            return;
        };
        if app.get_is_busy() || app.get_history_busy() {
            return;
        }
        let ids = app
            .get_history_items()
            .iter()
            .filter(|row| row.update_eligible)
            .map(|row| row.id)
            .collect::<Vec<_>>();
        app.set_history_selection(crate::ui::runtime::ModelRc::new(
            crate::ui::runtime::VecModel::from(ids),
        ));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refresh_preserves_each_account_filters_and_excludes_non_account_sources() {
        let mut entry = HistoryEntry {
            source_url: "https://twitter.com/Demo/media".into(),
            profile: Some(ExtractionProfile {
                media_type: "photo".into(),
                retweets: "include".into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (source, profile) = profile(&entry).unwrap();
        assert_eq!(source, "https://x.com/demo/media");
        assert_eq!(profile.media_type, "photo");
        assert_eq!(profile.retweets, "include");
        entry.source_url = "https://x.com/i/bookmarks".into();
        assert!(super::profile(&entry).is_none());
        entry.source_url = "https://x.com/demo".into();
        entry.profile = None;
        assert_eq!(super::profile(&entry).unwrap().1.media_type, "all");
    }

    #[test]
    fn refresh_moves_the_removed_original_mode_to_the_combined_include_fetch() {
        let entry = HistoryEntry {
            source_url: "https://x.com/demo".into(),
            profile: Some(ExtractionProfile {
                media_type: "all".into(),
                retweets: "original".into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (_, profile) = profile(&entry).unwrap();
        assert_eq!(profile.retweets, "include");
        assert!(profile.media_reposts);
        assert!(!profile.media_timeline);
    }
}
