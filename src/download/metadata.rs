use crate::AppWindow;
use crate::download::engine::MediaCandidate;
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};

static INSTALL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static TOOL: Mutex<Option<PathBuf>> = Mutex::new(None);
const RELEASE: &str = "https://sourceforge.net/projects/exiftool/best_release.json";
const MAX_ARCHIVE: usize = 64 * 1024 * 1024;

pub fn supported(path: &Path) -> bool {
    path.extension().is_some_and(|ext| {
        ["jpg", "jpeg", "mp4"]
            .iter()
            .any(|value| ext.eq_ignore_ascii_case(value))
    })
}
fn command(path: &Path) -> Command {
    let mut cmd = Command::new(path);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    cmd
}
async fn version(path: &Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }
    let output = tokio::time::timeout(Duration::from_secs(10), command(path).arg("-ver").output())
        .await
        .ok()?
        .ok()?;
    let value = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (output.status.success() && valid_version(&value)).then_some(value)
}
fn valid_version(value: &str) -> bool {
    let parts = value.split('.').collect::<Vec<_>>();
    parts.len() >= 2
        && value.len() < 24
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|c| c.is_ascii_digit()))
}
fn portable_version_key(path: &Path) -> Vec<u64> {
    path.parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .trim_start_matches('v')
        .split('.')
        .map(|part| part.parse().unwrap_or(0))
        .collect()
}
fn candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(path) = TOOL.lock().unwrap().clone() {
        paths.push(path);
    }
    if let Some(root) = crate::download::engine::app_data_dir() {
        if let Some(version) = std::fs::read(root.join("exiftool-active.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|value| {
                value
                    .get("version")
                    .and_then(|v| v.as_str())
                    .filter(|v| valid_version(v))
                    .map(str::to_owned)
            })
        {
            paths.push(
                root.join("exiftool")
                    .join(format!("v{version}"))
                    .join("exiftool.exe"),
            );
        }
        paths.push(root.join("exiftool.exe"));
        paths.push(root.join("exiftool"));
        if let Ok(entries) = std::fs::read_dir(root.join("exiftool")) {
            let mut versions = entries
                .filter_map(Result::ok)
                .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
                .map(|entry| entry.path().join("exiftool.exe"))
                .collect::<Vec<_>>();
            versions.sort_by_key(|path| std::cmp::Reverse(portable_version_key(path)));
            paths.extend(versions);
        }
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(root) = exe.parent()
    {
        paths.push(root.join("exiftool.exe"));
    }
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path).map(|directory| {
            directory.join(if cfg!(windows) {
                "exiftool.exe"
            } else {
                "exiftool"
            })
        }));
    }
    paths
}
pub async fn installed_version() -> Option<String> {
    for path in candidates() {
        if let Some(version) = version(&path).await {
            return Some(version);
        }
    }
    None
}

async fn cancellable<T>(
    future: impl std::future::Future<Output = Result<T, reqwest::Error>>,
    cancel: &AtomicBool,
) -> Result<T, String> {
    tokio::select! {result=future=>result.map_err(|e|e.to_string()),_=cancelled(cancel)=>Err("ExifTool setup cancelled.".into())}
}
async fn cancelled(cancel: &AtomicBool) {
    while !cancel.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
}
fn publish(ui: &crate::ui::runtime::Weak<AppWindow>, status: &str, progress: f32, detail: String) {
    let status = status.to_owned();
    let _ = ui.upgrade_in_event_loop(move |app| {
        if app.get_tool_update_busy() {
            app.set_tool_update_status(status);
            app.set_tool_update_progress(progress);
            app.set_tool_update_detail(detail);
        } else {
            app.set_status(status);
            app.set_progress(progress);
            app.set_progress_detail(detail);
        }
    });
}

