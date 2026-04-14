# Architecture

**Analysis Date:** 2026-04-14

## Pattern Overview

**Overall:** Three-manager supervisor architecture with subprocess isolation

**Key Characteristics:**
- Supervisor orchestrates three independent managers (tools → agents → channels)
- All external processes (agents, channels, MCP tools) run as isolated subprocesses communicating via JSON-RPC 2.0 over stdio
- Inter-manager communication uses `tokio::sync::mpsc` channels via typed `ManagerHandle<C>` — no shared mutable state
- Crash isolation per-manager and per-channel with exponential backoff and crash loop detection

## Layers

**Binary Layer (entry point):**
- Purpose: CLI parsing, tracing init, supervisor bootstrap
- Location: `crates/anyclaw/src/`
- Contains: `main.rs` (entry), `cli.rs` (Clap derive), `supervisor.rs` (re-export), `init.rs`, `status.rs`, `banner.rs`
- Depends on: `anyclaw-supervisor`, `anyclaw-config`, `anyclaw-core`

**Supervisor Layer:**
- Purpose: Boot/shutdown orchestration, health monitoring, crash recovery
- Location: `crates/anyclaw-supervisor/src/lib.rs`
- Contains: `Supervisor` struct, `ManagerSlot`, `ManagerKind` enum, `create_manager()` factory, `admin_server` module
- Depends on: `anyclaw-agents`, `anyclaw-channels`, `anyclaw-tools`, `anyclaw-core`, `anyclaw-config`
- Key constant: `MANAGER_ORDER: [&str; 3] = ["tools", "agents", "channels"]` — boot order is load-bearing, DO NOT change

**Manager Layer:**
- Purpose: Domain-specific lifecycle management for tools, agents, and channels
- Location: `crates/anyclaw-tools/`, `crates/anyclaw-agents/`, `crates/anyclaw-channels/`
- All implement the `Manager` trait from `crates/anyclaw-core/src/manager.rs`
- Each manager owns its subprocess connections and internal state

**Core Layer:**
- Purpose: Shared primitives — Manager trait, backoff, crash tracking, cross-manager command types, session persistence
- Location: `crates/anyclaw-core/src/`
- Contains: `Manager` trait, `ManagerHandle<C>`, `ExponentialBackoff`, `CrashTracker`, `SessionStore` trait, ID newtypes
- Used by: All internal crates

**SDK Layer (external-facing):**
- Purpose: Traits and harnesses for external implementors building agents, channels, or tools
- Location: `crates/anyclaw-sdk-types/`, `crates/anyclaw-sdk-agent/`, `crates/anyclaw-sdk-channel/`, `crates/anyclaw-sdk-tool/`
- `anyclaw-sdk-types` is a dependency-free leaf crate with shared wire types
- Each SDK crate defines a trait + harness/server that handles JSON-RPC framing

**Config Layer:**
- Purpose: Figment-based config loading with defaults → YAML → env var layering
- Location: `crates/anyclaw-config/src/`
- Contains: `AnyclawConfig`, `AgentConfig`, `ChannelConfig`, `McpServerConfig`, binary path resolution

## Manager Implementations

**ToolsManager (`crates/anyclaw-tools/src/manager.rs`):**
- Spawns external MCP server subprocesses via `ExternalMcpServer` (`crates/anyclaw-tools/src/external.rs`)
- Loads WASM-sandboxed tools via `WasmToolRunner` (`crates/anyclaw-tools/src/wasm_runner.rs`) with per-invocation fuel budgets and WASI isolation
- Hosts an `AggregatedToolServer` implementing rmcp's `ServerHandler` — merges native, WASM, and external tools into a single MCP endpoint over HTTP (StreamableHttpService, stateful mode)
- Advertises tool URLs to agents via `ToolsCommand::GetMcpUrls`
- Receives commands via `ManagerHandle<ToolsCommand>`

**AgentsManager (`crates/anyclaw-agents/src/manager.rs`):**
- Manages agent subprocess lifecycle via `AgentConnection` (`crates/anyclaw-agents/src/connection.rs`)
- Implements ACP (Agent Client Protocol) over JSON-RPC 2.0 stdio: `initialize`, `session/new`, `session/prompt`, `session/cancel`, `session/load`, `session/resume`
- Supports two spawn backends: `LocalBackend` (`local_backend.rs`) and `DockerBackend` (`docker_backend.rs`)
- Multi-session model: `session_map: HashMap<SessionKey, String>` maps channel identity → ACP session ID
- Crash recovery: respawn → re-initialize → attempt `session/resume` (preferred) or `session/load` (fallback) → fresh session if both fail
- Session persistence via `SessionStore` trait (`crates/anyclaw-core/src/session_store.rs`) with SQLite implementation (`sqlite_session_store.rs`)
- Bridge-collapsed architecture: `spawn_with_bridge()` pushes incoming messages directly to manager's shared channel — no intermediate forwarding task
- Receives commands via `ManagerHandle<AgentsCommand>`, sends events to channels via `mpsc::Sender<ChannelEvent>`

**ChannelsManager (`crates/anyclaw-channels/src/manager.rs`):**
- Manages channel subprocesses via `ChannelConnection` (`crates/anyclaw-channels/src/connection.rs`)
- Per-channel crash isolation: each channel gets its own `ChannelSlot` with independent backoff and crash tracker
- Session-keyed routing: `routing_table: HashMap<SessionKey, RoutingEntry>` maps session key → (channel_id, acp_session_id, slot_index)
- Per-session FIFO message queue (`SessionQueue` in `crates/anyclaw-channels/src/session_queue.rs`) with two-phase collect+flush for message merging
- Port discovery: channel subprocesses emit `PORT:{n}` to stderr, forwarded via `watch::Receiver<u16>`
- Receives commands via `ManagerHandle<ChannelsCommand>`, receives events from agents via `mpsc::Receiver<ChannelEvent>`

