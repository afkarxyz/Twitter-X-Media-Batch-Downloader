use crate::AppWindow;
use crate::download::engine as downloader;
use crate::download::gifs as gif_conversion;
use crate::platform::logging as session_log;
use futures_util::StreamExt;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::io::AsyncWriteExt;

const RELEASES: &str = "https://api.github.com/repos/spotbye/Dependencies/releases?per_page=100";
const MAX_ARCHIVE: u64 = 256 * 1024 * 1024;
const MAX_BINARY: u64 = 512 * 1024 * 1024;
static INSTALL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Clone, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    digest: Option<String>,
}
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

fn asset_name() -> Result<&'static str, String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", _) => Ok("ffmpeg-windows.zip"),
        ("linux", "x86_64") => Ok("ffmpeg-linux-amd64.zip"),
        ("linux", "aarch64") => Ok("ffmpeg-linux-arm64v8.zip"),
        ("macos", "x86_64") => Ok("ffmpeg-macos-amd64.zip"),
        ("macos", "aarch64") => Ok("ffmpeg-macos-arm64.zip"),
        _ => Err("Automatic FFmpeg setup is unavailable on this platform.".into()),
    }
}
fn select_release(releases: Vec<Release>, name: &str) -> Option<(String, Asset)> {
    releases
        .into_iter()
        .find(|release| {
            !release.draft
                && !release.prerelease
                && release.tag_name.starts_with("FFmpeg-")
                && release.tag_name[7..].starts_with(|character: char| character.is_ascii_digit())
        })
        .and_then(|release| {
            release
                .assets
                .into_iter()
                .find(|asset| asset.name == name)
                .map(|asset| (release.tag_name, asset))
        })
}
async fn resolve(client: &reqwest::Client) -> Result<(String, Asset), String> {
    let name = asset_name()?;
    if let Ok(response) = client
        .get(RELEASES)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        && response.status().is_success()
        && let Ok(bytes) = response.bytes().await
        && let Ok(releases) = serde_json::from_slice(&bytes)
        && let Some(release) = select_release(releases, name)
    {
        return Ok(release);
    }
    // Same pinned fallback as the old project's dependency installer.
    Ok((
        "FFmpeg-9.0".into(),
        Asset {
            name: name.into(),
            browser_download_url: format!(
                "https://github.com/spotbye/Dependencies/releases/download/FFmpeg-9.0/{name}"
            ),
            digest: None,
        },
    ))
}

