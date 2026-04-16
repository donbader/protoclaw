# anyclaw-sdk-tool

Build MCP-compatible tool servers for [anyclaw](https://github.com/donbader/anyclaw) — implement the `Tool` trait and the SDK handles all MCP protocol framing over stdio.

[![crates.io](https://img.shields.io/crates/v/anyclaw-sdk-tool.svg)](https://crates.io/crates/anyclaw-sdk-tool)
[![docs.rs](https://img.shields.io/docsrs/anyclaw-sdk-tool)](https://docs.rs/anyclaw-sdk-tool)

> ⚠️ **Unstable** — APIs may change between releases.

## Quick Example

```rust
use anyclaw_sdk_tool::{Tool, ToolSdkError, ToolServer};
use serde_json::{Value, json};

struct MyTool;

impl Tool for MyTool {
    fn name(&self) -> &str { "my-tool" }

    fn description(&self) -> &str { "Does something useful" }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value, ToolSdkError> {
        let query = input["query"].as_str().unwrap_or("");
        Ok(json!({ "result": format!("processed: {query}") }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ToolServer::new(vec![Box::new(MyTool)])
        .serve_stdio()
        .await
}
```

One binary can register multiple tools — pass them all to `ToolServer::new`. The server dispatches `call_tool` requests by name.

## Going Further

- **[docs.rs](https://docs.rs/anyclaw-sdk-tool)** — full API reference, error types, `ToolSdkError` variants
- **[Building extensions guide](https://github.com/donbader/anyclaw/blob/main/docs/building-extensions.md)** — end-to-end walkthrough for building and deploying a tool
- **[system-info reference implementation](https://github.com/donbader/anyclaw/tree/main/ext/tools/system-info)** — a minimal working tool binary

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
