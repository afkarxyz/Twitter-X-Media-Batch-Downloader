use crate::download::engine as downloader;

#[derive(Clone, Default, PartialEq, Eq)]
pub struct DateRange {
    pub enabled: bool,
    pub start: String,
    pub end: String,
}

impl DateRange {
    pub fn validate(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        if self.start.is_empty() && self.end.is_empty() {
            return Err("Enter a start date or an end date.".into());
        }
        for value in [&self.start, &self.end] {
            if value.is_empty() {
                continue;
            }
            let date = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
                .map_err(|_| "Use valid dates in YYYY-MM-DD format.".to_owned())?;
            if date.format("%Y-%m-%d").to_string() != *value {
                return Err("Use valid dates in YYYY-MM-DD format.".into());
            }
        }
        if !self.start.is_empty() && !self.end.is_empty() && self.start > self.end {
            return Err("The end date must not be before the start date.".into());
        }
        Ok(())
    }

    pub fn url(
        &self,
        source: &str,
        media_type: &str,
        retweets: &str,
        text_tweets: bool,
    ) -> Result<String, String> {
        self.validate()?;
        if !self.enabled {
            let source = source.trim();
            // Single-account input accepts @handles like batch/date requests.
            // Keep the extractor's existing bare-handle and full-URL behavior.
            if source.starts_with('@') && !source.contains('/') {
                account_source(source)?;
                return Ok(source.trim_start_matches('@').to_owned());
            }
            return Ok(source.to_owned());
        }
        let account = account_source(source)?;
        if account.ends_with("/likes") || account.ends_with("/with_replies") {
            return Err(
                "Date filters require a profile or media URL, not likes or replies.".into(),
            );
        }
        let handle = account.split('/').nth(3).unwrap();
        let mut terms = vec![format!("from:{handle}")];
        if !self.start.is_empty() {
            terms.push(format!("since:{}", self.start));
        }
        if !self.end.is_empty() {
            terms.push(format!("until:{}", self.end));
        }
        match media_type {
            "photo" => terms.push("filter:images".into()),
            "video" | "animated_gif" => terms.push("filter:videos".into()),
            _ if !text_tweets => terms.push("filter:media".into()),
            _ => {}
        }
        if retweets == "skip" {
            terms.push("-filter:retweets".into());
        }
        let mut url = reqwest::Url::parse("https://x.com/search").unwrap();
        url.query_pairs_mut()
            .append_pair("q", &terms.join(" "))
            .append_pair("src", "typed_query")
            .append_pair("f", "live");
        Ok(url.to_string())
    }
}

pub fn account_source(input: &str) -> Result<String, String> {
    let input = input.trim();
    let (handle, suffix) = if input.contains('/') {
        let url = reqwest::Url::parse(&if input.contains("://") {
            input.to_owned()
        } else {
            format!("https://{input}")
        })
        .map_err(|_| "Use an @handle or an X/Twitter profile URL.".to_owned())?;
        if !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
            || !matches!(
                url.host_str(),
                Some(
                    "x.com"
                        | "www.x.com"
                        | "twitter.com"
                        | "www.twitter.com"
                        | "mobile.twitter.com"
                )
            )
        {
            return Err("Use an @handle or an X/Twitter profile URL.".into());
        }
        let parts = url
            .path()
            .split('/')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        if parts.is_empty()
            || parts.len() > 2
            || (parts.len() == 2 && !matches!(parts[1], "media" | "likes" | "with_replies"))
        {
            return Err("Batch accounts require profile, media, likes, or replies URLs.".into());
        }
        (
            parts[0].trim_start_matches('@').to_owned(),
            parts
                .get(1)
                .map(|part| format!("/{part}"))
                .unwrap_or_default(),
        )
    } else {
        (input.trim_start_matches('@').to_owned(), String::new())
    };
    if handle.is_empty()
        || handle.len() > 15
        || !handle
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        || matches!(
            handle.to_ascii_lowercase().as_str(),
            "search" | "i" | "home" | "explore"
        )
    {
        return Err("Use a valid X/Twitter account handle.".into());
    }
    Ok(format!(
        "https://x.com/{}{suffix}",
        handle.to_ascii_lowercase()
    ))
}

/// Match the website's default media strategy without changing explicit special sources.
pub fn uses_media_timeline(source: &str, retweets: &str, text_tweets: bool) -> bool {
    if text_tweets || retweets != "skip" {
        return false;
    }
    account_source(source)
        .ok()
        .and_then(|source| reqwest::Url::parse(&source).ok())
        .is_some_and(|url| url.path().trim_matches('/').split('/').count() == 1)
}

