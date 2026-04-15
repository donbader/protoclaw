# ext/tools/ — Building Tool Extensions

Tool extensions are standalone binaries that implement the MCP (Model Context Protocol) over stdio. The `ToolsManager` spawns them as child processes and aggregates their tools into a single HTTP endpoint that agents connect to.

The `anyclaw-sdk-tool` crate provides the `Tool` trait and `ToolServer` harness — implement the trait, register your tools, and the SDK handles all MCP protocol framing.

## Why ext/ and not crates/

These are standalone binaries, not libraries. They depend on SDK crates but are architecturally separate — they're spawned as child processes with piped stdio. Putting them in `ext/` makes the subprocess boundary explicit.

## How It Works

```
Agent ──HTTP──▶ AggregatedToolServer ──stdio──▶ Your Tool Binary
                (ToolsManager)                   (MCP over rmcp)
```

1. `ToolsManager` spawns your binary as a subprocess
2. rmcp handles MCP protocol negotiation over stdio
3. Your tools are merged with all other tools into a single aggregated HTTP endpoint
4. Agents receive the endpoint URL in `session/new` and connect directly

You never handle HTTP or protocol framing — just implement `Tool` and let the SDK do the rest.

## Implementing a Tool

### 1. Create the binary

```
ext/tools/my-tool/
├── Cargo.toml
└── src/
    └── main.rs
```

### Cargo.toml

```toml
[package]
name = "my-tool"
edition.workspace = true
rust-version.workspace = true
publish = false

[dependencies]
anyclaw-sdk-tool = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true, features = ["full"] }
```

Add to workspace members in root `Cargo.toml`.

### main.rs — Tool trait implementation

```rust
// D-03: Tool trait I/O is serde_json::Value by design — JSON Schema input has no fixed
// Rust type, and tool output is arbitrary JSON. See crates/anyclaw-sdk-tool/src/trait_def.rs.
#![allow(clippy::disallowed_types)]

use anyclaw_sdk_tool::{Tool, ToolSdkError, ToolServer};

struct MyTool;

impl Tool for MyTool {
    fn name(&self) -> &str {
        "my-tool"
    }

    fn description(&self) -> &str {
        "Does something useful"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The query to process"
                }
            },
            "required": ["query"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, input: serde_json::Value) -> Result<serde_json::Value, ToolSdkError> {
        let query = input.get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Your tool logic here
        Ok(serde_json::json!({ "result": format!("processed: {query}") }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ToolServer::new(vec![Box::new(MyTool)])
        .serve_stdio()
        .await
}
```

### Tool trait contract

| Method | Returns | Purpose |
|--------|---------|---------|
| `name()` | `&str` | Unique tool name — used for routing in `call_tool` |
| `description()` | `&str` | Human-readable description shown to agents |
| `input_schema()` | `Value` | JSON Schema object describing expected input |
| `execute(Value)` | `Result<Value, ToolSdkError>` | Run the tool — input matches your schema, output is arbitrary JSON |

- `execute()` errors become `CallToolResult::error` (content-level), not MCP protocol errors
- Return `ToolSdkError::ExecutionFailed("reason")` for tool failures
- Return `ToolSdkError::InvalidInput("reason")` for bad input

### Multiple tools in one binary

A single binary can serve multiple tools. Register them all with `ToolServer::new()`:

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ToolServer::new(vec![
        Box::new(MyTool),
        Box::new(AnotherTool),
    ])
    .serve_stdio()
    .await
}
```

Each tool must have a unique `name()`. The `ToolServer` dispatches `call_tool` requests by name match.

## Configuration

Tool extensions are configured in `anyclaw.yaml` under the `tools.mcp_servers` key:

```yaml
tools:
  mcp_servers:
    my-tool:
      binary: /path/to/my-tool          # or @built-in/tools/my-tool
      args: []
      enabled: true
      description: "Does something useful"
      options:                           # arbitrary key-value, set as env vars on subprocess
        MY_API_KEY: "${MY_API_KEY}"
```

- `@built-in/tools/<name>` resolves to `{extensions_dir}/tools/<name>`
- `options` are set as environment variables on the spawned subprocess
- `args` are passed as CLI arguments to the binary
- If the binary fails to start or MCP negotiation fails, it's logged and skipped — not fatal to startup

### WASM tools

Tools can also be WASM modules (sandboxed via wasmtime). These use a different config shape:

```yaml
tools:
  wasm_tools:
    my-wasm-tool:
      module: /path/to/tool.wasm
      description: "Sandboxed tool"
      input_schema: '{"type":"object","properties":{"input":{"type":"string"}}}'
      sandbox:
        fuel_limit: 1000000
        epoch_timeout_secs: 30
        memory_limit_bytes: 67108864    # 64MB
        preopened_dirs: []
```

WASM tools receive input via stdin and write output to stdout. Each invocation gets a fresh store with fuel budget and epoch timeout.

## Testing

Unit test your `Tool` implementation directly — no need for MCP framing:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_name() {
        assert_eq!(MyTool.name(), "my-tool");
    }

    #[test]
    fn tool_input_schema_is_object() {
        let schema = MyTool.input_schema();
        assert!(schema.is_object());
        assert_eq!(schema["type"], "object");
    }

    #[tokio::test]
    async fn execute_returns_expected_output() {
        let result = MyTool
            .execute(serde_json::json!({ "query": "hello" }))
            .await
            .expect("execute should succeed");
        assert_eq!(result["result"], "processed: hello");
    }

    #[tokio::test]
    async fn execute_handles_missing_field() {
        let result = MyTool
            .execute(serde_json::json!({}))
            .await
            .expect("execute should succeed");
        assert_eq!(result["result"], "processed: ");
    }
}
```

Test naming follows BDD convention: `when_action_then_result` or `given_precondition_when_action_then_result`. Use `rstest` for parameterized tests.

For integration testing through the full supervisor pipeline, add your tool binary to the build step and configure it in the test's `anyclaw.yaml`.

## Anti-Patterns

- **Don't handle MCP framing manually** — `ToolServer` + rmcp handle all protocol details. Implement `Tool` only.
- **Don't return MCP protocol errors for tool failures** — use `ToolSdkError::ExecutionFailed`, which becomes `CallToolResult::error` (content-level). Protocol errors (`McpError`) are only for unknown tool names.
- **Don't depend on internal crates** — only use `anyclaw-sdk-tool` and `anyclaw-sdk-types`. SDK crates are the public API boundary.
- **Don't read env vars for config in the tool logic** — config options from `anyclaw.yaml` are set as env vars on the subprocess by the `ToolsManager`. If you need runtime config, read env vars set by the supervisor, not arbitrary system env vars.
- **Don't write to stderr for protocol messages** — stderr is captured for logging only. All MCP communication goes through stdout/stdin via rmcp.
- **Don't use `println!`** — rmcp owns stdout for MCP framing. Debug output goes to stderr or `tracing`.
