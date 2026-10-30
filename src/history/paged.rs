use super::storage::{self, StoredEntry};
use crate::{
    MediaItem,
    download::engine::{self as downloader, HistoryEntry, MediaCandidate},
    ui::runtime::{self, Model, ModelRc},
};
use rusqlite::Connection;
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
};

#[derive(Clone, Debug)]
pub struct Descriptor {
    pub source: usize,
    pub ordinal: usize,
    pub index: i32,
    pub kind: String,
    pub status: String,
    pub saved_path: String,
    pub is_new: bool,
    pub stamp: Option<i64>,
}

#[derive(Debug)]
pub struct StoredView {
    pub sources: Vec<StoredEntry>,
    pub rows: RwLock<Vec<Descriptor>>,
    stats: RwLock<Stats>,
    pub positions: HashMap<i32, usize>,
    page_positions: HashMap<(usize, usize), Vec<(usize, usize)>>,
    pub header: HistoryEntry,
}

#[derive(Default, Debug)]
struct Stats {
    kinds: HashMap<(String, String), usize>,
    new: usize,
    total: usize,
}
impl Stats {
    fn add(&mut self, row: &Descriptor) {
        *self
            .kinds
            .entry((row.kind.clone(), row.status.clone()))
            .or_default() += 1;
        self.new += usize::from(row.is_new);
        self.total += 1;
    }
    fn remove(&mut self, row: &Descriptor) {
        if let Some(count) = self.kinds.get_mut(&(row.kind.clone(), row.status.clone())) {
            *count -= 1;
        }
        self.new -= usize::from(row.is_new);
        self.total -= 1;
    }
    fn count(&self, kind: &str, pending: bool) -> usize {
        self.kinds
            .iter()
            .filter(|((media, status), _)| {
                (!pending || status != "Saved")
                    && match kind {
                        "library-saved" => status == "Saved",
                        "library-missing" => status == "Missing",
                        "library-pending" => status != "Saved",
                        _ => crate::media::results::matches_kind(media, kind),
                    }
            })
            .map(|(_, count)| count)
            .sum()
    }
}

