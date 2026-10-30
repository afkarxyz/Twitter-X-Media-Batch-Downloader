use crate::AppWindow;
use crate::UpdateInfo;
use serde_json::Value;
use std::{process::Stdio, time::Duration};
const APP_API: &str =
    "https://api.github.com/repos/afkarxyz/Twitter-X-Media-Batch-Downloader/releases/latest";
const APP_RELEASES: &str = "https://github.com/afkarxyz/Twitter-X-Media-Batch-Downloader/releases";
const FFMPEG_API: &str = "https://api.github.com/repos/spotbye/Dependencies/releases?per_page=100";
const FFMPEG_RELEASES: &str = "https://github.com/spotbye/Dependencies/releases";
const EXIF_API: &str = "https://sourceforge.net/projects/exiftool/best_release.json";
const EXIF_RELEASES: &str = "https://sourceforge.net/projects/exiftool/files/";

fn version(value: &str) -> Option<Vec<u64>> {
    let value = value
        .strip_prefix("FFmpeg-")
        .unwrap_or(value)
        .trim_start_matches('v');
    let value = value.split('-').next()?;
    if value.is_empty() {
        return None;
    }
    value
        .split('.')
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()
}
fn compare(current: &str, latest: &str) -> Option<std::cmp::Ordering> {
    let mut current = version(current)?;
    let mut latest = version(latest)?;
    let len = current.len().max(latest.len());
    current.resize(len, 0);
    latest.resize(len, 0);
    Some(current.cmp(&latest))
}
fn row(
    name: &str,
    current: Option<String>,
    latest: Result<String, String>,
    url: &str,
) -> UpdateInfo {
    let installed = current.clone().unwrap_or_else(|| "Not installed".into());
    match latest {
        Err(error) => UpdateInfo {
            name: name.into(),
            installed,
            latest: "—".into(),
            status: format!("Check failed: {error}"),
            url: url.into(),
        },
        Ok(latest) => {
            let status = match current
                .as_deref()
                .and_then(|current| compare(current, &latest))
            {
                Some(std::cmp::Ordering::Less) => "Update available",
                Some(std::cmp::Ordering::Equal) => "Up to date",
                Some(std::cmp::Ordering::Greater) => {
                    "Installed version is newer than the published release"
                }
                None => {
                    if current.is_none() {
                        "Not installed"
                    } else {
                        "Version could not be compared"
                    }
                }
            };
            UpdateInfo {
                name: name.into(),
                installed,
                latest,
                status: status.into(),
                url: url.into(),
            }
        }
    }
}
async fn json(client: &reqwest::Client, url: &str) -> Result<Value, String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    if response
        .content_length()
        .is_some_and(|size| size > 4 * 1024 * 1024)
    {
        return Err("Release response is too large.".into());
    }
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err("Release response is too large.".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
async fn installed_ffmpeg() -> Option<String> {
    for path in crate::download::gifs::candidates() {
        if !path.is_file() {
            continue;
        }
        let mut command = tokio::process::Command::new(&path);
        command
            .arg("-version")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(0x08000000);
        if let Ok(Ok(output)) = tokio::time::timeout(Duration::from_secs(5), command.output()).await
            && output.status.success()
            && let Some(value) = String::from_utf8_lossy(&output.stdout)
                .strip_prefix("ffmpeg version ")
                .and_then(|line| line.split_whitespace().next())
        {
            return Some(value.to_owned());
        }
    }
    None
}
fn app_latest(data: &Value) -> Result<String, String> {
    if data.get("draft").and_then(Value::as_bool) == Some(true)
        || data.get("prerelease").and_then(Value::as_bool) == Some(true)
    {
        return Err("No stable application release was returned.".into());
    }
    data.get("tag_name")
        .and_then(Value::as_str)
        .filter(|tag| version(tag).is_some())
        .map(str::to_owned)
        .ok_or_else(|| "Invalid application release version.".into())
}
fn ffmpeg_latest(data: &Value) -> Result<String, String> {
    let asset = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", _) => "ffmpeg-windows.zip",
        ("linux", "aarch64") => "ffmpeg-linux-arm64v8.zip",
        ("linux", _) => "ffmpeg-linux-amd64.zip",
        ("macos", "aarch64") => "ffmpeg-macos-arm64.zip",
        _ => "ffmpeg-macos-amd64.zip",
    };
    data.as_array()
        .into_iter()
        .flatten()
        .find_map(|release| {
            if release.get("draft").and_then(Value::as_bool) == Some(true)
                || release.get("prerelease").and_then(Value::as_bool) == Some(true)
            {
                return None;
            }
            let tag = release.get("tag_name")?.as_str()?;
            if !tag.starts_with("FFmpeg-")
                || version(tag).is_none()
                || !release
                    .get("assets")?
                    .as_array()?
                    .iter()
                    .any(|entry| entry.get("name").and_then(Value::as_str) == Some(asset))
            {
                return None;
            }
            Some(tag.to_owned())
        })
        .ok_or_else(|| "No compatible stable FFmpeg release was found.".into())
}
fn exif_latest(data: &Value) -> Result<String, String> {
    let release = data.get("release").ok_or("Missing ExifTool release.")?;
    let name = release
        .get("filename")
        .and_then(Value::as_str)
        .ok_or("Missing ExifTool filename.")?
        .trim_start_matches('/');
    let value = name
        .strip_prefix("exiftool-")
        .and_then(|name| {
            name.strip_suffix("_64.zip")
                .or_else(|| name.strip_suffix("_32.zip"))
        })
        .or_else(|| {
            name.strip_prefix("Image-ExifTool-")
                .and_then(|name| name.strip_suffix(".tar.gz"))
        })
        .filter(|version_name| version(version_name).is_some())
        .ok_or("Invalid ExifTool release version.")?;
    Ok(value.into())
}
pub async fn check() -> Result<Vec<UpdateInfo>, String> {
    let client = crate::network::proxy::builder()
        .user_agent("TwitterXMediaBatchDownloader/6.0")
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let (app, ffmpeg, exif, installed_ffmpeg, installed_exif) = tokio::join!(
        json(&client, APP_API),
        json(&client, FFMPEG_API),
        json(&client, EXIF_API),
        installed_ffmpeg(),
        crate::download::metadata::installed_version()
    );
    Ok(vec![
        row(
            "Application",
            Some(env!("CARGO_PKG_VERSION").into()),
            app.and_then(|data| app_latest(&data)),
            APP_RELEASES,
        ),
        row(
            "FFmpeg",
            installed_ffmpeg,
            ffmpeg.and_then(|data| ffmpeg_latest(&data)),
            FFMPEG_RELEASES,
        ),
        row(
            "ExifTool",
            installed_exif,
            exif.and_then(|data| exif_latest(&data)),
            EXIF_RELEASES,
        ),
    ])
}
/// Check only application releases at launch; offline checks remain silent.
pub fn startup(app: &AppWindow) {
    if !app.get_notify_updates() {
        return;
    }
    let ui = app.as_weak();
    tokio::spawn(async move {
        let result = async {
            let client = crate::network::proxy::builder()
                .user_agent("TwitterXMediaBatchDownloader/6.0")
                .timeout(Duration::from_secs(15))
                .build()
                .map_err(|e| e.to_string())?;
            app_latest(&json(&client, APP_API).await?)
        }
        .await;
        if let Ok(latest) = result
            && compare(env!("CARGO_PKG_VERSION"), &latest) == Some(std::cmp::Ordering::Less)
        {
            let _ = ui.upgrade_in_event_loop(move |app| {
                if app.get_notify_updates() {
                    app.set_startup_update_version(latest);
                }
            });
        }
    });
}
pub fn register(app: &AppWindow) {
    let ui = app.as_weak();
    app.on_check_all_updates(move || {
        let Some(app) = ui.upgrade() else {
            return;
        };
        if app.get_updates_busy() {
            return;
        }
        app.set_updates_busy(true);
        app.set_updates_error("".into());
        let ui = ui.clone();
        tokio::spawn(async move {
            let result = check().await;
            let _ = ui.upgrade_in_event_loop(move |app| {
                app.set_updates_busy(false);
                match result {
                    Ok(rows) => app.set_update_items(crate::ui::runtime::ModelRc::new(
                        crate::ui::runtime::VecModel::from(rows),
                    )),
                    Err(error) => app.set_updates_error(error),
                }
            });
        });
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions_and_failed_checks_are_not_confused_with_up_to_date() {
        assert_eq!(
            compare("6.0.0", "v5.0.1"),
            Some(std::cmp::Ordering::Greater)
        );
        assert_eq!(
            compare("7.1.2", "FFmpeg-9.0"),
            Some(std::cmp::Ordering::Less)
        );
        assert_eq!(compare("13.59", "13.59.0"), Some(std::cmp::Ordering::Equal));
        assert!(
            row("FFmpeg", Some("7.0".into()), Err("offline".into()), "url")
                .status
                .starts_with("Check failed")
        );
    }
    #[test]
    fn startup_notice_requires_a_newer_stable_release() {
        let latest =
            app_latest(&serde_json::json!({"tag_name":"v99.0.0","draft":false,"prerelease":false}))
                .unwrap();
        assert_eq!(
            compare(env!("CARGO_PKG_VERSION"), &latest),
            Some(std::cmp::Ordering::Less)
        );
        assert!(app_latest(&serde_json::json!({"tag_name":"v99.0.0","draft":true})).is_err());
        assert!(app_latest(&serde_json::json!({"tag_name":"v99.0.0","prerelease":true})).is_err());
        assert!(app_latest(&serde_json::json!({"tag_name":"unexpected"})).is_err());
        assert!(compare("6.0.0", "v6.0.0") == Some(std::cmp::Ordering::Equal));
    }
    #[tokio::test]
    #[ignore = "checks official application and tool release APIs"]
    async fn official_update_endpoints_return_rows() {
        let rows = check().await.unwrap();
        assert_eq!(rows.len(), 3);
        for row in rows {
            assert!(!row.name.is_empty());
            eprintln!("{} · latest {} · {}", row.name, row.latest, row.status);
        }
    }
}
