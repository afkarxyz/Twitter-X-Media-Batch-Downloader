use crate::AppWindow;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    future::Future,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

fn cooldowns() -> &'static Mutex<HashMap<[u8; 32], Instant>> {
    static DATA: OnceLock<Mutex<HashMap<[u8; 32], Instant>>> = OnceLock::new();
    DATA.get_or_init(Default::default)
}
fn key(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}
fn note(token: &str, delay: Duration) {
    let mut data = cooldowns().lock().unwrap();
    let now = Instant::now();
    data.retain(|_, deadline| *deadline > now);
    let deadline = now + delay;
    data.entry(key(token))
        .and_modify(|old| *old = (*old).max(deadline))
        .or_insert(deadline);
}
#[derive(Debug, PartialEq)]
enum Failure {
    Transient,
    Limited(Duration),
    Permanent,
}
fn classify(error: &str) -> Failure {
    if let Some((_, rest)) = error.split_once("XTRACTOR_ERROR_JSON:")
        && let Ok(data) = serde_json::from_str::<Value>(rest.lines().next().unwrap_or_default())
    {
        return match data["kind"].as_str().unwrap_or_default() {
            "network" | "server_transient" => Failure::Transient,
            "rate_limited" => {
                let reset = data["reset_at_unix"]
                    .as_i64()
                    .map(|reset| (reset - chrono::Utc::now().timestamp()).max(0) as u64)
                    .unwrap_or(0);
                let retry = data["retry_after_seconds"].as_u64().unwrap_or(0);
                Failure::Limited(Duration::from_secs(reset.max(retry).max(1)))
            }
            _ => Failure::Permanent,
        };
    }
    let text = error.to_ascii_lowercase();
    if ["rate_limited", "rate limit", "429 too many"]
        .iter()
        .any(|term| text.contains(term))
    {
        Failure::Limited(Duration::from_secs(60))
    } else if [
        "server_transient",
        "network:",
        "timed out",
        "502 bad",
        "503 service",
        "504 gateway",
        "connection reset",
        "temporarily unavailable",
    ]
    .iter()
    .any(|term| text.contains(term))
    {
        Failure::Transient
    } else {
        Failure::Permanent
    }
}
async fn wait(
    ui: &crate::ui::runtime::Weak<AppWindow>,
    cancel: &AtomicBool,
    deadline: Instant,
    label: &str,
) -> Result<(), String> {
    while Instant::now() < deadline {
        if cancel.load(Ordering::Relaxed) {
            return Err("Extraction cancelled.".into());
        }
        let seconds = deadline.saturating_duration_since(Instant::now()).as_secs() + 1;
        let status = format!("{label} · retry in {seconds}s…");
        let _ = ui.upgrade_in_event_loop(move |app| app.set_status(status));
        let tick = Instant::now() + Duration::from_secs(1);
        while Instant::now() < tick.min(deadline) {
            if cancel.load(Ordering::Relaxed) {
                return Err("Extraction cancelled.".into());
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
    Ok(())
}
/// The end of `token`'s rate-limit cooldown, while it lasts.
fn cooling(token: &str) -> Option<Instant> {
    cooldowns()
        .lock()
        .unwrap()
        .get(&key(token))
        .copied()
        .filter(|deadline| *deadline > Instant::now())
}
/// Moves `token` to the next saved token that is neither invalid nor cooling down,
/// when Settings allows it, and makes that token the active one.
fn switch_token(ui: &crate::ui::runtime::Weak<AppWindow>, token: &mut String) -> bool {
    if !crate::accounts::tokens::rotation_enabled() {
        return false;
    }
    let Some(next) = crate::accounts::tokens::rotation()
        .into_iter()
        .find(|candidate| candidate != token && cooling(candidate).is_none())
    else {
        return false;
    };
    let message = format!(
        "Rate limited on {}. Continuing with the saved auth_token {}.",
        crate::accounts::tokens::label(token),
        crate::accounts::tokens::label(&next)
    );
    crate::platform::logging::warn(message.clone());
    let active = next.clone();
    let _ = ui.upgrade_in_event_loop(move |app| {
        crate::accounts::tokens::activate(&app, &active);
        crate::app::notifications::show_toast(&app, message, "info");
    });
    *token = next;
    true
}
/// Fetches one page through `fetch`, which receives the token to use.
///
/// A `fetch_page` waits for the Fetch pacing delay before each attempt and, when its
/// token is rate limited and Settings allows switching, continues with another saved
/// token; `token` then holds the token the page was fetched with.
pub async fn page<F, Fut>(
    ui: crate::ui::runtime::Weak<AppWindow>,
    cancel: Arc<AtomicBool>,
    token: &mut String,
    fetch_page: bool,
    mut fetch: F,
) -> Result<Value, String>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<Value, String>>,
{
    let mut attempt = 0;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("Extraction cancelled.".into());
        }
        if let Some(deadline) = cooling(token) {
            if fetch_page && switch_token(&ui, token) {
                continue;
            }
            wait(&ui, &cancel, deadline, "Rate limited").await?;
        }
        // Waited before the timeout starts; dropping the guard ends this request.
        let _pace = if fetch_page {
            Some(crate::fetch::pacing::start(&ui, &cancel).await?)
        } else {
            None
        };
        let result = tokio::time::timeout(crate::network::timeouts::fetch(), fetch(token.clone()))
            .await
            .unwrap_or_else(|_| {
                Err(format!(
                    "xtractor timed out after {} seconds.",
                    crate::network::timeouts::fetch().as_secs()
                ))
            });
        match result {
            Ok(data) => {
                if data["rate_limit"]["remaining"].as_i64() == Some(0)
                    && let Some(reset) = data["rate_limit"]["reset_at"]
                        .as_str()
                        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                {
                    let delay = (reset.timestamp() - chrono::Utc::now().timestamp()).max(0) as u64;
                    if delay <= 3600 {
                        note(token, Duration::from_secs(delay));
                    }
                }
                return Ok(data);
            }
            Err(error) => {
                let failure = classify(&error);
                if cancel.load(Ordering::Relaxed) {
                    return Err("Extraction cancelled.".into());
                }
                let delay = match failure {
                    Failure::Limited(delay) if delay <= Duration::from_secs(3600) => {
                        note(token, delay);
                        // Another token is a fresh start, not a retry of this one.
                        if fetch_page && switch_token(&ui, token) {
                            continue;
                        }
                        delay
                    }
                    Failure::Transient => Duration::from_secs(1 << attempt),
                    _ => return Err(error),
                };
                if attempt == 3 {
                    return Err(error);
                }
                attempt += 1;
                crate::platform::logging::warn(format!(
                    "Retrying fetch after a temporary error (attempt {attempt})."
                ));
                if failure == Failure::Transient {
                    wait(&ui, &cancel, Instant::now() + delay, "Fetch interrupted").await?;
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn permanent_errors_do_not_retry_and_structured_cooldowns_are_preserved() {
        assert_eq!(classify("auth_invalid: expired token"), Failure::Permanent);
        assert_eq!(
            classify("parse account: rest_id missing"),
            Failure::Permanent
        );
        assert_eq!(
            classify(
                "XTRACTOR_ERROR_JSON:{\"kind\":\"rate_limited\",\"retry_after_seconds\":3}\nerror"
            ),
            Failure::Limited(Duration::from_secs(3))
        );
        assert_eq!(
            classify("XTRACTOR_ERROR_JSON:{\"kind\":\"network\"}"),
            Failure::Transient
        );
    }
    #[tokio::test]
    async fn retries_real_failures_without_retrying_auth_and_cancels_shared_wait() {
        use std::sync::atomic::AtomicUsize;
        let calls = AtomicUsize::new(0);
        let cancel = Arc::new(AtomicBool::new(false));
        let result = page(
            crate::ui::runtime::Weak::default(),
            cancel.clone(),
            &mut "retry-fixture".to_string(),
            false,
            |_| async {
                if calls.fetch_add(1, Ordering::Relaxed) == 0 {
                    Err("network: fixture reset".into())
                } else {
                    Ok(serde_json::json!({"tweets":[]}))
                }
            },
        )
        .await;
        assert!(result.is_ok());
        assert_eq!(calls.load(Ordering::Relaxed), 2);
        let result = page(
            crate::ui::runtime::Weak::default(),
            cancel.clone(),
            &mut "auth-fixture".to_string(),
            false,
            |_| async { Err("auth_invalid: expired".into()) },
        )
        .await;
        assert!(result.is_err());
        note("shared-fixture", Duration::from_secs(60));
        let stop = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(40)).await;
            stop.store(true, Ordering::Relaxed);
        });
        let began = Instant::now();
        assert_eq!(
            page(
                crate::ui::runtime::Weak::default(),
                cancel,
                &mut "shared-fixture".to_string(),
                false,
                |_| async { panic!("must wait before request") }
            )
            .await
            .unwrap_err(),
            "Extraction cancelled."
        );
        assert!(began.elapsed() < Duration::from_secs(1));
    }

    // Paces through the shared gate and restores it to off.
    #[tokio::test]
    async fn fetch_pages_wait_for_the_pacing_delay_and_account_search_does_not() {
        let _shared = crate::fetch::pacing::TEST_LOCK.lock().await;
        crate::fetch::pacing::configure(200, 0);
        let cancel = Arc::new(AtomicBool::new(false));
        let fetch = |_: String| async { Ok(serde_json::json!({"tweets":[]})) };
        let ui = crate::ui::runtime::Weak::default;
        page(
            ui(),
            cancel.clone(),
            &mut "pacing-fixture".to_string(),
            true,
            fetch,
        )
        .await
        .unwrap();
        let began = Instant::now();
        page(
            ui(),
            cancel.clone(),
            &mut "pacing-fixture".to_string(),
            true,
            fetch,
        )
        .await
        .unwrap();
        let paced = began.elapsed();
        let began = Instant::now();
        page(
            ui(),
            cancel.clone(),
            &mut "pacing-fixture".to_string(),
            false,
            fetch,
        )
        .await
        .unwrap();
        let unpaced = began.elapsed();
        crate::fetch::pacing::configure(0, 0);
        assert!(
            paced >= Duration::from_millis(190),
            "paced page waited {paced:?}"
        );
        assert!(
            unpaced < Duration::from_millis(100),
            "account search waited {unpaced:?}"
        );
    }

    /// Saved tokens A (active), B, C (checked invalid) and D; returns the tokens tried.
    async fn rotate(
        start: &str,
        fetch_page: bool,
        limited: &'static [&'static str],
    ) -> (Result<Value, String>, Vec<String>, String, Duration) {
        let tried = Mutex::new(Vec::new());
        let mut token = start.to_string();
        let began = Instant::now();
        let result = page(
            crate::ui::runtime::Weak::default(),
            Arc::new(AtomicBool::new(false)),
            &mut token,
            fetch_page,
            |token: String| {
                tried.lock().unwrap().push(token.clone());
                let first = tried.lock().unwrap().iter().filter(|t| **t == token).count() == 1;
                async move {
                    if limited.contains(&token.as_str()) && first {
                        Err("XTRACTOR_ERROR_JSON:{\"kind\":\"rate_limited\",\"retry_after_seconds\":1}".into())
                    } else {
                        Ok(serde_json::json!({"tweets":[]}))
                    }
                }
            },
        )
        .await;
        (result, tried.into_inner().unwrap(), token, began.elapsed())
    }

    #[tokio::test]
    async fn rate_limited_fetch_pages_continue_with_the_next_saved_token() {
        use crate::accounts::tokens::{self, Record, Validation};
        let _shared = crate::fetch::pacing::TEST_LOCK.lock().await;
        let record = |token: &str, status: &str| Record {
            token: token.into(),
            validation: Validation {
                status: status.into(),
                ..Default::default()
            },
            ..Default::default()
        };
        tokens::initialize(&crate::download::engine::AppSettings {
            auth_token: "rotate-a".into(),
            auth_token_history: vec![
                record("rotate-b", "valid"),
                record("rotate-c", "invalid"),
                record("rotate-d", ""),
            ],
            ..Default::default()
        });
        tokens::set_rotation(true);

        // A is rate limited: B continues at once, without waiting out A's cooldown.
        let (result, tried, token, elapsed) = rotate("rotate-a", true, &["rotate-a"]).await;
        assert!(result.is_ok());
        assert_eq!(tried, ["rotate-a", "rotate-b"]);
        assert_eq!(token, "rotate-b", "later pages keep the token that worked");
        assert!(elapsed < Duration::from_millis(500), "waited {elapsed:?}");

        // A is still cooling down and B is limited too: C was found invalid, so D.
        let (result, tried, token, _) = rotate("rotate-a", true, &["rotate-b"]).await;
        assert!(result.is_ok());
        assert_eq!(
            tried,
            ["rotate-b", "rotate-d"],
            "A is skipped while it cools down"
        );
        assert_eq!(token, "rotate-d");

        // Account search keeps its token and waits out the cooldown instead.
        let (result, tried, token, elapsed) = rotate("rotate-e", false, &["rotate-e"]).await;
        assert!(result.is_ok());
        assert_eq!(tried, ["rotate-e", "rotate-e"]);
        assert_eq!(token, "rotate-e");
        assert!(elapsed >= Duration::from_millis(900), "waited {elapsed:?}");

        // With switching off, a fetch page waits too.
        tokens::set_rotation(false);
        let (result, tried, _, elapsed) = rotate("rotate-f", true, &["rotate-f"]).await;
        assert!(result.is_ok());
        assert_eq!(tried, ["rotate-f", "rotate-f"]);
        assert!(elapsed >= Duration::from_millis(900), "waited {elapsed:?}");
    }
}
