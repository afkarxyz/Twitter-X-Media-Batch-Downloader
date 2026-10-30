use crate::AppWindow;
use crate::PreviewPlayback;
use crate::download::engine::{self as downloader, MediaCandidate};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};

#[cfg(windows)]
mod native;
mod proxy;

pub(super) type Frame = Arc<egui::ColorImage>;
#[derive(Default)]
pub(super) struct PlaybackState {
    playing: bool,
    loading: bool,
    position: f64,
    duration: f64,
    can_seek: bool,
}

struct Packet {
    generation: u64,
    index: i32,
    state: PlaybackState,
    frame: Option<Frame>,
    error: Option<String>,
}
#[derive(Default)]
struct Mailbox {
    packet: Mutex<Option<Packet>>,
    scheduled: AtomicBool,
}

// Only the Windows player reads the media and controls; other platforms reply
// that inline playback needs Windows.
#[cfg_attr(not(windows), allow(dead_code))]
enum Action {
    Toggle,
    Seek(f64),
    Mute(bool),
    Volume(f64),
}
#[cfg_attr(not(windows), allow(dead_code))]
enum Command {
    Start {
        generation: u64,
        index: i32,
        uri: String,
        looping: bool,
        muted: bool,
        volume: f64,
    },
    Stop,
    FrameReady(u64),
    Action(u64, Action),
    Shutdown,
}

