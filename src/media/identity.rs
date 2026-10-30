use crate::download::engine::MediaCandidate;

/// The source asset ID does not change when image size or video bitrate changes.
pub fn asset_id(item: &MediaCandidate) -> String {
    let Ok(url) = reqwest::Url::parse(&item.url) else {
        return String::new();
    };
    let parts = url
        .path_segments()
        .map(|parts| parts.collect::<Vec<_>>())
        .unwrap_or_default();
    if matches!(url.host_str(), Some("video.twimg.com"))
        && let Some(index) = parts
            .iter()
            .position(|part| matches!(*part, "ext_tw_video" | "amplify_video" | "tweet_video"))
        && let Some(id) = parts.get(index + 1)
    {
        return id.split('.').next().unwrap_or_default().into();
    }
    parts
        .last()
        .map(|name| {
            name.split('.')
                .next()
                .unwrap_or_default()
                .split(':')
                .next()
                .unwrap_or_default()
                .to_owned()
        })
        .unwrap_or_default()
}

pub fn key(item: &MediaCandidate) -> String {
    let asset = if reqwest::Url::parse(&item.url)
        .ok()
        .is_some_and(|url| matches!(url.host_str(), Some("pbs.twimg.com" | "video.twimg.com")))
    {
        asset_id(item)
    } else {
        String::new()
    };
    let source = if asset.is_empty() {
        item.url.split('?').next().unwrap_or(&item.url)
    } else {
        &asset
    };
    format!("{}|{}|{}", item.tweet_id, item.media_type, source)
}

pub fn post_timestamp(item: &MediaCandidate) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_str(&item.created_at, "%a %b %d %H:%M:%S %z %Y")
        .or_else(|_| chrono::DateTime::parse_from_rfc3339(&item.created_at))
        .ok()
        .map(|date| date.with_timezone(&chrono::Utc))
        .or_else(|| {
            let id = item.tweet_id.parse::<u64>().ok()?;
            if id < (1 << 22) {
                return None;
            }
            chrono::DateTime::from_timestamp_millis(((id >> 22) + 1_288_834_974_657) as i64)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_survives_quality_changes_and_keeps_distinct_assets() {
        let mut image = MediaCandidate {
            url: "https://pbs.twimg.com/media/ABC.jpg?name=small".into(),
            tweet_id: "100".into(),
            media_type: "photo".into(),
            ..Default::default()
        };
        let original = key(&image);
        image.url = "https://pbs.twimg.com/media/ABC?format=png&name=orig".into();
        assert_eq!(key(&image), original);
        image.url = "https://pbs.twimg.com/media/DEF.jpg".into();
        assert_ne!(key(&image), original);
        image.media_type = "video".into();
        image.url = "https://video.twimg.com/ext_tw_video/123/pu/vid/avc1/320x180/aaa.mp4".into();
        let original = key(&image);
        image.url =
            "https://video.twimg.com/ext_tw_video/123/pu/vid/avc1/1280x720/bbb.mp4?tag=12".into();
        assert_eq!(key(&image), original);
    }
    #[test]
    fn post_dates_use_utc_and_never_substitute_the_download_date() {
        let mut item = MediaCandidate {
            created_at: "Sun Aug 31 17:52:02 +0000 2025".into(),
            ..Default::default()
        };
        assert_eq!(
            post_timestamp(&item)
                .unwrap()
                .format("%Y-%m-%d_%H-%M-%S")
                .to_string(),
            "2025-08-31_17-52-02"
        );
        item.created_at = "2025-09-01T00:52:02+07:00".into();
        assert_eq!(
            post_timestamp(&item)
                .unwrap()
                .format("%Y-%m-%d")
                .to_string(),
            "2025-08-31"
        );
        item.created_at.clear();
        assert!(post_timestamp(&item).is_none());
        item.tweet_id = "1989033613824815258".into();
        assert!(post_timestamp(&item).is_some());
    }
}
