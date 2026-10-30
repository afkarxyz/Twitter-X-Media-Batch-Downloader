//! One persistent extractor, with bounded frames and exclusive request ownership.
use crate::download::engine::ExtractionOptions;
use crate::fetch::strategy::Request;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::Mutex,
};
static SHUTDOWN: AtomicBool = AtomicBool::new(false);
const FRAME_LIMIT: usize = 32 * 1024 * 1024;
#[derive(Eq, PartialEq)]
struct Binary {
    path: PathBuf,
    length: u64,
    modified: Option<SystemTime>,
    proxy: u64,
}
struct Worker {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    binary: Binary,
    sequence: u64,
    logs: tokio::task::JoinHandle<()>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
        self.logs.abort();
    }
}
#[derive(Default)]
struct Pool {
    worker: Option<Worker>,
}
fn pool() -> &'static Mutex<Pool> {
    static POOL: OnceLock<Mutex<Pool>> = OnceLock::new();
    POOL.get_or_init(Default::default)
}
async fn cancelled(flag: &AtomicBool) {
    while !flag.load(Ordering::Relaxed) && !SHUTDOWN.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
async fn frame(output: &mut BufReader<ChildStdout>) -> Result<Value, String> {
    let mut bytes = Vec::new();
    loop {
        let chunk = output.fill_buf().await.map_err(|e| e.to_string())?;
        if chunk.is_empty() {
            return Err("xtractor worker closed its output.".into());
        }
        let end = chunk
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(chunk.len(), |index| index + 1);
        if bytes.len() + end > FRAME_LIMIT {
            return Err("xtractor worker response exceeds 32 MiB.".into());
        }
        bytes.extend_from_slice(&chunk[..end]);
        let complete = bytes.last() == Some(&b'\n');
        output.consume(end);
        if complete {
            break;
        }
    }
    serde_json::from_slice(&bytes).map_err(|_| "Invalid JSON from xtractor worker.".into())
}
impl Worker {
    async fn spawn(binary: Binary) -> Result<Self, String> {
        let mut command = Command::new(&binary.path);
        crate::network::proxy::apply_child(&mut command);
        command
            .arg("--worker")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        if let Some(directory) = binary.path.parent() {
            command.current_dir(directory);
        }
        #[cfg(windows)]
        command.creation_flags(0x0800_0000);
        Self::spawn_command(binary, command).await
    }
    async fn spawn_command(binary: Binary, mut command: Command) -> Result<Self, String> {
        let mut child = command
            .spawn()
            .map_err(|e| format!("Could not start xtractor worker: {e}"))?;
        let input = child.stdin.take().ok_or("Missing worker stdin")?;
        let output = BufReader::new(child.stdout.take().ok_or("Missing worker stdout")?);
        let mut stderr = BufReader::new(child.stderr.take().ok_or("Missing worker stderr")?);
        let logs = tokio::spawn(async move {
            let mut discarded = Vec::new();
            loop {
                let Ok(bytes) = stderr.fill_buf().await else {
                    break;
                };
                if bytes.is_empty() {
                    break;
                }
                let end = bytes
                    .iter()
                    .position(|byte| *byte == b'\n')
                    .map_or(bytes.len(), |index| index + 1);
                if discarded.len() + end <= 8192 {
                    discarded.extend_from_slice(&bytes[..end]);
                } else {
                    discarded.clear();
                }
                let complete = bytes[end - 1] == b'\n';
                stderr.consume(end);
                if complete {
                    if !discarded.is_empty() {
                        crate::platform::logging::info(format!(
                            "xtractor worker: {}",
                            String::from_utf8_lossy(&discarded).trim()
                        ));
                    }
                    discarded.clear();
                }
            }
        });
        let mut worker = Self {
            child,
            input,
            output,
            binary,
            sequence: 0,
            logs,
        };
        let hello = tokio::time::timeout(Duration::from_secs(3), frame(&mut worker.output))
            .await
            .map_err(|_| "Worker handshake timed out".to_string())??;
        if hello["ready"] != true || hello["protocol"] != 1 {
            return Err("Unsupported xtractor worker protocol".into());
        }
        Ok(worker)
    }
    async fn exchange(
        &mut self,
        options: &ExtractionOptions,
        route: &Request,
        count: usize,
    ) -> Result<Value, String> {
        self.sequence = self.sequence.wrapping_add(1);
        let id = self.sequence.to_string();
        let request = json!({"id":id,"url":route.source,"auth_token":options.auth_token,"cursor":route.cursor,"count":count.clamp(1,100),"type":"all","retweets":route.retweets,"text_tweets":true,"no_videos":false});
        let mut bytes = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
        if bytes.len() > 63 * 1024 {
            return Err("Extractor request is too large.".into());
        }
        bytes.push(b'\n');
        self.input
            .write_all(&bytes)
            .await
            .map_err(|_| "Could not send worker request".to_string())?;
        self.input.flush().await.map_err(|e| e.to_string())?;
        let response = frame(&mut self.output).await?;
        if response["id"].as_str() != Some(&id) {
            return Err("Worker response does not match request".into());
        }
        Ok(response)
    }
}
pub async fn validate(executable: &Path) -> Result<(), String> {
    let metadata = std::fs::metadata(executable).map_err(|error| error.to_string())?;
    let mut worker = Worker::spawn(Binary {
        path: executable.to_owned(),
        length: metadata.len(),
        modified: metadata.modified().ok(),
        proxy: crate::network::proxy::generation(),
    })
    .await?;
    worker.child.kill().await.map_err(|error| error.to_string())
}

pub async fn request(
    executable: &Path,
    options: &ExtractionOptions,
    route: &Request,
    count: usize,
    cancel: Arc<AtomicBool>,
) -> Result<Value, String> {
    let metadata = std::fs::metadata(executable)
        .map_err(|error| format!("Could not read xtractor worker binary: {error}"))?;
    let binary = Binary {
        path: executable.to_owned(),
        length: metadata.len(),
        modified: metadata.modified().ok(),
        proxy: crate::network::proxy::generation(),
    };
    let mut pool = tokio::select! {guard=pool().lock()=>guard,_=cancelled(&cancel)=>return Err("Extraction cancelled.".into())};
    if pool
        .worker
        .as_ref()
        .is_some_and(|worker| worker.binary != binary)
    {
        pool.worker = None;
    }
    if pool.worker.is_none() {
        let worker = tokio::select! {result=Worker::spawn(binary)=>result,_=cancelled(&cancel)=>return Err("Extraction cancelled.".into())}
            .map_err(|error| format!("Could not initialize xtractor worker. Update Xtractor in Settings. {error}"))?;
        pool.worker = Some(worker);
    }
    // Taking ownership prevents cancellation/timeout from leaving an unread response in the pool.
    let mut worker = pool.worker.take().unwrap();
    let result = tokio::select! {
        result=tokio::time::timeout(crate::network::timeouts::fetch(),worker.exchange(options,route,count))=>match result {Ok(result)=>result,Err(_)=>Err("xtractor worker timed out.".into())},
        _=cancelled(&cancel)=>Err("Extraction cancelled.".into()),
    };
    match result {
        Ok(response) => {
            pool.worker = Some(worker);
            decode(response)
        }
        Err(error) => Err(error),
    }
}
fn decode(response: Value) -> Result<Value, String> {
    if let Some(error) = response.get("error").filter(|value| !value.is_null()) {
        return Err(format!(
            "XTRACTOR_ERROR_JSON:{}\nerror: {}",
            error,
            error["message"].as_str().unwrap_or("Extraction failed")
        ));
    }
    response
        .get("data")
        .filter(|value| value.is_object())
        .cloned()
        .ok_or_else(|| "Worker response is missing data".into())
}
pub async fn shutdown() {
    SHUTDOWN.store(true, Ordering::Relaxed);
    pool().lock().await.worker = None;
}
#[cfg(test)]
mod tests {
    #[cfg(windows)]
    async fn fixture(script: &str) -> Result<super::Worker, String> {
        use super::*;
        let mut command = Command::new("powershell.exe");
        command.args(["-NoProfile", "-NonInteractive", "-Command", script]);
        command
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .creation_flags(0x0800_0000);
        let binary = Binary {
            path: PathBuf::from("fixture"),
            length: 0,
            modified: None,
            proxy: 0,
        };
        Worker::spawn_command(binary, command).await
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn worker_rejects_cli_output_and_incompatible_protocols() {
        for (script, expected) in [
            (
                "[Console]::Out.WriteLine('usage: xtractor <url>')",
                "Invalid JSON",
            ),
            (
                "[Console]::Out.WriteLine('{\"ready\":true,\"protocol\":2}')",
                "Unsupported xtractor worker protocol",
            ),
            (
                "[Console]::Out.WriteLine('{\"ready\":false,\"protocol\":1}')",
                "Unsupported xtractor worker protocol",
            ),
        ] {
            let error = fixture(script)
                .await
                .err()
                .expect("Invalid worker accepted");
            assert!(error.contains(expected), "{error}");
        }
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn worker_stream_reuses_one_process_and_poisoned_frames_are_rejected() {
        use super::*;
        let script = r#"
[Console]::Out.WriteLine('{"ready":true,"protocol":1}')
$count=0
while ($null -ne ($line=[Console]::In.ReadLine())) {
  $request=$line | ConvertFrom-Json
  $count++
  if ($request.url -eq 'bad') {[Console]::Out.WriteLine('invalid JSON');continue}
  $reply=@{id=$request.id;data=@{sequence=$count;pid=$PID;cursor=$request.cursor}}
  [Console]::Out.WriteLine(($reply | ConvertTo-Json -Compress -Depth 4))
}
"#;
        let mut worker = fixture(script).await.unwrap();
        let options = ExtractionOptions {
            url: "https://x.com/demo".into(),
            auth_token: "a".repeat(40),
            media_timeline: true,
            media_reposts: false,
            max_posts: 20,
            page_size: 20,
            media_type: "all".into(),
            retweets: "skip".into(),
            no_videos: false,
            text_tweets: false,
            cursor: String::new(),
            previous_json: None,
            refresh_boundary: Default::default(),
        };
        let mut route =
            crate::fetch::strategy::request(&options.url, true, false, "skip", "").unwrap();
        let first = decode(worker.exchange(&options, &route, 20).await.unwrap()).unwrap();
        route.cursor = "next".into();
        let second = decode(worker.exchange(&options, &route, 20).await.unwrap()).unwrap();
        assert_eq!(first["pid"], second["pid"]);
        assert_eq!(second["sequence"], 2);
        assert_eq!(second["cursor"], "next");
        route.source = "bad".into();
        assert!(
            worker
                .exchange(&options, &route, 20)
                .await
                .unwrap_err()
                .contains("Invalid JSON")
        );
    }
    #[tokio::test]
    async fn shutdown_signals_inflight_requests_before_waiting_for_the_pool() {
        let guard = super::pool().lock().await;
        let active = tokio::spawn(async move {
            let flag = std::sync::atomic::AtomicBool::new(false);
            super::cancelled(&flag).await;
            drop(guard);
        });
        tokio::time::timeout(std::time::Duration::from_secs(1), super::shutdown())
            .await
            .expect("Shutdown waited on a request without cancelling it");
        active.await.unwrap();
        super::SHUTDOWN.store(false, std::sync::atomic::Ordering::Relaxed);
    }
    #[test]
    fn decoding_preserves_retry_metadata_and_rejects_missing_payloads() {
        let error=super::decode(serde_json::json!({"error":{"kind":"rate_limited","message":"limited","retry_after_seconds":5}})).unwrap_err();
        assert!(error.contains("XTRACTOR_ERROR_JSON:") && error.contains("retry_after_seconds"));
        assert!(super::decode(serde_json::json!({"id":"1"})).is_err());
        assert!(super::decode(serde_json::json!({"data":{"accounts":[]}})).is_ok());
    }
}
