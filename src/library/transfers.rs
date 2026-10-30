use super::{Context, catalog};
use crate::{AppWindow, download::engine as downloader};
use std::{
    path::Path,
    sync::{Arc, Mutex, atomic::Ordering},
};

pub(super) fn register(app: &AppWindow, context: Context) {
    register_refetch(
        app,
        context.clone(),
        crate::app::fetch::spawn_extraction_job,
    );
    app.on_library_download(move |index| {
        let Some(app) = context.ui.upgrade() else {
            return;
        };
        if app.get_is_busy() || app.get_history_busy() || app.get_tool_update_busy() || !app.get_library().job.is_empty() {
            return;
        }
        let key = app.get_library().account;
        let Some(account) = context
            .catalog
            .lock()
            .unwrap()
            .iter()
            .find(|account| account.key == key)
            .cloned()
        else {
            return;
        };
        let tasks: Vec<_> = account
            .entry
            .media
            .iter()
            .filter(|item| {
                item.status != "Saved" && (index < 0 || item.media_index == index as usize)
            })
            .cloned()
            .collect();
        if tasks.is_empty() {
            return;
        }
        let settings = crate::app::settings::settings_from_app(&app);
        let mut policy = crate::download::policy::from_app(&app);
        policy.separate_types = false;
        let owner = context.clone();
        owner.cancel.store(false, Ordering::Relaxed);
        app.update_library(|data| {
            data.job = key.clone();
            data.feedback_active = true;
            data.progress_detail.clear();
            data.queue.clear();
            data.error.clear();
        });
        app.set_is_busy(true);
        app.set_is_extracting(false);
        app.set_status(crate::i18n::translate(&app.global::<crate::I18n>().get_language(), "Downloading library media…"));
        app.set_progress(0.0);
        app.set_progress_detail(format!("0 / {} files saved", tasks.len()));
        let gif_options = crate::download::gifs::Options::from_values(
            &settings.gif_quality,
            &settings.gif_resolution,
        );
        tokio::spawn(async move {
            let total = tasks.len();
            let mut completed = Vec::new();
            let mut downloaded_bytes = 0u64;
            let mut errors = Vec::new();
            for (position, mut item) in tasks.into_iter().enumerate() {
                if owner.cancel.load(Ordering::Relaxed) {
                    break;
                }
                let folder = if item.saved_path.is_empty() {
                    Path::new(&account.entry.download.as_ref().unwrap().output_dir)
                } else {
                    Path::new(&item.saved_path)
                        .parent()
                        .unwrap_or(Path::new(""))
                };
                let (root, name) =
                    if let (Some(parent), Some(name)) = (folder.parent(), folder.file_name()) {
                        (parent.to_path_buf(), name.to_string_lossy().into_owned())
                    } else {
                        (
                            settings
                                .output_dir
                                .clone()
                                .or_else(|| crate::platform::paths::output_dir(None).ok())
                                .unwrap_or_default(),
                            key.clone(),
                        )
                    };
                let result = downloader::download_selected_file(
                    item.clone(),
                    item.media_index,
                    root,
                    settings.filename_format.clone(),
                    settings.folder_name_format.clone(),
                    Some(name),
                    settings.video_quality.clone(),
                    settings.image_size.clone(),
                    settings.convert_gif,
                    gif_options,
                    settings.retry_attempts,
                    owner.ui.clone(),
                    owner.cancel.clone(),
                    policy,
                )
                .await;
                let cancelled = result.is_err() && owner.cancel.load(Ordering::Relaxed);
                match result {
                    Ok((path, warning)) => {
                        if let Some(warning) = warning {
                            errors.push(warning);
                        }
                        item.status = "Saved".into();
                        item.saved_path = path.to_string_lossy().into_owned();
                        item.integrity_error.clear();
                        item.saved_bytes = tokio::fs::metadata(&path).await.map(|data| data.len()).unwrap_or_default();
                        item.saved_sha256.clear();
                        if let Some(signature) =
                            crate::download::integrity::signature(path, owner.cancel.clone()).await
                        {
                            signature.apply(&mut item);
                        }
                        downloaded_bytes += item.saved_bytes;
                        completed.push(item);
                    }
                    Err(error) if !cancelled => errors.push(error),
                    Err(_) => {},
                }
                if cancelled { break; }
                let saved = completed.len();
                let bytes = downloaded_bytes;
                let ui = owner.ui.clone();
                let _ = ui.upgrade_in_event_loop(move |app| {
                    app.set_progress(100.0 * (position + 1) as f32 / total as f32);
                    app.set_progress_detail(format!("{saved} / {total} files saved · {} downloaded", downloader::format_bytes(bytes)));
                });
                if policy.delay_ms > 0 {
                    tokio::select! { _ = tokio::time::sleep(std::time::Duration::from_millis(
                        policy.delay_ms.min(3_600_000) as u64,
                    )) => {}, _ = async { while !owner.cancel.load(Ordering::Relaxed) { tokio::time::sleep(std::time::Duration::from_millis(50)).await; } } => {} }
                }
            }
            let count = completed.len();
            if !completed.is_empty() {
                let mut entries = owner.history.lock().unwrap();
                let mut snapshot = account.entry.clone();
                for item in completed {
                    let identity = crate::media::identity::key(&item);
                    if let Some(previous) = snapshot
                        .media
                        .iter_mut()
                        .find(|previous| crate::media::identity::key(previous) == identity)
                    {
                        *previous = item;
                    }
                }
                // Keep successful files even if the fetch sessions were removed during this job.
                entries.retain(|entry| entry.id != snapshot.id);
                entries.insert(0, snapshot);
                if let Some(account) = catalog::accounts(&entries)
                    .into_iter()
                    .find(|account| account.key == key)
                {
                    catalog::persist(&mut entries, &account);
                }
            }
            let ui = owner.ui.clone();
            let _ = ui.upgrade_in_event_loop(move |app| {
                app.set_is_busy(false);
                app.update_library(|data| {
                    data.job.clear();
                    data.error = errors.first().cloned().unwrap_or_default();
                });
                app.invoke_library_refresh();
                let entries = owner.history.lock().unwrap();
                let filter = owner.filter.lock().unwrap();
                crate::history::view_model::refresh(&app, &entries, &filter.0, false);
                crate::app::notifications::show_library_toast(
                    &app,
                    {
                        let summary = crate::i18n::translate(&app.global::<crate::I18n>().get_language(), "{} library files downloaded.").replace("{}", &count.to_string());
                        if let Some(error) = errors.first() { format!("{summary}\n{error}") } else { summary }
                    },
                    if errors.is_empty() { "info" } else { "error" },
                );
                app.update_library(|data| data.feedback_active = false);
            });
        });
    });
}