impl StoredView {
    pub fn prepare(
        entries: &[HistoryEntry],
        id: &str,
        individual: bool,
    ) -> rusqlite::Result<Option<Arc<Self>>> {
        let Some(selected) = entries.iter().find(|entry| entry.id == id) else {
            return Ok(None);
        };
        let key = crate::fetch::sessions::key(selected);
        let mut entries = if individual || selected.download.is_some() {
            vec![selected]
        } else {
            entries
                .iter()
                .filter(|entry| {
                    entry.download.is_none() && crate::fetch::sessions::key(entry) == key
                })
                .collect::<Vec<_>>()
        };
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.id.parse::<u64>().unwrap_or_default()));
        if entries.iter().any(|entry| entry.stored.is_none()) {
            return Ok(None);
        }
        let mut sources = Vec::new();
        let mut rows = Vec::new();
        let mut seen = HashSet::new();
        for (source, entry) in entries.iter().enumerate() {
            let stored = entry.stored.as_ref().unwrap().clone();
            let connection = Connection::open_with_flags(
                stored.path.as_ref(),
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )?;
            let mut statement = connection.prepare("SELECT ordinal,media_index,identity,kind,status,is_new,sort_stamp,saved_path FROM history_media_index WHERE entry_id=?1 ORDER BY ordinal")?;
            let mut cursor = statement.query([&stored.id])?;
            while let Some(row) = cursor.next()? {
                let identity: Vec<u8> = row.get(2)?;
                if !individual && !seen.insert(identity) {
                    continue;
                }
                rows.push(Descriptor {
                    source,
                    ordinal: row.get::<_, i64>(0)? as usize,
                    index: if individual {
                        row.get::<_, i64>(1)? as i32
                    } else {
                        rows.len() as i32
                    },
                    kind: row.get(3)?,
                    status: row.get(4)?,
                    saved_path: row.get(7)?,
                    is_new: row.get::<_, bool>(5)? && (individual || source == 0),
                    stamp: row.get(6)?,
                });
            }
            sources.push(stored);
        }
        let positions = rows
            .iter()
            .enumerate()
            .map(|(position, row)| (row.index, position))
            .collect();
        let mut page_positions: HashMap<_, Vec<_>> = HashMap::new();
        for (position, row) in rows.iter().enumerate() {
            page_positions
                .entry((row.source, row.ordinal / storage::PAGE_SIZE))
                .or_default()
                .push((row.ordinal % storage::PAGE_SIZE, position));
        }
        let mut stats = Stats::default();
        for row in &rows {
            stats.add(row);
        }
        Ok(Some(Arc::new(Self {
            sources,
            rows: RwLock::new(rows),
            stats: RwLock::new(stats),
            positions,
            page_positions,
            header: entries[0].clone(),
        })))
    }

    pub fn candidate(&self, index: i32) -> rusqlite::Result<MediaCandidate> {
        let row = self
            .rows
            .read()
            .unwrap()
            .get(
                *self
                    .positions
                    .get(&index)
                    .ok_or(rusqlite::Error::QueryReturnedNoRows)?,
            )
            .cloned()
            .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        let source = &self.sources[row.source];
        let connection = Connection::open_with_flags(
            source.path.as_ref(),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        let mut page =
            storage::read_page(&connection, &source.id, row.ordinal / storage::PAGE_SIZE)?;
        let mut item = page.swap_remove(row.ordinal % storage::PAGE_SIZE);
        item.media_index = index as usize;
        item.is_new = row.is_new;
        Ok(item)
    }

    pub fn media(
        &self,
        kind: &str,
        selected: &HashSet<usize>,
    ) -> rusqlite::Result<Vec<MediaCandidate>> {
        let rows = self
            .rows
            .read()
            .unwrap()
            .iter()
            .filter(|row| {
                if kind == "selected" {
                    selected.contains(&(row.index as usize))
                } else {
                    crate::media::results::matches_kind(&row.kind, kind)
                }
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut page = None;
        let mut result = Vec::new();
        for row in rows {
            let key = (row.source, row.ordinal / storage::PAGE_SIZE);
            if page.as_ref().is_none_or(|(previous, _)| previous != &key) {
                let source = &self.sources[row.source];
                let connection = Connection::open_with_flags(
                    source.path.as_ref(),
                    rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
                )?;
                page = Some((key, storage::read_page(&connection, &source.id, key.1)?));
            }
            let mut item = page.as_ref().unwrap().1[row.ordinal % storage::PAGE_SIZE].clone();
            item.media_index = row.index as usize;
            item.is_new = row.is_new;
            result.push(item);
        }
        Ok(result)
    }

    pub fn response(&self, individual: bool) -> rusqlite::Result<String> {
        let mut entries = Vec::new();
        for source in &self.sources {
            let connection = Connection::open_with_flags(
                source.path.as_ref(),
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )?;
            let mut entry = self.header.clone();
            entry.id = source.id.clone();
            entry.stored = None;
            entry.json = storage::read_response(&connection, &source.id)?;
            entries.push(entry);
            if individual {
                break;
            }
        }
        if entries.len() == 1 {
            return Ok(entries.remove(0).json);
        }
        Ok(crate::fetch::archives::merge(&entries[0], &entries).json)
    }
}

type PageKey = (usize, usize);
#[derive(Default)]
struct Cache {
    pages: HashMap<PageKey, (Vec<MediaItem>, u64)>,
    pending: HashSet<PageKey>,
    pinned: HashSet<PageKey>,
    failed: HashSet<PageKey>,
    clock: u64,
}

#[derive(Clone)]
pub struct PagedModel {
    pub view: Arc<StoredView>,
    cache: Arc<Mutex<Cache>>,
    sender: mpsc::Sender<PageKey>,
    revision: Arc<AtomicU64>,
    alive: Arc<()>,
    language: Arc<Mutex<String>>,
    #[cfg(test)]
    pub decoded_pages: Arc<AtomicU64>,
}

impl PagedModel {
    pub fn for_app(app: &crate::AppWindow, view: Arc<StoredView>) -> Self {
        let model = Self::new(view);
        model.set_language(app.global::<crate::I18n>().get_language());
        model
    }
    pub fn set_language(&self, language: String) {
        *self.language.lock().unwrap() = language;
    }
    pub fn new(view: Arc<StoredView>) -> Self {
        let cache = Arc::new(Mutex::new(Cache::default()));
        let (sender, receiver) = mpsc::channel::<PageKey>();
        let state = cache.clone();
        let data = view.clone();
        let alive = Arc::new(());
        let weak = Arc::downgrade(&alive);
        #[cfg(test)]
        let decoded_pages = Arc::new(AtomicU64::new(0));
        #[cfg(test)]
        let decoded = decoded_pages.clone();
        std::thread::spawn(move || {
            while let Ok(key) = receiver.recv() {
                if weak.strong_count() == 0 {
                    break;
                }
                let source = &data.sources[key.0];
                let result = Connection::open_with_flags(
                    source.path.as_ref(),
                    rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
                )
                .and_then(|connection| storage::read_page(&connection, &source.id, key.1))
                .map(|candidates| downloader::to_ui_rows(&candidates));
                #[cfg(test)]
                decoded.fetch_add(1, Ordering::Relaxed);
                let state = state.clone();
                let weak = weak.clone();
                let _ = runtime::invoke_from_event_loop(move || {
                    if weak.upgrade().is_none() {
                        return;
                    }
                    let mut cache = state.lock().unwrap();
                    cache.pending.remove(&key);
                    match result {
                        Ok(mut rows) => {
                            for row in &mut rows {
                                row.preview_image = crate::media::images::image(
                                    crate::media::images::Kind::Thumbnail,
                                    &row.thumbnail_url,
                                );
                                if row.media_type == "photo" {
                                    row.detail_image = crate::media::images::image(
                                        crate::media::images::Kind::Preview,
                                        &row.preview_url,
                                    );
                                }
                            }
                            cache.clock += 1;
                            let clock = cache.clock;
                            cache.pages.insert(key, (rows, clock));
                            trim(&mut cache);
                        }
                        Err(error) => {
                            cache.failed.insert(key);
                            crate::platform::logging::error(format!(
                                "Could not load saved media page: {error}"
                            ));
                        }
                    }
                });
            }
        });
        Self {
            view,
            cache,
            sender,
            revision: Arc::new(AtomicU64::new(0)),
            alive,
            language: Arc::new(Mutex::new("en".into())),
            #[cfg(test)]
            decoded_pages,
        }
    }

    fn get(&self, position: usize, request: bool) -> Option<MediaItem> {
        let descriptor = self.view.rows.read().unwrap().get(position)?.clone();
        let key = (descriptor.source, descriptor.ordinal / storage::PAGE_SIZE);
        let mut cache = self.cache.lock().unwrap();
        cache.clock += 1;
        let clock = cache.clock;
        if let Some((rows, used)) = cache.pages.get_mut(&key) {
            *used = clock;
            let mut row = rows.get(descriptor.ordinal % storage::PAGE_SIZE)?.clone();
            row.media_index = descriptor.index;
            row.is_new = descriptor.is_new;
            row.status = descriptor.status;
            row.saved_path = descriptor.saved_path;
            return Some(row);
        }
        let failed = cache.failed.contains(&key);
        if request && !failed && cache.pending.insert(key) {
            let _ = self.sender.send(key);
        }
        Some(MediaItem {
            media_index: descriptor.index,
            media_type: descriptor.kind,
            status: if failed {
                "Failed".into()
            } else {
                descriptor.status
            },
            is_new: descriptor.is_new,
            payload_pending: !failed,
            title: if failed {
                crate::i18n::translate(
                    &self.language.lock().unwrap(),
                    "Could not load saved media.",
                )
            } else {
                crate::i18n::translate(&self.language.lock().unwrap(), "Loading media…")
            },
            ..Default::default()
        })
    }

    pub fn find(&self, index: i32) -> Option<MediaItem> {
        self.get(*self.view.positions.get(&index)?, true)
    }
    pub fn count(&self, kind: &str) -> usize {
        self.view.stats.read().unwrap().count(kind, false)
    }
    pub fn eligible_count(&self, kind: &str) -> usize {
        self.view.stats.read().unwrap().count(kind, true)
    }
    pub fn new_count(&self) -> usize {
        self.view.stats.read().unwrap().new
    }
    pub fn eligible(&self, kind: &str) -> Vec<i32> {
        self.view
            .rows
            .read()
            .unwrap()
            .iter()
            .filter(|row| row.status != "Saved" && matches(row, kind))
            .map(|row| row.index)
            .collect()
    }
    pub fn ordered(&self, kind: &str, limit: usize, order: i32) -> ModelRc<MediaItem> {
        let rows = self.view.rows.read().unwrap();
        let mut positions = (0..rows.len())
            .filter(|&index| matches(&rows[index], kind))
            .collect::<Vec<_>>();
        if order != 0 {
            positions.sort_by(|&a, &b| {
                match (rows[a].stamp, rows[b].stamp) {
                    (Some(a), Some(b)) => {
                        if order == 1 {
                            b.cmp(&a)
                        } else {
                            a.cmp(&b)
                        }
                    }
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    _ => std::cmp::Ordering::Equal,
                }
                .then_with(|| rows[a].index.cmp(&rows[b].index))
            });
        }
        positions.truncate(limit);
        ModelRc::new(Ordered {
            base: self.clone(),
            positions,
        })
    }
    pub fn apply_candidates(&self, candidates: &[MediaCandidate]) {
        let mut rows = self.view.rows.write().unwrap();
        let mut cache = self.cache.lock().unwrap();
        for candidate in candidates {
            let Some(&position) = self.view.positions.get(&(candidate.media_index as i32)) else {
                continue;
            };
            let descriptor = &mut rows[position];
            self.view.stats.write().unwrap().remove(descriptor);
            descriptor.status.clone_from(&candidate.status);
            descriptor.saved_path.clone_from(&candidate.saved_path);
            descriptor.is_new = candidate.is_new;
            self.view.stats.write().unwrap().add(descriptor);
            if let Some((page, _)) = cache
                .pages
                .get_mut(&(descriptor.source, descriptor.ordinal / storage::PAGE_SIZE))
                && let Some(row) = page.get_mut(descriptor.ordinal % storage::PAGE_SIZE)
            {
                row.status.clone_from(&candidate.status);
                row.saved_path.clone_from(&candidate.saved_path);
                row.is_new = candidate.is_new;
            }
        }
        self.revision.fetch_add(1, Ordering::Relaxed);
    }
    pub fn update_loaded(&self, mut update: impl FnMut(&mut MediaItem)) {
        let descriptors = self.view.rows.read().unwrap();
        let mut cache = self.cache.lock().unwrap();
        for (key, (page, _)) in &mut cache.pages {
            if let Some(positions) = self.view.page_positions.get(key) {
                for &(slot, position) in positions {
                    if let Some(row) = page.get_mut(slot) {
                        row.media_index = descriptors[position].index;
                        update(row);
                    }
                }
            }
        }
    }
    pub fn pin(&self, indices: &HashSet<i32>) {
        let rows = self.view.rows.read().unwrap();
        let mut cache = self.cache.lock().unwrap();
        cache.pinned = indices
            .iter()
            .filter_map(|index| self.view.positions.get(index))
            .map(|&position| {
                let row = &rows[position];
                (row.source, row.ordinal / storage::PAGE_SIZE)
            })
            .collect();
        trim(&mut cache);
    }

    pub fn refresh_status(&self) {
        let view = self.view.clone();
        let cache = self.cache.clone();
        let revision = self.revision.clone();
        let generation = revision.load(Ordering::Relaxed);
        let alive = Arc::downgrade(&self.alive);
        std::thread::spawn(move || {
            let result = (|| -> rusqlite::Result<Vec<(usize, String, String)>> {
                let rows = view.rows.read().unwrap();
                let mut changed = Vec::new();
                for (source, stored) in view.sources.iter().enumerate() {
                    let connection = Connection::open_with_flags(
                        stored.path.as_ref(),
                        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
                    )?;
                    let mut statement=connection.prepare("SELECT ordinal,status,saved_path FROM history_media_index WHERE entry_id=?1")?;
                    let mut cursor = statement.query([&stored.id])?;
                    while let Some(row) = cursor.next()? {
                        let ordinal = row.get::<_, i64>(0)? as usize;
                        let status: String = row.get(1)?;
                        let path: String = row.get(2)?;
                        if let Some(positions) = view
                            .page_positions
                            .get(&(source, ordinal / storage::PAGE_SIZE))
                            && let Some((_, position)) = positions
                                .iter()
                                .find(|(slot, _)| *slot == ordinal % storage::PAGE_SIZE)
                            && (rows[*position].status != status
                                || rows[*position].saved_path != path)
                        {
                            changed.push((*position, status, path));
                        }
                    }
                }
                Ok(changed)
            })();
            let _ = runtime::invoke_from_event_loop(move || {
                if alive.upgrade().is_none() || revision.load(Ordering::Relaxed) != generation {
                    return;
                }
                match result {
                    Ok(changed) => {
                        if changed.is_empty() {
                            return;
                        }
                        let mut rows = view.rows.write().unwrap();
                        let mut stats = view.stats.write().unwrap();
                        let mut cache = cache.lock().unwrap();
                        for (position, status, path) in changed {
                            let row = &mut rows[position];
                            stats.remove(row);
                            row.status = status;
                            row.saved_path = path;
                            stats.add(row);
                            if let Some((page, _)) = cache
                                .pages
                                .get_mut(&(row.source, row.ordinal / storage::PAGE_SIZE))
                                && let Some(item) = page.get_mut(row.ordinal % storage::PAGE_SIZE)
                            {
                                item.status = row.status.clone();
                                item.saved_path = row.saved_path.clone();
                            }
                        }
                        revision.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(error) => crate::platform::logging::error(format!(
                        "Could not refresh saved media status: {error}"
                    )),
                }
            });
        });
    }
}

fn trim(cache: &mut Cache) {
    while cache.pages.len() > cache.pinned.len() + 8 {
        let oldest = cache
            .pages
            .iter()
            .filter(|(key, _)| !cache.pinned.contains(key))
            .min_by_key(|(_, (_, used))| used)
            .map(|(key, _)| *key);
        if let Some(oldest) = oldest {
            cache.pages.remove(&oldest);
        } else {
            break;
        }
    }
}

fn matches(row: &Descriptor, kind: &str) -> bool {
    match kind {
        "library-saved" => row.status == "Saved",
        "library-missing" => row.status == "Missing",
        "library-pending" => row.status != "Saved",
        _ => crate::media::results::matches_kind(&row.kind, kind),
    }
}

impl Model for PagedModel {
    type Data = MediaItem;
    fn row_count(&self) -> usize {
        self.view.rows.read().unwrap().len()
    }
    fn row_data(&self, row: usize) -> Option<MediaItem> {
        self.get(row, true)
    }
    fn revision(&self) -> u64 {
        self.revision.load(Ordering::Relaxed)
    }
    fn with_row(&self, row: usize, visitor: &mut dyn FnMut(&MediaItem)) {
        if let Some(row) = self.get(row, false) {
            visitor(&row);
        }
    }
    fn set_row_data(&self, row: usize, value: MediaItem) {
        let mut rows = self.view.rows.write().unwrap();
        let Some(descriptor) = rows.get_mut(row) else {
            return;
        };
        if descriptor.status != value.status || descriptor.is_new != value.is_new {
            self.view.stats.write().unwrap().remove(descriptor);
            descriptor.status = value.status.clone();
            descriptor.is_new = value.is_new;
            self.view.stats.write().unwrap().add(descriptor);
            self.revision.fetch_add(1, Ordering::Relaxed);
        }
        if let Some((page, _)) = self
            .cache
            .lock()
            .unwrap()
            .pages
            .get_mut(&(descriptor.source, descriptor.ordinal / storage::PAGE_SIZE))
            && let Some(row) = page.get_mut(descriptor.ordinal % storage::PAGE_SIZE)
        {
            *row = value;
        }
    }
}

struct Ordered {
    base: PagedModel,
    positions: Vec<usize>,
}
impl Model for Ordered {
    type Data = MediaItem;
    fn row_count(&self) -> usize {
        self.positions.len()
    }
    fn row_data(&self, row: usize) -> Option<MediaItem> {
        self.base.row_data(*self.positions.get(row)?)
    }
    fn revision(&self) -> u64 {
        self.base.revision()
    }
    fn with_row(&self, row: usize, visitor: &mut dyn FnMut(&MediaItem)) {
        if let Some(position) = self.positions.get(row) {
            self.base.with_row(*position, visitor);
        }
    }
}

pub fn position(items: &ModelRc<MediaItem>, index: i32) -> Option<i32> {
    if let Some(model) = items.as_any().downcast_ref::<PagedModel>() {
        return model.view.positions.get(&index).map(|value| *value as i32);
    }
    items.as_any().downcast_ref::<Ordered>().map(|model| {
        let rows = model.base.view.rows.read().unwrap();
        model
            .positions
            .iter()
            .position(|position| rows[*position].index == index)
            .map_or(-1, |value| value as i32)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn filtering_sorting_and_sparse_visible_rows_only_decode_requested_pages() {
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let path = fixture.0.join("paged.sqlite3");
        let connection = Connection::open(&path).unwrap();
        crate::history::db::initialize_database(&connection).unwrap();
        let media = (0..2200)
            .map(|index| MediaCandidate {
                media_index: index,
                username: "demo".into(),
                tweet_id: index.to_string(),
                media_type: if index % 65 == 0 { "video" } else { "photo" }.into(),
                url: format!("https://example.invalid/{index}.mp4"),
                status: "Ready".into(),
                ..Default::default()
            })
            .collect::<Vec<_>>();
        connection.execute("INSERT INTO history_entries(id,fetched_at,source_url,username,media_json,response_json) VALUES('1','today','https://x.com/demo','demo','[]','{}')",[]).unwrap();
        let transaction = connection.unchecked_transaction().unwrap();
        storage::write_payload(
            &transaction,
            "1",
            "demo",
            &media,
            "{\"account\":{\"username\":\"demo\"},\"tweets\":[]}",
        )
        .unwrap();
        transaction.commit().unwrap();
        let entries = crate::history::db::read_summaries(&connection, Arc::new(path)).unwrap();
        assert!(entries[0].media.is_empty());
        let view = StoredView::prepare(&entries, "1", false).unwrap().unwrap();
        let model = PagedModel::new(view);
        assert_eq!(model.count("all"), 2200);
        assert_eq!(model.count("video"), 34);
        let visible = model.ordered("video", 30, 1);
        assert_eq!(visible.row_count(), 30);
        assert_eq!(
            model.decoded_pages.load(Ordering::Relaxed),
            0,
            "Metadata filters read media payloads"
        );
        let keep = model
            .eligible("video")
            .into_iter()
            .take(30)
            .collect::<HashSet<_>>();
        model.pin(&keep);
        for position in 0..30 {
            visible.row_data(position).unwrap();
        }
        let ctx = egui::Context::default();
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            runtime::pump(&ctx);
            if (0..30).all(|position| !visible.row_data(position).unwrap().payload_pending) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "Visible sparse pages did not settle"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(model.decoded_pages.load(Ordering::Relaxed), 30);
        assert_eq!(
            visible.row_data(29).unwrap().url,
            "https://example.invalid/1885.mp4"
        );
        assert_eq!(position(&visible, 1885), Some(29));
        model.pin(&HashSet::from([1885]));
        assert!(model.cache.lock().unwrap().pages.len() <= 9);
        assert_eq!(
            model
                .view
                .media("selected", &HashSet::from([1885]))
                .unwrap()
                .len(),
            1
        );
        assert!(model.view.response(true).unwrap().contains("tweets"));
    }
}
