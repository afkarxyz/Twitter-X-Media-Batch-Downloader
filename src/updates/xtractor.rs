use crate::AppWindow;
use crate::download::engine as downloader;
use crate::platform::logging as session_log;
use crate::platform::paths::XTRACTOR_BINARY;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::io::AsyncWriteExt;

const RELEASES: &str = "https://api.github.com/repos/afkarxyz/xtractor-bin/releases?per_page=20";
const MAX_ARCHIVE: u64 = 64 * 1024 * 1024;
const MAX_BINARY: u64 = 128 * 1024 * 1024;
static INSTALL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Clone, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    digest: Option<String>,
}
#[derive(Clone, Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    assets: Vec<Asset>,
}
#[derive(Serialize, Deserialize)]
struct Version {
    version: String,
    asset_name: String,
    #[serde(default)]
    sha256: String,
}

fn asset_name() -> Result<&'static str, String> {
    asset_for(std::env::consts::OS, std::env::consts::ARCH)
}

fn asset_for(os: &str, arch: &str) -> Result<&'static str, String> {
    match (os, arch) {
        ("windows", "x86_64" | "aarch64") => Ok("windows-amd64.zip"),
        ("linux", "x86_64") => Ok("linux-amd64.zip"),
        ("linux", "aarch64") => Ok("linux-arm64.zip"),
        ("macos", "x86_64") => Ok("macos-amd64.zip"),
        ("macos", "aarch64") => Ok("macos-arm64.zip"),
        _ => Err("Xtractor updates are unavailable on this platform.".into()),
    }
}

fn select_release(releases: Vec<Release>, asset_name: &str) -> Result<(String, Asset), String> {
    for release in releases {
        if release.draft || release.prerelease {
            continue;
        }
        if let Some(asset) = release
            .assets
            .into_iter()
            .find(|asset| asset.name == asset_name)
        {
            if !asset
                .browser_download_url
                .starts_with("https://github.com/afkarxyz/xtractor-bin/releases/download/")
            {
                return Err("Invalid xtractor release asset URL.".into());
            }
            return Ok((release.tag_name, asset));
        }
    }
    Err("No compatible stable xtractor release was found.".into())
}

fn client() -> Result<reqwest::Client, String> {
    crate::network::proxy::builder()
        .user_agent("TwitterXMediaBatchDownloader/6.0")
        .timeout(Duration::from_secs(120))
        .connect_timeout(Duration::from_secs(15))
        .build()
        .map_err(|error| error.to_string())
}

async fn latest(client: &reqwest::Client) -> Result<(String, Asset), String> {
    let response = client
        .get(RELEASES)
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err("No public Xtractor release is available at afkarxyz/xtractor-bin yet.".into());
    }
    let data = response
        .error_for_status()
        .map_err(|error| error.to_string())?
        .bytes()
        .await
        .map_err(|error| error.to_string())?;
    select_release(
        serde_json::from_slice(&data).map_err(|error| error.to_string())?,
        asset_name()?,
    )
}

