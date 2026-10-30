use reqwest::{ClientBuilder, Url};
use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};

static PROXY: Mutex<Option<Url>> = Mutex::new(None);
static GENERATION: AtomicU64 = AtomicU64::new(0);
pub fn validate(value: &str) -> Result<Option<Url>, &'static str> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    let url = Url::parse(value.trim())
        .map_err(|_| "Enter a full HTTP, HTTPS, SOCKS5 or SOCKS5H proxy URL.")?;
    if !matches!(url.scheme(), "http" | "https" | "socks5" | "socks5h")
        || url.host_str().is_none()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return Err(
            "Use an HTTP, HTTPS, SOCKS5 or SOCKS5H proxy URL without a path, query or fragment.",
        );
    }
    if matches!(url.scheme(), "socks5" | "socks5h") && url.port().is_none() {
        return Err("A SOCKS proxy requires a port.");
    }
    Ok(Some(url))
}
pub fn configure(value: &str) -> Result<(), &'static str> {
    *PROXY.lock().unwrap() = validate(value)?;
    GENERATION.fetch_add(1, Ordering::Relaxed);
    Ok(())
}
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}
fn loopback(url: &Url) -> bool {
    url.host_str().is_some_and(|host| {
        host.eq_ignore_ascii_case("localhost")
            || host
                .trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    })
}
fn with_proxy(proxy: Option<Url>) -> ClientBuilder {
    let builder = reqwest::Client::builder();
    if let Some(proxy) = proxy {
        builder.no_proxy().proxy(reqwest::Proxy::custom(move |url| {
            if loopback(url) {
                None
            } else {
                Some(proxy.clone())
            }
        }))
    } else {
        builder
    }
}
pub fn builder() -> ClientBuilder {
    with_proxy(PROXY.lock().unwrap().clone())
}
pub fn apply_child(command: &mut tokio::process::Command) {
    if let Some(proxy) = PROXY.lock().unwrap().as_ref() {
        for name in [
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "http_proxy",
            "https_proxy",
            "all_proxy",
        ] {
            command.env(name, proxy.as_str());
        }
        for name in ["NO_PROXY", "no_proxy"] {
            command.env(name, "localhost,127.0.0.1,::1");
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    #[test]
    fn malformed_proxies_do_not_expose_credentials_and_loopback_stays_local() {
        for value in [
            "http://user:secret@example.com/path",
            "ftp://user:secret@example.com",
            "socks5://example.com",
            "not a url",
        ] {
            let error = validate(value).unwrap_err();
            assert!(!error.contains("secret"));
        }
        assert!(validate("").unwrap().is_none());
        for url in ["http://127.0.0.1:8", "http://[::1]:8", "http://localhost:8"] {
            assert!(loopback(&Url::parse(url).unwrap()));
        }
        assert!(!loopback(&Url::parse("http://remote.test").unwrap()));
    }
    #[tokio::test]
    async fn manual_http_proxy_routes_external_requests_with_authentication() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = [0; 4096];
            let size = stream.read(&mut bytes).await.unwrap();
            let request = String::from_utf8_lossy(&bytes[..size]).to_lowercase();
            assert!(request.contains("http://remote.test/data"));
            assert!(request.contains("proxy-authorization: basic"));
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\nproxied",
                )
                .await
                .unwrap();
        });
        let client = with_proxy(validate(&format!("http://user:pass@{address}")).unwrap())
            .build()
            .unwrap();
        assert_eq!(
            client
                .get("http://remote.test/data")
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap(),
            "proxied"
        );
        server.await.unwrap();
    }
    #[tokio::test]
    async fn socks5h_resolves_the_destination_through_the_proxy() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut greeting = [0; 2];
            stream.read_exact(&mut greeting).await.unwrap();
            let mut methods = vec![0; greeting[1] as usize];
            stream.read_exact(&mut methods).await.unwrap();
            stream.write_all(&[5, 0]).await.unwrap();
            let mut header = [0; 4];
            stream.read_exact(&mut header).await.unwrap();
            assert_eq!(header, [5, 1, 0, 3]);
            let mut count = [0];
            stream.read_exact(&mut count).await.unwrap();
            let mut domain = vec![0; count[0] as usize];
            stream.read_exact(&mut domain).await.unwrap();
            assert_eq!(domain, b"remote.test");
            let mut port = [0; 2];
            stream.read_exact(&mut port).await.unwrap();
            stream
                .write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 80])
                .await
                .unwrap();
            let mut request = [0; 4096];
            assert!(stream.read(&mut request).await.unwrap() > 0);
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nsocks",
                )
                .await
                .unwrap();
        });
        let client = with_proxy(validate(&format!("socks5h://{address}")).unwrap())
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap();
        assert_eq!(
            client
                .get("http://remote.test/data")
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap(),
            "socks"
        );
        server.await.unwrap();
    }
}
