use crate::AppWindow;
use crate::AuthTokenItem;
use crate::download::engine::AppSettings;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, OnceLock, atomic::AtomicBool};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Validation {
    pub status: String,
    pub username: String,
    pub checked_at: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Record {
    pub token: String,
    pub saved_at: String,
    pub validation: Validation,
}
#[derive(Clone, Default)]
struct State {
    active: Record,
    history: Vec<Record>,
}
fn catalog() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(State::default()))
}
fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}
fn normalized(mut state: State) -> State {
    let mut seen = std::collections::HashSet::new();
    state.history.retain(|entry| {
        !entry.token.is_empty()
            && entry.token != state.active.token
            && seen.insert(entry.token.clone())
    });
    state.history.truncate(5);
    state
}
pub fn initialize(settings: &AppSettings) {
    *catalog().lock().unwrap() = normalized(State {
        active: Record {
            token: settings.auth_token.clone(),
            validation: settings.auth_token_validation.clone(),
            ..Default::default()
        },
        history: settings.auth_token_history.clone(),
    });
}
pub fn snapshot(active: &str) -> (Vec<Record>, Validation) {
    let state = catalog().lock().unwrap();
    (
        state.history.clone(),
        if state.active.token == active {
            state.active.validation.clone()
        } else {
            Validation::default()
        },
    )
}
fn switched(state: &State, token: &str) -> State {
    if state.active.token == token {
        return state.clone();
    }
    let previous = state
        .history
        .iter()
        .find(|entry| entry.token == token)
        .cloned();
    let mut next = state.clone();
    if !state.active.token.is_empty() {
        let mut old = state.active.clone();
        old.saved_at = now();
        next.history.insert(0, old);
    }
    next.active = previous.unwrap_or_else(|| Record {
        token: token.into(),
        saved_at: now(),
        ..Default::default()
    });
    normalized(next)
}
fn persist(app: &AppWindow, state: &State, write: &dyn Fn(AppSettings) -> bool) -> bool {
    let mut settings = crate::app::settings::settings_from_app(app);
    settings.auth_token = state.active.token.clone();
    settings.auth_token_history = state.history.clone();
    settings.auth_token_validation = state.active.validation.clone();
    if !write(settings) {
        crate::app::notifications::show_toast(
            app,
            "Could not save authentication settings.",
            "error",
        );
        return false;
    }
    *catalog().lock().unwrap() = state.clone();
    true
}
fn refresh(app: &AppWindow) {
    let state = catalog().lock().unwrap().clone();
    app.set_auth_validation_status(state.active.validation.status.clone());
    app.set_auth_validation_username(state.active.validation.username.clone());
    app.set_auth_validation_checked_at(state.active.validation.checked_at.clone());
    app.set_auth_token_history(crate::ui::runtime::ModelRc::new(
        crate::ui::runtime::VecModel::from(
            state
                .history
                .iter()
                .enumerate()
                .map(|(index, entry)| AuthTokenItem {
                    index: index as i32,
                    token: entry.token.clone(),
                    status: entry.validation.status.clone(),
                    username: entry.validation.username.clone(),
                    checked_at: entry.validation.checked_at.clone(),
                })
                .collect::<Vec<_>>(),
        ),
    ));
}
fn busy(app: &AppWindow) -> bool {
    app.get_is_busy()
        || app.get_auth_check_busy()
        || app.get_account_search_busy()
        || app.get_history_busy()
        || app.get_xtractor_busy()
        || app.get_tool_update_busy()
}
pub fn register(app: &AppWindow) {
    register_with_writer(app, crate::download::engine::save_settings);
}
pub(crate) fn register_with_writer(
    app: &AppWindow,
    write: impl Fn(AppSettings) -> bool + Send + Sync + 'static,
) {
    let ui = app.as_weak();
    register_with_services(app, write, move |token| {
        let ui = ui.clone();
        Box::pin(async move { check(&token, Some(ui)).await })
    });
}
pub(crate) type CheckFuture =
    std::pin::Pin<Box<dyn std::future::Future<Output = Validation> + Send>>;
