use crate::AppWindow;
use crate::HistoryItem;
use crate::download::engine::HistoryEntry;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex, OnceLock},
};

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Label {
    pub group: String,
    pub tags: Vec<String>,
    #[serde(default)]
    pub color: String,
}
type Catalog = BTreeMap<String, Label>;
fn catalog() -> &'static Mutex<Catalog> {
    static DATA: OnceLock<Mutex<Catalog>> = OnceLock::new();
    DATA.get_or_init(|| Mutex::new(BTreeMap::new()))
}
pub fn initialize() {
    match crate::history::db::load_account_labels() {
        Ok(data) => *catalog().lock().unwrap() = data,
        Err(error) => {
            crate::platform::logging::error(format!("Could not load account labels: {error}"))
        }
    }
}
fn parse_color(value: &str) -> Option<[u8; 3]> {
    let value = value.strip_prefix('#')?;
    if value.len() != 6 || !value.bytes().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some([
        u8::from_str_radix(&value[0..2], 16).ok()?,
        u8::from_str_radix(&value[2..4], 16).ok()?,
        u8::from_str_radix(&value[4..6], 16).ok()?,
    ])
}
pub fn normalize(group: &str, tags: &str) -> Result<Label, String> {
    let group = group.trim();
    if group.chars().count() > 64 || group.chars().any(char::is_control) {
        return Err("Group names must be at most 64 characters.".into());
    }
    let mut seen = BTreeSet::new();
    let mut values = Vec::new();
    for tag in tags
        .split(',')
        .map(|value| value.trim().trim_start_matches('#'))
        .filter(|value| !value.is_empty())
    {
        if tag.chars().count() > 40 || tag.chars().any(char::is_control) {
            return Err("Tags must be at most 40 characters.".into());
        }
        if seen.insert(tag.to_lowercase()) {
            values.push(tag.to_owned());
        }
    }
    if values.len() > 16 {
        return Err("Use at most 16 tags per account.".into());
    }
    Ok(Label {
        group: group.into(),
        tags: values,
        color: String::new(),
    })
}
pub fn decorate(_app: &AppWindow, mut rows: Vec<HistoryItem>) -> Vec<HistoryItem> {
    let data = catalog().lock().unwrap();
    for row in &mut rows {
        if let Some(label) = data.get(&row.handle.trim_start_matches('@').to_ascii_lowercase()) {
            if let Some(rgb) = parse_color(&label.color) {
                row.account_color = crate::ui::runtime::Color::from_rgb(rgb[0], rgb[1], rgb[2]);
            }
            row.account_labels = [label.group.clone()]
                .into_iter()
                .filter(|value| !value.is_empty())
                .chain(label.tags.iter().map(|tag| format!("#{tag}")))
                .collect::<Vec<_>>()
                .join(" · ");
        }
    }
    rows
}
pub(crate) fn decorate_library(app: &AppWindow, rows: Vec<HistoryItem>) -> Vec<HistoryItem> {
    let mut rows = decorate(app, rows);
    let labels = catalog().lock().unwrap();
    let mut groups = BTreeSet::new();
    for row in &mut rows {
        if let Some(label) = labels.get(&row.handle.to_ascii_lowercase()) {
            row.group_label.clone_from(&label.group);
            if !label.group.is_empty() {
                groups.insert(label.group.clone());
            }
        }
    }
    app.update_library(|data| {
        data.groups = groups.into_iter().collect();
        if !data.groups.contains(&data.group_filter) {
            data.group_filter.clear();
        }
        data.selection
            .retain(|key| rows.iter().any(|row| &row.id == key));
    });
    rows
}
pub fn txt(entries: &[HistoryEntry], ids: &[String]) -> String {
    let mut handles = BTreeSet::new();
    for entry in entries
        .iter()
        .filter(|entry| ids.iter().any(|id| id == &entry.id))
    {
        let handle = entry
            .username
            .trim()
            .trim_start_matches('@')
            .to_ascii_lowercase();
        if !handle.is_empty()
            && handle.len() <= 15
            && handle
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_')
        {
            handles.insert(handle);
        }
    }
    handles
        .into_iter()
        .map(|handle| format!("@{handle}\n"))
        .collect()
}
pub fn register(
    app: &AppWindow,
    history: Arc<Mutex<Vec<HistoryEntry>>>,
    filter: Arc<Mutex<(String, bool)>>,
) {
    register_with_writer(
        app,
        history,
        filter,
        crate::history::db::save_account_labels,
    );
}
pub(crate) fn register_with_writer(
    app: &AppWindow,
    history: Arc<Mutex<Vec<HistoryEntry>>>,
    filter: Arc<Mutex<(String, bool)>>,
    write: fn(Vec<(String, Label)>) -> Result<(), String>,
) {
    let ui = app.as_weak();
    let records = history.clone();
    let current = filter.clone();
    app.on_apply_account_labels(move |group, tags, color| {
        let Some(app) = ui.upgrade() else {
            return false;
        };
        if app.get_is_busy() || app.get_history_busy() {
            return false;
        }
        let mut label = match normalize(&group, &tags) {
            Ok(label) => label,
            Err(error) => {
                app.set_account_labels_error(error);
                return false;
            }
        };
        if !color.is_empty() && parse_color(&color).is_none() {
            app.set_account_labels_error("Choose a valid group color.".into());
            return false;
        }
        label.color = color.to_string();
        let library = app.get_library();
        let handles = library
            .items
            .iter()
            .filter(|row| library.selection.contains(&row.id))
            .map(|row| row.handle.trim_start_matches('@').to_ascii_lowercase())
            .filter(|handle| !handle.is_empty())
            .collect::<BTreeSet<_>>();
        if handles.is_empty() {
            app.set_account_labels_error("Select accounts first.".into());
            return false;
        }
        let mut updates = handles
            .into_iter()
            .map(|handle| (handle, label.clone()))
            .collect::<Vec<_>>();
        if !label.group.is_empty() {
            for (handle, existing) in catalog().lock().unwrap().iter() {
                if existing.group == label.group && !updates.iter().any(|(key, _)| key == handle) {
                    let mut updated = existing.clone();
                    updated.color = label.color.clone();
                    updates.push((handle.clone(), updated));
                }
            }
        }
        app.set_history_busy(true);
        app.set_account_labels_error("".into());
        app.set_account_labels_open(false);
        let ui = ui.clone();
        let history = records.clone();
        let filter = current.clone();
        tokio::spawn(async move {
            let persisted = updates.clone();
            let result = tokio::task::spawn_blocking(move || write(persisted)).await;
            let _ = ui.upgrade_in_event_loop(move |app| {
                app.set_history_busy(false);
                match result {
                    Ok(Ok(())) => {
                        catalog().lock().unwrap().extend(updates);
                        app.invoke_library_refresh();
                        let entries = history.lock().unwrap();
                        let filter = filter.lock().unwrap();
                        crate::history::view_model::refresh(&app, &entries, &filter.0, filter.1);
                        crate::app::notifications::show_library_toast(
                            &app,
                            "Account groups and tags saved.",
                            "info",
                        );
                    }
                    result => crate::app::notifications::show_library_toast(
                        &app,
                        format!("Could not save account labels: {result:?}"),
                        "error",
                    ),
                }
            });
        });
        true
    });
    let ui = app.as_weak();
    app.on_export_accounts_txt(move || {
        let Some(app) = ui.upgrade() else {
            return;
        };
        if app.get_history_busy() {
            return;
        }
        let ids = app
            .get_history_items()
            .iter()
            .map(|row| row.id)
            .collect::<Vec<_>>();
        let text = txt(&history.lock().unwrap(), &ids);
        if text.is_empty() {
            crate::app::notifications::show_toast(&app, "No accounts to export.", "info");
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Account list", &["txt"])
            .set_file_name("twitterdl-accounts.txt")
            .save_file()
        else {
            return;
        };
        let ui = ui.clone();
        tokio::spawn(async move {
            let result = tokio::fs::write(path, text).await;
            let _ = ui.upgrade_in_event_loop(move |app| {
                crate::app::notifications::show_toast(
                    &app,
                    if result.is_ok() {
                        "Account list exported."
                    } else {
                        "Could not export account list."
                    },
                    if result.is_ok() { "info" } else { "error" },
                )
            });
        });
    });
}

pub(crate) fn replace_catalog(data: BTreeMap<String, Label>) {
    *catalog().lock().unwrap() = data;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test(flavor = "current_thread")]
    async fn library_labels_save_without_fetch_history_and_failed_writes_preserve_labels() {
        let original = catalog().lock().unwrap().clone();
        let key = "label_fixture";
        catalog().lock().unwrap().insert(
            key.into(),
            Label {
                group: "Old group".into(),
                tags: vec!["old".into()],
                color: "#112233".into(),
            },
        );
        let app = AppWindow::new().unwrap();
        app.update_library(|data| {
            data.items =
                crate::ui::runtime::ModelRc::new(crate::ui::runtime::VecModel::from(vec![
                    HistoryItem {
                        id: key.into(),
                        handle: key.into(),
                        ..Default::default()
                    },
                ]));
            data.selection = vec![key.into()];
        });
        let history = Arc::new(Mutex::new(Vec::new()));
        let filter = Arc::new(Mutex::new((String::new(), false)));
        register_with_writer(&app, history.clone(), filter.clone(), |_| {
            Err("Fixture disk error".into())
        });
        assert!(app.invoke_apply_account_labels(
            "New group".into(),
            "#art".into(),
            "#1447e6".into()
        ));
        let wait = async {
            let ctx = egui::Context::default();
            for _ in 0..500 {
                crate::ui::runtime::pump(&ctx);
                if !app.get_history_busy() {
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
            panic!("Label write did not finish");
        };
        wait.await;
        assert_eq!(catalog().lock().unwrap()[key].group, "Old group");
        register_with_writer(&app, history.clone(), filter, |updates| {
            assert!(
                updates
                    .iter()
                    .any(|(handle, label)| handle == "label_fixture"
                        && label.tags == ["art"]
                        && label.color == "#1447e6")
            );
            Ok(())
        });
        assert!(app.invoke_apply_account_labels(
            "New group".into(),
            "#art".into(),
            "#1447e6".into()
        ));
        let ctx = egui::Context::default();
        for _ in 0..500 {
            crate::ui::runtime::pump(&ctx);
            if !app.get_history_busy() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        assert!(!app.get_history_busy());
        assert_eq!(catalog().lock().unwrap()[key].group, "New group");
        assert!(history.lock().unwrap().is_empty());
        let rows = decorate_library(&app, app.get_library().items.iter().collect());
        assert_eq!(rows[0].account_labels, "New group · #art");
        assert_eq!(app.get_library().groups, ["New group"]);
        app.set_history_group_filter("unrelated".into());
        assert_eq!(
            decorate(&app, rows).len(),
            1,
            "Library groups filtered fetch History"
        );
        *catalog().lock().unwrap() = original;
    }

    #[test]
    fn labels_and_txt_deduplicate_accounts_and_tags() {
        let label = normalize(" Artists ", "#art, ART,photo").unwrap();
        assert_eq!(label.group, "Artists");
        assert_eq!(label.tags, ["art", "photo"]);
        assert!(normalize(&"x".repeat(65), "").is_err());
        let records = vec![
            HistoryEntry {
                id: "1".into(),
                username: "NASA".into(),
                ..Default::default()
            },
            HistoryEntry {
                id: "2".into(),
                username: "nasa".into(),
                ..Default::default()
            },
            HistoryEntry {
                id: "3".into(),
                username: "demo".into(),
                ..Default::default()
            },
        ];
        assert_eq!(
            txt(&records, &["1".into(), "2".into(), "3".into()]),
            "@demo\n@nasa\n"
        );
    }
}
