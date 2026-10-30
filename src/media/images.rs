//! Load remote images on demand. Decoded images live only in this process.
use crate::AppWindow;
use crate::ui::runtime::{Image, Model, Rgba8Pixel, SharedPixelBuffer, VecModel};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    io::Cursor,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Kind {
    Thumbnail,
    Preview,
    Avatar,
    SquareAvatar,
    Banner,
}

type Key = (Kind, String);
#[derive(Default)]
struct Memory {
    images: HashMap<Key, (Image, usize, u64)>,
    pending: HashSet<Key>,
    failed: HashMap<Key, Instant>,
    bytes: usize,
    clock: u64,
}
thread_local! { static MEMORY: RefCell<Memory> = RefCell::new(Memory::default()); }
const MEMORY_LIMIT: usize = 64 * 1024 * 1024;

impl Memory {
    /// Insert an image, evicting the least recently used ones except `keep`.
    fn store(&mut self, key: Key, image: Image, bytes: usize, keep: Option<&Key>) {
        while self.bytes + bytes > MEMORY_LIMIT {
            let Some(oldest) = self
                .images
                .iter()
                .filter(|(key, _)| keep != Some(*key))
                .min_by_key(|(_, (_, _, used))| *used)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            let (_, size, _) = self.images.remove(&oldest).unwrap();
            self.bytes -= size;
        }
        self.clock += 1;
        self.failed.remove(&key);
        if let Some((_, size, _)) = self.images.insert(key, (image, bytes, self.clock)) {
            self.bytes -= size;
        }
        self.bytes += bytes;
    }
}

pub fn image(kind: Kind, url: &str) -> Image {
    MEMORY.with_borrow_mut(|memory| {
        memory.clock += 1;
        let clock = memory.clock;
        memory
            .images
            .get_mut(&(kind, url.to_owned()))
            .map(|(image, _, used)| {
                *used = clock;
                image.clone()
            })
            .unwrap_or_default()
    })
}

pub fn pending(kind: Kind, url: &str) -> bool {
    MEMORY.with_borrow(|memory| memory.pending.contains(&(kind, url.to_owned())))
}

pub fn register(app: &AppWindow) {
    let ui = app.as_weak();
    app.on_load_thumbnail(move |index| {
        if let Some(app) = ui.upgrade()
            && let Some(item) = crate::media::results::find(&app.get_media_items(), index)
            && item.preview_image.size().width == 0
        {
            request(&app, Kind::Thumbnail, item.thumbnail_url.to_string());
        }
    });
    let ui = app.as_weak();
    app.on_load_media_preview(move |index| {
        if let Some(app) = ui.upgrade()
            && let Some(item) = crate::media::results::find(&app.get_media_items(), index)
        {
            request(&app, Kind::Thumbnail, item.thumbnail_url.to_string());
            request(
                &app,
                avatar_kind(item.author_avatar_square),
                item.author_avatar_url.clone(),
            );
            if item.media_type == "photo" {
                request(&app, Kind::Preview, item.preview_url.to_string());
            }
        }
    });
    let ui = app.as_weak();
    app.on_release_preview_images(move |selected| {
        if let Some(app) = ui.upgrade() {
            let items = app.get_media_items();
            if let Some(model) = items.as_any().downcast_ref::<VecModel<crate::MediaItem>>() {
                let positions = model.with_rows(|rows| {
                    rows.iter()
                        .enumerate()
                        .filter_map(|(index, item)| {
                            (item.media_index != selected
                                && (item.detail_image.size().width > 0
                                    || item.author_avatar_image.size().width > 0))
                                .then_some(index)
                        })
                        .collect::<Vec<_>>()
                });
                for index in positions {
                    let mut item = model.row_data(index).unwrap();
                    item.detail_image = Image::default();
                    item.author_avatar_image = Image::default();
                    model.set_row_data(index, item);
                }
            }
        }
    });
    let ui = app.as_weak();
    app.on_load_profile_images(move || {
        if let Some(app) = ui.upgrade() {
            let profile = app.get_account_profile();
            request(
                &app,
                avatar_kind(profile.avatar_square),
                profile.avatar_url.to_string(),
            );
            request(&app, Kind::Banner, profile.banner_url.to_string());
        }
    });
    let ui = app.as_weak();
    app.on_load_history_avatars(move || {
        if let Some(app) = ui.upgrade() {
            load_history_avatars(&app);
        }
    });
}

