use crate::ui::runtime::{FilterModel, Model, ModelRc};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use crate::AppWindow;
use crate::MediaItem;
use crate::MediaResults;
use crate::MediaSelection;
#[cfg(test)]
use crate::download::engine as downloader;
use crate::history::paged::PagedModel;

pub fn matches_kind(media_type: &str, kind: &str) -> bool {
    kind == "all" || kind.split('|').any(|kind| media_type == kind)
}

fn matches(item: &MediaItem, kind: &str) -> bool {
    match kind {
        "library-saved" => item.status == "Saved",
        "library-missing" => item.status == "Missing",
        "library-pending" => item.status != "Saved",
        _ => matches_kind(item.media_type.as_str(), kind),
    }
}

pub fn toggle_kind(current: &str, kind: &str) -> String {
    if kind == "all" {
        return "all".into();
    }
    if current == "all" {
        return kind.into();
    }
    let mut kinds = current
        .split('|')
        .filter(|value| *value != "none" && !value.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if kinds.iter().any(|value| value == kind) {
        kinds.retain(|value| value != kind);
    } else {
        kinds.push(kind.into());
    }
    if kinds.is_empty() {
        "none".into()
    } else {
        kinds.join("|")
    }
}
#[derive(Default)]
pub(crate) struct ViewCache {
    source: Option<ModelRc<MediaItem>>,
    key: Option<(String, i32, i32, u64, usize)>,
    view: Option<ModelRc<MediaItem>>,
}
impl ViewCache {
    pub fn source_changed(&self, source: &ModelRc<MediaItem>) -> bool {
        self.source
            .as_ref()
            .is_none_or(|previous| !previous.ptr_eq(source))
    }
    pub fn get(
        &mut self,
        source: ModelRc<MediaItem>,
        kind: &str,
        limit: i32,
        order: i32,
    ) -> ModelRc<MediaItem> {
        let key = (
            kind.to_owned(),
            limit,
            order,
            source.revision(),
            source.row_count(),
        );
        if self.source_changed(&source) || self.key.as_ref() != Some(&key) {
            self.view = Some(ordered(&source, kind, limit, order));
            self.source = Some(source);
            self.key = Some(key);
        }
        self.view.as_ref().unwrap().clone()
    }
}
#[derive(Default)]
struct Counts {
    source: Option<std::rc::Weak<dyn Model<Data = MediaItem>>>,
    revision: u64,
    kinds: HashMap<String, i32>,
    total: i32,
    new: i32,
}
impl Counts {
    fn refresh(&mut self, source: &ModelRc<MediaItem>) {
        if self
            .source
            .as_ref()
            .is_some_and(|previous| previous.ptr_eq(&source.downgrade()))
            && self.revision == source.revision()
        {
            return;
        }
        self.kinds.clear();
        self.total = 0;
        self.new = 0;
        source.visit(&mut |_, row| {
            if let Some(count) = self.kinds.get_mut(&row.media_type) {
                *count += 1;
            } else {
                self.kinds.insert(row.media_type.clone(), 1);
            }
            self.total += 1;
            self.new += i32::from(row.is_new);
            for kind in ["library-saved", "library-missing", "library-pending"] {
                if matches(row, kind) {
                    *self.kinds.entry(kind.into()).or_default() += 1;
                }
            }
            true
        });
        self.source = Some(source.downgrade());
        self.revision = source.revision();
    }
    fn count(&self, kind: &str) -> i32 {
        if kind == "all" {
            self.total
        } else if kind.starts_with("library-") {
            self.kinds.get(kind).copied().unwrap_or_default()
        } else {
            self.kinds
                .iter()
                .filter(|(media_type, _)| matches_kind(media_type, kind))
                .map(|(_, count)| count)
                .sum()
        }
    }
}

pub fn find(items: &ModelRc<MediaItem>, index: i32) -> Option<MediaItem> {
    if let Some(model) = items.as_any().downcast_ref::<PagedModel>() {
        model.find(index)
    } else {
        items.find_row(|item| item.media_index == index)
    }
}

pub fn register(app: &AppWindow) {
    let selection = app.global::<MediaSelection>();
    selection.on_all_selected(|indices, items, kind| {
        if indices.row_count() == 0 {
            return false;
        }
        let selected = indices.iter().collect::<HashSet<_>>();
        if let Some(model) = items.as_any().downcast_ref::<PagedModel>() {
            let count = model.eligible_count(&kind);
            if count == 0 || selected.len() < count {
                return false;
            }
            let eligible = model.eligible(&kind);
            return !eligible.is_empty() && eligible.iter().all(|index| selected.contains(index));
        }
        let mut eligible = false;
        let mut complete = true;
        items.visit(&mut |_, item| {
            if item.status != "Saved" && matches(item, &kind) {
                eligible = true;
                complete = selected.contains(&item.media_index);
            }
            complete
        });
        eligible && complete
    });
    let ui = app.as_weak();
    selection.on_clear(move || {
        if let Some(app) = ui.upgrade() {
            app.global::<MediaSelection>()
                .set_indices(crate::ui::runtime::ModelRc::default());
        }
    });
    let ui = app.as_weak();
    selection.on_toggle(move |index| {
        let Some(app) = ui.upgrade() else { return };
        if app.get_is_busy()
            || app.get_xtractor_busy()
            || find(&app.get_media_items(), index).is_none_or(|item| item.status == "Saved")
        {
            return;
        }
        let selection = app.global::<MediaSelection>();
        let mut values = selection.get_indices().iter().collect::<HashSet<_>>();
        if !values.remove(&index) {
            values.insert(index);
        }
        set_selection(&selection, values);
    });
    let ui = app.as_weak();
    selection.on_toggle_filter(move |kind| {
        let Some(app) = ui.upgrade() else { return };
        if app.get_is_busy() || app.get_xtractor_busy() {
            return;
        }
        let items = app.get_media_items();
        let eligible = if let Some(model) = items.as_any().downcast_ref::<PagedModel>() {
            model.eligible(&kind)
        } else {
            items
                .iter()
                .filter(|item| item.status != "Saved" && matches(item, &kind))
                .map(|item| item.media_index)
                .collect::<Vec<_>>()
        };
        let selection = app.global::<MediaSelection>();
        let mut values = selection.get_indices().iter().collect::<HashSet<_>>();
        if eligible.iter().all(|index| values.contains(index)) {
            for index in eligible {
                values.remove(&index);
            }
        } else {
            values.extend(eligible);
        }
        set_selection(&selection, values);
    });
    let results = app.global::<MediaResults>();
    let counts = Rc::new(RefCell::new(Counts::default()));
    results.on_count({
        let counts = counts.clone();
        move |items, kind| {
            if let Some(model) = items.as_any().downcast_ref::<PagedModel>() {
                return model.count(&kind).min(i32::MAX as usize) as i32;
            }
            let mut counts = counts.borrow_mut();
            counts.refresh(&items);
            counts.count(&kind)
        }
    });
    results.on_new_count(move |items| {
        if let Some(model) = items.as_any().downcast_ref::<PagedModel>() {
            return model.new_count().min(i32::MAX as usize) as i32;
        }
        let mut counts = counts.borrow_mut();
        counts.refresh(&items);
        counts.new
    });
    results.on_position(|items, source_index| {
        if let Some(position) = crate::history::paged::position(&items, source_index) {
            return position;
        }
        let mut position = -1;
        items.visit(&mut |index, item| {
            if item.media_index == source_index {
                position = index as i32;
                false
            } else {
                true
            }
        });
        position
    });
}

/// Callbacks like `all-selected` read the whole media list without tracking
/// rows, so they only re-run when their arguments change. After statuses are
/// written in place, re-set the same selection to refresh those labels.
pub fn refresh_selection_state(app: &AppWindow) {
    let selection = app.global::<MediaSelection>();
    let values = selection.get_indices().iter().collect();
    set_selection(&selection, values);
}

fn set_selection(selection: &MediaSelection, values: HashSet<i32>) {
    let mut values = values.into_iter().collect::<Vec<_>>();
    values.sort_unstable();
    selection.set_indices(crate::ui::runtime::ModelRc::new(
        crate::ui::runtime::VecModel::from(values),
    ));
}

pub fn retain_pending_selection(app: &AppWindow) {
    let items = app.get_media_items();
    let eligible = if let Some(model) = items.as_any().downcast_ref::<PagedModel>() {
        model.eligible("all").into_iter().collect::<HashSet<_>>()
    } else {
        items
            .iter()
            .filter(|item| item.status != "Saved")
            .map(|item| item.media_index)
            .collect()
    };
    let selection = app.global::<MediaSelection>();
    set_selection(
        &selection,
        selection
            .get_indices()
            .iter()
            .filter(|index| eligible.contains(index))
            .collect(),
    );
}

fn filter(items: &ModelRc<MediaItem>, kind: &str, limit: i32) -> ModelRc<MediaItem> {
    if let Some(model) = items.as_any().downcast_ref::<PagedModel>() {
        return model.ordered(kind, limit.max(0) as usize, 0);
    }
    let limit = limit.max(0) as usize;
    let mut visible = HashSet::new();
    items.visit(&mut |_, item| {
        if visible.len() < limit && matches(item, kind) {
            visible.insert(item.media_index);
        }
        visible.len() < limit
    });
    // Keep a live model so async image updates reach existing grid/preview rows.
    ModelRc::new(FilterModel::new(items.clone(), move |item| {
        visible.contains(&item.media_index)
    }))
}

fn ordered(items: &ModelRc<MediaItem>, kind: &str, limit: i32, order: i32) -> ModelRc<MediaItem> {
    if let Some(model) = items.as_any().downcast_ref::<PagedModel>() {
        return model.ordered(kind, limit.max(0) as usize, order);
    }
    if order == 0 {
        return filter(items, kind, limit);
    }
    let sorted = ModelRc::new(crate::ui::runtime::SortModel::new(
        items.clone(),
        move |left: &MediaItem, right: &MediaItem| match (
            left.sort_key.is_empty(),
            right.sort_key.is_empty(),
        ) {
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            _ => {
                let dates = if order == 1 {
                    right.sort_key.cmp(&left.sort_key)
                } else {
                    left.sort_key.cmp(&right.sort_key)
                };
                dates.then_with(|| left.media_index.cmp(&right.media_index))
            }
        },
    ));
    filter(&sorted, kind, limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sparse_filters_inspect_metadata_without_cloning_skipped_rows() {
        struct Tracked {
            rows: crate::ui::runtime::VecModel<MediaItem>,
            reads: Rc<std::cell::Cell<usize>>,
        }
        impl Model for Tracked {
            type Data = MediaItem;
            fn row_count(&self) -> usize {
                self.rows.row_count()
            }
            fn row_data(&self, row: usize) -> Option<MediaItem> {
                self.reads.set(self.reads.get() + 1);
                self.rows.row_data(row)
            }
            fn visit(&self, visitor: &mut dyn FnMut(usize, &MediaItem) -> bool) {
                self.rows.visit(visitor);
            }
        }
        let reads = Rc::new(std::cell::Cell::new(0));
        let source = ModelRc::new(Tracked {
            rows: crate::ui::runtime::VecModel::from(
                (0..5000)
                    .map(|index| MediaItem {
                        media_index: index,
                        media_type: if index < 4980 { "photo" } else { "text" }.into(),
                        ..Default::default()
                    })
                    .collect::<Vec<_>>(),
            ),
            reads: reads.clone(),
        });
        let visible = ordered(&source, "text", 30, 0);
        assert_eq!(visible.row_count(), 20);
        assert_eq!(reads.get(), 0);
        assert_eq!(visible.iter().take(5).count(), 5);
        assert_eq!(reads.get(), 5);
    }

    #[test]
    fn cached_views_and_counts_follow_row_updates_and_model_replacements() {
        let model = Rc::new(crate::ui::runtime::VecModel::from(vec![
            MediaItem {
                media_index: 7,
                media_type: "photo".into(),
                sort_key: "2025".into(),
                ..Default::default()
            },
            MediaItem {
                media_index: 42,
                media_type: "video".into(),
                sort_key: "2026".into(),
                ..Default::default()
            },
        ]));
        let source = ModelRc::from(model.clone());
        let mut cache = ViewCache::default();
        let first = cache.get(source.clone(), "all", 1, 1);
        assert_eq!(first.row_data(0).unwrap().media_index, 42);
        assert!(first.ptr_eq(&cache.get(source.clone(), "all", 1, 1)));
        let mut counts = Counts::default();
        let references = Rc::strong_count(&model);
        counts.refresh(&source);
        assert_eq!(Rc::strong_count(&model), references);
        assert_eq!(counts.count("photo|video"), 2);
        let mut changed = model.row_data(0).unwrap();
        changed.sort_key = "2027".into();
        changed.media_type = "video".into();
        changed.is_new = true;
        model.set_row_data(0, changed);
        assert_eq!(
            cache
                .get(source.clone(), "all", 1, 1)
                .row_data(0)
                .unwrap()
                .media_index,
            7
        );
        counts.refresh(&source);
        assert_eq!(
            (counts.count("photo"), counts.count("video"), counts.new),
            (0, 2, 1)
        );
        let replacement = ModelRc::new(crate::ui::runtime::VecModel::from(vec![MediaItem {
            media_index: 99,
            media_type: "text".into(),
            ..Default::default()
        }]));
        assert!(cache.source_changed(&replacement));
        assert_eq!(
            cache
                .get(replacement.clone(), "all", 30, 0)
                .row_data(0)
                .unwrap()
                .media_index,
            99
        );
        counts.refresh(&replacement);
        assert_eq!((counts.total, counts.new, counts.count("video")), (1, 0, 0));
    }

    #[test]
    fn sorting_is_stable_keeps_source_indices_and_live_row_updates() {
        let model = std::rc::Rc::new(crate::ui::runtime::VecModel::from(vec![
            MediaItem {
                media_index: 9,
                media_type: "photo".into(),
                sort_key: "2025-01-01T00:00:00".into(),
                ..Default::default()
            },
            MediaItem {
                media_index: 3,
                media_type: "photo".into(),
                sort_key: "2026-01-01T00:00:00".into(),
                ..Default::default()
            },
            MediaItem {
                media_index: 7,
                media_type: "video".into(),
                sort_key: "2025-06-01T00:00:00".into(),
                ..Default::default()
            },
            MediaItem {
                media_index: 12,
                media_type: "photo".into(),
                ..Default::default()
            },
        ]));
        let items = ModelRc::from(model.clone());
        let newest = ordered(&items, "all", 2, 1);
        assert_eq!(
            newest
                .iter()
                .map(|item| item.media_index)
                .collect::<Vec<_>>(),
            [3, 7]
        );
        assert_eq!(
            ordered(&items, "photo", 10, 2)
                .iter()
                .map(|item| item.media_index)
                .collect::<Vec<_>>(),
            [9, 3, 12]
        );
        let mut row = model.row_data(1).unwrap();
        row.status = "Saved".into();
        model.set_row_data(1, row);
        assert_eq!(newest.row_data(0).unwrap().status, "Saved");
    }

    #[test]
    fn filters_keep_source_indices_and_order_for_preview_and_download() {
        let items = downloader::model_from_rows(
            ["photo", "animated_gif", "video", "photo"]
                .into_iter()
                .enumerate()
                .map(|(index, kind)| MediaItem {
                    media_type: kind.into(),
                    media_index: index as i32,
                    ..Default::default()
                })
                .collect(),
        );
        let photos = filter(&items, "photo", 60);
        assert_eq!(
            photos
                .iter()
                .map(|item| item.media_index)
                .collect::<Vec<_>>(),
            [0, 3]
        );
        assert_eq!(
            filter(&items, "video", 60).row_data(0).unwrap().media_index,
            2
        );
        assert_eq!(
            filter(&items, "animated_gif", 60)
                .row_data(0)
                .unwrap()
                .media_index,
            1
        );
        assert_eq!(filter(&items, "all", 2).row_count(), 2);
        assert_eq!(filter(&items, "text", 60).row_count(), 0);
        assert_eq!(filter(&items, "all", 0).row_count(), 0);
        let source = items
            .as_any()
            .downcast_ref::<crate::ui::runtime::VecModel<MediaItem>>()
            .unwrap();
        let mut updated = source.row_data(3).unwrap();
        updated.title = "Async thumbnail loaded".into();
        updated.status = "Saved".into();
        source.set_row_data(3, updated);
        assert_eq!(photos.row_data(1).unwrap().title, "Async thumbnail loaded");
        assert_eq!(photos.row_data(1).unwrap().status, "Saved");
    }
}

#[cfg(test)]
mod combination_tests {
    use super::*;
    #[test]
    fn combined_filters_keep_indices_and_text_remains_an_independent_type() {
        assert_eq!(toggle_kind("all", "photo"), "photo");
        assert_eq!(toggle_kind("photo", "video"), "photo|video");
        assert_eq!(toggle_kind("photo|video", "photo"), "video");
        assert_eq!(toggle_kind("video", "video"), "none");
        assert!(matches_kind("photo", "photo|video"));
        assert!(!matches_kind("text", "photo|video"));
        let items = downloader::model_from_rows(
            ["photo", "animated_gif", "video", "text"]
                .into_iter()
                .enumerate()
                .map(|(index, kind)| MediaItem {
                    media_type: kind.into(),
                    media_index: (index * 2) as i32,
                    ..Default::default()
                })
                .collect(),
        );
        assert_eq!(
            filter(&items, "photo|video", 30)
                .iter()
                .map(|item| item.media_index)
                .collect::<Vec<_>>(),
            [0, 4]
        );
        assert_eq!(
            filter(&items, "text", 30).row_data(0).unwrap().media_index,
            6
        );
    }
}