fn hash(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let size = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if size == 0 {
            break;
        }
        digest.update(&buffer[..size]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn version_of(binary: &Path) -> Option<String> {
    if let Some(version) = crate::platform::binary_version::read(binary) {
        return Some(version);
    }
    #[cfg(not(windows))]
    return command_version(binary);
    #[cfg(windows)]
    None
}

#[cfg(any(not(windows), test))]
fn command_version(binary: &Path) -> Option<String> {
    use std::process::Stdio;

    let mut command = std::process::Command::new(binary);
    command
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().ok()?;
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                let mut output = String::new();
                child
                    .stdout
                    .take()?
                    .take(512)
                    .read_to_string(&mut output)
                    .ok()?;
                let version = output.trim().strip_prefix("xtractor ")?;
                return (!version.is_empty() && version.len() <= 128).then(|| version.to_owned());
            }
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

fn installed(root: &Path) -> (bool, String) {
    let binary = root.join(XTRACTOR_BINARY);
    if !binary.is_file() {
        return (false, String::new());
    }
    let version = version_of(&binary)
        .or_else(|| {
            std::fs::read(root.join("xtractor-version.json"))
                .ok()
                .filter(|bytes| bytes.len() < 16 * 1024)
                .and_then(|data| serde_json::from_slice::<Version>(&data).ok())
                .filter(|version| {
                    !version.sha256.is_empty()
                        && hash(&binary).ok().as_deref() == Some(version.sha256.as_str())
                })
                .map(|version| version.version)
        })
        .unwrap_or_default();
    (true, version)
}

fn update_available(installed: &str, latest: &str) -> bool {
    if installed.is_empty() {
        return true;
    }
    let numbers = |version: &str| {
        version
            .trim_start_matches(|c: char| !c.is_ascii_digit())
            .split(|c: char| !c.is_ascii_digit())
            .filter(|part| !part.is_empty())
            .filter_map(|part| part.parse::<u64>().ok())
            .collect::<Vec<_>>()
    };
    let mut current = numbers(installed);
    let mut newest = numbers(latest);
    if current.is_empty() || newest.is_empty() {
        return installed != latest;
    }
    let length = current.len().max(newest.len());
    current.resize(length, 0);
    newest.resize(length, 0);
    newest > current
}

fn publish(
    ui: &Option<crate::ui::runtime::Weak<AppWindow>>,
    status: &str,
    progress: f32,
    detail: String,
) {
    if let Some(ui) = ui {
        let status = status.to_owned();
        let _ = ui.upgrade_in_event_loop(move |app| {
            app.set_xtractor_status(status.clone());
            app.set_xtractor_progress(progress);
            app.set_xtractor_detail(detail);
            if app.get_is_busy() {
                app.set_status(status);
                app.set_progress(progress);
            }
        });
    }
}

fn extract(archive: &Path, staging: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive).map_err(|error| error.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(|error| error.to_string())?;
        let Some(path) = entry.enclosed_name() else {
            continue;
        };
        if entry.is_dir() || path.file_name().is_none_or(|name| name != XTRACTOR_BINARY) {
            continue;
        }
        if entry.size() == 0 || entry.size() > MAX_BINARY {
            return Err("Invalid xtractor binary size.".into());
        }
        let mut output = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(staging)
            .map_err(|error| error.to_string())?;
        let size = std::io::copy(&mut entry.by_ref().take(MAX_BINARY + 1), &mut output)
            .map_err(|error| error.to_string())?;
        output.flush().map_err(|error| error.to_string())?;
        if size > MAX_BINARY {
            return Err("Xtractor binary exceeded its size limit.".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(staging, std::fs::Permissions::from_mode(0o755))
                .map_err(|error| error.to_string())?;
        }
        return Ok(());
    }
    Err("Xtractor binary was not found in the archive.".into())
}

async fn validate(path: &Path) -> Result<(), String> {
    crate::updates::worker::validate(path)
        .await
        .map_err(|error| {
            format!("Downloaded binary did not pass xtractor worker validation: {error}")
        })
}

fn replace(root: &Path, staging: &Path, version: Version, key: &str) -> Result<PathBuf, String> {
    let binary = root.join(XTRACTOR_BINARY);
    let metadata = root.join("xtractor-version.json");
    let backup = root.join(format!(".xtractor-{key}.backup"));
    let metadata_backup = root.join(format!(".xtractor-{key}.json.backup"));
    let metadata_stage = root.join(format!(".xtractor-{key}.json.part"));
    let legacy_metadata = version_of(staging).is_none();
    if legacy_metadata {
        std::fs::write(
            &metadata_stage,
            serde_json::to_vec_pretty(&version).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
    }
    let had_binary = binary.exists();
    let had_metadata = metadata.exists();
    let result = (|| {
        if had_binary {
            std::fs::rename(&binary, &backup).map_err(|error| {
                format!("Could not replace xtractor; it may be in use: {error}")
            })?;
        }
        if had_metadata {
            std::fs::rename(&metadata, &metadata_backup).map_err(|error| error.to_string())?;
        }
        std::fs::rename(staging, &binary).map_err(|error| error.to_string())?;
        if legacy_metadata {
            std::fs::rename(&metadata_stage, &metadata).map_err(|error| error.to_string())?;
        }
        Ok(binary.clone())
    })();
    if result.is_err() {
        if backup.exists() {
            let _ = std::fs::remove_file(&binary);
            let _ = std::fs::rename(&backup, &binary);
        } else if !had_binary {
            let _ = std::fs::remove_file(&binary);
        }
        if metadata_backup.exists() {
            let _ = std::fs::remove_file(&metadata);
            let _ = std::fs::rename(&metadata_backup, &metadata);
        }
        let _ = std::fs::remove_file(&metadata_stage);
    } else {
        let _ = std::fs::remove_file(backup);
        let _ = std::fs::remove_file(metadata_backup);
    }
    result
}

async fn install(
    client: &reqwest::Client,
    root: &Path,
    release: (String, Asset),
    ui: Option<crate::ui::runtime::Weak<AppWindow>>,
) -> Result<PathBuf, String> {
    let _serial = INSTALL.lock().await;
    install_archive(client, root, release, ui).await
}

async fn install_archive(
    client: &reqwest::Client,
    root: &Path,
    release: (String, Asset),
    ui: Option<crate::ui::runtime::Weak<AppWindow>>,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(root.join(".xtractor-install.lock"))
        .map_err(|error| error.to_string())?;
    lock.try_lock()
        .map_err(|_| "Xtractor is being updated by another instance.".to_owned())?;
    let key = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let archive = root.join(format!(".xtractor-{key}.zip.part"));
    let staging = root.join(format!(
        ".xtractor-{key}{}",
        if cfg!(windows) { ".exe" } else { ".tmp" }
    ));
    let work = async {
        publish(&ui, "Downloading xtractor…", 0.0, String::new());
        let response = client
            .get(&release.1.browser_download_url)
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?;
        let total = response.content_length();
        if total.is_some_and(|size| size > MAX_ARCHIVE) {
            return Err("Xtractor archive exceeded its size limit.".into());
        }
        let mut file = tokio::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&archive)
            .await
            .map_err(|error| error.to_string())?;
        let mut bytes = response.bytes_stream();
        let mut downloaded = 0;
        let mut digest = Sha256::new();
        let mut last = std::time::Instant::now() - Duration::from_secs(1);
        while let Some(chunk) = bytes.next().await {
            let chunk = chunk.map_err(|error| error.to_string())?;
            downloaded += chunk.len() as u64;
            if downloaded > MAX_ARCHIVE {
                return Err("Xtractor archive exceeded its size limit.".into());
            }
            digest.update(&chunk);
            file.write_all(&chunk)
                .await
                .map_err(|error| error.to_string())?;
            if last.elapsed() >= Duration::from_millis(100) {
                publish(
                    &ui,
                    "Downloading xtractor…",
                    total.map_or(0.0, |total| downloaded as f32 / total.max(1) as f32 * 90.0),
                    total.map_or_else(
                        || downloader::format_bytes(downloaded),
                        |total| {
                            format!(
                                "{} / {}",
                                downloader::format_bytes(downloaded),
                                downloader::format_bytes(total)
                            )
                        },
                    ),
                );
                last = std::time::Instant::now();
            }
        }
        file.flush().await.map_err(|error| error.to_string())?;
        drop(file);
        if let Some(expected) = &release.1.digest {
            let actual = format!("sha256:{:x}", digest.finalize());
            if !actual.eq_ignore_ascii_case(expected) {
                return Err("Xtractor archive checksum mismatch.".into());
            }
        }
        publish(&ui, "Installing xtractor…", 92.0, String::new());
        let input = archive.clone();
        let output = staging.clone();
        tokio::task::spawn_blocking(move || extract(&input, &output))
            .await
            .map_err(|error| error.to_string())??;
        validate(&staging).await?;
        let version = Version {
            version: release.0,
            asset_name: release.1.name,
            sha256: hash(&staging)?,
        };
        let root = root.to_path_buf();
        let stage = staging.clone();
        tokio::task::spawn_blocking(move || replace(&root, &stage, version, &key))
            .await
            .map_err(|error| error.to_string())?
    }
    .await;
    let _ = tokio::fs::remove_file(archive).await;
    let _ = tokio::fs::remove_file(staging).await;
    work
}

async fn ensure_in(
    client: &reqwest::Client,
    root: &Path,
    release: impl std::future::Future<Output = Result<(String, Asset), String>>,
    ui: Option<crate::ui::runtime::Weak<AppWindow>>,
) -> Result<PathBuf, String> {
    let _serial = INSTALL.lock().await;
    let binary = root.join(XTRACTOR_BINARY);
    if binary.is_file() {
        return Ok(binary);
    }
    publish(&ui, "Preparing xtractor download…", 0.0, String::new());
    install_archive(client, root, release.await?, ui).await
}

pub async fn ensure_installed(
    ui: Option<crate::ui::runtime::Weak<AppWindow>>,
) -> Result<PathBuf, String> {
    if let Ok(path) = downloader::resolve_xtractor_path() {
        return Ok(path);
    }
    let root = downloader::app_data_dir().ok_or("Could not locate the xtractor data folder.")?;
    // Dependency setup finishes even if the fetch or token check that requested it is cancelled.
    tokio::spawn(async move {
        if let Some(ui) = &ui {
            let _ = ui.upgrade_in_event_loop(|app| app.set_xtractor_busy(true));
        }
        let result = async {
            let client = client()?;
            ensure_in(&client, &root, latest(&client), ui.clone()).await
        }
        .await;
        let local = tokio::task::spawn_blocking(move || installed(&root))
            .await
            .unwrap_or_default();
        if let Some(ui) = ui {
            let succeeded = result.is_ok();
            let status = match &result {
                Ok(_) => "Xtractor installed successfully.".to_owned(),
                Err(error) => error.clone(),
            };
            let _ = ui.upgrade_in_event_loop(move |app| {
                app.set_xtractor_busy(false);
                app.set_xtractor_installed(local.0);
                app.set_xtractor_version(local.1);
                if local.0 {
                    app.set_xtractor_setup_required(false);
                }
                app.set_xtractor_status(status);
                app.set_xtractor_detail(String::new());
                app.set_xtractor_progress(if succeeded { 100.0 } else { 0.0 });
            });
        }
        result
    })
    .await
    .map_err(|error| error.to_string())?
}

pub fn register(app: &AppWindow) {
    let Some(root) = downloader::app_data_dir() else {
        return;
    };
    register_in(app, root);
}

fn register_in(app: &AppWindow, root: PathBuf) {
    app.set_xtractor_setup_required(!root.join(XTRACTOR_BINARY).is_file());
    let cache = Arc::new(Mutex::new(None::<(String, Asset)>));
    let ui = app.as_weak();
    let path = root.clone();
    tokio::spawn(async move {
        let value = tokio::task::spawn_blocking(move || installed(&path))
            .await
            .unwrap_or_default();
        let _ = ui.upgrade_in_event_loop(move |app| {
            if !app.get_xtractor_busy()
                && app.get_xtractor_latest().is_empty()
                && app.get_xtractor_version().is_empty()
            {
                app.set_xtractor_installed(value.0);
                app.set_xtractor_version(value.1);
                app.set_xtractor_setup_required(!value.0);
            }
        });
    });
    let ui = app.as_weak();
    let cached = cache.clone();
    let path = root.clone();
    app.on_check_xtractor_update(move || {
        let Some(app) = ui.upgrade() else { return };
        if app.get_xtractor_busy() || app.get_is_busy() || app.get_ffmpeg_busy() {
            return;
        }
        app.set_xtractor_busy(true);
        app.set_xtractor_status("Checking xtractor updates…".into());
        let ui = ui.clone();
        let cached = cached.clone();
        let root = path.clone();
        tokio::spawn(async move {
            let result = async { latest(&client()?).await }.await;
            let local = tokio::task::spawn_blocking(move || installed(&root))
                .await
                .unwrap_or_default();
            if let Ok(release) = &result {
                *cached.lock().unwrap() = Some(release.clone());
            }
            let _ = ui.upgrade_in_event_loop(move |app| {
                app.set_xtractor_busy(false);
                app.set_xtractor_installed(local.0);
                app.set_xtractor_version(local.1.clone());
                match result {
                    Ok(release) => {
                        let available = update_available(&local.1, &release.0);
                        app.set_xtractor_latest(release.0);
                        app.set_xtractor_update_available(available);
                        app.set_xtractor_status(
                            if available {
                                "Xtractor release available."
                            } else {
                                "Xtractor is up to date."
                            }
                            .into(),
                        );
                    }
                    Err(error) => {
                        session_log::error(format!("Xtractor update check failed: {error}"));
                        app.set_xtractor_status(
                            "Could not check xtractor updates. Please try again.".into(),
                        );
                    }
                }
            });
        });
    });
    let ui = app.as_weak();
    app.on_update_xtractor(move || {
        let Some(app) = ui.upgrade() else { return };
        if app.get_xtractor_busy() || app.get_is_busy() || app.get_ffmpeg_busy() || app.get_history_busy() { return; }
        app.set_xtractor_busy(true); app.set_xtractor_status("Preparing xtractor update…".into()); app.set_xtractor_progress(0.0); app.set_xtractor_detail("".into());
        let ui = ui.clone(); let cache = cache.clone(); let root = root.clone();
        tokio::spawn(async move {
            let result = async {
                let client = client()?;
                let release = cache.lock().unwrap().clone();
                let release = match release { Some(release) => release, None => latest(&client).await? };
                install(&client, &root, release, Some(ui.clone())).await
            }.await;
            let local = tokio::task::spawn_blocking(move || installed(&root)).await.unwrap_or_default();
            let _ = ui.upgrade_in_event_loop(move |app| {
                app.set_xtractor_busy(false); app.set_xtractor_installed(local.0); app.set_xtractor_version(local.1);
                match result {
                    Ok(_) => { app.set_xtractor_setup_required(false); app.set_xtractor_detail(String::new()); app.set_xtractor_update_available(false); app.set_xtractor_status("Xtractor updated successfully.".into()); app.set_xtractor_progress(100.0); }
                    Err(error) => { session_log::error(format!("Xtractor update failed: {error}")); app.set_xtractor_detail(error); app.set_xtractor_status(if local.0 { "Could not update xtractor. Your previous binary is preserved. Please try again." } else { "Could not install xtractor. Please try again." }.into()); }
                }
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn startup_requires_setup_only_when_the_portable_binary_is_missing() {
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let missing = AppWindow::new().unwrap();
        register_in(&missing, fixture.0.clone());
        assert!(missing.get_xtractor_setup_required());
        std::fs::write(fixture.0.join(XTRACTOR_BINARY), b"existing extractor").unwrap();
        let existing = AppWindow::new().unwrap();
        register_in(&existing, fixture.0.clone());
        assert!(!existing.get_xtractor_setup_required());
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    #[test]
    fn release_assets_match_each_supported_platform_and_reject_other_repositories() {
        for (os, arch, expected) in [
            ("windows", "x86_64", "windows-amd64.zip"),
            ("windows", "aarch64", "windows-amd64.zip"),
            ("linux", "x86_64", "linux-amd64.zip"),
            ("linux", "aarch64", "linux-arm64.zip"),
            ("macos", "x86_64", "macos-amd64.zip"),
            ("macos", "aarch64", "macos-arm64.zip"),
        ] {
            assert_eq!(asset_for(os, arch).unwrap(), expected);
        }
        assert!(asset_for("linux", "x86").is_err());
        for repo in [
            "afkarxyz/xtractor-binaries",
            "afkarxyz/xtractor-bin.evil",
            "nexryx/xtractor-bin",
        ] {
            let release = Release {
                tag_name: "v1.0.0".into(),
                draft: false,
                prerelease: false,
                assets: vec![Asset {
                    name: "windows-amd64.zip".into(),
                    browser_download_url: format!(
                        "https://github.com/{repo}/releases/download/v1.0.0/windows-amd64.zip"
                    ),
                    digest: None,
                }],
            };
            assert!(select_release(vec![release], "windows-amd64.zip").is_err());
        }
    }

    #[tokio::test]
    async fn existing_extractor_skips_release_lookup_and_missing_release_leaves_no_installation() {
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let client = client().unwrap();
        let missing = async { Err("No release published yet".into()) };
        assert_eq!(
            ensure_in(&client, &fixture.0, missing, None)
                .await
                .unwrap_err(),
            "No release published yet"
        );
        assert!(!fixture.0.join(XTRACTOR_BINARY).exists());
        let binary = fixture.0.join(XTRACTOR_BINARY);
        std::fs::write(&binary, b"already installed").unwrap();
        let release = std::future::ready(Err(
            "An installed extractor must not request a release".into()
        ));
        assert_eq!(
            ensure_in(&client, &fixture.0, release, None).await.unwrap(),
            binary
        );
    }

    #[test]
    fn stable_release_selection_and_numeric_versions() {
        let releases = serde_json::from_str(r#"[
            {"tag_name":"v99","draft":true,"assets":[]},
            {"tag_name":"v5-preview","prerelease":true,"assets":[]},
            {"tag_name":"v2","assets":[{"name":"windows-amd64.zip","browser_download_url":"https://github.com/afkarxyz/xtractor-bin/releases/download/v2/windows-amd64.zip"}]}
        ]"#).unwrap();
        assert_eq!(
            select_release(releases, "windows-amd64.zip").unwrap().0,
            "v2"
        );
        assert!(update_available("v1.9", "v1.10"));
        assert!(!update_available("v2.0", "v1.10"));
        assert!(!update_available("v1.4", "v1.4.0"));
        assert!(update_available("", "v1.4"));
    }

    #[test]
    fn installed_version_requires_the_recorded_binary_hash() {
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let binary = fixture.0.join(XTRACTOR_BINARY);
        std::fs::write(&binary, b"old binary").unwrap();
        let version = Version {
            version: "v1.0".into(),
            asset_name: "windows-amd64.zip".into(),
            sha256: hash(&binary).unwrap(),
        };
        std::fs::write(
            fixture.0.join("xtractor-version.json"),
            serde_json::to_vec(&version).unwrap(),
        )
        .unwrap();
        assert_eq!(installed(&fixture.0), (true, "v1.0".into()));
        std::fs::write(&binary, b"different binary").unwrap();
        assert_eq!(installed(&fixture.0), (true, String::new()));
    }

    #[cfg(windows)]
    #[test]
    fn versioned_binary_installs_without_json_and_overrides_stale_legacy_metadata() {
        let source =
            PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/cmd.exe");
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let stage = fixture.0.join("new xtractor.exe");
        std::fs::copy(source, &stage).unwrap();
        let version =
            version_of(&stage).expect("The Windows executable must contain a version resource");
        let old = fixture.0.join(XTRACTOR_BINARY);
        std::fs::write(&old, b"previous binary").unwrap();
        let metadata = fixture.0.join("xtractor-version.json");
        std::fs::write(&metadata, b"stale metadata").unwrap();
        let path = replace(
            &fixture.0,
            &stage,
            Version {
                version: "v9999".into(),
                asset_name: "windows-amd64.zip".into(),
                sha256: hash(&stage).unwrap(),
            },
            "versioned",
        )
        .unwrap();
        assert!(path.is_file());
        assert!(!metadata.exists());
        assert_eq!(installed(&fixture.0), (true, version.clone()));
        std::fs::write(
            &metadata,
            serde_json::to_vec(&Version {
                version: "v9999".into(),
                asset_name: "windows-amd64.zip".into(),
                sha256: hash(&path).unwrap(),
            })
            .unwrap(),
        )
        .unwrap();
        assert_eq!(installed(&fixture.0), (true, version));
        assert!(!fixture.0.join("xtractor-signer.json").exists());
        assert!(crate::platform::binary_version::read(&fixture.0.join("missing.exe")).is_none());
        assert!(!std::fs::read_dir(&fixture.0).unwrap().any(|entry| {
            let name = entry.unwrap().file_name().to_string_lossy().into_owned();
            name.ends_with(".backup") || name.ends_with(".part")
        }));
    }

    #[cfg(windows)]
    #[test]
    fn failed_metadata_publish_restores_the_previous_binary() {
        use std::os::windows::fs::OpenOptionsExt;
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let binary = fixture.0.join(XTRACTOR_BINARY);
        let metadata = fixture.0.join("xtractor-version.json");
        let stage = fixture.0.join("new.exe");
        std::fs::write(&binary, b"original executable").unwrap();
        std::fs::write(&metadata, b"original metadata").unwrap();
        std::fs::write(&stage, b"new executable").unwrap();
        let held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&metadata)
            .unwrap();
        let version = Version {
            version: "v2".into(),
            asset_name: "windows-amd64.zip".into(),
            sha256: hash(&stage).unwrap(),
        };
        assert!(replace(&fixture.0, &stage, version, "test").is_err());
        assert_eq!(std::fs::read(binary).unwrap(), b"original executable");
        assert_eq!(std::fs::read(&metadata).unwrap(), b"original metadata");
        drop(held);
    }

    #[tokio::test]
    async fn failed_archive_download_preserves_the_previous_installation() {
        use tokio::io::AsyncReadExt;
        let fixture = crate::download::saved_files::tests::Fixture::new();
        std::fs::write(fixture.0.join(XTRACTOR_BINARY), b"old binary").unwrap();
        std::fs::write(fixture.0.join("xtractor-version.json"), b"old metadata").unwrap();
        let server = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = server.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (mut socket, _) = server.accept().await.unwrap();
            let mut bytes = [0; 2048];
            assert!(socket.read(&mut bytes).await.unwrap() > 0);
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nbad")
                .await
                .unwrap();
        });
        let asset = Asset {
            name: "windows-amd64.zip".into(),
            browser_download_url: format!("http://{address}/asset"),
            digest: Some(format!("sha256:{}", "0".repeat(64))),
        };
        assert!(
            install(&client().unwrap(), &fixture.0, ("v2".into(), asset), None)
                .await
                .is_err()
        );
        task.await.unwrap();
        assert_eq!(
            std::fs::read(fixture.0.join(XTRACTOR_BINARY)).unwrap(),
            b"old binary"
        );
        assert_eq!(
            std::fs::read(fixture.0.join("xtractor-version.json")).unwrap(),
            b"old metadata"
        );
        assert!(!std::fs::read_dir(&fixture.0).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".part")
        }));
    }

    #[tokio::test]
    #[ignore = "Requires XTRACTOR_TEST_BINARY pointing to a built extractor; uses a local HTTP server"]
    async fn first_use_download_is_shared_and_installs_a_valid_worker_without_bundled_files() {
        use tokio::io::AsyncReadExt;
        let source = PathBuf::from(
            std::env::var_os("XTRACTOR_TEST_BINARY").expect("Set XTRACTOR_TEST_BINARY"),
        );
        let expected_version =
            version_of(&source).expect("The test extractor must report its own version");
        assert_eq!(command_version(&source), Some(expected_version.clone()));
        let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        archive
            .start_file(
                XTRACTOR_BINARY,
                zip::write::SimpleFileOptions::default().unix_permissions(0o755),
            )
            .unwrap();
        archive.write_all(&std::fs::read(&source).unwrap()).unwrap();
        let payload = archive.finish().unwrap().into_inner();
        let digest = format!("sha256:{:x}", Sha256::digest(&payload));
        let server = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = server.local_addr().unwrap();
        let served = tokio::spawn(async move {
            let (mut socket, _) = server.accept().await.unwrap();
            let mut request = [0; 4096];
            assert!(socket.read(&mut request).await.unwrap() > 0);
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        payload.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            socket.write_all(&payload).await.unwrap();
        });
        let fixture = crate::download::saved_files::tests::Fixture::new();
        std::fs::write(fixture.0.join("config.json"), b"fixture config").unwrap();
        std::fs::write(fixture.0.join("batch-queue.json"), b"fixture queue").unwrap();
        std::fs::write(fixture.0.join("xtractor-signer.json"), b"fixture signer").unwrap();
        let client = client().unwrap();
        let asset = Asset {
            name: asset_name().unwrap().into(),
            browser_download_url: format!("http://{address}/archive.zip"),
            digest: Some(digest),
        };
        let (first, second) = tokio::join!(
            ensure_in(
                &client,
                &fixture.0,
                std::future::ready(Ok(("v1.0.0".into(), asset.clone()))),
                None
            ),
            ensure_in(
                &client,
                &fixture.0,
                std::future::ready(Ok(("v1.0.0".into(), asset))),
                None
            ),
        );
        let path = first.unwrap();
        assert_eq!(second.unwrap(), path);
        served.await.unwrap();
        assert_eq!(hash(&source).unwrap(), hash(&path).unwrap());
        assert_eq!(installed(&fixture.0), (true, expected_version));
        assert!(!fixture.0.join("xtractor-version.json").exists());
        assert_eq!(
            std::fs::read(fixture.0.join("config.json")).unwrap(),
            b"fixture config"
        );
        assert_eq!(
            std::fs::read(fixture.0.join("batch-queue.json")).unwrap(),
            b"fixture queue"
        );
        assert_eq!(
            std::fs::read(fixture.0.join("xtractor-signer.json")).unwrap(),
            b"fixture signer"
        );
        assert!(!std::fs::read_dir(&fixture.0).unwrap().any(|entry| {
            let name = entry.unwrap().file_name().to_string_lossy().into_owned();
            name.ends_with(".part") || name.ends_with(".backup")
        }));
    }

    #[tokio::test]
    #[ignore = "downloads the official xtractor asset into an isolated fixture"]
    async fn github_update_validates_and_records_the_binary_without_touching_user_data() {
        let fixture = crate::download::saved_files::tests::Fixture::new();
        std::fs::write(fixture.0.join("config.json"), b"fixture config").unwrap();
        let app = AppWindow::new().unwrap();
        register_in(&app, fixture.0.clone());
        assert!(app.get_xtractor_setup_required());
        app.invoke_update_xtractor();
        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        let ctx = egui::Context::default();
        while app.get_xtractor_busy() && std::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(20)).await;
            crate::ui::runtime::pump(&ctx);
        }
        assert!(!app.get_xtractor_busy(), "Download did not finish");
        assert!(
            app.get_xtractor_installed(),
            "{}: {}",
            app.get_xtractor_status(),
            app.get_xtractor_detail()
        );
        assert!(!app.get_xtractor_setup_required());
        let path = fixture.0.join(XTRACTOR_BINARY);
        validate(&path).await.unwrap();
        let local = installed(&fixture.0);
        assert!(local.0);
        assert_eq!(local.1, app.get_xtractor_version());
        assert_eq!(app.get_xtractor_progress(), 100.0);
        assert_eq!(
            std::fs::read(fixture.0.join("config.json")).unwrap(),
            b"fixture config"
        );
        assert!(!std::fs::read_dir(&fixture.0).unwrap().any(|entry| {
            let name = entry.unwrap().file_name().to_string_lossy().into_owned();
            name.ends_with(".part") || name.ends_with(".backup")
        }));
    }
}
