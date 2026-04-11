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
    #[cfg(test)]
    pub fn from_stream(
        stream: impl futures_core::Stream<Item = reqwest::Result<bytes::Bytes>> + Unpin + Send + 'static,
    ) -> Self {
        Self {
            stream: Box::new(stream),
            buffer: Vec::new(),
            raw_buffer: String::new(),
            pending_event_type: None,
            pending_data: None,
        }
    }

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
                            let trimmed = data_val.trim().to_string();
                            self.pending_data = Some(match self.pending_data.take() {
                                Some(existing) => format!("{existing}\n{trimmed}"),
                                None => trimmed,
                            });
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

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use tokio_stream::iter as stream_iter;

    fn bytes_stream(
        chunks: Vec<&'static str>,
    ) -> impl futures_core::Stream<Item = reqwest::Result<bytes::Bytes>> + Unpin + Send + 'static
    {
        stream_iter(chunks.into_iter().map(|s| Ok(bytes::Bytes::from(s))))
    }

    #[test]
    fn when_sse_event_constructed_then_fields_accessible() {
        let event = SseEvent {
            event_type: Some("message".to_string()),
            data: "hello world".to_string(),
        };
        assert_eq!(event.event_type.as_deref(), Some("message"));
        assert_eq!(event.data, "hello world");
    }

    #[test]
    fn when_sse_event_has_no_event_type_then_event_type_is_none() {
        let event = SseEvent {
            event_type: None,
            data: "data only".to_string(),
        };
        assert!(event.event_type.is_none());
        assert_eq!(event.data, "data only");
    }

    #[rstest]
    #[tokio::test]
    async fn when_sse_collector_receives_data_only_event_then_collects_it() {
        let raw = "data: hello\n\n";
        let mut collector = SseCollector::from_stream(bytes_stream(vec![raw]));
        let events = collector.collect_events(Duration::from_millis(200)).await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello");
        assert!(events[0].event_type.is_none());
    }

    #[rstest]
    #[tokio::test]
    async fn when_sse_event_line_parsed_then_event_and_data_extracted() {
        let raw = "event: update\ndata: {\"key\":\"val\"}\n\n";
        let mut collector = SseCollector::from_stream(bytes_stream(vec![raw]));
        let events = collector.collect_events(Duration::from_millis(200)).await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type.as_deref(), Some("update"));
        assert_eq!(events[0].data, r#"{"key":"val"}"#);
    }

    #[rstest]
    #[tokio::test]
    async fn when_sse_collector_receives_events_then_collects_them() {
        let raw = "data: first\n\ndata: second\n\n";
        let mut collector = SseCollector::from_stream(bytes_stream(vec![raw]));
        let events = collector.collect_events(Duration::from_millis(200)).await;
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].data, "first");
        assert_eq!(events[1].data, "second");
    }

    #[rstest]
    #[tokio::test]
    async fn when_sse_collector_next_event_called_then_returns_first_event() {
        let raw = "data: only\n\n";
        let mut collector = SseCollector::from_stream(bytes_stream(vec![raw]));
        let event = collector.next_event(Duration::from_millis(200)).await;
        assert!(event.is_some());
        assert_eq!(event.unwrap().data, "only");
    }

    #[rstest]
    #[tokio::test]
    async fn when_sse_stream_is_empty_then_collect_events_returns_empty_vec() {
        let mut collector = SseCollector::from_stream(bytes_stream(vec![]));
        let events = collector.collect_events(Duration::from_millis(50)).await;
        assert!(events.is_empty());
    }
}
