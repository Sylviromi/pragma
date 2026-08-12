//! Shared SSE (server-sent events) streaming helper.
//!
//! Both Anthropic and OpenAI-compatible providers stream their responses as
//! SSE lines over an HTTP chunked body. The fiddly parts — multi-byte UTF-8
//! splitting across HTTP chunks, `\r\n` handling, idle-timeout detection and
//! clean-vs-broken stream end — are identical, so they live here instead of
//! being copied into every provider.

use futures_util::StreamExt;
use std::time::Duration;

/// Default wait for the next chunk before declaring the connection dead.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(60);

/// Wraps an HTTP response body and yields SSE `data:` payloads line by line.
///
/// `[DONE]` / `message_stop` are *not* interpreted here — providers pass the
/// payloads through and recognize their own end-of-stream markers. The wrapper
/// only reports whether the connection broke (`broken()`); a clean EOF simply
/// ends the `next_data()` iteration.
pub struct SseStream {
    stream: futures_util::stream::BoxStream<'static, Result<bytes::Bytes, reqwest::Error>>,
    buf: Vec<u8>,
    broken: bool,
}

impl SseStream {
    pub fn new(resp: reqwest::Response) -> Self {
        Self {
            stream: resp.bytes_stream().boxed(),
            buf: Vec::new(),
            broken: false,
        }
    }

    /// True if the connection broke: idle timeout, or a transport error.
    pub fn broken(&self) -> bool {
        self.broken
    }

    /// Pull the next `data:` payload, blocking up to the idle timeout per
    /// chunk. Returns `None` on clean EOF or when the connection broke.
    pub async fn next_data(&mut self) -> Option<String> {
        loop {
            // Drain every complete line already buffered before fetching more.
            while let Some(nl) = self.buf.iter().position(|&b| b == b'\n') {
                let line_bytes = &self.buf[..nl];
                let line_bytes = line_bytes.strip_suffix(b"\r").unwrap_or(line_bytes);
                let line = String::from_utf8_lossy(line_bytes).into_owned();
                self.buf.drain(..=nl);
                if let Some(data) = line.strip_prefix("data: ") {
                    return Some(data.to_owned());
                }
                // `event:` / `id:` / blank lines carry no payload — skip.
            }
            match tokio::time::timeout(IDLE_TIMEOUT, self.stream.next()).await {
                Err(_) => {
                    self.broken = true;
                    return None;
                }
                Ok(None) => return None,
                Ok(Some(Err(_))) => {
                    self.broken = true;
                    return None;
                }
                Ok(Some(Ok(b))) => self.buf.extend_from_slice(&b),
            }
        }
    }
}
