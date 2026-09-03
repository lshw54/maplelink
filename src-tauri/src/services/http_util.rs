//! Bounded reads for bodies that come from somewhere we do not control.
//!
//! Every GitHub request in this app can be answered by a proxy mirror instead —
//! that is what the mirrors are for — so "the server would not do that" is not
//! an assumption available here. A body is read up to a stated limit and no
//! further.

use futures_util::StreamExt;

/// The Chrome UA every reqwest client presents to beanfun and GitHub. The
/// Chrome major must match the `sec-ch-ua` brand string in beanfun_service.
pub const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36";

/// Read a response body, stopping at `limit` rather than at whatever the sender
/// decides to send.
///
/// The declared length only sizes the buffer, and only as far as `limit`: it is
/// the sender's claim about itself, and a claim of 999999999999 asks the
/// allocator for 931 GB, which fails, which aborts the process.
pub async fn read_capped(response: reqwest::Response, limit: u64) -> Result<Vec<u8>, String> {
    let reserve = response.content_length().unwrap_or(0).min(limit);
    let mut body: Vec<u8> = Vec::with_capacity(reserve as usize);

    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("{e}"))?;
        if body.len() as u64 + chunk.len() as u64 > limit {
            return Err(format!("the body ran past {limit} bytes"));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// [`read_capped`], decoded as UTF-8. `None` covers an oversized body and one
/// that is not text alike — every caller treats them the same way.
pub async fn read_capped_text(response: reqwest::Response, limit: u64) -> Option<String> {
    String::from_utf8(read_capped(response, limit).await.ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    /// Serve one response and hand back its URL. A real socket rather than a
    /// constructed `Response`, so `Content-Length` reaches the code under test
    /// the way a mirror would actually send it.
    fn serve_once(headers: &str, body: &'static [u8]) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let headers = headers.to_string();
        std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let _ = sock.read(&mut [0u8; 1024]);
            let _ = write!(sock, "HTTP/1.1 200 OK\r\n{headers}\r\n\r\n");
            let _ = sock.write_all(body);
        });
        format!("http://{addr}/")
    }

    #[tokio::test]
    async fn a_body_under_the_limit_is_read_whole() {
        let url = serve_once("Content-Length: 5", b"hello");
        let response = reqwest::get(&url).await.unwrap();
        assert_eq!(read_capped(response, 1024).await.unwrap(), b"hello");
    }

    /// Refused rather than truncated: a half-read update manifest is not
    /// something a caller should have to notice.
    #[tokio::test]
    async fn a_body_over_the_limit_is_refused() {
        let url = serve_once("Content-Length: 5", b"hello");
        let response = reqwest::get(&url).await.unwrap();
        assert!(read_capped(response, 2).await.is_err());
    }

    /// The reservation follows the limit, not the claim. Without the `min` this
    /// asks for 931 GB and the process dies before the assert runs.
    #[tokio::test]
    async fn an_absurd_content_length_does_not_size_the_buffer() {
        let url = serve_once("Content-Length: 999999999999", b"hi");
        let response = reqwest::get(&url).await.unwrap();
        // The connection closes early, so the read fails — the point is that we
        // are still here to see it fail.
        let _ = read_capped(response, 64 * 1024).await;
    }

    #[tokio::test]
    async fn text_comes_back_decoded() {
        let url = serve_once("Content-Length: 5", b"hosts");
        let response = reqwest::get(&url).await.unwrap();
        assert_eq!(
            read_capped_text(response, 1024).await,
            Some("hosts".to_string())
        );
    }
}