async fn valid(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let mut command = tokio::process::Command::new(path);
    command
        .arg("-version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    matches!(tokio::time::timeout(Duration::from_secs(5), command.output()).await, Ok(Ok(output)) if output.status.success() && output.stdout.starts_with(b"ffmpeg version "))
}
fn publish(
    ui: &Option<crate::ui::runtime::Weak<AppWindow>>,
    status: &str,
    progress: f32,
    detail: String,
    for_media: bool,
) {
    if let Some(ui) = ui {
        let status = status.to_string();
        let _ = ui.upgrade_in_event_loop(move |app| {
            if for_media && app.get_is_busy() {
                app.set_status(status.clone());
                app.set_progress(progress);
                app.set_progress_detail(detail.clone());
            }
            app.set_ffmpeg_status(status.clone());
            if app.get_tool_update_busy() {
                app.set_tool_update_status(status);
                app.set_tool_update_progress(progress);
                app.set_tool_update_detail(detail.clone());
            }
            app.set_ffmpeg_progress(progress);
            app.set_ffmpeg_detail(detail.clone());
        });
    }
}
async fn cancelled(cancel: &AtomicBool) {
    while !cancel.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn unpack(archive: &Path, staging: &Path, digest: Option<&str>) -> Result<(), String> {
    if let Some(digest) = digest.filter(|digest| !digest.is_empty()) {
        let expected = digest
            .strip_prefix("sha256:")
            .ok_or("Unsupported FFmpeg checksum.")?;
        let mut source = std::fs::File::open(archive).map_err(|error| error.to_string())?;
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let count = source
                .read(&mut buffer)
                .map_err(|error| error.to_string())?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
        if format!("{:x}", hash.finalize()) != expected.to_ascii_lowercase() {
            return Err("FFmpeg archive checksum did not match.".into());
        }
    }
    let file = std::fs::File::open(archive).map_err(|error| error.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
    let name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(|error| error.to_string())?;
        if entry.is_dir()
            || Path::new(entry.name())
                .file_name()
                .is_none_or(|base| base != name)
        {
            continue;
        }
        if entry.size() == 0 || entry.size() > MAX_BINARY {
            return Err("Invalid FFmpeg binary size.".into());
        }
        let mut out = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(staging)
            .map_err(|error| error.to_string())?;
        let size = std::io::copy(&mut entry.by_ref().take(MAX_BINARY + 1), &mut out)
            .map_err(|error| error.to_string())?;
        if size > MAX_BINARY {
            return Err("FFmpeg binary exceeded its size limit.".into());
        }
        out.flush().map_err(|error| error.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(staging, std::fs::Permissions::from_mode(0o755))
                .map_err(|error| error.to_string())?;
        }
        return Ok(());
    }
    Err("FFmpeg binary was not found in the archive.".into())
}

async fn install(
    client: &reqwest::Client,
    root: &Path,
    release: (String, Asset),
    ui: Option<crate::ui::runtime::Weak<AppWindow>>,
    cancel: Arc<AtomicBool>,
    for_media: bool,
    force: bool,
) -> Result<PathBuf, String> {
    tokio::fs::create_dir_all(root)
        .await
        .map_err(|error| error.to_string())?;
    let key = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let archive = root.join(format!(".ffmpeg-{key}.zip.part"));
    let staging = root.join(format!(
        ".ffmpeg-{key}{}",
        if cfg!(windows) { ".exe" } else { ".tmp" }
    ));
    let destination = root.join(if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    });
    let work = async {
        session_log::info(format!(
            "Downloading {} from {}.",
            release.0, release.1.browser_download_url
        ));
        let response = tokio::select! {
            response = client.get(&release.1.browser_download_url).send() => response,
            _ = cancelled(&cancel) => return Err("FFmpeg setup cancelled.".into()),
        }
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?;
        let total = response.content_length();
        if total.is_some_and(|size| size > MAX_ARCHIVE) {
            return Err("FFmpeg archive exceeded its size limit.".into());
        }
        let mut file = tokio::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&archive)
            .await
            .map_err(|error| error.to_string())?;
        let mut stream = response.bytes_stream();
        let mut downloaded = 0u64;
        let mut last = Instant::now() - Duration::from_secs(1);
        loop {
            let chunk = tokio::select! { chunk = stream.next() => chunk, _ = cancelled(&cancel) => return Err("FFmpeg setup cancelled.".into()) };
            let Some(chunk) = chunk else { break };
            let chunk = chunk.map_err(|error| error.to_string())?;
            downloaded += chunk.len() as u64;
            if downloaded > MAX_ARCHIVE {
                return Err("FFmpeg archive exceeded its size limit.".into());
            }
            file.write_all(&chunk)
                .await
                .map_err(|error| error.to_string())?;
            if last.elapsed() >= Duration::from_millis(100) {
                let detail = total
                    .map(|total| {
                        format!(
                            "{} / {}",
                            downloader::format_bytes(downloaded),
                            downloader::format_bytes(total)
                        )
                    })
                    .unwrap_or_else(|| downloader::format_bytes(downloaded));
                publish(
                    &ui,
                    "Downloading FFmpeg…",
                    total
                        .map(|total| downloaded as f32 / total.max(1) as f32 * 95.0)
                        .unwrap_or(0.0),
                    detail,
                    for_media,
                );
                last = Instant::now();
            }
        }
        file.flush().await.map_err(|error| error.to_string())?;
        drop(file);
        publish(&ui, "Installing FFmpeg…", 95.0, String::new(), for_media);
        let source = archive.clone();
        let target = staging.clone();
        let digest = release.1.digest.clone();
        tokio::task::spawn_blocking(move || unpack(&source, &target, digest.as_deref()))
            .await
            .map_err(|error| error.to_string())??;
        if cancel.load(Ordering::Relaxed) {
            return Err("FFmpeg setup cancelled.".into());
        }
        if !valid(&staging).await {
            return Err("The downloaded FFmpeg binary could not be started.".into());
        }
        if !force && valid(&destination).await {
            return Ok(destination.clone());
        }
        crate::updates::publish::replace_when_released(&staging, &destination, &cancel)
            .await
            .map_err(|error| error.to_string())?;
        let metadata = serde_json::json!({"version":release.0.trim_start_matches("FFmpeg-"),"asset_name":release.1.name});
        let _ = tokio::fs::write(root.join("ffmpeg-version.json"), metadata.to_string()).await;
        Ok(destination.clone())
    };
    // Do not cancel spawn_blocking unpack midway: it must finish before its staging file is cleaned.
    let result = work.await;
    let _ = tokio::fs::remove_file(&archive).await;
    let _ = tokio::fs::remove_file(&staging).await;
    result
}

