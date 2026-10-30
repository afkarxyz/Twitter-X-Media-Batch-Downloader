use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::process::Command;

static VALIDATED: Mutex<Option<PathBuf>> = Mutex::new(None);

#[derive(Clone, Copy, Default)]
pub struct Options {
    quality: i32,
    resolution: i32,
}

impl Options {
    pub fn from_indices(quality: i32, resolution: i32) -> Self {
        Self {
            quality: if quality == 1 { 1 } else { 0 },
            resolution: if (0..=3).contains(&resolution) {
                resolution
            } else {
                0
            },
        }
    }

    pub fn from_values(quality: &str, resolution: &str) -> Self {
        Self::from_indices(
            i32::from(quality == "better"),
            ["original", "high", "medium", "low"]
                .iter()
                .position(|value| *value == resolution)
                .unwrap_or(0) as i32,
        )
    }

    pub fn quality_index(self) -> i32 {
        self.quality
    }
    pub fn resolution_index(self) -> i32 {
        self.resolution
    }
    pub fn quality_value(self) -> &'static str {
        if self.quality == 1 { "better" } else { "fast" }
    }
    pub fn resolution_value(self) -> &'static str {
        ["original", "high", "medium", "low"][self.resolution as usize]
    }

    fn add_filters(self, command: &mut Command) {
        let width = [None, Some(800), Some(600), Some(400)][self.resolution as usize];
        if self.quality == 0 {
            if let Some(width) = width {
                command.args(["-vf", &format!("scale={width}:-1")]);
            }
        } else {
            let scale = width
                .map(|width| format!("scale={width}:-1:flags=lanczos,"))
                .unwrap_or_default();
            let filter = format!(
                "{scale}split[s0][s1];[s0]palettegen=stats_mode=full[p];[s1][p]paletteuse=dither=sierra2_4a"
            );
            let fps = match self.resolution {
                2 => "10",
                3 => "8",
                _ => "15",
            };
            command.args(["-filter_complex", &filter, "-r", fps]);
        }
    }
}

pub fn prefer_executable(path: PathBuf) {
    *VALIDATED.lock().unwrap() = Some(path);
}

pub fn candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(executable) = std::env::current_exe()
        && let Some(parent) = executable.parent()
    {
        paths.push(parent.join(".twitterxmediabatchdownloader/ffmpeg.exe"));
        paths.push(parent.join("ffmpeg.exe"));
    }
    if let Some(directory) = crate::download::engine::app_data_dir() {
        paths.push(directory.join("ffmpeg.exe"));
    }
    if let Some(path) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&path) {
            paths.push(directory.join(if cfg!(windows) {
                "ffmpeg.exe"
            } else {
                "ffmpeg"
            }));
        }
    }
    paths
}

pub fn executable() -> Result<PathBuf, String> {
    if let Some(path) = VALIDATED
        .lock()
        .unwrap()
        .as_ref()
        .filter(|path| path.is_file())
    {
        return Ok(path.clone());
    }
    candidates()
        .into_iter()
        .find(|path| path.is_file())
        .ok_or_else(|| "FFmpeg is required for GIF conversion.".into())
}

fn command(executable: &Path) -> Command {
    let mut command = Command::new(executable);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    command
}

pub async fn convert_download(
    input: &Path,
    options: Options,
    cancel: Arc<AtomicBool>,
) -> Result<PathBuf, String> {
    let output = convert_file(input, options, cancel).await?;
    // Only remove the MP4 that this download created after conversion succeeds.
    let _ = tokio::fs::remove_file(input).await;
    Ok(output)
}

/// Produce a GIF while preserving the source. Archive callers decide when to
/// remove originals after their history transaction has committed.
pub async fn convert_file(
    input: &Path,
    options: Options,
    cancel: Arc<AtomicBool>,
) -> Result<PathBuf, String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("GIF conversion cancelled.".into());
    }
    static CONVERSIONS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);
    let _permit = loop {
        tokio::select! {
            permit = CONVERSIONS.acquire() => {break permit.map_err(|e|e.to_string())?;},
            _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                if cancel.load(Ordering::Relaxed) {return Err("GIF conversion cancelled.".into());}
            }
        }
    };
    if cancel.load(Ordering::Relaxed) {
        return Err("GIF conversion cancelled.".into());
    }
    let executable = executable()?;
    let mut suffix = 0;
    let output;
    let temporary;
    loop {
        let stem = input.file_stem().unwrap_or_default().to_string_lossy();
        let filename = if suffix == 0 {
            format!("{stem}.gif")
        } else {
            format!("{stem} ({suffix}).gif")
        };
        let candidate = input.with_file_name(filename);
        let part = candidate.with_extension("gif.part");
        if candidate.exists() {
            suffix += 1;
            continue;
        }
        match tokio::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&part)
            .await
        {
            Ok(file) => {
                drop(file);
                output = candidate;
                temporary = part;
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                suffix += 1;
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    let mut conversion = command(&executable);
    conversion
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(input);
    options.add_filters(&mut conversion);
    let result = run(
        conversion.args(["-loop", "0", "-f", "gif"]).arg(&temporary),
        cancel,
    )
    .await;
    if let Err(error) = result {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(error);
    }
    // Publish without replacing a concurrently created output. Filesystems
    // without hard links (e.g. portable exFAT drives) use exclusive creation.
    if let Err(error) = tokio::fs::hard_link(&temporary, &output).await {
        let publication = async {
            let mut destination = tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&output)
                .await?;
            let copied = async {
                let mut source = tokio::fs::File::open(&temporary).await?;
                tokio::io::copy(&mut source, &mut destination).await?;
                destination.sync_all().await
            }
            .await;
            drop(destination);
            if copied.is_err() {
                let _ = tokio::fs::remove_file(&output).await;
            }
            copied
        }
        .await;
        if let Err(copy_error) = publication {
            let _ = tokio::fs::remove_file(&temporary).await;
            return Err(format!("Could not save GIF: {copy_error} ({error})"));
        }
    }
    let _ = tokio::fs::remove_file(&temporary).await;
    Ok(output)
}