pub(super) fn register_refetch(
    app: &AppWindow,
    context: Context,
    runner: crate::fetch::batch::Runner,
) {
    let owner = context.clone();
    app.on_library_refetch(move |key| start_refetch(&owner, runner, vec![key]));
    app.on_library_refetch_selected(move || {
        if let Some(app) = context.ui.upgrade() {
            start_refetch(&context, runner, app.get_library().selection);
        }
    });
}

fn start_refetch(context: &Context, runner: crate::fetch::batch::Runner, keys: Vec<String>) {
    let Some(app) = context.ui.upgrade() else {
        return;
    };
    if app.get_is_busy()
        || app.get_history_busy()
        || app.get_auth_check_busy()
        || app.get_tool_update_busy()
        || app.get_xtractor_busy()
        || !app.get_library().job.is_empty()
    {
        return;
    }
    if app.get_saved_auth_token().trim().is_empty() {
        crate::app::notifications::show_library_toast(
            &app,
            "An X auth_token is required before extraction.",
            "error",
        );
        return;
    }
    let accounts: Arc<Vec<_>> = Arc::new(
        context
            .catalog
            .lock()
            .unwrap()
            .iter()
            .filter(|account| keys.contains(&account.key))
            .cloned()
            .collect(),
    );
    if accounts.is_empty() {
        return;
    }
    context.cancel.store(false, Ordering::Relaxed);
    app.update_library(|data| {
        data.feedback_active = true;
        data.error.clear();
        data.queue = accounts
            .iter()
            .map(|account| crate::LibraryJob {
                key: account.key.clone(),
                status: "Queued".into(),
                ..Default::default()
            })
            .collect();
    });
    next_refetch(context.clone(), runner, accounts, 0);
}

