# anyclaw-jsonrpc — JSON-RPC 2.0 Codec

Hand-rolled NDJSON codec for JSON-RPC 2.0 over stdio. No framework — just types, a codec, and an error enum.

## Files

| File | Purpose |
|------|---------|
| `codec.rs` | `NdJsonCodec` — tokio-util `Decoder`/`Encoder` for line-delimited JSON |
| `types.rs` | `JsonRpcRequest`, `JsonRpcResponse`, `JsonRpcError`, `JsonRpcMessage` |
| `error.rs` | `FramingError` enum (thiserror) |

## Key Types

```rust
pub struct NdJsonCodec;  // Implements Decoder<Item=Value> + Encoder<Value>

pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
}

pub enum JsonRpcMessage {  // #[serde(untagged)]
    Request(JsonRpcRequest),
    Response(JsonRpcResponse),
}
```

## Framing

NDJSON: one JSON object per line, terminated by `\n`. The codec:
- Decodes by scanning for `\n`, parsing the line as JSON
- Encodes by serializing to compact JSON + `\n`
- Handles CRLF (`\r\n`) line endings
- Skips empty lines
- 32MB max line size (`MAX_LINE_SIZE`) — rejects oversized lines with `io::Error`

## Anti-Patterns (this crate)

- **Don't change `MAX_LINE_SIZE`** without updating the oversized line test
- **Don't add HTTP transport** — this crate is stdio-only; HTTP belongs in channel implementations
- **Don't add protocol logic** — this crate is pure framing; ACP method handling belongs in `anyclaw-agents`
