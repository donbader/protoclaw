use bytes::{BufMut, BytesMut};
use std::io;
use tokio_util::codec::{Decoder, Encoder};

const MAX_LINE_SIZE: usize = 32 * 1024 * 1024;

pub struct NdJsonCodec;

impl NdJsonCodec {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NdJsonCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder for NdJsonCodec {
    type Item = serde_json::Value;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        loop {
            let newline_pos = src.iter().position(|b| *b == b'\n');
            match newline_pos {
                Some(pos) => {
                    let line = src.split_to(pos + 1);
                    let end = if pos > 0 && line[pos - 1] == b'\r' {
                        pos - 1
                    } else {
                        pos
                    };
                    let trimmed = &line[..end];
                    if trimmed.is_empty() {
                        continue;
                    }
                    if trimmed.len() > MAX_LINE_SIZE {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "line of {} bytes exceeds {} byte limit",
                                trimmed.len(),
                                MAX_LINE_SIZE
                            ),
                        ));
                    }
                    match serde_json::from_slice::<serde_json::Value>(trimmed) {
                        Ok(value) => return Ok(Some(value)),
                        Err(_) => continue,
                    }
                }
                None => {
                    if src.len() > MAX_LINE_SIZE {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "accumulated {} bytes without newline, exceeds {} byte limit",
                                src.len(),
                                MAX_LINE_SIZE
                            ),
                        ));
                    }
                    return Ok(None);
                }
            }
        }
    }
}

impl Encoder<serde_json::Value> for NdJsonCodec {
    type Error = io::Error;

    fn encode(&mut self, item: serde_json::Value, dst: &mut BytesMut) -> Result<(), Self::Error> {
        serde_json::to_writer((&mut *dst).writer(), &item)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        dst.extend_from_slice(b"\n");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BufMut;

    fn codec() -> NdJsonCodec {
        NdJsonCodec::new()
    }

    #[test]
    fn encode_appends_newline() {
        let mut codec = codec();
        let mut buf = BytesMut::new();
        let value = serde_json::json!({"method": "test"});
        codec.encode(value, &mut buf).unwrap();
        assert!(buf.ends_with(b"\n"), "encoded output must end with newline");
        assert!(
            buf.len() > 1,
            "encoded output must have content before newline"
        );
    }

    #[test]
    fn encode_produces_compact_json() {
        let mut codec = codec();
        let mut buf = BytesMut::new();
        let value = serde_json::json!({"key": "value", "num": 42});
        codec.encode(value, &mut buf).unwrap();
        let output = std::str::from_utf8(&buf).unwrap();
        let line = output.trim_end_matches('\n');
        assert!(
            !line.contains('\n'),
            "compact JSON must not contain internal newlines"
        );
        let reparsed: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(reparsed["key"], "value");
    }

    #[test]
    fn decode_complete_line() {
        let mut codec = codec();
        let mut buf = BytesMut::from("{\"method\":\"test\"}\n");
        let result = codec.decode(&mut buf).unwrap();
        assert!(result.is_some(), "complete line must decode");
        assert_eq!(result.unwrap()["method"], "test");
    }

    #[test]
    fn decode_no_newline_returns_none() {
        let mut codec = codec();
        let mut buf = BytesMut::from("{\"method\":\"test\"}");
        let result = codec.decode(&mut buf).unwrap();
        assert!(
            result.is_none(),
            "incomplete line (no newline) must return None"
        );
    }

    #[test]
    fn decode_empty_line_skipped() {
        let mut codec = codec();
        let mut buf = BytesMut::from("\n{\"method\":\"test\"}\n");
        let result = codec.decode(&mut buf).unwrap();
        assert!(result.is_some(), "must skip empty line and return JSON");
        assert_eq!(result.unwrap()["method"], "test");
    }

    #[test]
    fn decode_invalid_json_skipped() {
        let mut codec = codec();
        let mut buf = BytesMut::from("not-json\n{\"method\":\"test\"}\n");
        let result = codec.decode(&mut buf).unwrap();
        assert!(
            result.is_some(),
            "must skip invalid JSON line and return next valid JSON"
        );
        assert_eq!(result.unwrap()["method"], "test");
    }

    #[test]
    fn decode_invalid_json_only_returns_none() {
        let mut codec = codec();
        let mut buf = BytesMut::from("not-json\n");
        let result = codec.decode(&mut buf).unwrap();
        assert!(
            result.is_none(),
            "invalid JSON with no valid follow-up returns None"
        );
        assert!(buf.is_empty(), "invalid line must be consumed from buffer");
    }

    #[test]
    fn round_trip() {
        let mut codec = codec();
        let mut buf = BytesMut::new();
        let value = serde_json::json!({"jsonrpc": "2.0", "method": "test", "id": 1});
        codec.encode(value.clone(), &mut buf).unwrap();
        let decoded = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(value, decoded);
    }

    #[test]
    fn sequential_messages() {
        let mut codec = codec();
        let mut buf = BytesMut::new();
        let msg1 = serde_json::json!({"id": 1});
        let msg2 = serde_json::json!({"id": 2});
        let msg3 = serde_json::json!({"id": 3});
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
                assert!(result.is_none(), "should not decode at byte {i}");
            } else {
                assert_eq!(result.unwrap(), value);
            }
        }
    }

    #[test]
    fn multibyte_utf8_japanese() {
        let mut codec = codec();
        let mut buf = BytesMut::new();
        let value = serde_json::json!({"text": "こんにちは"});
        codec.encode(value.clone(), &mut buf).unwrap();
        let decoded = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(value, decoded);
    }

    #[test]
    fn multibyte_utf8_emoji() {
        let mut codec = codec();
        let mut buf = BytesMut::new();
        let value = serde_json::json!({"text": "Hello 🌍"});
        codec.encode(value.clone(), &mut buf).unwrap();
        let decoded = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(value, decoded);
    }

    #[test]
    fn unicode_line_separator_in_string() {
        let mut codec = codec();
        let json_with_ls = "{\"text\":\"before\\u2028after\"}\n";
        let mut buf = BytesMut::from(json_with_ls);
        let result = codec.decode(&mut buf).unwrap();
        assert!(
            result.is_some(),
            "U+2028 in JSON string must NOT split the line"
        );
        let val = result.unwrap();
        assert_eq!(val["text"], "before\u{2028}after");
    }

    #[test]
    fn oversized_line_returns_error() {
        let mut codec = codec();
        let big_value = "x".repeat(33 * 1024 * 1024);
        let line = format!("{{\"data\":\"{big_value}\"}}\n");
        let mut buf = BytesMut::from(line.as_str());
        let result = codec.decode(&mut buf);
        assert!(result.is_err(), "line exceeding 32MB must return error");
    }

    #[test]
    fn crlf_line_ending() {
        let mut codec = codec();
        let mut buf = BytesMut::from("{\"method\":\"test\"}\r\n");
        let result = codec.decode(&mut buf).unwrap();
        assert!(result.is_some(), "CRLF line ending must decode correctly");
        assert_eq!(result.unwrap()["method"], "test");
    }
}
