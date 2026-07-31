//! A loopback HTTP proxy that puts the webviews' traffic back inside this
//! process.
//!
//! WebView2 does its networking from its own `msedgewebview2.exe` children, not
//! from us. Chinese game accelerators hook by executable name — which is the
//! whole reason the app offers to rename itself to `Beanfun.exe` — so anything a
//! webview fetches (the GamaPass sign-in, reCAPTCHA, the member centre, the
//! Classic portal) travels outside the tunnel the user set up, while our own
//! `reqwest` calls travel inside it.
//!
//! Pointing WebView2 at `--proxy-server=127.0.0.1:<port>` fixes that: the
//! connection the accelerator sees is the one *this* process opens to beanfun.
//! `CONNECT` is tunnelled byte-for-byte, so TLS is never touched, terminated, or
//! inspected — we only move bytes.

use std::sync::OnceLock;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Port of the running proxy, once it has bound.
static PROXY_PORT: OnceLock<u16> = OnceLock::new();

/// Largest request head we'll buffer before giving up on a client.
const MAX_HEAD: usize = 32 * 1024;

/// The proxy's port, or `None` if it never started.
pub fn port() -> Option<u16> {
    PROXY_PORT.get().copied()
}

/// Bind the proxy on a loopback port and serve it in the background.
///
/// Idempotent: later calls return the port already in use. Binding to port 0
/// means the OS picks a free one, so nothing can clash with whatever else the
/// machine is running.
pub async fn start() -> Option<u16> {
    if let Some(existing) = port() {
        return Some(existing);
    }

    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!("webview proxy: could not bind: {e}");
            return None;
        }
    };
    let bound = listener.local_addr().ok()?.port();
    let _ = PROXY_PORT.set(bound);

    tauri::async_runtime::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((client, _)) => {
                    tokio::spawn(async move {
                        if let Err(e) = serve(client).await {
                            tracing::debug!("webview proxy: connection ended: {e}");
                        }
                    });
                }
                Err(e) => {
                    tracing::warn!("webview proxy: accept failed, stopping: {e}");
                    break;
                }
            }
        }
    });

    tracing::info!("webview proxy listening on 127.0.0.1:{bound}");
    Some(bound)
}

/// Serve one client connection: a `CONNECT` tunnel, or a plain HTTP request in
/// absolute-form.
async fn serve(mut client: TcpStream) -> std::io::Result<()> {
    let mut head = Vec::with_capacity(1024);
    let mut buf = [0u8; 4096];

    let head_end = loop {
        let read = client.read(&mut buf).await?;
        if read == 0 {
            return Ok(()); // client hung up before finishing a request
        }
        head.extend_from_slice(&buf[..read]);
        if let Some(end) = find_head_end(&head) {
            break end;
        }
        if head.len() > MAX_HEAD {
            return Ok(());
        }
    };

    let text = String::from_utf8_lossy(&head[..head_end]).into_owned();
    let Some((method, target)) = request_line(&text) else {
        return Ok(());
    };

    if method.eq_ignore_ascii_case("CONNECT") {
        // Tunnel: reply, then shuttle bytes in both directions untouched.
        let mut server = TcpStream::connect(with_default_port(target, 443)).await?;
        client
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await?;
        tokio::io::copy_bidirectional(&mut client, &mut server).await?;
        return Ok(());
    }

    // Plain HTTP arrives in absolute-form (`GET http://host/path`); origin
    // servers want the path alone.
    let Some((authority, path)) = split_absolute_url(target) else {
        return Ok(());
    };
    let mut server = TcpStream::connect(with_default_port(&authority, 80)).await?;
    let rewritten = text.replacen(target, &path, 1);
    server.write_all(rewritten.as_bytes()).await?;
    server.write_all(b"\r\n\r\n").await?;
    // Anything already read past the head belongs to the body.
    if head.len() > head_end + 4 {
        server.write_all(&head[head_end + 4..]).await?;
    }
    tokio::io::copy_bidirectional(&mut client, &mut server).await?;
    Ok(())
}

/// Offset of the blank line ending the request head, if it has arrived.
fn find_head_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

/// Method and target from a request head.
fn request_line(head: &str) -> Option<(&str, &str)> {
    let first = head.lines().next()?;
    let mut parts = first.split_whitespace();
    Some((parts.next()?, parts.next()?))
}

/// Add a default port when the authority carries none. IPv6 literals already
/// bring their own brackets, and a port always follows the closing one.
fn with_default_port(authority: &str, default: u16) -> String {
    let has_port = match authority.rfind(']') {
        Some(bracket) => authority[bracket..].contains(':'),
        None => authority.contains(':'),
    };
    if has_port {
        authority.to_string()
    } else {
        format!("{authority}:{default}")
    }
}

/// Split `http://host[:port]/path` into its authority and path.
fn split_absolute_url(url: &str) -> Option<(String, String)> {
    let rest = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))?;
    match rest.find('/') {
        Some(slash) => Some((rest[..slash].to_string(), rest[slash..].to_string())),
        None => Some((rest.to_string(), "/".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_head_boundary_only_once_complete() {
        assert_eq!(find_head_end(b"GET / HTTP/1.1\r\nHost: x\r\n"), None);
        assert_eq!(
            find_head_end(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n"),
            Some(23)
        );
    }

    #[test]
    fn reads_method_and_target() {
        let (method, target) = request_line("CONNECT beanfun.com:443 HTTP/1.1\r\nHost: x").unwrap();
        assert_eq!(method, "CONNECT");
        assert_eq!(target, "beanfun.com:443");
        assert!(request_line("").is_none());
    }

    #[test]
    fn adds_a_port_only_when_one_is_missing() {
        assert_eq!(with_default_port("beanfun.com", 443), "beanfun.com:443");
        assert_eq!(
            with_default_port("beanfun.com:8443", 443),
            "beanfun.com:8443"
        );
        // An IPv6 literal's own colons must not read as a port.
        assert_eq!(with_default_port("[::1]", 80), "[::1]:80");
        assert_eq!(with_default_port("[::1]:8080", 80), "[::1]:8080");
    }

    #[test]
    fn splits_absolute_urls_into_authority_and_path() {
        assert_eq!(
            split_absolute_url("http://tw.beanfun.com/a/b?c=1"),
            Some(("tw.beanfun.com".into(), "/a/b?c=1".into()))
        );
        // No path at all still addresses the root.
        assert_eq!(
            split_absolute_url("http://tw.beanfun.com"),
            Some(("tw.beanfun.com".into(), "/".into()))
        );
        assert_eq!(split_absolute_url("/relative"), None);
    }
}
