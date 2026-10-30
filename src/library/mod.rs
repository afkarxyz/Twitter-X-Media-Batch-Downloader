pub(crate) mod catalog;
mod transfers;

use crate::download::engine::{self as downloader, BackendState, HistoryEntry};
use crate::ui::runtime::{Model, ModelRc};
use crate::{AppWindow, HistoryItem, MediaItem, MediaSelection};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

pub const TAB: i32 = 4;

#[derive(Clone)]
pub(super) struct Context {
    pub ui: crate::ui::runtime::Weak<AppWindow>,
    pub backend: Arc<Mutex<BackendState>>,
    pub history: Arc<Mutex<Vec<HistoryEntry>>>,
    pub filter: Arc<Mutex<(String, bool)>>,
    pub cancel: Arc<AtomicBool>,
    pub catalog: Arc<Mutex<Arc<Vec<catalog::Account>>>>,
}

#[derive(Clone)]
struct Workspace {
    backend: BackendState,
    model: ModelRc<MediaItem>,
    profile: crate::AccountProfile,
    selection: ModelRc<i32>,
}

impl Workspace {
    fn capture(app: &AppWindow, backend: &Arc<Mutex<BackendState>>) -> Self {
        Self {
            backend: backend.lock().unwrap().clone(),
            model: app.get_media_items(),
            profile: app.get_account_profile(),
            selection: app.global::<MediaSelection>().get_indices(),
        }
    }
    fn restore(self, app: &AppWindow, backend: &Arc<Mutex<BackendState>>) {
        *backend.lock().unwrap() = self.backend;
        app.set_media_items(self.model);
        app.set_account_profile(self.profile);
        app.global::<MediaSelection>().set_indices(self.selection);
    }
}

fn publish_account(
    app: &AppWindow,
    backend: &Arc<Mutex<BackendState>>,
    account: &catalog::Account,
    files: Vec<crate::DownloadedFile>,
    gif_count: i32,
) {
    let retaining = app.get_library().view_ready;
    let previous = backend.lock().unwrap();
    let selected = app.global::<MediaSelection>().get_indices();
    let selected_keys = if retaining {
        previous
            .candidates
            .iter()
            .filter(|item| {
                selected
                    .iter()
                    .any(|index| index == item.media_index as i32)
            })
            .map(crate::media::identity::key)
            .collect::<std::collections::HashSet<_>>()
    } else {
        Default::default()
    };
    let same_media = retaining
        && previous.candidates.len() == account.entry.media.len()
        && previous
            .candidates
            .iter()
            .zip(&account.entry.media)
            .all(|(old, new)| crate::media::identity::key(old) == crate::media::identity::key(new));
    drop(previous);
    app.set_selected_media_index(-1);
    *backend.lock().unwrap() = BackendState {
        candidates: account.entry.media.clone(),
        json: account.entry.json.clone(),
        profile: account.entry.profile.clone(),
        ..Default::default()
    };
    let model = downloader::to_ui_model(&account.entry.media);
    for (index, item) in account.entry.media.iter().enumerate() {
        if item.status == "Saved"
            && item.media_type == "photo"
            && let Ok(url) = reqwest::Url::from_file_path(&item.saved_path)
        {
            let mut row = model.row_data(index).unwrap();
            row.thumbnail_url = url.to_string();
            row.preview_url = url.to_string();
            row.preview_image =
                crate::media::images::image(crate::media::images::Kind::Thumbnail, url.as_str());
            row.detail_image =
                crate::media::images::image(crate::media::images::Kind::Preview, url.as_str());
            model.set_row_data(index, row);
        }
    }
    let current = app.get_media_items();
    if !same_media || downloader::update_rows_in_place(&current, model.iter().collect()).is_some() {
        app.set_media_items(model);
    }
    app.set_account_profile(downloader::account_profile_from_json(&account.entry.json));
    app.set_download_batch_id(account.entry.id.clone());
    app.set_download_batch_title(account.entry.account_name.clone());
    app.set_download_batch_folder(account.entry.download.as_ref().unwrap().output_dir.clone());
    app.set_download_batch_size(downloader::format_bytes(
        account.entry.download.as_ref().unwrap().total_bytes,
    ));
    app.set_download_batch_files(ModelRc::new(crate::ui::runtime::VecModel::from(files)));
    app.set_archive_gif_count(gif_count);
    let selected = account
        .entry
        .media
        .iter()
        .filter(|item| selected_keys.contains(&crate::media::identity::key(item)))
        .map(|item| item.media_index as i32)
        .collect::<Vec<_>>();
    app.global::<MediaSelection>()
        .set_indices(ModelRc::new(crate::ui::runtime::VecModel::from(selected)));
    app.update_library(|data| {
        data.view_ready = true;
        data.title.clone_from(&account.entry.account_name);
        data.folder = account.entry.download.as_ref().unwrap().output_dir.clone();
        data.available = account.available.min(i32::MAX as usize) as i32;
        data.missing = account.missing.min(i32::MAX as usize) as i32;
        data.pending = account.pending.min(i32::MAX as usize) as i32;
    });
}

