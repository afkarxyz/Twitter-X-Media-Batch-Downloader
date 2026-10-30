//! A per-preview loopback stream adds X's Referer header and forwards Range
//! requests. Closing the preview cancels the listener and all active streams.
use futures_util::StreamExt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::{JoinHandle, JoinSet},
};

pub(super) struct ProxyServer {
    pub uri: String,
    stop: Option<oneshot::Sender<()>>,
    _task: JoinHandle<()>,
}

impl ProxyServer {
    pub async fn start(url: String, generation: u64) -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|error| error.to_string())?;
        let address = listener.local_addr().map_err(|error| error.to_string())?;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = format!("/preview/{generation}-{nonce}/video.mp4");
        let uri = format!("http://{address}{path}");
        let client = crate::network::proxy::builder()
            .user_agent("Mozilla/5.0 TwitterXMediaBatchDownloader/1.0")
            .connect_timeout(crate::network::timeouts::connect())
            .read_timeout(crate::network::timeouts::http())
            .build()
            .map_err(|error| error.to_string())?;
        let (stop, mut stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            let mut connections = JoinSet::new();
            loop {
                tokio::select! {
                    _ = &mut stopped => break,
                    accepted = listener.accept() => {
                        let Ok((socket, _)) = accepted else { break };
                        let client = client.clone(); let url = url.clone(); let path = path.clone();
                        connections.spawn(async move {
                            let _ = serve(socket, client, &url, &path).await;
                        });
                    },
                    _ = connections.join_next(), if !connections.is_empty() => {}
                }
            }
            connections.abort_all();
            while connections.join_next().await.is_some() {}
        });
        Ok(Self {
            uri,
            stop: Some(stop),
            _task: task,
        })
    }
}

impl Drop for ProxyServer {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        // The stop branch drops the JoinSet, including upstream response bodies.
        // Leave it scheduled instead of aborting before it can cancel its children.
    }
}

async fn serve(
    mut socket: TcpStream,
    client: reqwest::Client,
    url: &str,
    path: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut request = Vec::new();
    let mut chunk = [0u8; 2048];
    loop {
        let count =
            tokio::time::timeout(Duration::from_secs(10), socket.read(&mut chunk)).await??;
        if count == 0 {
            return Ok(());
        }
        request.extend_from_slice(&chunk[..count]);
        if request.len() > 16_384 {
            return Ok(());
        }
        if request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            break;
        }
    }
    let request = std::str::from_utf8(&request)?;
    let mut lines = request.split("\r\n");
    let mut first = lines.next().unwrap_or_default().split_whitespace();
    let method = first.next().unwrap_or_default();
    let target = first.next().unwrap_or_default();
    if target != path || !matches!(method, "GET" | "HEAD") {
        socket
            .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await?;
        return Ok(());
    }
    let mut upstream = client
        .request(
            if method == "HEAD" {
                reqwest::Method::HEAD
            } else {
                reqwest::Method::GET
            },
            url,
        )
        .header(reqwest::header::REFERER, "https://x.com/")
        .header(reqwest::header::ACCEPT_ENCODING, "identity");
    if let Some(range) = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("range"))
        .map(|(_, value)| value.trim())
    {
        upstream = upstream.header(reqwest::header::RANGE, range);
    }
    let response = match upstream.send().await {
        Ok(response) => response,
        Err(_) => {
            socket
                .write_all(
                    b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await?;
            return Ok(());
        }
    };
    let status = response.status();
    let mut headers = format!(
        "HTTP/1.1 {} {}\r\nConnection: close\r\n",
        status.as_u16(),
        status.canonical_reason().unwrap_or("Response")
    );
    for name in [
        "content-type",
        "content-length",
        "content-range",
        "accept-ranges",
    ] {
        if let Some(value) = response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
        {
            headers.push_str(&format!("{name}: {value}\r\n"));
        }
    }
    headers.push_str("\r\n");
    socket.write_all(headers.as_bytes()).await?;
    if method == "GET" {
        let mut stream = response.bytes_stream();
        while let Some(bytes) = stream.next().await {
            socket.write_all(&bytes?).await?;
        }
    }
    socket.shutdown().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn forwards_range_head_and_referer_then_stops() {
        let upstream = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = upstream.local_addr().unwrap();
        let upstream_task = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut socket, _) = upstream.accept().await.unwrap();
                let mut request = [0; 4096];
                let length = socket.read(&mut request).await.unwrap();
                let request = String::from_utf8_lossy(&request[..length]).to_lowercase();
                assert!(request.contains("referer: https://x.com/"));
                if request.starts_with("head ") {
                    socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\nContent-Type: video/mp4\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n").await.unwrap();
                } else {
                    assert!(request.contains("range: bytes=4-7"));
                    socket.write_all(b"HTTP/1.1 206 Partial Content\r\nContent-Length: 4\r\nContent-Type: video/mp4\r\nContent-Range: bytes 4-7/10\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\nEFGH").await.unwrap();
                }
            }
        });
        let proxy = ProxyServer::start(format!("http://{address}/video.mp4"), 1)
            .await
            .unwrap();
        let client = reqwest::Client::new();
        let response = client
            .get(&proxy.uri)
            .header("Range", "bytes=4-7")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::PARTIAL_CONTENT);
        assert_eq!(response.headers()["content-range"], "bytes 4-7/10");
        assert_eq!(&response.bytes().await.unwrap()[..], b"EFGH");
        let response = client.head(&proxy.uri).send().await.unwrap();
        assert_eq!(response.headers()["content-length"], "10");
        assert!(response.bytes().await.unwrap().is_empty());
        let address = reqwest::Url::parse(&proxy.uri).unwrap();
        let response = client
            .get(format!(
                "http://127.0.0.1:{}/other",
                address.port().unwrap()
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);
        upstream_task.await.unwrap();
        drop(proxy);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        loop {
            if TcpStream::connect(("127.0.0.1", address.port().unwrap()))
                .await
                .is_err()
            {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "Preview listener did not stop"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}