pub(crate) fn register_with_services(
    app: &AppWindow,
    write: impl Fn(AppSettings) -> bool + Send + Sync + 'static,
    checker: impl Fn(String) -> CheckFuture + Send + Sync + 'static,
) {
    let checker = Arc::new(checker);
    let write: Arc<dyn Fn(AppSettings) -> bool + Send + Sync> = Arc::new(write);
    let ui = app.as_weak();
    let save = write.clone();
    app.on_save_auth_token(move || {
        let Some(app) = ui.upgrade() else {
            return;
        };
        if busy(&app) {
            return;
        }
        let token = app.get_auth_token().trim().to_owned();
        let state = catalog().lock().unwrap().clone();
        let next = switched(&state, &token);
        if persist(&app, &next, &*save) {
            app.set_auth_token(token.clone());
            app.set_saved_auth_token(token);
            refresh(&app);
        }
    });
    let ui = app.as_weak();
    let save = write.clone();
    app.on_use_auth_token_history(move |index| {
        let Some(app) = ui.upgrade() else {
            return;
        };
        if busy(&app) {
            return;
        }
        let state = catalog().lock().unwrap().clone();
        let Some(entry) = state.history.get(index as usize) else {
            return;
        };
        let next = switched(&state, &entry.token);
        if persist(&app, &next, &*save) {
            app.set_auth_token(next.active.token.clone());
            app.set_saved_auth_token(next.active.token.clone());
            refresh(&app);
        }
    });
    let ui = app.as_weak();
    let save = write.clone();
    app.on_remove_auth_token_history(move |index| {
        let Some(app) = ui.upgrade() else {
            return;
        };
        if busy(&app) {
            return;
        }
        let mut state = catalog().lock().unwrap().clone();
        if index < 0 || index as usize >= state.history.len() {
            return;
        }
        state.history.remove(index as usize);
        if persist(&app, &state, &*save) {
            refresh(&app);
        }
    });
    let ui = app.as_weak();
    let save = write.clone();
    app.on_check_auth_token(move |index| {
        let Some(app) = ui.upgrade() else {
            return;
        };
        if busy(&app) {
            return;
        }
        let token = if index == -1 {
            app.get_saved_auth_token().to_string()
        } else {
            let state = catalog().lock().unwrap();
            let Some(entry) = state.history.get(index as usize) else {
                return;
            };
            entry.token.clone()
        };
        if token.is_empty() {
            return;
        }
        app.set_auth_check_busy(true);
        app.set_auth_check_index(index);
        let ui = ui.clone();
        let save = save.clone();
        let checker = checker.clone();
        tokio::spawn(async move {
            let validation = checker(token.clone()).await;
            let _ = ui.upgrade_in_event_loop(move |app| {
                app.set_auth_check_busy(false);
                app.set_auth_check_index(-2);
                let mut state = catalog().lock().unwrap().clone();
                if state.active.token == token {
                    state.active.validation = validation;
                } else if let Some(entry) =
                    state.history.iter_mut().find(|entry| entry.token == token)
                {
                    entry.validation = validation;
                } else {
                    return;
                }
                if persist(&app, &state, &*save) {
                    refresh(&app);
                }
            });
        });
    });
    refresh(app);
}