pub(crate) fn retain_thumbnails(app: &AppWindow, visible: &HashSet<i32>) {
    let selected = app.get_selected_media_index();
    let items = app.get_media_items();
    if let Some(model) = items
        .as_any()
        .downcast_ref::<crate::history::paged::PagedModel>()
    {
        let mut keep = visible.clone();
        keep.insert(selected);
        model.pin(&keep);
        model.update_loaded(|row| {
            if !visible.contains(&row.media_index) {
                row.preview_image = Image::default();
            }
            if row.media_index != selected {
                row.author_avatar_image = Image::default();
                row.detail_image = Image::default();
            }
        });
        return;
    }
    let Some(model) = items.as_any().downcast_ref::<VecModel<crate::MediaItem>>() else {
        return;
    };
    let positions = model.with_rows(|rows| {
        rows.iter()
            .enumerate()
            .filter_map(|(index, row)| {
                ((row.preview_image.size().width > 0 && !visible.contains(&row.media_index))
                    || (row.author_avatar_image.size().width > 0 && row.media_index != selected))
                    .then_some(index)
            })
            .collect::<Vec<_>>()
    });
    for index in positions {
        let mut row = model.row_data(index).unwrap();
        if !visible.contains(&row.media_index) {
            row.preview_image = Image::default();
        }
        if row.media_index != selected {
            row.author_avatar_image = Image::default();
        }
        model.set_row_data(index, row);
    }
}

pub fn avatar_kind(square: bool) -> Kind {
    if square {
        Kind::SquareAvatar
    } else {
        Kind::Avatar
    }
}

pub fn load_history_avatars(app: &AppWindow) {
    if app.get_selected_tab() == 1 {
        for item in app
            .get_history_items()
            .iter()
            .chain(app.get_fetch_sessions_items().iter())
        {
            request(
                app,
                avatar_kind(item.avatar_square),
                item.avatar_url.to_string(),
            );
        }
    }
    if app.get_selected_tab() == crate::library::TAB {
        for item in app.get_library().items.iter() {
            request(app, avatar_kind(item.avatar_square), item.avatar_url);
        }
    }
}

pub(crate) fn request(app: &AppWindow, kind: Kind, url: String) {
    if !reqwest::Url::parse(&url).is_ok_and(|url| {
        matches!(url.scheme(), "http" | "https")
            || (app.get_selected_tab() == crate::library::TAB
                && url.scheme() == "file"
                && matches!(kind, Kind::Thumbnail | Kind::Preview))
    }) {
        return;
    }
    let Ok(runtime) = tokio::runtime::Handle::try_current() else {
        return;
    };
    let key = (kind, url.clone());
    let ready = MEMORY.with_borrow(|memory| memory.images.contains_key(&key));
    if ready {
        apply(app, kind, &url, image(kind, &url));
        return;
    }
    if !MEMORY.with_borrow_mut(|memory| {
        if memory
            .failed
            .get(&key)
            .is_some_and(|failed| failed.elapsed() < Duration::from_secs(30))
        {
            return false;
        }
        memory.pending.insert(key.clone())
    }) {
        return;
    }
    let ui = app.as_weak();
    runtime.spawn(async move {
        let decoded = load(&url, kind).await;
        let _ = ui.upgrade_in_event_loop(move |app| {
            MEMORY.with_borrow_mut(|memory| {
                memory.pending.remove(&key);
            });
            if let Some(decoded) = decoded {
                let bytes = decoded.as_raw().len();
                let pixels = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                    decoded.as_raw(),
                    decoded.width(),
                    decoded.height(),
                );
                let loaded = Image::from_rgba8(pixels);
                // The open preview is never read back through image(), so it would
                // look oldest; evicting it only loses the copy that model rebuilds use.
                let shown = crate::media::results::find(
                    &app.get_media_items(),
                    app.get_selected_media_index(),
                )
                .map(|item| (Kind::Preview, item.preview_url));
                MEMORY.with_borrow_mut(|memory| {
                    memory.store(key, loaded.clone(), bytes, shown.as_ref());
                });
                apply(&app, kind, &url, loaded);
            } else {
                MEMORY.with_borrow_mut(|memory| {
                    memory.failed.insert(key, Instant::now());
                });
            }
        });
    });
}