pub(crate) struct Controller {
    sender: mpsc::Sender<Command>,
    generation: Arc<AtomicU64>,
    proxy: Mutex<Option<proxy::ProxyServer>>,
    runtime: tokio::runtime::Handle,
    worker: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl Controller {
    pub(crate) fn shutdown(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.proxy
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        let _ = self.sender.send(Command::Shutdown);
        if let Some(worker) = self
            .worker
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        {
            let _ = worker.join();
        }
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Register one player for the window. Selection changes own its lifecycle;
/// poster caching can update media_items without restarting the playback clock.
pub fn register(app: &AppWindow) -> Arc<Controller> {
    let (sender, receiver) = mpsc::channel();
    let generation = Arc::new(AtomicU64::new(0));
    let ui = app.as_weak();
    let worker_generation = generation.clone();
    let wake_sender = sender.clone();
    let worker = std::thread::Builder::new()
        .name("media-preview-player".into())
        .spawn(move || worker(receiver, ui, worker_generation, wake_sender))
        .expect("Could not start the preview player");
    let controller = Arc::new(Controller {
        sender,
        generation,
        proxy: Mutex::new(None),
        runtime: tokio::runtime::Handle::current(),
        worker: Mutex::new(Some(worker)),
    });
    let ui = app.as_weak();
    let player = controller.clone();
    app.on_preview_changed(move |index| {
        if let Some(app) = ui.upgrade() {
            select(&app, &player, index);
        }
    });
    let playback = app.global::<PreviewPlayback>();
    let player = controller.clone();
    playback.on_toggle(move || {
        let _ = player.sender.send(Command::Action(
            player.generation.load(Ordering::Acquire),
            Action::Toggle,
        ));
    });
    let player = controller.clone();
    playback.on_seek(move |seconds| {
        if seconds.is_finite() {
            let _ = player.sender.send(Command::Action(
                player.generation.load(Ordering::Acquire),
                Action::Seek(seconds as f64),
            ));
        }
    });
    let player = controller.clone();
    let ui = app.as_weak();
    playback.on_mute(move |muted| {
        if let Some(app) = ui.upgrade() {
            app.global::<PreviewPlayback>().set_muted(muted);
        }
        let _ = player.sender.send(Command::Action(
            player.generation.load(Ordering::Acquire),
            Action::Mute(muted),
        ));
    });
    let player = controller.clone();
    let ui = app.as_weak();
    playback.on_change_volume(move |volume| {
        if !volume.is_finite() {
            return;
        }
        let volume = volume.clamp(0.0, 1.0);
        if let Some(app) = ui.upgrade() {
            app.global::<PreviewPlayback>().set_volume(volume);
        }
        let _ = player.sender.send(Command::Action(
            player.generation.load(Ordering::Acquire),
            Action::Volume(volume as f64),
        ));
    });
    let player = controller.clone();
    let ui = app.as_weak();
    playback.on_retry(move || {
        if let Some(app) = ui.upgrade() {
            select(&app, &player, app.get_selected_media_index());
        }
    });
    controller
}

fn select(app: &AppWindow, controller: &Arc<Controller>, index: i32) {
    let generation = controller.generation.fetch_add(1, Ordering::AcqRel) + 1;
    controller
        .proxy
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .take();
    let _ = controller.sender.send(Command::Stop);
    let playback = app.global::<PreviewPlayback>();
    playback.set_frame(None);
    playback.set_frame_count(0);
    playback.set_playing(false);
    playback.set_loading(false);
    playback.set_ready(false);
    playback.set_position(0.0);
    playback.set_duration(0.0);
    playback.set_can_seek(false);
    playback.set_position_text("0:00".into());
    playback.set_duration_text("0:00".into());
    playback.set_error("".into());
    let Some(candidate) = displayed_candidate(app, index) else {
        return;
    };
    playback.set_loading(true);
    let muted = playback.get_muted();
    let volume = playback.get_volume() as f64;
    let ui = app.as_weak();
    let controller = controller.clone();
    controller.runtime.clone().spawn(async move {
        let result = tokio::select! {
            result = source(&candidate, generation) => result,
            _ = async { loop { if controller.generation.load(Ordering::Acquire) != generation { break; } tokio::time::sleep(Duration::from_millis(40)).await; } } => return,
        };
        match result {
            Ok((uri, proxy)) => {
                let mut active = controller
                    .proxy
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                if controller.generation.load(Ordering::Acquire) != generation {
                    return;
                }
                *active = proxy;
                let _ = controller.sender.send(Command::Start {
                    generation,
                    index,
                    uri,
                    looping: candidate.media_type == "animated_gif",
                    muted,
                    volume,
                });
            }
            Err(error) => post(
                &ui,
                &controller.generation,
                generation,
                index,
                PlaybackState::default(),
                None,
                Some(error),
                &Arc::new(Mailbox::default()),
            ),
        }
    });
}

fn displayed_candidate(app: &AppWindow, index: i32) -> Option<MediaCandidate> {
    if index < 0 {
        return None;
    }
    let row = crate::media::results::find(&app.get_media_items(), index)?;
    if !matches!(row.media_type.as_str(), "video" | "animated_gif") {
        return None;
    }
    Some(MediaCandidate {
        media_index: index as usize,
        url: if row.playback_url.is_empty() {
            row.url
        } else {
            row.playback_url
        },
        media_type: row.media_type,
        saved_path: row.saved_path,
        ..Default::default()
    })
}

async fn source(
    candidate: &MediaCandidate,
    generation: u64,
) -> Result<(String, Option<proxy::ProxyServer>), String> {
    // An existing downloaded video can be played directly, without a preview copy.
    let saved = std::path::Path::new(&candidate.saved_path);
    if !candidate.saved_path.is_empty()
        && saved.is_file()
        && !saved
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("gif"))
    {
        let uri =
            reqwest::Url::from_file_path(saved).map_err(|_| "Could not open the saved video.")?;
        return Ok((uri.to_string(), None));
    }
    // Use the best video variant, independently of the download quality setting.
    let url = downloader::selected_media_url(candidate, "highest", "orig");
    let parsed = reqwest::Url::parse(&url).map_err(|_| "This video has no playable URL.")?;
    if !matches!(parsed.scheme(), "https" | "http") {
        return Err("This video has no playable URL.".into());
    }
    let proxy = proxy::ProxyServer::start(url, generation).await?;
    Ok((proxy.uri.clone(), Some(proxy)))
}

fn clock(seconds: f64) -> String {
    let seconds = if seconds.is_finite() {
        seconds.max(0.0) as u64
    } else {
        0
    };
    if seconds >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}

fn post(
    ui: &crate::ui::runtime::Weak<AppWindow>,
    active: &Arc<AtomicU64>,
    generation: u64,
    index: i32,
    state: PlaybackState,
    frame: Option<Frame>,
    error: Option<String>,
    mailbox: &Arc<Mailbox>,
) {
    {
        let mut slot = mailbox
            .packet
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let frame = frame.or_else(|| {
            slot.as_mut()
                .filter(|old| old.generation == generation)
                .and_then(|old| old.frame.take())
        });
        *slot = Some(Packet {
            generation,
            index,
            state,
            frame,
            error,
        });
    }
    if mailbox.scheduled.swap(true, Ordering::AcqRel) {
        return;
    }
    let active = active.clone();
    let queued = mailbox.clone();
    if ui
        .upgrade_in_event_loop(move |app| {
            let packet = {
                let mut slot = queued
                    .packet
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                let value = slot.take();
                queued.scheduled.store(false, Ordering::Release);
                value
            };
            let Some(Packet {
                generation,
                index,
                state,
                frame,
                error,
            }) = packet
            else {
                return;
            };
            if active.load(Ordering::Acquire) != generation
                || app.get_selected_media_index() != index
                || app.get_selected_tab() != 0
            {
                return;
            }
            let playback = app.global::<PreviewPlayback>();
            if let Some(frame) = frame {
                playback.set_frame(Some(frame));
                playback.set_frame_count(playback.get_frame_count().saturating_add(1));
                playback.set_ready(true);
            }
            playback.set_loading(state.loading);
            playback.set_playing(state.playing);
            playback.set_position(state.position as f32);
            playback.set_duration(state.duration as f32);
            playback.set_can_seek(state.can_seek);
            playback.set_position_text(clock(state.position));
            playback.set_duration_text(clock(state.duration));
            if let Some(error) = error {
                playback.set_error(error);
                playback.set_loading(false);
                playback.set_playing(false);
            }
        })
        .is_err()
    {
        mailbox.scheduled.store(false, Ordering::Release);
    }
}

#[cfg(windows)]
fn worker(
    receiver: mpsc::Receiver<Command>,
    ui: crate::ui::runtime::Weak<AppWindow>,
    generation: Arc<AtomicU64>,
    wake_sender: mpsc::Sender<Command>,
) {
    #[cfg(test)]
    native::keep_test_apartment();
    use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
    let initialization = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
    let pending = Arc::new(Mailbox::default());
    let mut player: Option<native::NativePlayer> = None;
    let (mut current, mut index) = (0, -1);
    loop {
        match receiver.recv_timeout(if player.is_some() {
            Duration::from_millis(100)
        } else {
            Duration::from_millis(250)
        }) {
            Ok(Command::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Ok(Command::Stop) => {
                player = None;
                continue;
            }
            Ok(Command::Start {
                generation: requested,
                index: requested_index,
                uri,
                looping,
                muted,
                volume,
            }) => {
                if generation.load(Ordering::Acquire) != requested {
                    continue;
                }
                player = None;
                current = requested;
                index = requested_index;
                let result = match &initialization {
                    Ok(()) => {
                        let sender = wake_sender.clone();
                        let wake = Arc::new(move || {
                            let _ = sender.send(Command::FrameReady(requested));
                        });
                        native::NativePlayer::with_frame_signal(&uri, looping, muted, volume, wake)
                    }
                    Err(error) => Err(error.to_string()),
                };
                match result {
                    Ok(value) => player = Some(value),
                    Err(error) => post(
                        &ui,
                        &generation,
                        current,
                        index,
                        PlaybackState::default(),
                        None,
                        Some(error),
                        &pending,
                    ),
                }
            }
            Ok(Command::Action(requested, action))
                if requested == current && requested == generation.load(Ordering::Acquire) =>
            {
                if let Some(player) = &player {
                    let result = match action {
                        Action::Toggle => player.toggle(),
                        Action::Seek(seconds) => player.seek(seconds),
                        Action::Mute(value) => player.mute(value),
                        Action::Volume(value) => player.volume(value),
                    };
                    if let Err(error) = result {
                        post(
                            &ui,
                            &generation,
                            current,
                            index,
                            PlaybackState::default(),
                            None,
                            Some(error),
                            &pending,
                        );
                    }
                }
            }
            Ok(Command::FrameReady(requested)) if requested != current => continue,
            _ => {}
        }
        if current != generation.load(Ordering::Acquire) {
            player = None;
            continue;
        }
        if let Some(active) = &mut player {
            match active
                .state()
                .and_then(|state| active.frame().map(|frame| (state, frame)))
            {
                Ok((state, frame)) => post(
                    &ui,
                    &generation,
                    current,
                    index,
                    state,
                    frame,
                    None,
                    &pending,
                ),
                Err(error) => {
                    post(
                        &ui,
                        &generation,
                        current,
                        index,
                        PlaybackState::default(),
                        None,
                        Some(error),
                        &pending,
                    );
                    player = None;
                }
            }
        }
    }
    drop(player);
    if initialization.is_ok() {
        unsafe {
            RoUninitialize();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_fetch_previews_use_the_displayed_video_or_gif_and_highest_variant() {
        for kind in ["video", "animated_gif"] {
            let app = AppWindow::new().unwrap();
            app.set_is_busy(true);
            app.set_is_extracting(true);
            app.set_media_items(downloader::to_ui_model(&[MediaCandidate {
                media_index: 42,
                media_type: kind.into(),
                url: "https://example.invalid/low.mp4".into(),
                variants: vec![downloader::VideoVariant {
                    url: "https://example.invalid/high.mp4".into(),
                    bitrate: 2000,
                    ..Default::default()
                }],
                ..Default::default()
            }]));
            let item = displayed_candidate(&app, 42).unwrap();
            assert_eq!(item.url, "https://example.invalid/high.mp4");
            assert_eq!(item.media_type, kind);
            assert!(displayed_candidate(&app, 0).is_none());
            assert!(displayed_candidate(&app, -1).is_none());
            app.set_media_items(downloader::to_ui_model(&[MediaCandidate {
                media_index: 42,
                media_type: "photo".into(),
                ..Default::default()
            }]));
            assert!(displayed_candidate(&app, 42).is_none());
        }
    }

    #[cfg(windows)]
    #[tokio::test(flavor = "current_thread")]
    async fn displayed_video_starts_and_keeps_playing_as_fetch_results_grow() {
        let app = AppWindow::new().unwrap();
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/preview.mp4");
        let candidate = MediaCandidate {
            media_index: 42,
            media_type: "video".into(),
            saved_path: path.to_string_lossy().into_owned(),
            ..Default::default()
        };
        app.set_media_items(downloader::to_ui_model(std::slice::from_ref(&candidate)));
        app.set_is_busy(true);
        app.set_is_extracting(true);
        let player = register(&app);
        app.set_selected_media_index(42);
        let ctx = egui::Context::default();
        let deadline = std::time::Instant::now() + Duration::from_secs(12);
        while app.global::<PreviewPlayback>().get_frame_count() < 3 {
            crate::ui::runtime::pump(&ctx);
            assert!(
                std::time::Instant::now() < deadline,
                "Playback while fetching failed: {}",
                app.global::<PreviewPlayback>().get_error()
            );
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
        let generation = player.generation.load(Ordering::Acquire);
        let frames = app.global::<PreviewPlayback>().get_frame_count();
        app.set_media_items(downloader::to_ui_model(&[
            candidate,
            MediaCandidate {
                media_index: 43,
                media_type: "photo".into(),
                ..Default::default()
            },
        ]));
        while app.global::<PreviewPlayback>().get_frame_count() <= frames + 2 {
            crate::ui::runtime::pump(&ctx);
            assert!(
                std::time::Instant::now() < deadline,
                "New fetch results stopped playback"
            );
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
        assert_eq!(player.generation.load(Ordering::Acquire), generation);
        assert!(app.get_is_extracting());
        app.set_selected_media_index(-1);
        player.shutdown();
    }

    #[test]
    fn delivery_coalesces_to_the_latest_shared_frame_and_rejects_old_selections() {
        use crate::ui::runtime;
        let app = AppWindow::new().unwrap();
        app.set_selected_media_index(42);
        let playback = app.global::<PreviewPlayback>();
        let active = Arc::new(AtomicU64::new(1));
        let pending = Arc::new(Mailbox::default());
        let ctx = egui::Context::default();
        let mut latest = Arc::new(egui::ColorImage::filled([4, 3], egui::Color32::BLACK));
        for index in 0..120 {
            latest = Arc::new(egui::ColorImage::filled(
                [4, 3],
                egui::Color32::from_rgb(index, 44, 99),
            ));
            post(
                &app.as_weak(),
                &active,
                1,
                42,
                PlaybackState::default(),
                Some(latest.clone()),
                None,
                &pending,
            );
        }
        // A status-only update must preserve the most recent pending frame.
        post(
            &app.as_weak(),
            &active,
            1,
            42,
            PlaybackState::default(),
            None,
            None,
            &pending,
        );
        runtime::pump(&ctx);
        assert!(Arc::ptr_eq(&playback.get_frame().unwrap(), &latest));
        assert_eq!(
            playback.get_frame_count(),
            1,
            "A slow UI must skip obsolete frames instead of building a queue"
        );
        assert!(!pending.scheduled.load(Ordering::Acquire));
        assert!(pending.packet.lock().unwrap().is_none());

        post(
            &app.as_weak(),
            &active,
            1,
            42,
            PlaybackState::default(),
            Some(latest.clone()),
            None,
            &pending,
        );
        active.store(2, Ordering::Release);
        runtime::pump(&ctx);
        assert_eq!(playback.get_frame_count(), 1);
        app.set_selected_media_index(84);
        let next = Arc::new(egui::ColorImage::filled([3, 4], egui::Color32::WHITE));
        post(
            &app.as_weak(),
            &active,
            2,
            84,
            PlaybackState::default(),
            Some(next.clone()),
            None,
            &pending,
        );
        runtime::pump(&ctx);
        assert!(Arc::ptr_eq(&playback.get_frame().unwrap(), &next));
        assert_eq!(playback.get_frame_count(), 2);
    }
}

#[cfg(not(windows))]
fn worker(
    receiver: mpsc::Receiver<Command>,
    ui: crate::ui::runtime::Weak<AppWindow>,
    generation: Arc<AtomicU64>,
    _wake_sender: mpsc::Sender<Command>,
) {
    for command in receiver {
        match command {
            Command::Shutdown => break,
            Command::Start {
                generation: current,
                index,
                ..
            } => post(
                &ui,
                &generation,
                current,
                index,
                PlaybackState::default(),
                None,
                Some("Inline video playback requires Windows.".into()),
                &Arc::new(Mailbox::default()),
            ),
            _ => {}
        }
    }
}