fn unpack(bytes: Vec<u8>, directory: &Path) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    if zip.len() > 10000 {
        return Err("ExifTool archive contains too many files.".into());
    }
    let mut expanded = 0u64;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(|e| e.to_string())?;
        let path = entry
            .enclosed_name()
            .ok_or("Invalid ExifTool archive path.")?;
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err("ExifTool archive contains symbolic links.".into());
        }
        let components = path
            .components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let exe = components
            .last()
            .is_some_and(|name| name == "exiftool(-k).exe" || name == "exiftool.exe");
        let relative = if exe {
            PathBuf::from("exiftool.exe")
        } else if let Some(start) = components.iter().position(|name| name == "exiftool_files") {
            components[start..].iter().collect::<PathBuf>()
        } else {
            continue;
        };
        // Reject Windows drive/ADS names even when the ZIP path is relative.
        if relative
            .components()
            .any(|part| part.as_os_str().to_string_lossy().contains(':'))
        {
            return Err("Invalid ExifTool archive filename.".into());
        }
        let target = directory.join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(target).map_err(|e| e.to_string())?;
            continue;
        }
        expanded = expanded
            .checked_add(entry.size())
            .ok_or("Invalid ExifTool archive size.")?;
        if expanded > 256 * 1024 * 1024 {
            return Err("ExifTool archive expands beyond the size limit.".into());
        }
        std::fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(target)
            .map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut output).map_err(|e| e.to_string())?;
        output.sync_all().map_err(|e| e.to_string())?;
    }
    if !directory.join("exiftool.exe").is_file() || !directory.join("exiftool_files").is_dir() {
        return Err("ExifTool archive is missing its executable or support files.".into());
    }
    Ok(())
}

pub async fn ensure_available(
    ui: crate::ui::runtime::Weak<AppWindow>,
    cancel: Arc<AtomicBool>,
) -> Result<PathBuf, String> {
    let _guard = tokio::select! {guard=INSTALL.lock()=>guard,_=cancelled(&cancel)=>return Err("ExifTool setup cancelled.".into())};
    for path in candidates() {
        if version(&path).await.is_some() {
            *TOOL.lock().unwrap() = Some(path.clone());
            return Ok(path);
        }
    }
    let root = crate::download::engine::app_data_dir().ok_or("Could not locate portable data.")?;
    install(ui, cancel, root, false).await
}

pub async fn reinstall(
    ui: crate::ui::runtime::Weak<AppWindow>,
    cancel: Arc<AtomicBool>,
) -> Result<PathBuf, String> {
    let _guard = tokio::select! {guard=INSTALL.lock()=>guard,_=cancelled(&cancel)=>return Err("ExifTool setup cancelled.".into())};
    let root = crate::download::engine::app_data_dir().ok_or("Could not locate portable data.")?;
    install(ui, cancel, root, true).await
}