async fn run(command: &mut Command, cancel: Arc<AtomicBool>) -> Result<(), String> {
    let mut child = command.spawn().map_err(|error| error.to_string())?;
    let stderr = child.stderr.take().unwrap();
    let read_error = tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut reader = stderr;
        let mut bytes = Vec::new();
        let _ = reader.read_to_end(&mut bytes).await;
        bytes
    });
    let outcome = tokio::select! {
        status = child.wait() => status.map_err(|error| error.to_string()),
        _ = async { loop { if cancel.load(Ordering::Relaxed) { break; } tokio::time::sleep(std::time::Duration::from_millis(50)).await; } } => {
            let _ = child.kill().await; let _ = child.wait().await; Err("Download cancelled.".into())
        },
    };
    let bytes = read_error.await.unwrap_or_default();
    match outcome {
        Ok(status) if status.success() => Ok(()),
        Ok(_) => Err(format!(
            "GIF conversion failed: {}",
            String::from_utf8_lossy(&bytes).trim()
        )),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn archive_conversion_keeps_source_avoids_collisions_and_honors_cancellation() {
        if executable().is_err() {
            return;
        }
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let input = fixture.0.join("archive.mp4");
        let existing = fixture.0.join("archive.gif");
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/preview.mp4"),
            &input,
        )
        .unwrap();
        let original = std::fs::read(&input).unwrap();
        std::fs::write(&existing, b"keep existing").unwrap();
        let output = convert_file(&input, Options::default(), Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();
        assert_ne!(output, existing);
        assert_eq!(std::fs::read(&input).unwrap(), original);
        assert_eq!(std::fs::read(&existing).unwrap(), b"keep existing");
        assert!(std::fs::read(&output).unwrap().starts_with(b"GIF"));
        assert!(
            convert_file(&input, Options::default(), Arc::new(AtomicBool::new(true)))
                .await
                .is_err()
        );
        assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 3);
    }
    #[tokio::test]
    async fn converts_both_modes_at_every_resolution_to_animated_gif() {
        use image::AnimationDecoder;
        use std::io::BufReader;
        if executable().is_err() {
            return;
        }
        let fixture = crate::download::saved_files::tests::Fixture::new();
        for quality in ["fast", "better"] {
            for (resolution, dimensions, better_fps) in [
                ("original", (160, 96), 15.0),
                ("high", (800, 480), 15.0),
                ("medium", (600, 360), 10.0),
                ("low", (400, 240), 8.0),
            ] {
                let input = fixture.0.join(format!("{quality}-{resolution}.mp4"));
                tokio::fs::copy(
                    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/preview.mp4"),
                    &input,
                )
                .await
                .unwrap();
                let output = convert_download(
                    &input,
                    Options::from_values(quality, resolution),
                    Arc::new(AtomicBool::new(false)),
                )
                .await
                .unwrap();
                let decoder = image::codecs::gif::GifDecoder::new(BufReader::new(
                    std::fs::File::open(&output).unwrap(),
                ))
                .unwrap();
                let frames = decoder.into_frames().collect_frames().unwrap();
                assert!(frames.len() > 1, "{quality}/{resolution} must be animated");
                assert_eq!(frames[0].buffer().dimensions(), dimensions);
                let delays = frames
                    .iter()
                    .take(frames.len() - 1)
                    .map(|frame| {
                        let (numerator, denominator) = frame.delay().numer_denom_ms();
                        numerator as f64 / denominator as f64
                    })
                    .collect::<Vec<_>>();
                let average = delays.iter().sum::<f64>() / delays.len() as f64;
                let expected_delay = 1000.0 / if quality == "fast" { 20.0 } else { better_fps };
                assert!(
                    (average - expected_delay).abs() < expected_delay * 0.2,
                    "Unexpected frame timing in {quality}/{resolution}: {average} ms"
                );
                assert!(!input.exists());
            }
        }
    }

    #[tokio::test]
    async fn failed_conversion_preserves_original_and_existing_gif() {
        if executable().is_err() {
            return;
        }
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let input = fixture.0.join("sample.mp4");
        let existing = fixture.0.join("sample.gif");
        std::fs::write(&input, b"invalid MP4").unwrap();
        std::fs::write(&existing, b"keep existing GIF").unwrap();
        assert!(
            convert_download(&input, Options::default(), Arc::new(AtomicBool::new(false)))
                .await
                .is_err()
        );
        assert_eq!(std::fs::read(&input).unwrap(), b"invalid MP4");
        assert_eq!(std::fs::read(&existing).unwrap(), b"keep existing GIF");
        assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 2);
    }
}
