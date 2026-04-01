use bytes::{Buf, BufMut, BytesMut};
use std::io;
use tokio_util::codec::{Decoder, Encoder};

const MAX_FRAME_SIZE: usize = 32 * 1024 * 1024;

pub struct ContentLengthCodec {
    next_payload_len: Option<usize>,
}

impl ContentLengthCodec {
    pub fn new() -> Self {
        Self {
            next_payload_len: None,
        }
    }
}

impl Default for ContentLengthCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder for ContentLengthCodec {
    type Item = serde_json::Value;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if let Some(payload_len) = self.next_payload_len {
            if src.len() < payload_len {
                src.reserve(payload_len - src.len());
                return Ok(None);
            }
            let payload = src.split_to(payload_len);
            self.next_payload_len = None;

            let value: serde_json::Value = serde_json::from_slice(&payload)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            return Ok(Some(value));
        }

        let header_end = src.windows(4).position(|w| w == b"\r\n\r\n");

        let header_end = match header_end {
            Some(pos) => pos,
            None => {
                if src.len() > 256 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Header too long without terminator",
                    ));
                }
                return Ok(None);
            }
        };

        let header = std::str::from_utf8(&src[..header_end])
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let content_length = parse_content_length(header)?;

        if content_length > MAX_FRAME_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Frame of length {} exceeds maximum {}",
                    content_length, MAX_FRAME_SIZE
                ),
            ));
        }

        src.advance(header_end + 4);
        self.next_payload_len = Some(content_length);

        self.decode(src)
    }
}

impl Encoder<serde_json::Value> for ContentLengthCodec {
    type Error = io::Error;

    fn encode(&mut self, item: serde_json::Value, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let payload =
            serde_json::to_vec(&item).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let header = format!("Content-Length: {}\r\n\r\n", payload.len());

        dst.reserve(header.len() + payload.len());
        dst.put_slice(header.as_bytes());
        dst.put_slice(&payload);
        Ok(())
    }
}