fn apply(app: &AppWindow, kind: Kind, url: &str, loaded: Image) {
    if kind == Kind::Avatar
        && let Some(model) = app
            .get_account_search_items()
            .as_any()
            .downcast_ref::<VecModel<crate::AccountSearchItem>>()
    {
        for index in 0..model.row_count() {
            let mut row = model.row_data(index).unwrap();
            if row.avatar_url == url {
                row.avatar = loaded.clone();
                model.set_row_data(index, row);
            }
        }
    }
    // Update current rows by URL, preserving download status and rejecting stale selections.
    let items = app.get_media_items();
    if let Some(model) = items
        .as_any()
        .downcast_ref::<crate::history::paged::PagedModel>()
    {
        model.update_loaded(|row| {
            if kind == Kind::Thumbnail && row.thumbnail_url == url {
                row.preview_image = loaded.clone();
            } else if kind == Kind::Preview
                && row.preview_url == url
                && row.media_index == app.get_selected_media_index()
            {
                row.detail_image = loaded.clone();
            } else if avatar_kind(row.author_avatar_square) == kind
                && row.author_avatar_url == url
                && row.media_index == app.get_selected_media_index()
            {
                row.author_avatar_image = loaded.clone();
            }
        });
    }
    if let Some(model) = items.as_any().downcast_ref::<VecModel<crate::MediaItem>>() {
        let positions = model.with_rows(|rows| {
            rows.iter()
                .enumerate()
                .filter_map(|(index, item)| {
                    let matches = if kind == Kind::Thumbnail {
                        item.thumbnail_url == url && item.preview_image.size().width == 0
                    } else if avatar_kind(item.author_avatar_square) == kind {
                        item.media_index == app.get_selected_media_index()
                            && item.author_avatar_url == url
                            && item.author_avatar_image.size().width == 0
                    } else {
                        kind == Kind::Preview
                            && item.media_type == "photo"
                            && item.media_index == app.get_selected_media_index()
                            && item.preview_url == url
                            && item.detail_image.size().width == 0
                    };
                    matches.then_some(index)
                })
                .collect::<Vec<_>>()
        });
        for index in positions {
            let mut item = model.row_data(index).unwrap();
            if kind == Kind::Thumbnail {
                item.preview_image = loaded.clone();
            } else if matches!(kind, Kind::Avatar | Kind::SquareAvatar) {
                item.author_avatar_image = loaded.clone();
            } else {
                item.detail_image = loaded.clone();
            }
            model.set_row_data(index, item);
        }
    }
    if matches!(kind, Kind::Avatar | Kind::SquareAvatar | Kind::Banner) {
        let mut profile = app.get_account_profile();
        if kind == Kind::Banner && profile.banner_url == url && !profile.has_banner {
            profile.banner_image = loaded.clone();
            profile.has_banner = true;
            app.set_account_profile(profile);
        } else if avatar_kind(profile.avatar_square) == kind
            && profile.avatar_url == url
            && !profile.has_avatar
        {
            profile.avatar_image = loaded.clone();
            profile.has_avatar = true;
            app.set_account_profile(profile);
        }
        for history in [
            app.get_history_items(),
            app.get_fetch_sessions_items(),
            app.get_library().items,
        ] {
            if let Some(model) = history
                .as_any()
                .downcast_ref::<VecModel<crate::HistoryItem>>()
            {
                for index in 0..model.row_count() {
                    let mut item = model.row_data(index).unwrap();
                    if avatar_kind(item.avatar_square) == kind
                        && item.avatar_url == url
                        && !item.has_avatar
                    {
                        item.avatar_image = loaded.clone();
                        item.has_avatar = true;
                        model.set_row_data(index, item);
                    }
                }
            }
        }
        if let Some(model) = app
            .get_batch_fetch_items()
            .as_any()
            .downcast_ref::<VecModel<crate::BatchFetchItem>>()
        {
            for index in 0..model.row_count() {
                let mut item = model.row_data(index).unwrap();
                if avatar_kind(item.avatar_square) == kind
                    && item.avatar_url == url
                    && !item.has_avatar
                {
                    item.avatar_image = loaded.clone();
                    item.has_avatar = true;
                    model.set_row_data(index, item);
                }
            }
        }
    }
}