## Data Flow

**Inbound message (user → agent):**

1. Channel subprocess receives user message (e.g., Telegram webhook)
2. Channel sends `channel/sendMessage` JSON-RPC notification to `ChannelsManager` via stdio
3. `ChannelsManager` looks up or creates session via `AgentsCommand::CreateSession` → `AgentsManager`
4. `AgentsManager` creates ACP session (`session/new`) if needed, passing MCP server URLs from `ToolsManager`
5. `ChannelsManager` dispatches `AgentsCommand::PromptSession` with merged message content
6. `AgentsManager` sends `session/prompt` JSON-RPC to agent subprocess

**Outbound message (agent → user):**

1. Agent subprocess emits `session/update` JSON-RPC notifications (streaming chunks)
2. `AgentConnection` reader task pushes `SlotIncoming` directly to manager's shared `incoming_rx`
3. `AgentsManager` translates updates into `ChannelEvent::DeliverMessage` → sends via `mpsc::Sender<ChannelEvent>`
4. `ChannelsManager` receives `ChannelEvent`, looks up routing table, sends `channel/deliverMessage` to correct channel subprocess
5. Channel subprocess delivers to end user (e.g., Telegram API)

**Completion signal (two-phase):**

1. Streaming result (`session/update` with `sessionUpdate: "result"`) → `DeliverMessage` to channels, sets `streaming_completed` flag
2. RPC response to `session/prompt` → drains remaining `incoming_rx`, sends `SessionComplete` (sole sender)

**Tool invocation (agent → tools → agent):**

1. Agent connects directly to the aggregated MCP endpoint URL provided during `session/new`
2. Tool calls go through rmcp's HTTP transport to `AggregatedToolServer`
3. `AggregatedToolServer` routes to native host tools first, then external MCP servers by name match

**State Management:**
- Session state: `SessionStore` trait with `SqliteSessionStore` (rusqlite, bundled) or `NoopSessionStore`
- Per-session message queue: `SessionQueue` in `ChannelsManager` (in-memory FIFO)
- Routing table: `HashMap<SessionKey, RoutingEntry>` in `ChannelsManager` (in-memory)

## Error Handling

**Strategy:** `thiserror` typed enums in library crates, `anyhow` only at application entry points

**Patterns:**
- Library crates define domain-specific error enums: `ManagerError` (`crates/anyclaw-core/src/error.rs`), `AgentsError` (`crates/anyclaw-agents/src/error.rs`), `ChannelsError` (`crates/anyclaw-channels/src/error.rs`), `ToolsError` (`crates/anyclaw-tools/src/error.rs`), `AcpError` (`crates/anyclaw-agents/src/acp_error.rs`)
- `SupervisorError` in `crates/anyclaw-supervisor/src/lib.rs` wraps `ManagerError` with manager name context
- `anyhow` permitted only in: `crates/anyclaw/src/main.rs`, `init.rs`, `status.rs`
- Failed external MCP servers and invalid WASM modules are logged and skipped — not fatal to startup
- Bad channel binaries log errors and continue with `connection: None` — don't block other channels
- Use `.expect("reason")` for true invariants, `?` for fallible paths, bare `.unwrap()` only in tests

## Cross-Cutting Concerns

**Logging:** `tracing` crate exclusively — spans and events, not `println!` or `log`. Exception: CLI entry points before tracing init may use `println!`/`eprintln!`
**Validation:** Config validation in `crates/anyclaw-config/src/validate.rs`; ACP protocol version check during `initialize` handshake
**Authentication:** Not handled by anyclaw — delegated to channel implementations (e.g., Telegram bot token)
**Metrics:** `metrics` crate with Prometheus exporter — `anyclaw_manager_restarts_total`, `anyclaw_agents_connected`, `anyclaw_channels_running`
**Health:** Admin HTTP server (`crates/anyclaw-supervisor/src/admin_server.rs`) exposes `HealthSnapshot` with per-agent and per-channel status

## Key Design Decisions

**Why three managers (not one monolith):** Each manager has independent crash isolation. A channel crash doesn't take down the agent. Boot order (tools → agents → channels) ensures dependencies are ready before dependents start. Reverse shutdown (channels → agents → tools) drains gracefully.

**Why subprocess isolation:** Agents, channels, and MCP tools run as separate OS processes. A misbehaving agent can't corrupt the sidecar. Language-agnostic — any binary speaking JSON-RPC 2.0 over stdio works. Docker backend support for containerized agents.

**Why JSON-RPC 2.0 over stdio:** Simple, language-agnostic IPC. No network port management for subprocess communication. Line-delimited JSON via `LinesCodec` (`crates/anyclaw-jsonrpc/src/codec.rs`). Bidirectional — both sides can initiate requests (e.g., agent requests permission from user).

**Why bridge-collapsed connection architecture:** Original design had an intermediate forwarding task between `AgentConnection` and manager. Two-hop latency caused premature `SessionComplete` when `try_recv()` saw empty channel while events were still in the bridge queue. `spawn_with_bridge()` eliminates this by pushing directly to manager's shared channel.

---

*Architecture analysis: 2026-04-14*
