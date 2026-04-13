# anyclaw-sdk-tool — Tool SDK

SDK for building MCP tool extensions. Provides the `Tool` trait for tool logic and `ToolServer` that wraps rmcp's `ServerHandler` to serve tools over MCP stdio.

## Files

| File | Purpose |
|------|---------|
| `trait_def.rs` | `Tool` trait — implement this to build a tool |
| `server.rs` | `ToolServer` — MCP server wrapping rmcp `ServerHandler`, dispatches to `Tool` implementations |
| `error.rs` | `ToolSdkError` enum (thiserror) |
| `lib.rs` | Re-exports `Tool`, `ToolServer`, `ToolSdkError`, `schemars` |

## Key Types

```rust
#[async_trait]
pub trait Tool: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;
    async fn execute(&self, input: Value) -> Result<Value, ToolSdkError>;
}

pub struct ToolServer {
    tools: HashMap<String, Box<dyn Tool>>,
    server_info: ServerInfo,
}
```

## How to Implement

1. Create a struct implementing `Tool`
2. Return tool metadata: `name()`, `description()`, `input_schema()` (JSON Schema object)
3. Implement `execute()` — receives input as `Value`, returns output as `Value`
4. Register tools with `ToolServer::new(vec![Box::new(MyTool)])` 
5. Serve with `tool_server.serve_stdio().await`

**Example:**
```rust
struct MyTool;

#[async_trait]
impl Tool for MyTool {
    fn name(&self) -> &str { "my-tool" }
    fn description(&self) -> &str { "Does something useful" }
    fn input_schema(&self) -> Value { json!({"type": "object"}) }
    async fn execute(&self, input: Value) -> Result<Value, ToolSdkError> {
        Ok(json!({"result": "done"}))
    }
}

#[tokio::main]
async fn main() {
    ToolServer::new(vec![Box::new(MyTool)]).serve_stdio().await.unwrap();
}
```

## ToolServer Internals

- Implements rmcp `ServerHandler` — handles `list_tools` and `call_tool` MCP methods
- `build_tool_list()` converts `Tool` metadata to rmcp `RmcpTool` structs
- `dispatch_tool()` routes by name, converts `Tool::execute()` results to `CallToolResult`
- Error from `execute()` returns `CallToolResult::error` (not MCP protocol error) — tool failures are content, not protocol failures

## Anti-Patterns (this crate)

- **Don't depend on internal crates** — this is external-facing SDK
- **Don't handle MCP framing manually** — `ToolServer` + rmcp handle all protocol details
- **Don't return protocol errors for tool failures** — use `ToolSdkError::ExecutionFailed`, which becomes `CallToolResult::error` (content-level error, not MCP error)