fn rows(accounts: &[catalog::Account]) -> Vec<HistoryItem> {
    accounts
        .iter()
        .map(|account| {
            let profile = downloader::account_profile_from_json(&account.entry.json);
            HistoryItem {
                id: account.key.clone(),
                title: account.entry.account_name.clone(),
                handle: account.key.clone(),
                media_count: account.entry.media.len().min(i32::MAX as usize) as i32,
                saved_count: account.available.min(i32::MAX as usize) as i32,
                output_dir: account.entry.download.as_ref().unwrap().output_dir.clone(),
                size_text: downloader::format_bytes(
                    account.entry.download.as_ref().unwrap().total_bytes,
                ),
                avatar_url: profile.avatar_url,
                avatar_square: profile.avatar_square,
                avatar_image: profile.avatar_image,
                has_avatar: profile.has_avatar,
                ..Default::default()
            }
        })
        .collect()
}

pub fn register(
    app: &AppWindow,
    backend: Arc<Mutex<BackendState>>,
    history: Arc<Mutex<Vec<HistoryEntry>>>,
    filter: Arc<Mutex<(String, bool)>>,
    cancel: Arc<AtomicBool>,
) {
    let context = Context {
        ui: app.as_weak(),
        backend,
        history,
        filter,
        cancel,
        catalog: Arc::new(Mutex::new(Arc::new(Vec::new()))),
    };
    register_context(app, context);
}