fn verdict(error: &str) -> &'static str {
    let kind = error
        .split_once("XTRACTOR_ERROR_JSON:")
        .and_then(|(_, body)| serde_json::from_str::<serde_json::Value>(body.lines().next()?).ok())
        .and_then(|v| v["kind"].as_str().map(str::to_owned))
        .unwrap_or_default();
    let message = error.to_ascii_lowercase();
    if matches!(
        kind.as_str(),
        "auth_invalid" | "account_locked" | "user_suspended"
    ) || message.contains("could not authenticate")
        || message.contains("invalid or expired")
    {
        "invalid"
    } else if matches!(
        kind.as_str(),
        "rate_limited" | "network" | "server_transient" | "ip_blocked"
    ) || message.contains("timeout")
        || message.contains("rate limit")
        || message.contains("connection")
    {
        "retryable"
    } else {
        "unknown"
    }
}
async fn check(token: &str, ui: Option<crate::ui::runtime::Weak<AppWindow>>) -> Validation {
    let mut result = Validation {
        checked_at: now(),
        ..Default::default()
    };
    if token.len() != 40 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        result.status = "invalid".into();
        return result;
    }
    let options = crate::download::engine::ExtractionOptions {
        url: "https://x.com/i/bookmarks".into(),
        auth_token: token.into(),
        max_posts: 1,
        page_size: 1,
        media_type: "all".into(),
        text_tweets: true,
        retweets: "skip".into(),
        media_timeline: false,
        media_reposts: false,
        no_videos: false,
        cursor: String::new(),
        previous_json: None,
        refresh_boundary: Default::default(),
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let path = match crate::updates::xtractor::ensure_installed(ui).await {
        Ok(path) => path,
        Err(_) => {
            result.status = "retryable".into();
            return result;
        }
    };
    let operation = async {
        crate::download::engine::run_xtractor_page(&path, &options, 1, "", cancel.clone()).await
    };
    match tokio::time::timeout(std::time::Duration::from_secs(45), operation).await {
        Ok(Ok(data)) => {
            result.status = "valid".into();
            result.username = data["account"]["username"]
                .as_str()
                .unwrap_or_default()
                .into();
        }
        Ok(Err(error)) => {
            result.status = verdict(&error).into();
            if result.status == "unknown"
                && (error.to_ascii_lowercase().contains("instruction")
                    || error.to_ascii_lowercase().contains("timeline"))
                && let Some(username) = html_confirmation(token).await
            {
                result.status = "valid".into();
                result.username = username;
            }
        }
        Err(_) => {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            result.status = "retryable".into();
        }
    }
    result
}
async fn html_confirmation(token: &str) -> Option<String> {
    // Only X receives the cookie; never forward it to a token provider or webhook.
    let client = crate::network::proxy::builder()
        .timeout(std::time::Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() > 4
                || !matches!(
                    attempt.url().host_str(),
                    Some("x.com" | "www.x.com" | "twitter.com" | "www.twitter.com")
                )
            {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .build()
        .ok()?;
    let response = client
        .get("https://x.com/i/bookmarks")
        .header(
            "User-Agent",
            "Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0",
        )
        .header("Cookie", format!("auth_token={token}"))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    use futures_util::StreamExt;
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.ok()?;
        if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
            return None;
        }
        bytes.extend_from_slice(&chunk);
    }
    let body = String::from_utf8(bytes).ok()?;
    if !body.contains("profile_image_url_https") || !body.contains("\"can_dm\":") {
        return None;
    }
    let username = body.split("\"screen_name\":\"").nth(1)?.split('"').next()?;
    (!username.is_empty()).then(|| username.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "Uses the locally saved token to verify the requested X validation flow"]
    async fn live_check_validates_saved_token_without_logging_credentials() {
        let path = std::env::var_os("TWITTERDL_AUTH_LIVE_CONFIG")
            .expect("Set TWITTERDL_AUTH_LIVE_CONFIG to the local config file path");
        let settings: AppSettings = serde_json::from_slice(
            &std::fs::read(path).expect("Could not read the local validation fixture"),
        )
        .expect("Could not parse the local validation fixture");
        assert!(
            !settings.auth_token.is_empty(),
            "No saved token available for live verification"
        );
        let positive = check(&settings.auth_token, None).await;
        assert_eq!(positive.status, "valid");
        assert!(!positive.username.is_empty());
        let negative = check(&"0".repeat(40), None).await;
        assert_eq!(negative.status, "invalid");
        crate::updates::worker::shutdown().await;
    }
    #[test]
    fn history_keeps_five_unique_previous_tokens_and_restores_checked_metadata() {
        let mut state = State::default();
        for i in 0..8 {
            state = switched(&state, &format!("{i:040x}"));
        }
        assert_eq!(state.history.len(), 5);
        assert_eq!(state.history[0].token, format!("{:040x}", 6));
        state.history[2].validation.status = "valid".into();
        let restored = state.history[2].token.clone();
        state = switched(&state, &restored);
        assert_eq!(state.active.validation.status, "valid");
        assert!(!state.history.iter().any(|entry| entry.token == restored));
        assert_eq!(state.history.len(), 5);
    }
    #[test]
    fn temporary_failures_are_not_invalid_tokens() {
        assert_eq!(
            verdict("XTRACTOR_ERROR_JSON:{\"kind\":\"rate_limited\"}"),
            "retryable"
        );
        assert_eq!(
            verdict("XTRACTOR_ERROR_JSON:{\"kind\":\"auth_invalid\"}"),
            "invalid"
        );
        assert_eq!(
            verdict("XTRACTOR_ERROR_JSON:{\"kind\":\"ip_blocked\"}"),
            "retryable"
        );
        assert_eq!(verdict("parse timeline: instructions missing"), "unknown");
    }
}
