use crate::AccountSearchItem;
use crate::AppWindow;
use crate::download::engine::{self as downloader, ExtractionOptions};
use serde_json::Value;
use std::{
    collections::HashSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
fn query(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 200 || value.chars().any(char::is_control) {
        return Err("Enter 1–200 characters to search accounts.".into());
    }
    Ok(value.to_owned())
}
fn rows(data: &Value) -> Result<Vec<AccountSearchItem>, String> {
    let accounts = data.get("accounts").and_then(Value::as_array).ok_or(
        "Account search requires the updated xtractor. Rebuild and replace xtractor.exe first.",
    )?;
    let mut seen = HashSet::new();
    let mut rows = Vec::new();
    for account in accounts {
        let handle = account["username"].as_str().unwrap_or_default();
        if handle.is_empty()
            || handle.len() > 15
            || !handle
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_')
            || !seen.insert(handle.to_ascii_lowercase())
        {
            continue;
        }
        rows.push(AccountSearchItem {
            handle: handle.into(),
            name: account["name"]
                .as_str()
                .filter(|name| !name.is_empty())
                .unwrap_or(handle)
                .into(),
            bio: account["bio"].as_str().unwrap_or_default().into(),
            followers: account["followers"]
                .as_u64()
                .map(|count| count.to_string())
                .unwrap_or_default(),
            verified: account["verified"].as_bool().unwrap_or(false)
                || account["blue_verified"].as_bool().unwrap_or(false),
            protected: account["protected"].as_bool().unwrap_or(false),
            avatar_url: account["avatar"].as_str().unwrap_or_default().into(),
            avatar: Default::default(),
        });
    }
    Ok(rows)
}

fn append_page(
    existing: &mut Vec<AccountSearchItem>,
    incoming: Vec<AccountSearchItem>,
    data: &Value,
    cursor: &str,
) -> String {
    let mut handles = existing
        .iter()
        .map(|row| row.handle.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    let previous_count = existing.len();
    existing.extend(
        incoming
            .into_iter()
            .filter(|row| handles.insert(row.handle.to_ascii_lowercase())),
    );
    let next = data["next_cursor"].as_str().unwrap_or_default();
    if existing.len() == previous_count
        || data["has_more"].as_bool() != Some(true)
        || next == cursor
    {
        String::new()
    } else {
        next.to_owned()
    }
}

fn search_error(error: &str) -> String {
    let details = error
        .split_once("XTRACTOR_ERROR_JSON:")
        .and_then(|(_, rest)| {
            serde_json::from_str::<Value>(rest.lines().next().unwrap_or_default()).ok()
        });
    let kind = details
        .as_ref()
        .and_then(|data| data["kind"].as_str())
        .unwrap_or_default();
    let message = details
        .as_ref()
        .and_then(|data| data["message"].as_str())
        .unwrap_or(error);
    let lower = message.to_ascii_lowercase();
    if kind == "rate_limited" {
        let seconds = details
            .as_ref()
            .and_then(|data| data["retry_after_seconds"].as_u64())
            .unwrap_or(0);
        return if seconds > 0 {
            format!("X search is rate limited. Try again in {seconds}s.")
        } else {
            "X search is rate limited. Please try again later.".into()
        };
    }
    if kind == "auth_invalid" || kind == "account_locked" {
        return "The X session is expired or locked. Save a valid auth_token in Settings.".into();
    }
    if lower.contains("http 404") || lower.contains("search endpoint") {
        return "X search endpoint is unavailable. Update xtractor and try again.".into();
    }
    if kind == "network" || lower.contains("timed out") {
        return "Could not connect to X search. Check your connection or proxy and try again."
            .into();
    }
    if error == "Extraction cancelled." {
        return "Account search cancelled.".into();
    }
    if details.is_some() {
        return "X could not complete account search. Please try again; details are in the session log.".into();
    }
    error.to_owned()
}

pub fn register(app: &AppWindow) {
    let pending = Arc::new(Mutex::new(Arc::new(AtomicBool::new(false))));
    let ui = app.as_weak();
    let job = pending.clone();
    app.on_close_account_search(move || {
        job.lock().unwrap().store(true, Ordering::Relaxed);
        if let Some(app) = ui.upgrade() {
            app.set_account_search_open(false);
            app.set_account_search_busy(false);
            app.set_account_search_generation(app.get_account_search_generation().wrapping_add(1));
        }
    });
    let ui = app.as_weak();
    app.on_search_accounts(move |input, more| {
        let Some(app) = ui.upgrade() else {
            return;
        };
        if app.get_account_search_busy()
            || app.get_auth_check_busy()
            || app.get_is_busy()
            || app.get_history_busy()
            || app.get_tool_update_busy()
            || app.get_xtractor_busy()
        {
            return;
        }
        let query = match query(&input) {
            Ok(query) => query,
            Err(error) => {
                app.set_account_search_error(error);
                return;
            }
        };
        if app.get_saved_auth_token().trim().is_empty() {
            app.set_account_search_error(
                "Save an X auth_token in Settings before searching accounts.".into(),
            );
            return;
        }
        let more = more
            && app.get_account_search_result_query().as_str() == query
            && !app.get_account_search_cursor().is_empty();
        let cursor = if more {
            app.get_account_search_cursor().to_string()
        } else {
            String::new()
        };
        let mut url = reqwest::Url::parse("https://x.com/search").unwrap();
        url.query_pairs_mut()
            .append_pair("q", &query)
            .append_pair("f", "user");
        let options = ExtractionOptions {
            url: url.to_string(),
            media_timeline: false,
            media_reposts: false,
            auth_token: app.get_saved_auth_token().trim().into(),
            max_posts: 20,
            page_size: 20,
            media_type: "all".into(),
            retweets: "skip".into(),
            no_videos: false,
            text_tweets: true,
            cursor: cursor.clone(),
            previous_json: None,
            refresh_boundary: Default::default(),
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let previous = std::mem::replace(&mut *pending.lock().unwrap(), cancel.clone());
        previous.store(true, Ordering::Relaxed);
        app.set_account_search_generation(app.get_account_search_generation().wrapping_add(1));
        let generation = app.get_account_search_generation();
        app.set_account_search_result_query(query.clone());
        app.set_account_search_query(query);
        app.set_account_search_busy(true);
        app.set_account_search_error("".into());
        if !more {
            app.set_account_search_items(crate::ui::runtime::ModelRc::default());
            app.set_account_search_cursor("".into());
        }
        let ui = ui.clone();
        tokio::spawn(async move {
            let result = async {
                let executable =
                    crate::updates::xtractor::ensure_installed(Some(ui.clone())).await?;
                crate::fetch::retry::page(ui.clone(), cancel.clone(), &options.auth_token, || {
                    downloader::run_xtractor_page(
                        &executable,
                        &options,
                        20,
                        &cursor,
                        cancel.clone(),
                    )
                })
                .await
            }
            .await;
            let _ = ui.upgrade_in_event_loop(move |app| {
                if app.get_account_search_generation() != generation
                    || !app.get_account_search_open()
                {
                    return;
                }
                app.set_account_search_busy(false);
                match result {
                    Ok(data) => match rows(&data) {
                        Ok(new_rows) => {
                            let mut existing = if more {
                                app.get_account_search_items().iter().collect::<Vec<_>>()
                            } else {
                                Vec::new()
                            };
                            let previous_count = existing.len();
                            let next = append_page(&mut existing, new_rows, &data, &cursor);
                            app.set_account_search_cursor(next);
                            let avatars = existing[previous_count..]
                                .iter()
                                .map(|row| row.avatar_url.clone())
                                .collect::<Vec<_>>();
                            app.set_account_search_items(crate::ui::runtime::ModelRc::new(
                                crate::ui::runtime::VecModel::from(existing),
                            ));
                            for url in avatars {
                                crate::media::images::request(
                                    &app,
                                    crate::media::images::Kind::Avatar,
                                    url,
                                );
                            }
                        }
                        Err(error) => {
                            crate::platform::logging::warn(format!(
                                "Account search failed: {error}"
                            ));
                            app.set_account_search_error(search_error(&error));
                        }
                    },
                    Err(error) => app.set_account_search_error(error),
                }
            });
        });
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn account_pagination_stops_when_a_new_cursor_only_repeats_accounts_or_returns_empty() {
        let first = serde_json::json!({
            "accounts": [{"username":"Demo"}, {"username":"second"}],
            "has_more": true, "next_cursor": "page-1"
        });
        let mut existing = Vec::new();
        assert_eq!(
            append_page(&mut existing, rows(&first).unwrap(), &first, ""),
            "page-1"
        );
        for accounts in [
            serde_json::json!([]),
            serde_json::json!([{"username":"demo"}]),
        ] {
            let page = serde_json::json!({"accounts": accounts, "has_more": true, "next_cursor": "page-2"});
            let mut items = existing.clone();
            assert!(append_page(&mut items, rows(&page).unwrap(), &page, "page-1").is_empty());
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].handle, "Demo");
        }
        for (has_more, next, expected) in [
            (true, "page-2", "page-2"),
            (true, "page-1", ""),
            (false, "page-2", ""),
        ] {
            let page = serde_json::json!({"accounts": [{"username":"demo"}, {"username":"third"}], "has_more": has_more, "next_cursor": next});
            let mut items = existing.clone();
            assert_eq!(
                append_page(&mut items, rows(&page).unwrap(), &page, "page-1"),
                expected
            );
            assert_eq!(items.len(), 3);
            assert_eq!(items[2].handle, "third");
        }
    }
    #[test]
    fn follower_counts_distinguish_missing_from_real_zero() {
        let result=rows(&serde_json::json!({"accounts":[{"username":"missing"},{"username":"null","followers":null},{"username":"zero","followers":0},{"username":"known","followers":42}]})).unwrap();
        assert!(result[0].followers.is_empty());
        assert!(result[1].followers.is_empty());
        assert_eq!(result[2].followers, "0");
        assert_eq!(result[3].followers, "42");
    }
    #[test]
    fn search_errors_hide_internal_json_and_distinguish_endpoint_from_auth_and_rate_limit() {
        let old = r#"XTRACTOR_ERROR_JSON:{"kind":"user_not_found","message":"user_not_found: http 404:","retry_after_seconds":888}
error: user_not_found: http 404:"#;
        assert!(search_error(old).contains("endpoint"));
        assert!(!search_error(old).contains("XTRACTOR_ERROR_JSON"));
        assert!(!search_error(old).contains("888"));
        assert!(
            search_error(r#"XTRACTOR_ERROR_JSON:{"kind":"auth_invalid","message":"expired"}"#)
                .contains("auth_token")
        );
        assert!(
            search_error(r#"XTRACTOR_ERROR_JSON:{"kind":"rate_limited","retry_after_seconds":5}"#)
                .contains("5s")
        );
    }
    #[test]
    fn search_validates_queries_and_safely_deduplicates_account_results() {
        assert!(query(" ").is_err());
        assert!(query(&"x".repeat(201)).is_err());
        let result=rows(&serde_json::json!({"accounts":[{"username":"Demo","name":"Demo"},{"username":"demo"},{"username":"bad/name"}]})).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].handle, "Demo");
        assert!(
            rows(&serde_json::json!({"tweets":[]}))
                .unwrap_err()
                .contains("updated xtractor")
        );
    }
}