fn next_refetch(
    context: Context,
    runner: crate::fetch::batch::Runner,
    accounts: Arc<Vec<catalog::Account>>,
    position: usize,
) {
    let Some(app) = context.ui.upgrade() else {
        return;
    };
    if position >= accounts.len() || context.cancel.load(Ordering::Relaxed) {
        app.set_is_busy(false);
        app.set_is_extracting(false);
        app.update_library(|data| {
            data.job.clear();
            data.feedback_active = false;
            data.loading = false;
            for job in &mut data.queue {
                if job.status == "Queued" {
                    job.status = "Cancelled".into();
                }
            }
        });
        app.invoke_library_refresh();
        return;
    }
    let account = &accounts[position];
    let key = account.key.clone();
    let mut profile = account.entry.profile.clone().unwrap();
    profile.media_timeline = crate::fetch::request::uses_media_timeline(
        &profile.url,
        &profile.retweets,
        profile.text_tweets,
    );
    let options = downloader::ExtractionOptions {
        url: profile.url.clone(),
        media_timeline: profile.media_timeline,
        media_reposts: false,
        auth_token: app.get_saved_auth_token(),
        max_posts: usize::MAX,
        page_size: downloader::POSTS_PER_REQUEST,
        media_type: profile.media_type.clone(),
        retweets: profile.retweets.clone(),
        no_videos: false,
        text_tweets: profile.text_tweets,
        cursor: String::new(),
        previous_json: None,
        refresh_boundary: Default::default(),
    };
    app.update_library(|data| {
        data.job = key.clone();
        data.queue[position].status = "Fetching".into();
    });
    app.set_is_busy(true);
    app.set_is_extracting(true);
    app.set_progress(0.0);
    app.set_progress_detail(String::new());
    app.set_status(format!(
        "{} · @{} ({}/{})",
        crate::i18n::translate(
            &app.global::<crate::I18n>().get_language(),
            "Updating library account…"
        ),
        key,
        position + 1,
        accounts.len()
    ));
    let owner = context.clone();
    runner(
        context.ui.clone(),
        options,
        profile,
        Arc::new(Mutex::new(downloader::BackendState::default())),
        context.history.clone(),
        context.filter.clone(),
        context.cancel.clone(),
        Some(Box::new(move |result| {
            if let Some(app) = owner.ui.upgrade() {
                app.set_is_busy(true);
                app.update_library(|data| data.loading = true);
            }
            let work = move || {
                if !result.history_id.is_empty() {
                    let entries = owner.history.lock().unwrap().clone();
                    if let Some(account) = catalog::accounts(&entries)
                        .into_iter()
                        .find(|account| account.key == key)
                    {
                        catalog::persist(&mut owner.history.lock().unwrap(), &account);
                    }
                }
                let ui = owner.ui.clone();
                let _ = ui.upgrade_in_event_loop(move |app| {
                    let cancelled = result.cancelled || owner.cancel.load(Ordering::Relaxed);
                    if cancelled {
                        owner.cancel.store(true, Ordering::Relaxed);
                    }
                    app.update_library(|data| {
                        data.loading = false;
                        let job = &mut data.queue[position];
                        job.status = if cancelled {
                            "Cancelled"
                        } else if result.error.is_some() {
                            "Failed"
                        } else {
                            "Completed"
                        }
                        .into();
                        job.media_count = result.media_count.min(i32::MAX as usize) as i32;
                        job.error = result.error.clone().unwrap_or_default();
                        if !job.error.is_empty() {
                            data.error.clone_from(&job.error);
                        }
                    });
                    if let Some(error) = result.error {
                        crate::app::notifications::show_library_toast(&app, error, "error");
                    }
                    next_refetch(owner, runner, accounts, position + 1);
                });
            };
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn_blocking(work);
            } else {
                std::thread::spawn(work);
            }
        })),
    );
}
