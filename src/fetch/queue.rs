use crate::download::engine::ExtractionProfile;
use crate::fetch::request::{self as fetch_request, DateRange};
use serde::{Deserialize, Serialize};
use std::{
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::mpsc,
    thread::JoinHandle,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Row {
    pub source: String,
    pub status: String,
    pub history_id: String,
    pub media_count: usize,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub version: u8,
    pub enabled: bool,
    #[serde(default)]
    pub from_history: bool,
    pub entries: Vec<Row>,
    pub profiles: Vec<ExtractionProfile>,
}

impl Snapshot {
    fn validate(&self) -> Result<(), String> {
        if self.version != 1
            || self.entries.len() > 100
            || self.entries.len() != self.profiles.len()
        {
            return Err("Unsupported batch queue format.".into());
        }
        for (row, profile) in self.entries.iter().zip(&self.profiles) {
            let source = fetch_request::account_source(&row.source)?;
            let range = DateRange {
                enabled: profile.date_filter_enabled,
                start: profile.start_date.clone(),
                end: profile.end_date.clone(),
            };
            if (!profile.input_url.is_empty() && profile.input_url != source)
                || source != row.source
                || profile.url
                    != range.url(
                        &source,
                        &profile.media_type,
                        &profile.retweets,
                        profile.text_tweets,
                    )?
                || !matches!(
                    profile.media_type.as_str(),
                    "all" | "photo" | "video" | "animated_gif"
                )
                || !matches!(profile.retweets.as_str(), "skip" | "include" | "original")
                || profile.page_size != crate::download::engine::POSTS_PER_REQUEST
                || !matches!(
                    row.status.as_str(),
                    "Queued" | "Fetching" | "Completed" | "Incomplete" | "Failed" | "Cancelled"
                )
            {
                return Err("Invalid account or filters in the saved batch queue.".into());
            }
        }
        Ok(())
    }
}

pub fn load(path: &Path) -> Result<Option<Snapshot>, String> {
    match std::fs::metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
        Ok(metadata) if metadata.len() > 1024 * 1024 => {
            return Err("Saved batch queue is too large.".into());
        }
        _ => {}
    }
    let mut data = Vec::new();
    std::fs::File::open(path)
        .map_err(|error| error.to_string())?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut data)
        .map_err(|error| error.to_string())?;
    if data.len() > 1024 * 1024 {
        return Err("Saved batch queue is too large.".into());
    }
    let snapshot: Snapshot = serde_json::from_slice(&data).map_err(|error| error.to_string())?;
    snapshot.validate()?;
    Ok(Some(snapshot))
}

fn save(path: &Path, snapshot: &Snapshot) -> io::Result<()> {
    let temporary = path.with_extension(format!("json.part.{}", std::process::id()));
    let result = (|| {
        std::fs::create_dir_all(
            path.parent()
                .ok_or_else(|| io::Error::other("Missing queue directory"))?,
        )?;
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(&serde_json::to_vec(snapshot)?)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

pub type Sender = mpsc::Sender<Option<Snapshot>>;
pub struct Guard {
    sender: Sender,
    worker: Option<JoinHandle<()>>,
}
impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.sender.send(None);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn writer(
    path: PathBuf,
    ui: Option<crate::ui::runtime::Weak<crate::AppWindow>>,
) -> io::Result<(Sender, Guard)> {
    let (sender, receiver) = mpsc::channel::<Option<Snapshot>>();
    let worker = std::thread::Builder::new()
        .name("twitterdl-batch-queue".into())
        .spawn(move || {
            while let Ok(Some(snapshot)) = receiver.recv() {
                if let Err(error) = save(&path, &snapshot) {
                    let message = format!("Could not save batch queue: {error}");
                    crate::platform::logging::error(&message);
                    if let Some(ui) = &ui {
                        let _ = ui.upgrade_in_event_loop(move |app| {
                            crate::app::notifications::show_toast(&app, message, "error")
                        });
                    }
                }
            }
        })?;
    Ok((
        sender.clone(),
        Guard {
            sender,
            worker: Some(worker),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queue_writer_drains_on_exit_and_rejects_invalid_inputs() {
        let fixture = crate::download::saved_files::tests::Fixture::new();
        let path = fixture.0.join("queue.json");
        let mut snapshot = Snapshot {
            version: 1,
            enabled: true,
            from_history: false,
            entries: vec![Row {
                source: "https://x.com/demo".into(),
                status: "Fetching".into(),
                history_id: "1".into(),
                media_count: 3,
            }],
            profiles: vec![ExtractionProfile {
                url: "https://x.com/demo".into(),
                page_size: 100,
                media_type: "all".into(),
                retweets: "skip".into(),
                ..Default::default()
            }],
        };
        let (sender, guard) = writer(path.clone(), None).unwrap();
        sender.send(Some(snapshot.clone())).unwrap();
        snapshot.entries[0].status = "Completed".into();
        sender.send(Some(snapshot.clone())).unwrap();
        drop(guard);
        assert_eq!(load(&path).unwrap().unwrap().entries[0].status, "Completed");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("auth_token") && !text.contains("error"));
        snapshot.profiles[0].url = "https://external.example/demo".into();
        save(&path, &snapshot).unwrap();
        assert!(load(&path).is_err());
        snapshot.enabled = false;
        snapshot.entries.clear();
        snapshot.profiles.clear();
        save(&path, &snapshot).unwrap();
        assert!(load(&path).unwrap().unwrap().entries.is_empty());
        std::fs::write(&path, b"broken json").unwrap();
        assert!(load(&path).is_err());
    }
}
