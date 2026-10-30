use serde::{Deserialize, Serialize};
use serde_json::Value;

const PREFIX: &str = "twitterdl-media-reposts-v1:";
#[derive(Serialize, Deserialize)]
struct Cursor {
    phase: String,
    cursor: String,
}
pub struct Request {
    pub source: String,
    pub cursor: String,
    pub retweets: String,
    pub combined: bool,
    pub reposts: bool,
}

pub fn combined(source: &str, retweets: &str, text: bool) -> bool {
    retweets == "include"
        && !text
        && crate::fetch::request::account_source(source).is_ok()
        && crate::fetch::request::account_source(source)
            .is_ok_and(|url| !url.ends_with("/likes") && !url.ends_with("/with_replies"))
}
fn encode(reposts: bool, cursor: String) -> String {
    format!(
        "{PREFIX}{}",
        serde_json::to_string(&Cursor {
            phase: if reposts { "reposts" } else { "media" }.into(),
            cursor
        })
        .unwrap()
    )
}
pub fn request(
    source: &str,
    media: bool,
    hybrid: bool,
    retweets: &str,
    cursor: &str,
) -> Result<Request, String> {
    if !hybrid {
        return Ok(Request {
            source: crate::fetch::request::xtractor_source(source, media),
            cursor: cursor.into(),
            retweets: retweets.into(),
            combined: false,
            reposts: false,
        });
    }
    let state = if let Some(value) = cursor.strip_prefix(PREFIX) {
        let state: Cursor =
            serde_json::from_str(value).map_err(|_| "Invalid combined fetch cursor.")?;
        if !matches!(state.phase.as_str(), "media" | "reposts") {
            return Err("Invalid combined fetch phase.".into());
        }
        state
    } else {
        Cursor {
            phase: "media".into(),
            cursor: cursor.into(),
        }
    };
    let reposts = state.phase == "reposts";
    let base = crate::fetch::request::account_source(source)?
        .trim_end_matches("/media")
        .to_owned();
    Ok(Request {
        source: if reposts {
            base
        } else {
            format!("{base}/media")
        },
        cursor: state.cursor,
        retweets: if reposts { "include" } else { "skip" }.into(),
        combined: true,
        reposts,
    })
}
pub fn finish(mut page: Value, request: &Request) -> Value {
    if !request.combined {
        return page;
    }
    let next = page
        .get("next_cursor")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let more = page
        .get("has_more")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && !next.is_empty();
    if request.reposts
        && let Some(tweets) = page.get_mut("tweets").and_then(Value::as_array_mut)
    {
        tweets.retain(|tweet| tweet.get("is_repost").and_then(Value::as_bool) == Some(true));
    }
    if !request.reposts && !more {
        page["has_more"] = Value::Bool(true);
        page["next_cursor"] = Value::String(encode(true, String::new()));
    } else if more {
        page["next_cursor"] = Value::String(encode(request.reposts, next));
    } else {
        page["has_more"] = Value::Bool(false);
        if let Some(object) = page.as_object_mut() {
            object.remove("next_cursor");
        }
    }
    page
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn hybrid_retains_media_then_only_reposts_with_resumable_empty_pages() {
        let media = request("demo", true, true, "include", "").unwrap();
        assert_eq!(media.source, "https://x.com/demo/media");
        assert_eq!(media.retweets, "skip");
        let end = finish(
            json!({"tweets":[{"tweet_id":"1"}],"has_more":false}),
            &media,
        );
        let reposts = request(
            "demo",
            true,
            true,
            "include",
            end["next_cursor"].as_str().unwrap(),
        )
        .unwrap();
        assert!(reposts.reposts && reposts.cursor.is_empty());
        let page = finish(
            json!({"tweets":[{"tweet_id":"2","is_repost":false}],"has_more":true,"next_cursor":"next"}),
            &reposts,
        );
        assert!(page["tweets"].as_array().unwrap().is_empty() && page["has_more"] == true);
        let resumed = request(
            "demo",
            true,
            true,
            "include",
            page["next_cursor"].as_str().unwrap(),
        )
        .unwrap();
        assert_eq!(resumed.cursor, "next");
        let done = finish(
            json!({"tweets":[{"tweet_id":"3","is_repost":true},{"tweet_id":"4"}],"has_more":false}),
            &resumed,
        );
        assert_eq!(done["tweets"].as_array().unwrap().len(), 1);
        assert_eq!(done["has_more"], false);
        assert!(request("demo", true, true, "include", &format!("{PREFIX}bad")).is_err());
        let old = request("demo", false, false, "include", "legacy").unwrap();
        assert_eq!(old.source, "demo");
        assert_eq!(old.cursor, "legacy");
    }
}
