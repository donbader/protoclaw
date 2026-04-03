# protoclaw-tools — MCP Host + WASM Sandbox

Manages tool availability: spawns external MCP servers, loads WASM-sandboxed tools, and serves an aggregated MCP endpoint over TCP.

## Files

| File | Purpose |
|------|---------|
| `manager.rs` | `ToolsManager` + `AggregatedToolServer` (rmcp `ServerHandler` impl) |
| `mcp_host.rs` | `McpHost` — registry for native `Tool` impls, dispatches calls |
| `wasm_runner.rs` | `WasmToolRunner` — shared wasmtime `Engine`, per-invocation `Store` with fuel/WASI |
| `wasm_tool.rs` | `WasmTool` — wraps a WASM module as a `Tool` impl |
| `external.rs` | `ExternalMcpServer` — spawns external MCP server subprocess via rmcp client |
| `error.rs` | `ToolsError` |

## Tool Types

| Type | Source | Registration |
|------|--------|-------------|
| Native | Rust `impl Tool` | `with_native_tools(vec![...])` |
| WASM | `.wasm` module file | `with_wasm_configs(vec![...])` from config |
| External MCP | Subprocess binary | `McpServerConfig` in `protoclaw.toml` |

## AggregatedToolServer

Implements rmcp's `ServerHandler` trait. Aggregates tools from all three sources into a single MCP endpoint:
- `list_tools()` — merges native host tools + external server tools
- `call_tool()` — routes to native host first, then external servers by name match

## TCP Listener (INCOMPLETE)

`start()` binds `127.0.0.1:0` and spawns an accept loop, but the TODO at line 235 means accepted TCP streams are NOT wired to the MCP protocol yet. The URL is still passed to agents via `GetMcpUrls` — agents will get the URL but tool calls over TCP won't work until Phase 7+.

## WASM Sandbox Model

- `WasmToolRunner` creates a shared `wasmtime::Engine` (compilation cache)
- Each tool invocation gets a fresh `Store` with:
  - Fuel budget (CPU instruction limit)
  - WASI context (controlled stdio, filesystem preopens)
  - Epoch interruption (wall-clock timeout)
- Invalid WASM modules are skipped with a warning — they don't block startup

## Anti-Patterns (this crate)

- Do not assume tools are callable end-to-end over TCP — the accept loop is a stub
- Failed external MCP servers and invalid WASM modules are logged and skipped, not fatal
- `cmd_rx` uses `unwrap_or_else` fallback (creates dummy channel) unlike other managers — this is intentional because tools manager can run without external commands
- `start()` skips MCP servers with `enabled = false` — no spawn attempt for disabled servers