async fn install(
    ui: crate::ui::runtime::Weak<AppWindow>,
    cancel: Arc<AtomicBool>,
    data_root: PathBuf,
    force: bool,
) -> Result<PathBuf, String> {
    if !cfg!(windows) {
        return Err("Install ExifTool in PATH to enable metadata on this platform.".into());
    }
    tokio::fs::create_dir_all(&data_root)
        .await
        .map_err(|e| e.to_string())?;
    let disk_lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(data_root.join(".exiftool-install.lock"))
        .map_err(|e| e.to_string())?;
    loop {
        match disk_lock.try_lock() {
            Ok(()) => break,
            Err(std::fs::TryLockError::WouldBlock) => {
                tokio::select! {_=tokio::time::sleep(Duration::from_millis(100))=>{},_=cancelled(&cancel)=>return Err("ExifTool setup cancelled.".into())}
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    publish(&ui, "Preparing ExifTool…", 0.0, String::new());
    let client = crate::network::proxy::builder()
        .user_agent("TwitterXMediaBatchDownloader/6.0")
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    let setup = async {
        let response = cancellable(client.get(RELEASE).send(), &cancel)
            .await?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        let bytes = cancellable(response.bytes(), &cancel).await?;
        if bytes.len() > 1024 * 1024 {
            return Err("ExifTool release response is too large.".into());
        }
        let data: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        let release = data
            .get("platform_releases")
            .and_then(|data| data.get("windows"))
            .or_else(|| data.get("release"))
            .ok_or("Missing ExifTool release.")?;
        let name = release
            .get("filename")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim_start_matches('/');
        let suffix = if cfg!(target_pointer_width = "64") {
            "_64.zip"
        } else {
            "_32.zip"
        };
        let ver = name
            .strip_prefix("exiftool-")
            .and_then(|name| name.strip_suffix(suffix))
            .filter(|ver| valid_version(ver))
            .ok_or("No compatible ExifTool Windows archive was found.")?;
        let checksum = release
            .get("sha256sum")
            .and_then(|v| v.as_str())
            .filter(|value| value.len() == 64 && value.bytes().all(|c| c.is_ascii_hexdigit()))
            .ok_or("ExifTool release has no SHA-256 checksum.")?
            .to_ascii_lowercase();
        let url = format!("https://downloads.sourceforge.net/project/exiftool/{name}");
        let response = cancellable(client.get(url).send(), &cancel)
            .await?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        let total = response.content_length();
        let mut stream = response.bytes_stream();
        let mut archive = Vec::new();
        let mut last = std::time::Instant::now();
        while let Some(chunk) = tokio::select! {chunk=stream.next()=>chunk,_=cancelled(&cancel)=>return Err("ExifTool setup cancelled.".into())}
        {
            let chunk = chunk.map_err(|e| e.to_string())?;
            if archive.len() + chunk.len() > MAX_ARCHIVE {
                return Err("ExifTool archive exceeds the size limit.".into());
            }
            archive.extend_from_slice(&chunk);
            if last.elapsed() > Duration::from_millis(150) {
                publish(
                    &ui,
                    "Downloading ExifTool…",
                    total.map_or(0.0, |total| {
                        archive.len() as f32 / total.max(1) as f32 * 90.0
                    }),
                    crate::download::engine::format_bytes(archive.len() as u64),
                );
                last = std::time::Instant::now();
            }
        }
        if format!("{:x}", Sha256::digest(&archive)) != checksum {
            return Err("ExifTool archive checksum did not match.".into());
        }
        let root = data_root.join("exiftool");
        tokio::fs::create_dir_all(&root)
            .await
            .map_err(|e| e.to_string())?;
        let staging = root.join(format!(
            ".install-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_millis()
        ));
        let target = root.join(format!("v{ver}"));
        if !force && version(&target.join("exiftool.exe")).await.as_deref() == Some(ver) {
            return Ok(target.join("exiftool.exe"));
        }
        publish(&ui, "Installing ExifTool…", 95.0, String::new());
        let unpack_dir = staging.clone();
        tokio::fs::create_dir(&staging)
            .await
            .map_err(|e| e.to_string())?;
        let outcome = async {
            tokio::task::spawn_blocking(move || unpack(archive, &unpack_dir))
                .await
                .map_err(|e| e.to_string())??;
            if version(&staging.join("exiftool.exe")).await.as_deref() != Some(ver) {
                return Err("ExifTool executable validation failed.".into());
            }
            if cancel.load(Ordering::Relaxed) {
                return Err("ExifTool setup cancelled.".into());
            }
            crate::updates::publish::replace_when_released(&staging, &target, &cancel)
                .await
                .map_err(|e| e.to_string())?;
            let active = data_root.join("exiftool-active.json");
            let pending = data_root.join(format!(".exiftool-active-{}.part", std::process::id()));
            tokio::fs::write(&pending, serde_json::json!({"version":ver}).to_string())
                .await
                .map_err(|e| e.to_string())?;
            if let Err(error) = tokio::fs::rename(&pending, &active).await {
                let _ = tokio::fs::remove_file(&pending).await;
                return Err(error.to_string());
            }
            Ok(target.join("exiftool.exe"))
        }
        .await;
        // Only this task's unique staging directory is removed.
        let _ = tokio::fs::remove_dir_all(&staging).await;
        outcome
    };
    let result = setup.await;
    if let Ok(path) = &result {
        *TOOL.lock().unwrap() = Some(path.clone());
        publish(&ui, "Downloading media…", 0.0, String::new());
    }
    result
}

pub fn comment(item: &MediaCandidate) -> String {
    let mut parts = Vec::new();
    if !item.tweet_url.is_empty() {
        parts.push(item.tweet_url.clone());
    }
    let original = reqwest::Url::parse(&item.url)
        .ok()
        .and_then(|url| {
            url.path_segments()
                .and_then(|mut parts| parts.next_back())
                .map(|name| {
                    name.split('.')
                        .next()
                        .unwrap_or_default()
                        .split(':')
                        .next()
                        .unwrap_or_default()
                        .to_owned()
                })
        })
        .unwrap_or_default();
    if !original.is_empty() {
        parts.push(original);
    }
    if !item.username.is_empty() {
        parts.push(format!("@{}", item.username.trim_start_matches('@')));
    }
    if !item.created_at.is_empty() {
        parts.push(item.created_at.clone());
    }
    if !item.tweet_text.is_empty() {
        parts.push(item.tweet_text.clone());
    }
    let full = parts.join(" | ");
    if full.len() <= 60_000 {
        return full;
    }
    let mut end = 59_997;
    while !full.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &full[..end])
}

async fn run_tag(
    tool: &Path,
    arguments: &str,
    cancel: Arc<AtomicBool>,
) -> Result<std::process::Output, String> {
    let mut cmd = command(tool);
    cmd.args(["-@", "-"]).stdin(Stdio::piped());
    let mut child = cmd.spawn().map_err(|error| error.to_string())?;
    let mut input = child.stdin.take().ok_or("ExifTool input is unavailable.")?;
    let write = input.write_all(arguments.as_bytes()).await;
    drop(input);
    if let Err(error) = write {
        let _ = child.kill().await;
        let _ = child.wait().await;
        return Err(error.to_string());
    }
    let mut stdout = child
        .stdout
        .take()
        .ok_or("ExifTool output is unavailable.")?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or("ExifTool diagnostics are unavailable.")?;
    let output = tokio::spawn(async move {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes).await;
        bytes
    });
    let diagnostics = tokio::spawn(async move {
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes).await;
        bytes
    });
    let status = tokio::select! {
        result = tokio::time::timeout(Duration::from_secs(60), child.wait()) => match result {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(_) => { let _=child.kill().await; let _=child.wait().await; Err("ExifTool timed out.".into()) },
        },
        _ = cancelled(&cancel) => { let _=child.kill().await; let _=child.wait().await; Err("Metadata tagging cancelled.".into()) },
    };
    let stdout = output.await.unwrap_or_default();
    let stderr = diagnostics.await.unwrap_or_default();
    Ok(std::process::Output {
        status: status?,
        stdout,
        stderr,
    })
}