fn parse_content_length(header: &str) -> Result<usize, io::Error> {
    for line in header.split("\r\n") {
        if let Some(value) = line.strip_prefix("Content-Length: ") {
            return value
                .trim()
                .parse::<usize>()
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e));
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "Missing Content-Length header",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codec() -> ContentLengthCodec {
        ContentLengthCodec::new()
    }

    #[test]
    fn encode_produces_content_length_header() {
        let mut codec = codec();
        let mut buf = BytesMut::new();
        let value = serde_json::json!({"jsonrpc": "2.0", "method": "test", "id": 1});
        codec.encode(value.clone(), &mut buf).unwrap();
        let output = String::from_utf8(buf.to_vec()).unwrap();
        assert!(output.starts_with("Content-Length: "));
        assert!(output.contains("\r\n\r\n"));
        let payload = serde_json::to_vec(&value).unwrap();
        assert!(output.starts_with(&format!("Content-Length: {}\r\n\r\n", payload.len())));
    }

    #[test]
    fn round_trip_simple_message() {
        let mut codec = codec();
        let mut buf = BytesMut::new();
        let value = serde_json::json!({"jsonrpc": "2.0", "method": "test", "id": 1});
        codec.encode(value.clone(), &mut buf).unwrap();
        let decoded = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(value, decoded);
    }

    #[test]
    fn decode_complete_message_returns_some() {
        let mut codec = codec();
        let payload = br#"{"jsonrpc":"2.0","method":"test"}"#;
        let header = format!("Content-Length: {}\r\n\r\n", payload.len());
        let mut buf = BytesMut::new();
        buf.put_slice(header.as_bytes());
        buf.put_slice(payload);
        let result = codec.decode(&mut buf).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap()["method"], "test");
    }

    #[test]
    fn decode_incomplete_header_returns_none() {
        let mut codec = codec();
        let mut buf = BytesMut::from("Content-Length: 10\r\n");
        let result = codec.decode(&mut buf).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn decode_incomplete_payload_returns_none() {
        let mut codec = codec();
        let mut buf = BytesMut::from("Content-Length: 100\r\n\r\n{\"partial\":");
        let result = codec.decode(&mut buf).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn decode_header_too_long_returns_error() {
        let mut codec = codec();
        let long_header = "X-Garbage: ".to_string() + &"a".repeat(300);
        let mut buf = BytesMut::from(long_header.as_str());
        let result = codec.decode(&mut buf);
        assert!(result.is_err());
    }

    #[test]
    fn decode_missing_content_length_returns_error() {
        let mut codec = codec();
        let mut buf = BytesMut::from("X-Other: 42\r\n\r\n{}");
        let result = codec.decode(&mut buf);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Missing Content-Length"));
    }

    #[test]
    fn decode_non_numeric_content_length_returns_error() {
        let mut codec = codec();
        let mut buf = BytesMut::from("Content-Length: abc\r\n\r\n");
        let result = codec.decode(&mut buf);
        assert!(result.is_err());
    }

    #[test]
    fn decode_oversized_frame_returns_error() {
        let mut codec = codec();
        let size = 33 * 1024 * 1024;
        let header = format!("Content-Length: {}\r\n\r\n", size);
        let mut buf = BytesMut::from(header.as_str());
        let result = codec.decode(&mut buf);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("exceeds maximum"));
    }

    #[test]
    fn encode_decode_multibyte_utf8_japanese() {
        let mut codec = codec();
        let mut buf = BytesMut::new();
        let value = serde_json::json!({"text": "こんにちは"});
        codec.encode(value.clone(), &mut buf).unwrap();
        let output = String::from_utf8(buf.to_vec()).unwrap();
        let payload_bytes = serde_json::to_vec(&value).unwrap();
        assert!(output.starts_with(&format!("Content-Length: {}\r\n\r\n", payload_bytes.len())));
        assert_ne!("こんにちは".len(), "こんにちは".chars().count());
        let mut decode_buf = BytesMut::from(output.as_str());
        let decoded = codec.decode(&mut decode_buf).unwrap().unwrap();
        assert_eq!(value, decoded);
    }

    #[test]
    fn encode_decode_emoji() {
        let mut codec = codec();
        let mut buf = BytesMut::new();
        let value = serde_json::json!({"text": "Hello 🌍"});
        codec.encode(value.clone(), &mut buf).unwrap();
        let mut decode_buf = buf.clone();
        let decoded = codec.decode(&mut decode_buf).unwrap().unwrap();
        assert_eq!(value, decoded);
    }

    #[test]
    fn multiple_messages_sequential() {
        let mut codec = codec();
        let mut buf = BytesMut::new();
        let msg1 = serde_json::json!({"jsonrpc": "2.0", "method": "first", "id": 1});
        let msg2 = serde_json::json!({"jsonrpc": "2.0", "method": "second", "id": 2});
        let msg3 = serde_json::json!({"jsonrpc": "2.0", "method": "third", "id": 3});
        codec.encode(msg1.clone(), &mut buf).unwrap();
        codec.encode(msg2.clone(), &mut buf).unwrap();
        codec.encode(msg3.clone(), &mut buf).unwrap();
        let d1 = codec.decode(&mut buf).unwrap().unwrap();
        let d2 = codec.decode(&mut buf).unwrap().unwrap();
        let d3 = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(msg1, d1);
        assert_eq!(msg2, d2);
        assert_eq!(msg3, d3);
        assert!(buf.is_empty());
    }

    #[test]
    fn partial_byte_by_byte_feed() {
        let mut codec = codec();
        let mut encode_buf = BytesMut::new();
        let value = serde_json::json!({"id": 1});
        codec.encode(value.clone(), &mut encode_buf).unwrap();
        let full_bytes = encode_buf.to_vec();

        let mut feed_buf = BytesMut::new();
        for (i, &byte) in full_bytes.iter().enumerate() {
            feed_buf.put_u8(byte);
            let result = codec.decode(&mut feed_buf).unwrap();
            if i < full_bytes.len() - 1 {
                assert!(result.is_none(), "should not decode at byte {}", i);
            } else {
                assert_eq!(result.unwrap(), value);
            }
        }
    }

    #[test]
    fn decode_eof_with_remaining_data() {
        let mut codec = codec();
        let mut buf = BytesMut::from("Content-Length: 50\r\n\r\n{\"partial\":");
        let result = codec.decode_eof(&mut buf);
        assert!(result.is_err() || result.unwrap().is_none());
    }
}
