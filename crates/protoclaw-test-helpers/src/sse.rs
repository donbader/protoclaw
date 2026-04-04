use std::time::Duration;

use futures_core::Stream;
use tokio_stream::StreamExt;

#[derive(Debug, Clone)]
pub struct SseEvent {
    pub event_type: Option<String>,
    pub data: String,
}

pub struct SseCollector {
    stream: Box<dyn Stream<Item = reqwest::Result<bytes::Bytes>> + Unpin + Send>,
    buffer: Vec<SseEvent>,
    raw_buffer: String,
    pending_event_type: Option<String>,
    pending_data: Option<String>,
}

impl SseCollector {
    pub async fn connect(port: u16) -> Self {
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("http://127.0.0.1:{port}/events"))
            .send()
            .await
            .expect("failed to connect to SSE endpoint");
        assert_eq!(resp.status(), 200, "SSE endpoint returned non-200");
        let byte_stream = resp.bytes_stream();
        Self {
            stream: Box::new(byte_stream),
            buffer: Vec::new(),
            raw_buffer: String::new(),
            pending_event_type: None,
            pending_data: None,
        }
    }

    pub async fn next_event(&mut self, timeout: Duration) -> Option<SseEvent> {
        if let Some(event) = self.buffer.pop() {
            return Some(event);
        }
        self.poll_events(timeout).await;
        self.buffer.pop()
    }

    pub async fn collect_events(&mut self, timeout: Duration) -> Vec<SseEvent> {
        self.poll_events(timeout).await;
        let mut events = std::mem::take(&mut self.buffer);
        events.reverse();
        events
    }

    async fn poll_events(&mut self, timeout: Duration) {
        let deadline = tokio::time::Instant::now() + timeout;

        while tokio::time::Instant::now() < deadline {
            let chunk = tokio::time::timeout_at(deadline, self.stream.next()).await;
            match chunk {
                Ok(Some(Ok(bytes))) => {
                    let text = String::from_utf8_lossy(&bytes);
                    self.raw_buffer.push_str(&text);

                    while let Some(newline_pos) = self.raw_buffer.find('\n') {
                        let line = self.raw_buffer[..newline_pos].to_string();
                        self.raw_buffer = self.raw_buffer[newline_pos + 1..].to_string();

                        let line = line.trim_end_matches('\r');

                        if let Some(event_val) = line.strip_prefix("event:") {
                            self.pending_event_type = Some(event_val.trim().to_string());
                        } else if let Some(data_val) = line.strip_prefix("data:") {
                            self.pending_data = Some(data_val.trim().to_string());
                        } else if line.is_empty() {
                            if let Some(data) = self.pending_data.take() {
                                self.buffer.push(SseEvent {
                                    event_type: self.pending_event_type.take(),
                                    data,
                                });
                            }
                            self.pending_event_type = None;
                        }
                    }
                }
                _ => break,
            }
        }

        if let Some(data) = self.pending_data.take() {
            self.buffer.push(SseEvent {
                event_type: self.pending_event_type.take(),
                data,
            });
        }

        self.buffer.reverse();
    }
}
