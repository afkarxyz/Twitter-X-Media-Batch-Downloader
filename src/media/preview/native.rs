//! Windows MediaPlayer owns the playback clock, audio, seeking, and looping.
//! Its frame-server output is uploaded into egui textures on the UI thread.
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use windows::{
    Foundation::{TimeSpan, TypedEventHandler, Uri},
    Graphics::DirectX::Direct3D11::IDirect3DSurface,
    Media::{
        Core::MediaSource,
        Playback::{
            MediaPlaybackSession, MediaPlaybackState, MediaPlayer, MediaPlayerFailedEventArgs,
        },
    },
    Win32::{
        Foundation::HMODULE,
        Graphics::{
            Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP},
            Direct3D10::ID3D10Multithread,
            Direct3D11::*,
            Dxgi::{
                Common::{
                    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC,
                },
                IDXGISurface,
            },
        },
        System::WinRT::Direct3D11::CreateDirect3D11SurfaceFromDXGISurface,
    },
    core::{HSTRING, Interface},
};

use super::{Frame, PlaybackState};

#[cfg(test)]
pub(super) fn keep_test_apartment() {
    // WinRT caches activation factories process-wide. Keep the shared MTA
    // alive between independently scheduled Rust tests instead of tearing it
    // down while another test is about to activate a MediaPlayer.
    static START: std::sync::Once = std::sync::Once::new();
    START.call_once(|| {
        let (ready, started) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
            let result =
                unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.map_err(|error| error.to_string());
            ready.send(result).unwrap();
            loop {
                std::thread::park();
            }
        });
        started.recv().unwrap().unwrap();
    });
}

pub(super) struct NativePlayer {
    player: MediaPlayer,
    session: MediaPlaybackSession,
    surface: Option<FrameSurface>,
    frame_available: Arc<AtomicBool>,
    failed: Arc<Mutex<Option<String>>>,
    frame_token: i64,
    failure_token: i64,
}

impl NativePlayer {
    #[cfg(test)]
    pub(super) fn new(uri: &str, looping: bool, muted: bool, volume: f64) -> Result<Self, String> {
        Self::create(uri, looping, muted, volume, None).map_err(|error| error.to_string())
    }