pub async fn embed(
    tool: &Path,
    path: &Path,
    item: &MediaCandidate,
    cancel: Arc<AtomicBool>,
) -> Result<(), String> {
    if !supported(path) {
        return Ok(());
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("Metadata tagging cancelled.".into());
    }
    // Failed/cancelled tagging never damages the downloaded file.
    let part = path.with_extension(format!(
        "{}.metadata-{}.part",
        path.extension().unwrap().to_string_lossy(),
        std::process::id()
    ));
    let tag_file = part.with_extension("metadata.txt.part");
    let mut output = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&part)
        .await
        .map_err(|error| error.to_string())?;
    let result = async {
        let mut input = tokio::fs::File::open(path).await.map_err(|error| error.to_string())?;
        tokio::select! {
            result = tokio::io::copy(&mut input, &mut output) => result.map_err(|error| error.to_string())?,
            _ = cancelled(&cancel) => return Err("Metadata tagging cancelled.".into()),
        };
        output.flush().await.map_err(|error| error.to_string())?;
        drop(output);
        let mut tag_output = tokio::fs::OpenOptions::new().create_new(true).write(true).open(&tag_file).await.map_err(|error|error.to_string())?;
        tag_output.write_all(comment(item).as_bytes()).await.map_err(|error|error.to_string())?;
        tag_output.flush().await.map_err(|error|error.to_string())?;
        drop(tag_output);
        // The Windows launcher may lose Unicode argv, so filenames and tag
        // payload travel through explicit UTF-8 streams/files instead.
        let arguments = format!("-overwrite_original\n-P\n-charset\nfilename=UTF8\n-charset\nUTF8\n-Comment<={}\n{}\n", tag_file.to_string_lossy(), part.to_string_lossy());
        let result = run_tag(tool, &arguments, cancel).await?;
        if !result.status.success() || !String::from_utf8_lossy(&result.stdout).contains("1 image files updated") {
            return Err(format!("ExifTool: {}", String::from_utf8_lossy(&result.stderr).trim()));
        }
        tokio::fs::rename(&part, path).await.map_err(|error|error.to_string())?;
        Ok(())
    }.await;
    let _ = tokio::fs::remove_file(&tag_file).await;
    let _ = tokio::fs::remove_file(&part).await;
    let mut exiftool_tmp = part.as_os_str().to_owned();
    exiftool_tmp.push("_exiftool_tmp");
    let _ = tokio::fs::remove_file(PathBuf::from(exiftool_tmp)).await;
    result
}