async fn load(url: &str, kind: Kind) -> Option<image::RgbaImage> {
    // Previews have their own slots so they never queue behind a grid of thumbnails.
    static SLOTS: std::sync::OnceLock<tokio::sync::Semaphore> = std::sync::OnceLock::new();
    static PREVIEW_SLOTS: std::sync::OnceLock<tokio::sync::Semaphore> = std::sync::OnceLock::new();
    let _slot = if kind == Kind::Preview {
        PREVIEW_SLOTS.get_or_init(|| tokio::sync::Semaphore::new(2))
    } else {
        SLOTS.get_or_init(|| tokio::sync::Semaphore::new(8))
    }
    .acquire()
    .await
    .ok()?;
    if let Some(path) = reqwest::Url::parse(url)
        .ok()
        .filter(|url| url.scheme() == "file")
        .and_then(|url| url.to_file_path().ok())
    {
        if tokio::fs::metadata(&path).await.ok()?.len() > 40_000_000 {
            return None;
        }
        return decode(tokio::fs::read(path).await.ok()?, kind).await;
    }
    static CLIENT: std::sync::Mutex<Option<(u64, reqwest::Client)>> = std::sync::Mutex::new(None);
    let client = {
        let mut cached = CLIENT.lock().unwrap();
        let generation = crate::network::proxy::generation();
        if cached
            .as_ref()
            .is_none_or(|(version, _)| *version != generation)
        {
            *cached = Some((
                generation,
                crate::network::proxy::builder()
                    .user_agent("Mozilla/5.0 TwitterXMediaBatchDownloader/1.0")
                    .connect_timeout(crate::network::timeouts::connect())
                    .build()
                    .ok()?,
            ));
        }
        cached.as_ref()?.1.clone()
    };
    let mut response = client
        .get(url)
        .timeout(crate::network::timeouts::http())
        .header(reqwest::header::REFERER, "https://x.com/")
        .send()
        .await
        .ok()?;
    let limit = if kind == Kind::Preview {
        20_000_000
    } else {
        5_000_000
    };
    if !response.status().is_success() || response.content_length().is_some_and(|size| size > limit)
    {
        return None;
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.ok()? {
        if bytes.len() + chunk.len() > limit as usize {
            return None;
        }
        bytes.extend_from_slice(&chunk);
    }
    decode(bytes, kind).await
}

async fn decode(bytes: Vec<u8>, kind: Kind) -> Option<image::RgbaImage> {
    tokio::task::spawn_blocking(move || {
        let mut reader = image::ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .ok()?;
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(96 * 1024 * 1024);
        reader.limits(limits);
        let decoded = reader.decode().ok()?;
        let resized = match kind {
            Kind::Thumbnail => {
                let bound = 512;
                if decoded.width() > bound || decoded.height() > bound {
                    decoded.thumbnail(bound, bound)
                } else {
                    decoded
                }
            }
            Kind::Preview => decoded,
            Kind::Banner => {
                decoded.resize_to_fill(1280, 300, image::imageops::FilterType::Lanczos3)
            }
            _ => decoded.resize_to_fill(256, 256, image::imageops::FilterType::Lanczos3),
        };
        let mut pixels = resized.to_rgba8();
        if kind == Kind::Avatar {
            let center = 127.5_f32;
            for (x, y, pixel) in pixels.enumerate_pixels_mut() {
                if (x as f32 - center).powi(2) + (y as f32 - center).powi(2) > center * center {
                    pixel.0[3] = 0;
                }
            }
        }
        Some(pixels)
    })
    .await
    .ok()?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn previews_preserve_original_pixels_while_grid_images_are_reduced() {
        let original = image::RgbaImage::from_fn(3072, 32, |x, y| {
            image::Rgba([(x % 256) as u8, (y * 7) as u8, 137, 255])
        });
        let mut encoded = Cursor::new(Vec::new());
        original
            .write_to(&mut encoded, image::ImageFormat::Png)
            .unwrap();
        let bytes = encoded.into_inner();
        let preview = decode(bytes.clone(), Kind::Preview).await.unwrap();
        assert_eq!(preview, original);
        let thumbnail = decode(bytes, Kind::Thumbnail).await.unwrap();
        assert_eq!(thumbnail.width(), 512);
        assert!(thumbnail.height() < original.height());
    }

    #[test]
    fn eviction_keeps_the_shown_preview_while_newer_thumbnails_arrive() {
        let mut memory = Memory::default();
        let shown = (Kind::Preview, "https://example.test/orig.jpg".to_owned());
        let half = MEMORY_LIMIT / 2;
        memory.store(shown.clone(), Image::default(), half, Some(&shown));
        for index in 0..8 {
            let key = (Kind::Thumbnail, format!("https://example.test/{index}.jpg"));
            memory.store(key, Image::default(), half / 3, Some(&shown));
        }
        assert!(memory.images.contains_key(&shown));
        assert!(memory.bytes <= MEMORY_LIMIT);
        memory.store(
            (Kind::Thumbnail, "https://example.test/next.jpg".into()),
            Image::default(),
            half,
            None,
        );
        assert!(!memory.images.contains_key(&shown));
    }

    #[test]
    fn original_preview_updates_only_the_selected_photo_and_keeps_its_thumbnail() {
        let app = AppWindow::new().unwrap();
        let thumbnail = Image::from_rgba8(SharedPixelBuffer::new(2, 2));
        let original = Image::from_rgba8(SharedPixelBuffer::new(8, 4));
        let url = "https://example.test/original.png";
        app.set_media_items(crate::ui::runtime::ModelRc::new(VecModel::from(
            [42, 84]
                .map(|index| crate::MediaItem {
                    media_index: index,
                    media_type: "photo".into(),
                    preview_url: url.into(),
                    preview_image: thumbnail.clone(),
                    ..Default::default()
                })
                .to_vec(),
        )));
        app.set_selected_media_index(42);
        apply(&app, Kind::Preview, url, original.clone());
        let selected = app.get_media_items().row_data(0).unwrap();
        assert_eq!(selected.detail_image.id, original.id);
        assert_eq!(selected.preview_image.id, thumbnail.id);
        assert_eq!(
            app.get_media_items()
                .row_data(1)
                .unwrap()
                .detail_image
                .size()
                .width,
            0
        );
    }

    #[tokio::test]
    async fn library_images_decode_from_local_files_without_a_remote_request() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/logo/logo.png");
        let dimensions = image::image_dimensions(&path).unwrap();
        let url = reqwest::Url::from_file_path(path).unwrap();
        let thumbnail = load(url.as_str(), Kind::Thumbnail).await.unwrap();
        assert!(thumbnail.width() > 0 && thumbnail.width() <= 512);
        let preview = load(url.as_str(), Kind::Preview).await.unwrap();
        assert_eq!(preview.dimensions(), dimensions);
    }
    #[test]
    fn preview_requests_the_selected_authors_avatar_and_releases_it_on_navigation() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let _entered = runtime.enter();
        let app = AppWindow::new().unwrap();
        register(&app);
        let url = "https://example.test/preview-author.png";
        let loaded = Image::from_rgba8(SharedPixelBuffer::new(2, 2));
        MEMORY.with_borrow_mut(|memory| {
            memory
                .images
                .insert((Kind::Avatar, url.into()), (loaded.clone(), 16, 0));
            memory.bytes += 16;
        });
        app.set_media_items(crate::ui::runtime::ModelRc::new(VecModel::from(vec![
            crate::MediaItem {
                media_index: 42,
                author_avatar_url: url.into(),
                status: "Saved".into(),
                ..Default::default()
            },
            crate::MediaItem {
                media_index: 84,
                author_avatar_url: "https://example.test/different.png".into(),
                ..Default::default()
            },
        ])));
        app.set_selected_media_index(42);
        app.invoke_load_thumbnail(42);
        assert_eq!(
            app.get_media_items()
                .row_data(0)
                .unwrap()
                .author_avatar_image
                .size()
                .width,
            0,
            "Scrolling thumbnail grids must not load author avatars"
        );
        app.invoke_load_media_preview(42);
        let row = app.get_media_items().row_data(0).unwrap();
        assert_eq!(row.author_avatar_image.id, loaded.id);
        assert_eq!(row.status, "Saved");
        assert_eq!(
            app.get_media_items()
                .row_data(1)
                .unwrap()
                .author_avatar_image
                .size()
                .width,
            0
        );
        app.set_selected_media_index(84);
        assert_eq!(
            app.get_media_items()
                .row_data(0)
                .unwrap()
                .author_avatar_image
                .size()
                .width,
            0
        );
        apply(&app, Kind::Avatar, url, loaded);
        assert_eq!(
            app.get_media_items()
                .row_data(0)
                .unwrap()
                .author_avatar_image
                .size()
                .width,
            0,
            "A late response must not retain the previous author's avatar"
        );
        MEMORY.with_borrow_mut(|memory| {
            memory.images.remove(&(Kind::Avatar, url.into()));
            memory.bytes -= 16;
        });
    }
    #[test]
    fn offscreen_thumbnails_are_released_without_changing_media_metadata() {
        let app = AppWindow::new().unwrap();
        let pixels = Image::from_rgba8(SharedPixelBuffer::new(2, 2));
        app.set_media_items(crate::ui::runtime::ModelRc::new(VecModel::from(
            (0..5)
                .map(|index| crate::MediaItem {
                    media_index: index,
                    preview_image: pixels.clone(),
                    thumbnail_url: format!("https://example.invalid/{index}.jpg"),
                    status: "Saved".into(),
                    ..Default::default()
                })
                .collect::<Vec<_>>(),
        )));
        retain_thumbnails(&app, &HashSet::from([1, 4]));
        for row in app.get_media_items().iter() {
            assert_eq!(
                row.preview_image.size().width > 0,
                [1, 4].contains(&row.media_index)
            );
            assert_eq!(row.status, "Saved");
            assert_eq!(
                row.thumbnail_url,
                format!("https://example.invalid/{}.jpg", row.media_index)
            );
        }
    }
}