fn register_context(app: &AppWindow, context: Context) {
    let workspace = Rc::new(RefCell::new(None::<Workspace>));
    let library_workspace = Rc::new(RefCell::new(None::<(String, Workspace)>));
    let generation = Arc::new(AtomicU64::new(0));
    let reset_home = workspace.clone();
    let reset_library = library_workspace.clone();
    let reset_generation = generation.clone();
    let reset_ui = app.as_weak();
    app.on_history_loaded(move || {
        *reset_home.borrow_mut() = None;
        *reset_library.borrow_mut() = None;
        reset_generation.fetch_add(1, Ordering::AcqRel);
        if let Some(app) = reset_ui.upgrade() {
            app.update_library(|data| {
                data.account.clear();
                data.view_ready = false;
                data.files_view = false;
                data.workspace_active = false;
                data.workspace_pending = false;
                data.items = Default::default();
                data.queue.clear();
                data.error.clear();
            });
            app.invoke_library_refresh();
        }
    });
    let weak = app.as_weak();
    let owner = context.clone();
    let saved = workspace.clone();
    let library_saved = library_workspace.clone();
    let refresh_generation = generation.clone();
    app.on_library_refresh(move || {
        let generation = refresh_generation.clone();
        let Some(app) = weak.upgrade() else { return };
        if app.get_selected_tab() != TAB {
            return;
        }
        let key = app.get_library().account;
        if !key.is_empty()
            && saved.borrow().is_none()
            && (!app.get_is_busy() || app.get_library().feedback_active)
        {
            *saved.borrow_mut() = Some(Workspace::capture(&app, &owner.backend));
            if let Some((cached_key, cached)) = library_saved.borrow().as_ref()
                && cached_key == &key
                && app.get_library().view_ready
            {
                cached.clone().restore(&app, &owner.backend);
            }
        }
        app.update_library(|data| {
            data.loading = true;
            data.workspace_active = saved.borrow().is_some();
            data.workspace_pending = !key.is_empty() && !data.workspace_active;
        });
        let version = generation.fetch_add(1, Ordering::AcqRel) + 1;
        let generation = generation.clone();
        let entries = owner.history.lock().unwrap().clone();
        let owner = owner.clone();
        let work = move || {
            if generation.load(Ordering::Acquire) != version {
                return;
            }
            let accounts = Arc::new(catalog::accounts(&entries));
            if generation.load(Ordering::Acquire) != version {
                return;
            }
            *owner.catalog.lock().unwrap() = accounts.clone();
            let selected = accounts.iter().find(|account| account.key == key);
            let files = selected
                .map(|account| crate::download::commands::file_rows(&account.entry))
                .unwrap_or_default();
            let gif_count = selected
                .map(|account| {
                    account
                        .entry
                        .media
                        .iter()
                        .filter(|item| crate::download::conversion::eligible(item))
                        .count() as i32
                })
                .unwrap_or_default();
            let ui = owner.ui.clone();
            let _ = ui.upgrade_in_event_loop(move |app| {
                if app.get_selected_tab() != TAB || generation.load(Ordering::Acquire) != version {
                    return;
                }
                let rows = crate::accounts::labels::decorate_library(&app, rows(&accounts));
                app.update_library(|data| {
                    data.loading = false;
                    data.items = downloader::model_from_history_rows(rows);
                });
                crate::media::images::load_history_avatars(&app);
                let data = app.get_library();
                if let Some(account) = accounts.iter().find(|account| account.key == data.account)
                    && data.workspace_active
                    && (!app.get_is_busy() || data.feedback_active)
                {
                    if !data.view_ready
                        || owner.backend.lock().unwrap().candidates != account.entry.media
                    {
                        if !data.view_ready {
                            catalog::persist(&mut owner.history.lock().unwrap(), account);
                        }
                        publish_account(&app, &owner.backend, account, files, gif_count);
                    } else {
                        app.set_download_batch_files(ModelRc::new(
                            crate::ui::runtime::VecModel::from(files),
                        ));
                        app.set_archive_gif_count(gif_count);
                        app.set_download_batch_id(account.entry.id.clone());
                        app.set_download_batch_title(account.entry.account_name.clone());
                        app.set_download_batch_folder(
                            account.entry.download.as_ref().unwrap().output_dir.clone(),
                        );
                        app.set_download_batch_size(downloader::format_bytes(
                            account.entry.download.as_ref().unwrap().total_bytes,
                        ));
                    }
                }
            });
        };
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn_blocking(work);
        } else {
            std::thread::spawn(work);
        }
    });
    let weak = app.as_weak();
    let owner = context.clone();
    app.on_library_open(move |key| {
        let Some(app) = weak.upgrade() else { return };
        if app.get_is_busy() || app.get_history_busy() {
            return;
        }
        app.set_selected_media_index(-1);
        app.update_library(|data| {
            data.account = key;
            data.error.clear();
            data.view_ready = false;
            data.files_view = false;
        });
        app.set_selected_tab(TAB);
        app.invoke_library_refresh();
    });
    let weak = app.as_weak();
    let owner_for_page = owner.clone();
    app.on_page_changed(move |tab| {
        let Some(app) = weak.upgrade() else { return };
        if tab == TAB {
            app.invoke_library_refresh();
        } else {
            generation.fetch_add(1, Ordering::AcqRel);
            let data = app.get_library();
            if data.workspace_active && data.view_ready {
                *library_workspace.borrow_mut() = Some((
                    data.account,
                    Workspace::capture(&app, &owner_for_page.backend),
                ));
            }
            app.update_library(|data| {
                data.workspace_active = false;
                data.workspace_pending = false;
                data.loading = false;
            });
            if let Some(saved) = workspace.borrow_mut().take() {
                saved.restore(&app, &owner_for_page.backend);
            }
            if tab == 1 {
                app.set_history_show_saved(false);
                app.invoke_search_history(app.get_history_search_query(), false);
            }
        }
    });
    transfers::register(app, context);
}

#[cfg(test)]
mod tests;