pub fn xtractor_source(source: &str, media_timeline: bool) -> String {
    if media_timeline
        && let Ok(account) = account_source(source)
        && let Ok(url) = reqwest::Url::parse(&account)
        && url.path().trim_matches('/').split('/').count() == 1
    {
        return format!("{account}/media");
    }
    source.to_owned()
}

pub fn batch_sources(input: &str) -> Result<Vec<String>, String> {
    let mut seen = std::collections::HashSet::new();
    let mut sources = Vec::new();
    for value in input
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .filter(|value| !value.is_empty())
    {
        let source = account_source(value)?;
        if seen.insert(source.clone()) {
            sources.push(source);
        }
        if sources.len() > 100 {
            return Err("Use at most 100 accounts in one batch.".into());
        }
    }
    if sources.is_empty() {
        return Err("Enter at least one account for the batch.".into());
    }
    Ok(sources)
}

pub fn normalized_mask(kind: &str, no_videos: bool) -> String {
    let kinds = ["photo", "video", "animated_gif"]
        .into_iter()
        .filter(|value| {
            crate::media::results::matches_kind(value, if kind.is_empty() { "all" } else { kind })
                && !(*value == "video" && no_videos)
        })
        .collect::<Vec<_>>();
    if kinds.len() == 3 {
        "all".into()
    } else if kinds.is_empty() {
        "none".into()
    } else {
        kinds.join("|")
    }
}
pub fn media_flags(app: &crate::AppWindow) -> (bool, bool, bool) {
    if app.get_media_filters_custom() {
        (
            app.get_include_photos(),
            app.get_include_videos(),
            app.get_include_gifs(),
        )
    } else {
        let index = app.get_media_type_index();
        (
            index == 0 || index == 1,
            (index == 0 || index == 2) && !app.get_no_videos(),
            index == 0 || index == 3,
        )
    }
}
pub fn media_mask(app: &crate::AppWindow) -> String {
    let (photo, video, gif) = media_flags(app);
    let types = [("photo", photo), ("video", video), ("animated_gif", gif)]
        .into_iter()
        .filter(|(_, enabled)| *enabled)
        .map(|(kind, _)| kind)
        .collect::<Vec<_>>();
    if types.len() == 3 {
        "all".into()
    } else if types.is_empty() {
        "none".into()
    } else {
        types.join("|")
    }
}
pub fn restore_filters(app: &crate::AppWindow, kind: &str, no_videos: bool) {
    app.set_include_photos(crate::media::results::matches_kind("photo", kind));
    app.set_include_videos(crate::media::results::matches_kind("video", kind) && !no_videos);
    app.set_include_gifs(crate::media::results::matches_kind("animated_gif", kind));
    app.set_no_videos(false);
    app.set_media_filters_custom(true);
}

pub fn range_from_app(app: &crate::AppWindow) -> DateRange {
    DateRange {
        enabled: app.get_date_filter_enabled(),
        start: app.get_fetch_start_date().trim().into(),
        end: app.get_fetch_end_date().trim().into(),
    }
}

