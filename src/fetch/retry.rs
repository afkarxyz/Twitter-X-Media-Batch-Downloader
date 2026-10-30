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
pub async fn page<F, Fut>(
    ui: crate::ui::runtime::Weak<AppWindow>,
    cancel: Arc<AtomicBool>,
    token: &str,
    mut fetch: F,
) -> Result<Value, String>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<Value, String>>,
{
    for attempt in 0..=3 {
        if cancel.load(Ordering::Relaxed) {
            return Err("Extraction cancelled.".into());
        }
        let deadline = cooldowns().lock().unwrap().get(&key(token)).copied();
        if let Some(deadline) = deadline {
            wait(&ui, &cancel, deadline, "Rate limited").await?;
        }
        let result = tokio::time::timeout(crate::network::timeouts::fetch(), fetch())
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
                        delay
                    }
                    Failure::Transient => Duration::from_secs(1 << attempt),
                    _ => return Err(error),
                };
                if attempt == 3 {
                    return Err(error);
                }
                crate::platform::logging::warn(format!(
                    "Retrying fetch after a temporary error (attempt {}).",
                    attempt + 1
                ));
                if failure == Failure::Transient {
                    wait(&ui, &cancel, Instant::now() + delay, "Fetch interrupted").await?;
                }
            }
        }
    }
    unreachable!()
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
            "retry-fixture",
            || async {
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
            "auth-fixture",
            || async { Err("auth_invalid: expired".into()) },
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
                "shared-fixture",
                || async { panic!("must wait before request") }
            )
            .await
            .unwrap_err(),
            "Extraction cancelled."
        );
        assert!(began.elapsed() < Duration::from_secs(1));
    }
}