    pub(super) fn with_frame_signal(
        uri: &str,
        looping: bool,
        muted: bool,
        volume: f64,
        signal: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self, String> {
        Self::create(uri, looping, muted, volume, Some(signal)).map_err(|error| error.to_string())
    }

    fn create(
        uri: &str,
        looping: bool,
        muted: bool,
        volume: f64,
        signal: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> windows::core::Result<Self> {
        let player = MediaPlayer::new()?;
        player.CommandManager()?.SetIsEnabled(false)?;
        player.SetIsVideoFrameServerEnabled(true)?;
        player.SetIsLoopingEnabled(looping)?;
        player.SetIsMuted(muted || looping)?;
        player.SetVolume(volume.clamp(0.0, 1.0))?;
        let frame_available = Arc::new(AtomicBool::new(false));
        let available = frame_available.clone();
        let wake = signal.clone();
        let frame_token = player.VideoFrameAvailable(&TypedEventHandler::new(move |_, _| {
            if !available.swap(true, Ordering::AcqRel)
                && let Some(wake) = &wake
            {
                wake();
            }
            Ok(())
        }))?;
        let failed = Arc::new(Mutex::new(None));
        let failure = failed.clone();
        let failure_token = player.MediaFailed(&TypedEventHandler::<
            MediaPlayer,
            MediaPlayerFailedEventArgs,
        >::new(move |_, args| {
            let message = args
                .as_ref()
                .and_then(|args| args.ErrorMessage().ok())
                .map(|text| text.to_string())
                .filter(|text| !text.is_empty())
                .unwrap_or_else(|| "Windows could not open this video.".into());
            *failure.lock().unwrap_or_else(|error| error.into_inner()) = Some(message);
            if let Some(wake) = &signal {
                wake();
            }
            Ok(())
        }))?;
        let session = player.PlaybackSession()?;
        let result = Self {
            player,
            session,
            surface: None,
            frame_available,
            failed,
            frame_token,
            failure_token,
        };
        let source = MediaSource::CreateFromUri(&Uri::CreateUri(&HSTRING::from(uri))?)?;
        result.player.SetSource(&source)?;
        result.player.Play()?;
        Ok(result)
    }

    pub(super) fn state(&self) -> Result<PlaybackState, String> {
        if let Some(error) = self
            .failed
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
        {
            return Err(error);
        }
        let state = self
            .session
            .PlaybackState()
            .map_err(|error| error.to_string())?;
        Ok(PlaybackState {
            playing: state == MediaPlaybackState::Playing,
            loading: matches!(
                state,
                MediaPlaybackState::None
                    | MediaPlaybackState::Opening
                    | MediaPlaybackState::Buffering
            ),
            position: self
                .session
                .Position()
                .map(|time| time.Duration as f64 / 10_000_000.0)
                .unwrap_or(0.0),
            duration: self
                .session
                .NaturalDuration()
                .map(|time| time.Duration as f64 / 10_000_000.0)
                .unwrap_or(0.0),
            can_seek: self.session.CanSeek().unwrap_or(false),
        })
    }

    pub(super) fn toggle(&self) -> Result<(), String> {
        if self.state()?.playing {
            self.player.Pause().map_err(|error| error.to_string())
        } else {
            let state = self.state()?;
            if state.duration > 0.0 && state.position >= state.duration - 0.05 {
                self.seek(0.0)?;
            }
            self.player.Play().map_err(|error| error.to_string())
        }
    }

    pub(super) fn seek(&self, seconds: f64) -> Result<(), String> {
        let state = self.state()?;
        if state.can_seek && seconds.is_finite() {
            self.session
                .SetPosition(TimeSpan {
                    Duration: (seconds.clamp(0.0, state.duration) * 10_000_000.0) as i64,
                })
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    pub(super) fn mute(&self, muted: bool) -> Result<(), String> {
        self.player
            .SetIsMuted(muted)
            .map_err(|error| error.to_string())
    }

    pub(super) fn volume(&self, volume: f64) -> Result<(), String> {
        self.player
            .SetVolume(volume.clamp(0.0, 1.0))
            .map_err(|error| error.to_string())
    }

    pub(super) fn frame(&mut self) -> Result<Option<Frame>, String> {
        if !self.frame_available.swap(false, Ordering::AcqRel) {
            return Ok(None);
        }
        let width = self
            .session
            .NaturalVideoWidth()
            .map_err(|error| error.to_string())?;
        let height = self
            .session
            .NaturalVideoHeight()
            .map_err(|error| error.to_string())?;
        if width == 0 || height == 0 {
            return Ok(None);
        }
        let scale = (960.0 / width as f64).min(540.0 / height as f64).min(1.0);
        let size = (
            (width as f64 * scale).round().max(1.0) as u32,
            (height as f64 * scale).round().max(1.0) as u32,
        );
        if self
            .surface
            .as_ref()
            .is_none_or(|surface| surface.size != size)
        {
            self.surface = Some(FrameSurface::new(size, true).map_err(|error| error.to_string())?);
        }
        match self.surface.as_ref().unwrap().copy(&self.player) {
            Ok(frame) => Ok(Some(frame)),
            Err(_) if self.surface.as_ref().unwrap().rgba => {
                self.surface =
                    Some(FrameSurface::new(size, false).map_err(|error| error.to_string())?);
                self.surface
                    .as_ref()
                    .unwrap()
                    .copy(&self.player)
                    .map(Some)
                    .map_err(|error| error.to_string())
            }
            Err(error) => Err(error.to_string()),
        }
    }
}

impl Drop for NativePlayer {
    fn drop(&mut self) {
        let _ = self.player.RemoveVideoFrameAvailable(self.frame_token);
        let _ = self.player.RemoveMediaFailed(self.failure_token);
        let _ = self.player.Pause();
        let _ = self.player.Close();
    }
}

struct FrameSurface {
    rgba: bool,
    size: (u32, u32),
    context: ID3D11DeviceContext,
    texture: ID3D11Texture2D,
    staging: ID3D11Texture2D,
    surface: IDirect3DSurface,
}

impl FrameSurface {
    fn new(size: (u32, u32), rgba: bool) -> windows::core::Result<Self> {
        let mut device = None;
        let mut context = None;
        let flags = D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT;
        // SAFETY: out parameters are valid local Options, with no adapter pointer.
        unsafe {
            if D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                flags,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
            .is_err()
            {
                D3D11CreateDevice(
                    None,
                    D3D_DRIVER_TYPE_WARP,
                    HMODULE::default(),
                    flags,
                    None,
                    D3D11_SDK_VERSION,
                    Some(&mut device),
                    None,
                    Some(&mut context),
                )?;
            }
        }
        let device = device.unwrap();
        let context = context.unwrap();
        // MediaPlayer can access the destination device from its decode threads.
        if let Ok(multithread) = context.cast::<ID3D10Multithread>() {
            unsafe {
                let _ = multithread.SetMultithreadProtected(true);
            }
        }
        let mut desc = D3D11_TEXTURE2D_DESC {
            Width: size.0,
            Height: size.1,
            MipLevels: 1,
            ArraySize: 1,
            Format: if rgba {
                DXGI_FORMAT_R8G8B8A8_UNORM
            } else {
                DXGI_FORMAT_B8G8R8A8_UNORM
            },
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE).0 as u32,
            ..Default::default()
        };
        let mut texture = None;
        unsafe {
            device.CreateTexture2D(&desc, None, Some(&mut texture))?;
        }
        let texture = texture.unwrap();
        let surface =
            unsafe { CreateDirect3D11SurfaceFromDXGISurface(&texture.cast::<IDXGISurface>()?)? }
                .cast::<IDirect3DSurface>()?;
        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        let mut staging = None;
        unsafe {
            device.CreateTexture2D(&desc, None, Some(&mut staging))?;
        }
        Ok(Self {
            rgba,
            size,
            context,
            texture,
            staging: staging.unwrap(),
            surface,
        })
    }

    fn copy(&self, player: &MediaPlayer) -> windows::core::Result<Frame> {
        player.CopyFrameToVideoSurface(&self.surface)?;
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: the staging texture is CPU-readable. Each row respects RowPitch
        // and only reads the declared width; the resource is unmapped afterwards.
        unsafe {
            self.context.CopyResource(&self.staging, &self.texture);
            self.context
                .Map(&self.staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
        }
        let mut image = egui::ColorImage::filled(
            [self.size.0 as usize, self.size.1 as usize],
            egui::Color32::TRANSPARENT,
        );
        for y in 0..self.size.1 as usize {
            let row = unsafe {
                std::slice::from_raw_parts(
                    (mapped.pData as *const u8).add(y * mapped.RowPitch as usize),
                    self.size.0 as usize * 4,
                )
            };
            let target =
                &mut image.pixels[y * self.size.0 as usize..(y + 1) * self.size.0 as usize];
            if self.rgba {
                // MediaPlayer's opaque RGBA frames already match Color32. Copy
                // directly into the upload buffer while respecting GPU row pitch.
                bytemuck::cast_slice_mut(target).copy_from_slice(row);
            } else {
                for (target, bgra) in target.iter_mut().zip(row.chunks_exact(4)) {
                    *target = egui::Color32::from_rgb(bgra[2], bgra[1], bgra[0]);
                }
            }
        }
        unsafe {
            self.context.Unmap(&self.staging, 0);
        }
        Ok(Arc::new(image))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};

    #[test]
    #[ignore = "Measures native frame delivery against the local 60 FPS fixture"]
    fn measure_native_frame_cadence() {
        keep_test_apartment();
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.unwrap();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/preview-60fps.mp4");
        let uri = reqwest::Url::from_file_path(path).unwrap().to_string();
        let (sender, receiver) = std::sync::mpsc::channel();
        let signal = Arc::new(move || {
            let _ = sender.send(());
        });
        let mut player = NativePlayer::with_frame_signal(&uri, true, true, 0.0, signal).unwrap();
        wait_until(|| player.frame().unwrap().is_some());
        let start = Instant::now();
        let mut frames = 0;
        let mut copy_time = Duration::ZERO;
        let mut gaps = Vec::new();
        let mut previous = start;
        while start.elapsed() < Duration::from_secs(4) {
            receiver.recv_timeout(Duration::from_secs(1)).unwrap();
            let copy_start = Instant::now();
            if player.frame().unwrap().is_some() {
                copy_time += copy_start.elapsed();
                let now = Instant::now();
                gaps.push(now.duration_since(previous));
                previous = now;
                frames += 1;
            }
        }
        gaps.sort();
        eprintln!(
            "native: {:.1} FPS, copy mean {:.2} ms, gap p95 {:.2} ms, max {:.2} ms",
            frames as f64 / start.elapsed().as_secs_f64(),
            copy_time.as_secs_f64() * 1000.0 / frames as f64,
            gaps[gaps.len() * 95 / 100].as_secs_f64() * 1000.0,
            gaps.last().unwrap().as_secs_f64() * 1000.0
        );
        assert!(frames > 0);
        drop(player);
        unsafe {
            RoUninitialize();
        }
    }

    fn wait_until(mut condition: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !condition() {
            assert!(
                Instant::now() < deadline,
                "Native playback did not reach the expected state"
            );
            std::thread::sleep(Duration::from_millis(15));
        }
    }

    #[test]
    fn native_frames_move_pause_seek_loop_and_close() {
        keep_test_apartment();
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.unwrap();
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/preview.mp4");
        let uri = reqwest::Url::from_file_path(path).unwrap().to_string();
        let mut player = NativePlayer::new(&uri, true, true, 0.0).unwrap();
        wait_until(|| player.state().unwrap().duration > 1.9);
        let mut first = None;
        wait_until(|| {
            first = player.frame().unwrap();
            first.is_some()
        });
        let first = first.unwrap();
        assert_eq!(first.size, [160, 96]);
        assert!(first.pixels.iter().all(|pixel| pixel.a() == 255));
        assert!(
            first
                .pixels
                .iter()
                .any(|pixel| pixel.r() > 40 || pixel.g() > 40 || pixel.b() > 40)
        );
        std::thread::sleep(Duration::from_millis(250));
        let mut second = None;
        wait_until(|| {
            second = player.frame().unwrap();
            second.is_some()
        });
        assert_ne!(
            first.pixels,
            second.unwrap().pixels,
            "Playback must deliver moving video frames"
        );
        player.toggle().unwrap();
        wait_until(|| !player.state().unwrap().playing);
        let paused = player.state().unwrap().position;
        std::thread::sleep(Duration::from_millis(200));
        assert!((player.state().unwrap().position - paused).abs() < 0.08);
        player.seek(1.2).unwrap();
        wait_until(|| (player.state().unwrap().position - 1.2).abs() < 0.1);
        assert!(player.player.IsMuted().unwrap());
        player.mute(false).unwrap();
        assert!(!player.player.IsMuted().unwrap());
        player.volume(0.0).unwrap(); // Exercise audio controls without sounding the test clip.
        assert_eq!(player.player.Volume().unwrap(), 0.0);
        player.seek(1.85).unwrap();
        player.toggle().unwrap();
        wait_until(|| player.state().unwrap().position < 0.8);
        assert!(player.state().unwrap().playing, "GIF playback must loop");
        let closed = player.player.clone();
        drop(player);
        assert!(
            closed.Play().is_err(),
            "Closing a preview must dispose the native player"
        );
        drop(closed);
        let mut player = NativePlayer::new(&uri, false, true, 0.0).unwrap();
        wait_until(|| player.state().unwrap().duration > 1.9 && player.state().unwrap().playing);
        wait_until(|| player.frame().unwrap().is_some());
        player.seek(1.85).unwrap();
        wait_until(|| !player.state().unwrap().playing && player.state().unwrap().position >= 1.9);
        drop(player);
        unsafe {
            RoUninitialize();
        }
    }

    #[test]
    #[ignore = "requires an explicit local fixture containing live media URLs"]
    fn live_x_media_decode_and_seek() {
        keep_test_apartment();
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let fixture = std::env::var_os("TWITTERDL_LIVE_PREVIEW_FIXTURE")
            .expect("Set TWITTERDL_LIVE_PREVIEW_FIXTURE");
        let candidates: Vec<crate::download::engine::MediaCandidate> =
            serde_json::from_slice(&std::fs::read(fixture).unwrap()).unwrap();
        assert!(!candidates.is_empty());
        for (index, candidate) in candidates.into_iter().enumerate() {
            let proxy = runtime
                .block_on(super::super::proxy::ProxyServer::start(
                    candidate.url,
                    index as u64,
                ))
                .unwrap();
            let looping = candidate.media_type == "animated_gif";
            let mut player = NativePlayer::new(&proxy.uri, looping, true, 0.0).unwrap();
            wait_until(|| {
                player.state().unwrap().duration > 0.0 && player.state().unwrap().playing
            });
            let mut first = None;
            wait_until(|| {
                first = player.frame().unwrap();
                first.is_some()
            });
            let first = first.unwrap();
            assert!(first.size[0] > 0 && first.size[1] > 0);
            let target = (player.state().unwrap().duration * 0.5).min(1.0);
            player.seek(target).unwrap();
            wait_until(|| player.state().unwrap().position >= target - 0.15);
            let mut second = None;
            wait_until(|| {
                second = player.frame().unwrap();
                second
                    .as_ref()
                    .is_some_and(|frame| frame.pixels != first.pixels)
            });
            eprintln!(
                "{} decoded and sought: {} × {}",
                candidate.media_type, first.size[0], first.size[1]
            );
            drop(player);
            drop(proxy);
        }
        drop(runtime);
        unsafe {
            RoUninitialize();
        }
    }

    #[test]
    fn native_player_decodes_the_range_proxy_stream() {
        keep_test_apartment();
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let (proxy, upstream_task) = runtime.block_on(async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let task = tokio::spawn(async move {
                loop {
                    let Ok((mut socket, _)) = listener.accept().await else { break };
                    tokio::spawn(async move {
                        let video = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/preview.mp4"));
                        let mut request = Vec::new(); let mut buffer = [0; 2048];
                        while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                            let length = socket.read(&mut buffer).await.unwrap();
                            if length == 0 { return; }
                            request.extend_from_slice(&buffer[..length]);
                        }
                        let request = String::from_utf8_lossy(&request).to_lowercase();
                        assert!(request.contains("referer: https://x.com/"));
                        let range = request.lines().find_map(|line| line.strip_prefix("range: bytes="));
                        let (start, end) = range.and_then(|range| range.split_once('-')).map(|(start, end)| (
                            start.parse::<usize>().unwrap_or(0), end.parse::<usize>().unwrap_or(video.len() - 1).min(video.len() - 1),
                        )).unwrap_or((0, video.len() - 1));
                        let status = if range.is_some() { "206 Partial Content" } else { "200 OK" };
                        let headers = format!("HTTP/1.1 {status}\r\nContent-Type: video/mp4\r\nAccept-Ranges: bytes\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{end}/{}\r\nConnection: close\r\n\r\n", end - start + 1, video.len());
                        let _ = socket.write_all(headers.as_bytes()).await;
                        if !request.starts_with("head ") { let _ = socket.write_all(&video[start..=end]).await; }
                    });
                }
            });
            let proxy = super::super::proxy::ProxyServer::start(format!("http://{address}/test.mp4"), 12).await.unwrap();
            (proxy, task)
        });
        let mut player = NativePlayer::new(&proxy.uri, false, true, 0.0).unwrap();
        wait_until(|| player.state().unwrap().duration > 1.9 && player.state().unwrap().playing);
        let mut frame = None;
        wait_until(|| {
            frame = player.frame().unwrap();
            frame.is_some()
        });
        assert_eq!(frame.unwrap().size[0], 160);
        player.seek(1.0).unwrap();
        wait_until(|| player.state().unwrap().position >= 0.9);
        drop(player);
        drop(proxy);
        upstream_task.abort();
        drop(runtime);
        unsafe {
            RoUninitialize();
        }
    }
}