pub async fn ensure_available(
    ui: crate::ui::runtime::Weak<AppWindow>,
    cancel: Arc<AtomicBool>,
    for_media: bool,
) -> Result<PathBuf, String> {
    let _lock = tokio::select! { guard = INSTALL.lock() => guard, _ = cancelled(&cancel) => return Err("FFmpeg setup cancelled.".into()) };
    let _ = ui.upgrade_in_event_loop(move |app| {
        app.set_ffmpeg_busy(true);
        app.set_ffmpeg_ready(false);
        app.set_ffmpeg_status("Checking FFmpeg…".into());
        if for_media {
            app.set_status("Preparing FFmpeg…".into());
            app.set_progress(0.0);
            app.set_progress_detail("".into());
        }
    });
    let result = prepare(Some(ui.clone()), cancel, for_media, false).await;
    let ready = result.is_ok();
    let status = result.as_ref().err().cloned().unwrap_or_default();
    let _ = ui.upgrade_in_event_loop(move |app| {
        app.set_ffmpeg_busy(false);
        app.set_ffmpeg_ready(ready);
        app.set_ffmpeg_status(status);
        if ready {
            app.set_ffmpeg_progress(100.0);
        }
        if ready && for_media {
            app.set_status("Downloading media…".into());
            app.set_progress(0.0);
            app.set_progress_detail("".into());
        }
    });
    result
}
async fn prepare(
    ui: Option<crate::ui::runtime::Weak<AppWindow>>,
    cancel: Arc<AtomicBool>,
    for_media: bool,
    force: bool,
) -> Result<PathBuf, String> {
    for path in gif_conversion::candidates() {
        if !force && valid(&path).await {
            gif_conversion::prefer_executable(path.clone());
            return Ok(path);
        }
    }
    let root = downloader::app_data_dir().ok_or("Could not locate the portable data directory.")?;
    tokio::fs::create_dir_all(&root)
        .await
        .map_err(|error| error.to_string())?;
    let disk_lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(root.join(".ffmpeg-install.lock"))
        .map_err(|error| error.to_string())?;
    loop {
        match disk_lock.try_lock() {
            Ok(()) => break,
            Err(std::fs::TryLockError::WouldBlock) => {
                tokio::select! { _ = tokio::time::sleep(Duration::from_millis(100)) => {}, _ = cancelled(&cancel) => return Err("FFmpeg setup cancelled.".into()) }
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    for path in gif_conversion::candidates() {
        if !force && valid(&path).await {
            gif_conversion::prefer_executable(path.clone());
            return Ok(path);
        }
    }
    let client = crate::network::proxy::builder()
        .user_agent("Twitter-X-Media-Batch-Downloader")
        .timeout(Duration::from_secs(600))
        .connect_timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| error.to_string())?;
    let release = tokio::select! { release = resolve(&client) => release?, _ = cancelled(&cancel) => return Err("FFmpeg setup cancelled.".into()) };
    let path = install(&client, &root, release, ui, cancel, for_media, force).await?;
    gif_conversion::prefer_executable(path.clone());
    Ok(path)
}

pub async fn reinstall(
    ui: crate::ui::runtime::Weak<AppWindow>,
    cancel: Arc<AtomicBool>,
) -> Result<PathBuf, String> {
    let _lock = tokio::select! {guard=INSTALL.lock()=>guard,_=cancelled(&cancel)=>return Err("FFmpeg setup cancelled.".into())};
    prepare(Some(ui), cancel, false, true).await
}

pub fn register(app: &AppWindow) {
    let ui = app.as_weak();
    app.on_ensure_ffmpeg(move || {
        let Some(app) = ui.upgrade() else { return };
        if !app.get_convert_gif() || app.get_ffmpeg_busy() {
            return;
        }
        app.set_ffmpeg_busy(true);
        app.set_ffmpeg_ready(false);
        app.set_ffmpeg_status("Checking FFmpeg…".into());
        app.set_ffmpeg_progress(0.0);
        app.set_ffmpeg_detail("".into());
        let ui = ui.clone();
        tokio::spawn(async move {
            let result =
                ensure_available(ui.clone(), Arc::new(AtomicBool::new(false)), false).await;
            if let Err(error) = result {
                session_log::error(format!("FFmpeg setup failed: {error}"));
                let _ = ui.upgrade_in_event_loop(move |app| {
                    crate::app::notifications::show_toast(&app, error, "error")
                });
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn archive() -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        writer.start_file("bin/readme.txt", options).unwrap();
        writer.write_all(b"not executable").unwrap();
        writer
            .start_file(
                if cfg!(windows) {
                    "bin/ffmpeg.exe"
                } else {
                    "bin/ffmpeg"
                },
                options,
            )
            .unwrap();
        writer.write_all(b"binary fixture").unwrap();
        writer.finish().unwrap().into_inner()
    }
    #[test]
    fn selects_stable_ffmpeg_assets_and_verifies_archive_digest() {
        let releases = serde_json::from_str(r#"[
            {"tag_name":"Xtractor-1.0","assets":[]},
            {"tag_name":"FFmpeg-10.0","prerelease":true,"assets":[]},
            {"tag_name":"FFmpeg-9.0","assets":[{"name":"ffmpeg-windows.zip","browser_download_url":"https://example.test/ffmpeg.zip"}]}
        ]"#).unwrap();
        let (tag, asset) = select_release(releases, "ffmpeg-windows.zip").unwrap();
        assert_eq!(tag, "FFmpeg-9.0");
        assert_eq!(
            asset.browser_download_url,
            "https://example.test/ffmpeg.zip"
        );
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let bytes = archive();
        let path = fixture.0.join("ffmpeg.zip");
        let staging = fixture.0.join("staging");
        std::fs::write(&path, &bytes).unwrap();
        assert!(unpack(&path, &staging, Some(&format!("sha256:{}", "0".repeat(64)))).is_err());
        assert!(!staging.exists());
        unpack(
            &path,
            &staging,
            Some(&format!("sha256:{:x}", Sha256::digest(&bytes))),
        )
        .unwrap();
        assert_eq!(std::fs::read(staging).unwrap(), b"binary fixture");
    }
    #[tokio::test]
    async fn a_failed_install_cleans_partial_files_and_never_publishes_a_binary() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let bytes = archive();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0u8; 4096];
            let _ = stream.read(&mut request).await;
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        bytes.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            stream.write_all(&bytes).await.unwrap();
        });
        let release = (
            "FFmpeg-9.0".into(),
            Asset {
                name: "ffmpeg.zip".into(),
                browser_download_url: format!("http://{address}/ffmpeg.zip"),
                digest: Some(format!("sha256:{}", "0".repeat(64))),
            },
        );
        let error = install(
            &reqwest::Client::new(),
            &fixture.0,
            release,
            None,
            Arc::new(AtomicBool::new(false)),
            false,
            false,
        )
        .await
        .unwrap_err();
        assert!(error.contains("checksum"));
        server.await.unwrap();
        assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 0);
    }
    #[tokio::test]
    #[ignore = "downloads the old project's GitHub FFmpeg asset into an isolated test fixture"]
    async fn github_install_runs_and_converts_a_real_video_to_gif() {
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let client = crate::network::proxy::builder()
            .user_agent("Twitter-X-Media-Batch-Downloader")
            .timeout(Duration::from_secs(600))
            .build()
            .unwrap();
        let release = resolve(&client).await.unwrap();
        let path = install(
            &client,
            &fixture.0,
            release,
            None,
            Arc::new(AtomicBool::new(false)),
            false,
            false,
        )
        .await
        .unwrap();
        assert!(valid(&path).await);
        gif_conversion::prefer_executable(path);
        let input = fixture.0.join("sample.mp4");
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/preview.mp4"),
            &input,
        )
        .unwrap();
        let output = gif_conversion::convert_download(
            &input,
            gif_conversion::Options::default(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();
        let data = std::fs::read(output).unwrap();
        assert!(data.starts_with(b"GIF89a"));
        assert!(!input.exists());
        assert!(!std::fs::read_dir(&fixture.0).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".part")
        }));
    }
}
