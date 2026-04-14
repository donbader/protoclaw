use crate::types::{JsonRpcMessage, JsonRpcRequest, JsonRpcResponse};
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
    type Item = JsonRpcMessage;
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
                    match serde_json::from_slice::<JsonRpcMessage>(trimmed) {
                        Ok(msg) => return Ok(Some(msg)),
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

impl Encoder<JsonRpcMessage> for NdJsonCodec {
    type Error = io::Error;

    fn encode(&mut self, item: JsonRpcMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        serde_json::to_writer((&mut *dst).writer(), &item)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        dst.extend_from_slice(b"\n");
        Ok(())
    }
}

/// Convenience encoder: encode a `JsonRpcRequest` directly without wrapping in `JsonRpcMessage`.
impl Encoder<JsonRpcRequest> for NdJsonCodec {
    type Error = io::Error;

    fn encode(&mut self, item: JsonRpcRequest, dst: &mut BytesMut) -> Result<(), Self::Error> {
        Encoder::<JsonRpcMessage>::encode(self, JsonRpcMessage::Request(item), dst)
    }
}

/// Convenience encoder: encode a `JsonRpcResponse` directly without wrapping in `JsonRpcMessage`.
impl Encoder<JsonRpcResponse> for NdJsonCodec {
    type Error = io::Error;

    fn encode(&mut self, item: JsonRpcResponse, dst: &mut BytesMut) -> Result<(), Self::Error> {
        Encoder::<JsonRpcMessage>::encode(self, JsonRpcMessage::Response(item), dst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{JsonRpcMessage, JsonRpcRequest, JsonRpcResponse, RequestId};
    use bytes::BufMut;
    use rstest::rstest;

    fn codec() -> NdJsonCodec {
        NdJsonCodec::new()
    }

    fn sample_request() -> JsonRpcMessage {
        JsonRpcMessage::Request(JsonRpcRequest::new(
            "test",
            Some(RequestId::Number(1)),
            None,
        ))
    }

    fn sample_response() -> JsonRpcMessage {
        JsonRpcMessage::Response(JsonRpcResponse::success(
            Some(RequestId::Number(1)),
            serde_json::json!("ok"),
        ))
    }

    // --- Decode tests ---

    #[rstest]
    fn when_decoding_request_line_then_returns_request_variant() {
        let mut codec = codec();
        let mut buf = BytesMut::from("{\"jsonrpc\":\"2.0\",\"method\":\"test\",\"id\":1}\n");
        let result = codec.decode(&mut buf).unwrap();
        assert!(
            result.is_some(),
            "complete JSON-RPC request line must decode"
        );
        match result.unwrap() {
            JsonRpcMessage::Request(req) => {
                assert_eq!(req.method, "test");
                assert_eq!(req.id, Some(RequestId::Number(1)));
            }
            _ => panic!("Expected Request variant"),
        }
    }

    #[rstest]
    fn when_decoding_response_line_then_returns_response_variant() {
        let mut codec = codec();
        let mut buf = BytesMut::from("{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":\"hello\"}\n");
        let result = codec.decode(&mut buf).unwrap();
        assert!(
            result.is_some(),
            "complete JSON-RPC response line must decode"
        );
        match result.unwrap() {
            JsonRpcMessage::Response(resp) => {
                assert_eq!(resp.result, Some(serde_json::json!("hello")));
            }
            _ => panic!("Expected Response variant"),
        }
    }

    #[rstest]
    fn when_line_has_no_newline_then_returns_none() {
        let mut codec = codec();
        let mut buf = BytesMut::from("{\"jsonrpc\":\"2.0\",\"method\":\"test\"}");
        let result = codec.decode(&mut buf).unwrap();
        assert!(
            result.is_none(),
            "incomplete line (no newline) must return None"
        );
    }

    #[rstest]
    fn when_line_is_empty_then_skips_and_decodes_next() {
        let mut codec = codec();
        let mut buf = BytesMut::from("\n{\"jsonrpc\":\"2.0\",\"method\":\"test\",\"id\":1}\n");
        let result = codec.decode(&mut buf).unwrap();
        assert!(
            result.is_some(),
            "must skip empty line and return JSON-RPC message"
        );
        assert!(matches!(result.unwrap(), JsonRpcMessage::Request(_)));
    }

    #[rstest]
    fn when_line_is_invalid_json_then_skips_and_decodes_next() {
        let mut codec = codec();
        let mut buf =
            BytesMut::from("not-json\n{\"jsonrpc\":\"2.0\",\"method\":\"test\",\"id\":1}\n");
        let result = codec.decode(&mut buf).unwrap();
        assert!(
            result.is_some(),
            "must skip invalid JSON and return next valid message"
        );
        assert!(matches!(result.unwrap(), JsonRpcMessage::Request(_)));
    }

    #[rstest]
    fn when_non_jsonrpc_json_line_then_skips() {
        let mut codec = codec();
        // Valid JSON but not a JSON-RPC message (no method, no result/error)
        let mut buf = BytesMut::from(
            "{\"foo\":\"bar\"}\n{\"jsonrpc\":\"2.0\",\"method\":\"test\",\"id\":1}\n",
        );
        let result = codec.decode(&mut buf).unwrap();
        assert!(
            result.is_some(),
            "must skip non-JSON-RPC line and return next valid message"
        );
        match result.unwrap() {
            JsonRpcMessage::Request(req) => assert_eq!(req.method, "test"),
            _ => panic!("Expected Request variant"),
        }
    }

    #[rstest]
    fn when_only_invalid_json_present_then_returns_none() {
        let mut codec = codec();
        let mut buf = BytesMut::from("not-json\n");
        let result = codec.decode(&mut buf).unwrap();
        assert!(
            result.is_none(),
            "invalid JSON with no valid follow-up returns None"
        );
        assert!(buf.is_empty(), "invalid line must be consumed from buffer");
    }

    #[rstest]
    fn when_line_ends_with_crlf_then_decodes_correctly() {
        let mut codec = codec();
        let mut buf = BytesMut::from("{\"jsonrpc\":\"2.0\",\"method\":\"test\",\"id\":1}\r\n");
        let result = codec.decode(&mut buf).unwrap();
        assert!(result.is_some(), "CRLF line ending must decode correctly");
        assert!(matches!(result.unwrap(), JsonRpcMessage::Request(_)));
    }

    #[rstest]
    fn when_line_exceeds_32mb_then_returns_error() {
        let mut codec = codec();
        let big_value = "x".repeat(33 * 1024 * 1024);
        let line = format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"big\",\"params\":{{\"data\":\"{big_value}\"}}}}\n"
        );
        let mut buf = BytesMut::from(line.as_str());
        let result = codec.decode(&mut buf);
        assert!(result.is_err(), "line exceeding 32MB must return error");
    }

    // --- Encode tests ---

    #[rstest]
    fn when_encoding_message_then_output_ends_with_newline() {
        let mut codec = codec();
        let mut buf = BytesMut::new();
        codec.encode(sample_request(), &mut buf).unwrap();
        assert!(buf.ends_with(b"\n"), "encoded output must end with newline");
        assert!(
            buf.len() > 1,
            "encoded output must have content before newline"
        );
    }

    #[rstest]
    fn when_encoding_message_then_output_is_compact_json() {
        let mut codec = codec();
        let mut buf = BytesMut::new();
        codec.encode(sample_request(), &mut buf).unwrap();
        let output = std::str::from_utf8(&buf).unwrap();
        let line = output.trim_end_matches('\n');
        assert!(
            !line.contains('\n'),
            "compact JSON must not contain internal newlines"
        );
        let reparsed: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(reparsed["method"], "test");
    }

    #[rstest]
    fn when_encoding_request_directly_then_output_valid() {
        let mut codec = codec();
        let mut buf = BytesMut::new();
        let req = JsonRpcRequest::new("direct", Some(RequestId::Number(5)), None);
        codec.encode(req, &mut buf).unwrap();
        let output = std::str::from_utf8(&buf).unwrap();
        let reparsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
        assert_eq!(reparsed["method"], "direct");
    }

    #[rstest]
    fn when_encoding_response_directly_then_output_valid() {
        let mut codec = codec();
        let mut buf = BytesMut::new();
        let resp = JsonRpcResponse::success(Some(RequestId::Number(5)), serde_json::json!("done"));
        codec.encode(resp, &mut buf).unwrap();
        let output = std::str::from_utf8(&buf).unwrap();
        let reparsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
        assert_eq!(reparsed["result"], "done");
    }

    // --- Round-trip tests ---

    #[rstest]
    fn when_encode_decode_request_then_round_trips() {
        let mut codec = codec();
        let mut buf = BytesMut::new();
        let msg = sample_request();
        codec.encode(msg.clone(), &mut buf).unwrap();
        let decoded = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(msg, decoded);
    }

    #[rstest]
    fn when_encode_decode_response_then_round_trips() {
        let mut codec = codec();
        let mut buf = BytesMut::new();
        let msg = sample_response();
        codec.encode(msg.clone(), &mut buf).unwrap();
        let decoded = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(msg, decoded);
    }

    #[rstest]
    fn when_encoding_multiple_messages_then_decodes_in_order() {
        let mut codec = codec();
        let mut buf = BytesMut::new();
        let msg1 =
            JsonRpcMessage::Request(JsonRpcRequest::new("m1", Some(RequestId::Number(1)), None));
        let msg2 =
            JsonRpcMessage::Request(JsonRpcRequest::new("m2", Some(RequestId::Number(2)), None));
        let msg3 = JsonRpcMessage::Response(JsonRpcResponse::success(
            Some(RequestId::Number(3)),
            serde_json::json!("ok"),
        ));
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

    #[rstest]
    fn when_feeding_bytes_one_at_a_time_then_decodes_on_final_byte() {
        let mut codec = codec();
        let mut encode_buf = BytesMut::new();
        let msg = sample_request();
        codec.encode(msg.clone(), &mut encode_buf).unwrap();
        let full_bytes = encode_buf.to_vec();

        let mut feed_buf = BytesMut::new();
        for (i, &byte) in full_bytes.iter().enumerate() {
            feed_buf.put_u8(byte);
            let result = codec.decode(&mut feed_buf).unwrap();
            if i < full_bytes.len() - 1 {
                assert!(result.is_none(), "should not decode at byte {i}");
            } else {
                assert_eq!(result.unwrap(), msg);
            }
        }
    }

    #[rstest]
    fn when_request_contains_multibyte_utf8_then_round_trips() {
        let mut codec = codec();
        let mut buf = BytesMut::new();
        let msg = JsonRpcMessage::Request(JsonRpcRequest::new(
            "test",
            Some(RequestId::Number(1)),
            Some(serde_json::json!({"text": "こんにちは 🌍"})),
        ));
        codec.encode(msg.clone(), &mut buf).unwrap();
        let decoded = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(msg, decoded);
    }
}