pub fn profile_from_app(
    app: &crate::AppWindow,
    source: &str,
    range: &DateRange,
) -> Result<downloader::ExtractionProfile, String> {
    let media_type = media_mask(app);
    if media_type == "none" && !app.get_text_tweets() {
        return Err("Select at least one media type or enable text posts.".into());
    }
    let retweets = ["skip", "include"][app.get_retweets_index().clamp(0, 1) as usize].to_owned();
    let url = range.url(source, &media_type, &retweets, app.get_text_tweets())?;
    Ok(downloader::ExtractionProfile {
        media_timeline: uses_media_timeline(&url, &retweets, app.get_text_tweets()),
        media_reposts: crate::fetch::strategy::combined(&url, &retweets, app.get_text_tweets()),
        url,
        page_size: downloader::POSTS_PER_REQUEST,
        no_videos: app.get_no_videos() && media_type != "video",
        text_tweets: app.get_text_tweets(),
        media_type,
        retweets,
        input_url: if range.enabled {
            source.trim().into()
        } else {
            String::new()
        },
        date_filter_enabled: range.enabled,
        start_date: if range.enabled {
            range.start.clone()
        } else {
            String::new()
        },
        end_date: if range.enabled {
            range.end.clone()
        } else {
            String::new()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn batch_inputs_are_validated_deduplicated_and_preserve_source_types() {
        assert_eq!(
            batch_sources("@NASA, https://twitter.com/nasa\nNASA; x.com/nasa/media?s=21").unwrap(),
            ["https://x.com/nasa", "https://x.com/nasa/media"]
        );
        for source in [
            "https://evil.example/nasa",
            "https://x.com/search?q=test",
            "https://x.com/i/bookmarks",
            "--json",
            "https://x.com/nasa/status/123",
        ] {
            assert!(account_source(source).is_err());
        }
    }
    #[test]
    fn single_at_handle_matches_bare_handle_without_changing_special_urls() {
        let range = DateRange::default();
        let bare = range.url("Miiramua", "all", "skip", false).unwrap();
        assert_eq!(bare, "Miiramua");
        for input in ["@Miiramua", "  @Miiramua  "] {
            assert_eq!(range.url(input, "all", "skip", false).unwrap(), bare);
        }
        for input in ["@", "@bad handle", "@bad-name"] {
            assert!(range.url(input, "all", "skip", false).is_err());
        }
        for input in [
            "https://x.com/search?q=%40Miiramua&f=live",
            "https://x.com/Miiramua/status/123",
            "https://x.com/i/bookmarks",
            "https://twitter.com/Miiramua/media",
        ] {
            assert_eq!(range.url(input, "all", "skip", false).unwrap(), input);
        }
        let dates = DateRange {
            enabled: true,
            start: "2026-10-01".into(),
            ..Default::default()
        };
        assert_eq!(
            dates.url("@Miiramua", "photo", "skip", false).unwrap(),
            dates.url("Miiramua", "photo", "skip", false).unwrap()
        );
    }
    #[test]
    fn default_profile_fetches_use_media_but_legacy_cursors_and_special_sources_keep_their_endpoint()
     {
        for source in [
            "Miiramua",
            "@Miiramua",
            "https://x.com/Miiramua",
            "https://twitter.com/Miiramua/?s=20",
        ] {
            assert!(uses_media_timeline(source, "skip", false));
            assert_eq!(
                xtractor_source(source, true),
                "https://x.com/miiramua/media"
            );
            assert_eq!(xtractor_source(source, false), source);
            assert!(!uses_media_timeline(source, "include", false));
            assert!(!uses_media_timeline(source, "original", false));
            assert!(!uses_media_timeline(source, "skip", true));
        }
        for source in [
            "https://x.com/Miiramua/media",
            "https://x.com/Miiramua/likes",
            "https://x.com/Miiramua/with_replies",
            "https://x.com/Miiramua/status/123",
            "https://x.com/i/bookmarks",
            "https://x.com/search?q=from%3AMiiramua",
        ] {
            assert!(!uses_media_timeline(source, "skip", false));
            assert_eq!(xtractor_source(source, true), source);
        }
        let legacy: downloader::ExtractionProfile =
            serde_json::from_str(r#"{"url":"Miiramua"}"#).unwrap();
        assert!(!legacy.media_timeline);
        let current = downloader::ExtractionProfile {
            media_timeline: true,
            url: "Miiramua".into(),
            ..Default::default()
        };
        let restored: downloader::ExtractionProfile =
            serde_json::from_str(&serde_json::to_string(&current).unwrap()).unwrap();
        assert!(restored.media_timeline);
        assert_eq!(
            xtractor_source(&restored.url, restored.media_timeline),
            "https://x.com/miiramua/media"
        );
    }
    #[test]
    fn date_queries_preserve_media_filters_and_validate_dates() {
        let mut range = DateRange {
            enabled: true,
            start: "2024-02-29".into(),
            end: "2024-03-02".into(),
        };
        let url =
            reqwest::Url::parse(&range.url("@NASA", "photo", "skip", false).unwrap()).unwrap();
        assert_eq!(
            url.query_pairs().find(|(key, _)| key == "q").unwrap().1,
            "from:nasa since:2024-02-29 until:2024-03-02 filter:images -filter:retweets"
        );
        assert!(
            range
                .url("https://x.com/nasa/likes", "all", "include", false)
                .is_err()
        );
        assert!(
            !range
                .url("nasa", "all", "include", true)
                .unwrap()
                .contains("filter")
        );
        range.start = "2023-02-29".into();
        assert!(range.validate().is_err());
        range.start = "2024-04-01".into();
        assert!(range.validate().is_err());
        range.enabled = false;
        assert_eq!(
            range
                .url("https://x.com/i/bookmarks", "all", "include", true)
                .unwrap(),
            "https://x.com/i/bookmarks"
        );
    }
}
