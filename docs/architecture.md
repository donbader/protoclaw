# Architecture

System overview and design of the protoclaw supervisor.

## Overview

Protoclaw is an infrastructure sidecar: it runs alongside your AI agent binary and handles subprocess management, message routing, crash recovery, and tool access. You bring the intelligence; protoclaw handles the plumbing.

```
                        ┌─────────────────────────────────┐
                        │           Supervisor             │
                        │                                  │
                        │  boot: tools → agents → channels │
                        │  shutdown: reverse order         │
                        └───────────────┬─────────────────┘
                                        │
          ┌─────────────────────────────┼──────────────────────────┐
          │                             │                          │
 ┌────────▼──────────┐      ┌───────────▼──────────┐   ┌──────────▼──────────┐
 │   ToolsManager    │      │    AgentsManager      │   │  ChannelsManager    │
 │                   │      │                       │   │                     │
 │  McpHost          │      │  ACP subprocess       │   │  Telegram           │
 │  WasmToolRunner   │◄─────│  (JSON-RPC / stdio)   │◄──│  debug-http         │
 └───────────────────┘      └───────────────────────┘   └─────────────────────┘
         ▲                            ▲                          │
         │ tool URLs at session/new   │ route messages           │ user messages
         └────────────────────────────┘◄─────────────────────────┘
```

Each manager owns one subprocess domain. Communication between managers is exclusively through typed `mpsc` channels via `ManagerHandle<C>`. No shared mutable state crosses manager boundaries.

## The Three Managers

### ToolsManager

Starts and monitors MCP servers and WASM tool runners. Holds the tool URLs that agents need during `session/new` to discover available tools. Must boot first.

- **MCP host** (`protoclaw-tools/src/mcp_host.rs`): Manages connections to external MCP server processes.
- **WASM runner** (`protoclaw-tools/src/wasm_runner.rs`): Executes WASM tools in an isolated sandbox.

### AgentsManager

Spawns the agent subprocess and manages the full ACP protocol lifecycle: `initialize` → `session/new` → prompt loop. Must boot after tools (needs tool URLs) and before channels (channels route to agents).

The ACP protocol is JSON-RPC 2.0 over stdio (NDJSON — one object per line). Each subprocess gets its own `ManagerSlot` with independent crash recovery.

### ChannelsManager

Spawns channel subprocesses (Telegram, debug-http) and routes messages bidirectionally. Boots last because it needs agents ready to accept messages.

Channel subprocesses receive all configuration through the JSON-RPC `initialize` handshake (`ChannelInitializeParams.options`), not environment variables. This makes channels testable and config-driven.

## ACP Protocol

ACP (Agent Communication Protocol) is JSON-RPC 2.0 over stdio, framed as newline-delimited JSON. The protocol lifecycle:

1. **`initialize`** — Supervisor sends capabilities to agent; agent responds with its capabilities.
2. **`session/new`** — Create a new conversation session. Tool URLs are provided at this point.
3. **`session/load`** — Restore an existing session after agent restart.
4. **Prompt loop** — `prompt/send` → agent processes → `prompt/response`. Agent may call tools via MCP during processing.

Framing is line-delimited: each JSON-RPC message is one line. No Content-Length headers, no HTTP overhead. Debuggable with `cat`.

## Manager Communication

Managers never import each other's crates. All cross-manager communication uses typed command enums sent through `ManagerHandle<C>`:

```
AgentsManager                           ChannelsManager
     │                                        │
     │  AgentsCommand via ManagerHandle       │
     │◄───────────────────────────────────────┤
     │                                        │
     │  ChannelEvent via AgentDispatch trait  │
     │───────────────────────────────────────►│
```

Reply channels use `tokio::oneshot`. The `AgentDispatch` trait in `protoclaw-channels` abstracts agent interaction without creating a crate dependency on `protoclaw-agents`.

## Boot and Shutdown Order

Boot order is load-bearing. `MANAGER_ORDER` in `supervisor.rs` is a constant:

```
BOOT:     tools → agents → channels
SHUTDOWN: channels → agents → tools
```

**Why this order:**

1. Tools must be ready first — agents need MCP URLs during `session/new`.
2. Agents must be ready before channels — channels route user messages to agents.
3. On shutdown, channels stop accepting messages first, then agents finish in-flight work, then tools stop.

Tests verify both boot and shutdown order explicitly. Do not reorder.

## Crash Recovery

Every subprocess slot (`ManagerSlot`) has independent crash recovery:

- **`ExponentialBackoff`**: 100ms base, doubles up to 30s cap. Applied between respawn attempts.
- **`CrashTracker`**: 5 crashes within 60s = crash loop. Supervisor stops retrying and logs an error.
- **Supervisor health loop**: Polls join handles each tick. Finished handle = subprocess exited = trigger respawn.

A crash in any subprocess doesn't affect others. A Telegram channel crash doesn't interrupt the agent session.

## Crate Dependency Flow

```
protoclaw (binary)
├── protoclaw-config
├── protoclaw-core
├── protoclaw-agents ──→ protoclaw-core, protoclaw-jsonrpc
├── protoclaw-channels ─→ protoclaw-core, protoclaw-jsonrpc, protoclaw-sdk-types
└── protoclaw-tools ───→ protoclaw-core

SDK crates (for external implementors):
├── protoclaw-sdk-types  (leaf — no internal deps)
├── protoclaw-sdk-agent ──→ sdk-types, jsonrpc
├── protoclaw-sdk-channel ─→ sdk-types, jsonrpc
└── protoclaw-sdk-tool ───→ sdk-types
```

`protoclaw-sdk-types` is the dependency-free leaf crate. All shared wire types (`ChannelEvent`, `SessionKey`, channel wire types) live there to avoid circular dependencies between `protoclaw-agents` and `protoclaw-channels`.

## Config System

Config uses Figment with three layers (later layers override earlier ones):

1. **Defaults** — compiled-in defaults (`log_level: "info"`, `extensions_dir: "/usr/local/bin"`)
2. **YAML file** — `protoclaw.yaml` in the working directory
3. **Environment variables** — `PROTOCLAW_` prefix, `__` as separator (e.g., `PROTOCLAW_LOG_LEVEL`)

`@built-in/` binary paths in config are resolved against `extensions_dir` by the supervisor before managers are constructed.
