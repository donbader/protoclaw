# anyclaw-tools — MCP Host + WASM Sandbox

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
| External MCP | Subprocess binary | `McpServerConfig` in `anyclaw.yaml` |

## AggregatedToolServer

Implements rmcp's `ServerHandler` trait. Aggregates tools from all three sources into a single MCP endpoint:
- `list_tools()` — merges native host tools + external server tools
- `call_tool()` — routes to native host first, then external servers by name match

Served over HTTP via rmcp's `StreamableHttpService` (stateful mode) on a random port bound to `0.0.0.0`. The advertised URL uses `tools_server_host` from `ToolsManagerConfig` (default `127.0.0.1`; set to the container hostname in Docker deployments so agent containers can reach it). The URL is registered in `server_urls` so `AgentsManager` can pass it to agents via `session/new` → `mcp_servers`. Each configured external MCP tool gets its own `McpServerUrl` entry pointing to the shared aggregated endpoint.

**StreamableHttpServerConfig requirements:**
- `stateful_mode = true` is mandatory — without it, rmcp treats each HTTP request as independent, breaking multi-turn tool conversations that rely on session state
- `cancellation_token` ties the MCP server lifecycle to the tools manager's cancel signal for clean shutdown

## WASM Sandbox Model

- `WasmToolRunner` creates a shared `wasmtime::Engine` (compilation cache)
- Each tool invocation gets a fresh `Store` with:
  - Fuel budget (CPU instruction limit)
  - WASI context (controlled stdio, filesystem preopens)
  - Epoch interruption (wall-clock timeout)
- Invalid WASM modules are skipped with a warning — they don't block startup

## Anti-Patterns (this crate)

- Failed external MCP servers and invalid WASM modules are logged and skipped, not fatal
- `cmd_rx` uses `unwrap_or_else` fallback (creates dummy channel) unlike other managers — this is intentional because tools manager can run without external commands
- `start()` skips MCP servers with `enabled = false` — no spawn attempt for disabled servers
- `start()` is annotated with `#[tracing::instrument]` — do not remove it; it provides a root span for the entire tool startup sequence visible in distributed traces