pub async fn tag_download(
    tool: Option<&Path>,
    path: &Path,
    item: &MediaCandidate,
    cancel: Arc<AtomicBool>,
) -> Option<String> {
    let tool = tool?;
    if let Err(error) = embed(tool, path, item, cancel).await {
        crate::platform::logging::warn(format!(
            "Metadata tagging failed for {}: {error}",
            path.display()
        ));
        Some(error)
    } else {
        None
    }
}

#[cfg(test)]
pub(crate) fn use_fixture_tool() -> Option<PathBuf> {
    let path = std::env::var_os("TWITTERDL_TEST_EXIFTOOL").map(PathBuf::from)?;
    if !path.is_file() {
        return None;
    }
    *TOOL.lock().unwrap() = Some(path.clone());
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "downloads the official ExifTool archive into an isolated test fixture"]
    async fn installs_official_windows_package_and_runs_the_complete_portable_tool() {
        if !cfg!(windows) {
            return;
        }
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let path = install(
            crate::ui::runtime::Weak::default(),
            Arc::new(AtomicBool::new(false)),
            fixture.0.clone(),
            false,
        )
        .await
        .unwrap();
        assert!(path.starts_with(&fixture.0));
        assert!(version(&path).await.is_some());
        assert!(path.parent().unwrap().join("exiftool_files").is_dir());
        assert!(
            std::fs::read_dir(fixture.0.join("exiftool"))
                .unwrap()
                .all(|entry| !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".install-"))
        );
    }

    #[test]
    fn comments_keep_unicode_boundaries_and_use_the_original_filename() {
        let item = MediaCandidate {
            url: "https://video.twimg.com/ext_tw_video/123/pu/vid/720x720/OriginalName.mp4?tag=12"
                .into(),
            tweet_url: "https://x.com/demo/status/123".into(),
            tweet_text: "🐦".repeat(25000),
            ..Default::default()
        };
        let value = comment(&item);
        assert!(value.starts_with("https://x.com/demo/status/123 | OriginalName"));
        assert!(value.len() <= 60000 && value.ends_with('…'));
    }
    #[test]
    fn archive_paths_cannot_escape_and_support_files_are_required() {
        use zip::{ZipWriter, write::SimpleFileOptions};
        let fixture = crate::download::saved_files::tests::Fixture::new();
        for name in ["../exiftool.exe", "exiftool-13.59/exiftool(-k).exe"] {
            let mut archive = ZipWriter::new(std::io::Cursor::new(Vec::new()));
            archive
                .start_file(name, SimpleFileOptions::default())
                .unwrap();
            std::io::Write::write_all(&mut archive, b"invalid executable").unwrap();
            let bytes = archive.finish().unwrap().into_inner();
            assert!(unpack(bytes, &fixture.0.join("output")).is_err());
            assert!(!fixture.0.join("exiftool.exe").exists());
        }
    }
    #[tokio::test]
    async fn actual_exiftool_roundtrip_preserves_jpeg_pixels_video_and_existing_metadata() {
        let Some(tool) = std::env::var_os("TWITTERDL_TEST_EXIFTOOL").map(PathBuf::from) else {
            return;
        };
        assert!(version(&tool).await.is_some());
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let item = MediaCandidate {
            url: "https://pbs.twimg.com/media/ABC.jpg?name=orig".into(),
            tweet_url: "https://x.com/demo/status/123".into(),
            username: "demo".into(),
            tweet_text: "Halo 世界 é 🐦\nSecond line; $(unchanged)".into(),
            created_at: "Sun Aug 31 17:52:02 +0000 2025".into(),
            ..Default::default()
        };
        let photo = fixture.0.join("photo with spaces.jpg");
        image::RgbImage::from_pixel(16, 12, image::Rgb([12, 44, 88]))
            .save(&photo)
            .unwrap();
        let expected_pixels = image::open(&photo).unwrap().to_rgb8();
        let existing = command(&tool)
            .arg("-overwrite_original")
            .arg("-Artist=Original camera author")
            .arg(&photo)
            .output()
            .await
            .unwrap();
        assert!(existing.status.success());
        embed(&tool, &photo, &item, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();
        let read = command(&tool)
            .args(["-j", "-Comment", "-Artist"])
            .arg(&photo)
            .output()
            .await
            .unwrap();
        let tags: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
        assert_eq!(tags[0]["Comment"], comment(&item));
        assert_eq!(tags[0]["Artist"], "Original camera author");
        assert_eq!(image::open(&photo).unwrap().to_rgb8(), expected_pixels);
        let unicode_photo = fixture.0.join("写真 世界 é.jpg");
        std::fs::copy(&photo, &unicode_photo).unwrap();
        embed(
            &tool,
            &unicode_photo,
            &item,
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();
        assert_eq!(
            image::open(&unicode_photo).unwrap().to_rgb8(),
            expected_pixels
        );
        let video = fixture.0.join("video.mp4");
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/preview.mp4"),
            &video,
        )
        .unwrap();
        let before = command(&tool)
            .args(["-j", "-Duration#"])
            .arg(&video)
            .output()
            .await
            .unwrap();
        embed(&tool, &video, &item, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();
        let read = command(&tool)
            .args(["-j", "-Comment", "-Duration#"])
            .arg(&video)
            .output()
            .await
            .unwrap();
        let tags: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
        let before: serde_json::Value = serde_json::from_slice(&before.stdout).unwrap();
        assert_eq!(tags[0]["Comment"], comment(&item));
        assert_eq!(tags[0]["Duration"], before[0]["Duration"]);
        let corrupt = fixture.0.join("corrupt.jpg");
        std::fs::write(&corrupt, b"keep original invalid data").unwrap();
        assert!(
            embed(&tool, &corrupt, &item, Arc::new(AtomicBool::new(false)))
                .await
                .is_err()
        );
        assert_eq!(
            std::fs::read(&corrupt).unwrap(),
            b"keep original invalid data"
        );
        let bytes = std::fs::read(&photo).unwrap();
        assert!(
            embed(&tool, &photo, &item, Arc::new(AtomicBool::new(true)))
                .await
                .is_err()
        );
        assert_eq!(std::fs::read(&photo).unwrap(), bytes);
        let png = fixture.0.join("unsupported.png");
        std::fs::write(&png, b"unchanged").unwrap();
        embed(
            Path::new("missing-tool"),
            &png,
            &item,
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();
        assert_eq!(std::fs::read(&png).unwrap(), b"unchanged");
        assert!(std::fs::read_dir(&fixture.0).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".part")
        }));
    }
}

#[cfg(test)]
mod portable_version_tests {
    use super::*;
    #[test]
    fn new_portable_versions_are_discovered_in_numeric_order() {
        let mut paths = [
            PathBuf::from("exiftool/v13.9/exiftool.exe"),
            PathBuf::from("exiftool/v13.59/exiftool.exe"),
            PathBuf::from("exiftool/v9.99/exiftool.exe"),
        ];
        paths.sort_by_key(|path| std::cmp::Reverse(portable_version_key(path)));
        assert_eq!(paths[0], PathBuf::from("exiftool/v13.59/exiftool.exe"));
    }
}
