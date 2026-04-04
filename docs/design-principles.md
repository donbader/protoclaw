# Protoclaw Design Principles

Design rationale and failure modes. Root AGENTS.md tells you WHAT exists; this doc tells you WHY.

## Core Invariants

1. **The agent must stay alive** — Crashes are expected. `ExponentialBackoff` (100ms base, doubles to 30s cap) + `CrashTracker` (5 crashes in 60s = crash loop). Supervisor detects finished join handles in its health loop and respawns with backoff. Only a crash loop stops retries.

2. **No shared mutable state between managers** — All cross-manager communication via `ManagerHandle<C>` (typed `mpsc::Sender<C>` wrapper). No `Arc<Mutex<>>` across manager boundaries. Data flow is explicit: commands go in, results come back via `oneshot` reply channels.

3. **Ordered lifecycle** — Boot: tools → agents → channels. Shutdown: reverse. `MANAGER_ORDER` in `supervisor.rs` is load-bearing — tools must be ready so agents get MCP URLs during `session/new`, agents must be ready so channels can route messages. Tests verify both orders explicitly.

4. **Typed errors at boundaries** — `thiserror` enums per crate (`ConfigError`, `ManagerError`, `AgentsError`, etc.). `anyhow` only at application edges (`main.rs`, `supervisor::run()`, `init.rs`, `status.rs`). Never cross a crate boundary with `anyhow::Error`.

5. **Subprocess isolation** — Agents, channels, and MCP tools run as separate processes communicating over stdio. A crash in any subprocess doesn't take down the supervisor. Each gets independent crash recovery via `ManagerSlot` (own `CancellationToken`, `ExponentialBackoff`, `CrashTracker`).

## Architecture Rationale

### Why Three Managers?

Each manager owns a distinct subprocess lifecycle with different communication patterns:

- **ToolsManager** starts MCP servers and holds their URLs. Must be ready first — agents need MCP URLs during `session/new` to tell the agent what tools are available.
- **AgentsManager** spawns the agent process and manages the ACP protocol lifecycle (initialize → session/new → prompt loop). Must be ready before channels — channels route user messages to agents.
- **ChannelsManager** spawns channel subprocesses (debug-http, telegram) and routes messages bidirectionally. Last to boot because it needs both tools (indirectly via agent) and agents ready.

Separating them means each can crash and recover independently. A Telegram channel crash doesn't affect the agent session. An MCP server going down doesn't kill the channel connections.

### Why ACP Over Stdio?

- **Subprocess isolation** — Agent crash doesn't take down the supervisor. The supervisor detects EOF on stdout and respawns.
- **NDJSON framing is simple** — One JSON object per line, no Content-Length headers, no HTTP overhead. Debuggable with `cat`.
- **No port allocation** — Stdio pipes are created by the OS at spawn time. No port conflicts, no firewall rules, works in containers without network config.
- **Language-agnostic** — Any process that reads/writes NDJSON on stdio can be an ACP agent. The protocol doesn't care about the implementation language.

### Why External Channel/Tool Binaries?

- **Language freedom** — Channel implementors don't need Rust. Write a Telegram bot in Python, a Slack bot in TypeScript — as long as it speaks JSON-RPC over stdio, protoclaw can manage it.
- **SDK crates provide the harness** — `ChannelHarness` and `ToolServer` handle all the JSON-RPC framing, initialize handshake, and message routing. Implementors only write business logic (the `Channel` or `Tool` trait).
- **Crash isolation** — A buggy channel binary crashes without affecting other channels or the agent. The supervisor restarts it independently.
- **Deployment flexibility** — Channel binaries can be pre-built, distributed separately, or resolved at runtime via `@built-in/` prefix against `extensions_dir`.

### Why ChannelEvent Lives in protoclaw-core?

`ChannelEvent` is the agents→channels message type (deliver message, route permission). Both `protoclaw-agents` and `protoclaw-channels` need it. Putting it in either crate would create a circular dependency. `protoclaw-core` is the shared foundation — both crates already depend on it.

## Failure Mode Catalog

| Failure | Detection | Recovery | Impact |
|---------|-----------|----------|--------|
| Agent process exits | stdout EOF in reader task | `ExponentialBackoff` respawn, `session/load` or `session/new` | Messages queued during recovery |
| Agent crash loop | `CrashTracker`: 5 crashes in 60s | Log error, stop retrying | Agent offline until manual intervention |
| Channel subprocess exits | Finished join handle in health check | Supervisor restarts with backoff | Channel temporarily unavailable |
| Channel crash loop | `CrashTracker` threshold exceeded | Stop retrying, log error | Channel offline until restart |
| MCP server unreachable | TCP connection refused in `McpHost` | Log error, agent gets empty tool list | Agent works without tools |
| Config file missing | Figment extraction fails at startup | Supervisor refuses to boot with clear error | No startup — intentional |
| Config validation fails | `validate()` catches bad binaries, duplicates | Boot aborted with specific error message | No startup — intentional |
| SIGTERM/SIGINT received | Signal handler in supervisor | Cancel root token → cascades to all child tokens → ordered shutdown | Clean exit |
| Manager `start()` fails | Error return from `boot_managers()` | Shutdown already-booted managers, exit | No partial boot state |

## Anti-Pattern Catalog

Each anti-pattern from root AGENTS.md with the reasoning behind it:

### No `unsafe`

The codebase proves safe Rust handles all use cases — async subprocess management, channel routing, codec framing. Introducing `unsafe` creates audit burden disproportionate to any performance gain. Zero unsafe blocks exist across ~12,700 lines.

### No shared mutable state between managers

Deadlock debugging in async Rust is extremely painful. `Arc<Mutex<>>` across manager boundaries creates invisible coupling — you can't reason about data flow by reading one manager's code. `mpsc` channels make data flow explicit, testable, and visible in type signatures (`ManagerHandle<AgentsCommand>`).

### No `anyhow` in library crates

Typed errors force callers to handle specific failure modes. `anyhow` erases this — callers must downcast or string-match, making error handling a guessing game. Each crate defines its own error enum (`ConfigError`, `ManagerError`, `AgentsError`) so callers know exactly what can go wrong.

### No bare `.unwrap()`

`.expect("reason")` documents the invariant. When it fires in production, the panic message tells you what assumption was violated (e.g., `.expect("piped stdin")` — you know the stdio pipe wasn't set up). Bare `.unwrap()` gives you only a file:line, requiring source code access to understand the failure.

### Do not change `MANAGER_ORDER`

Boot order is `tools → agents → channels` because:
1. Agents need tool URLs from ToolsManager during `session/new`
2. Channels need agents ready to route messages to

Shutdown is reverse (`channels → agents → tools`) because:
1. Channels should stop accepting messages first
2. Agents should finish in-flight work before tools disappear

Tests verify both orders explicitly. Reordering causes initialization races that may only manifest under load.

### Do not call `run()` without `start()`

`start()` does synchronous setup — spawn subprocess, bind port, create connections. `run()` enters the async event loop consuming `self`. Skipping `start()` means the event loop has nothing to process. The two-phase design lets the supervisor detect boot failures before committing to the run loop.

### Do not call `run()` twice

`cmd_rx` (the command receiver) is stored as `Option<Receiver>` and consumed via `.take()` on first `run()` call. A second `run()` would `.take()` a `None` and panic. This is intentional — `run()` consumes `self`, so the type system prevents it in normal usage. The `.take()` pattern exists because `start()` takes `&mut self` (needs to set up state) while `run()` takes `self` (consumes the manager).

### Do not remove the 50ms sleep in `poll_channels()`

Without it, the `select!` loop busy-spins when no messages are pending. The sleep yields back to the tokio runtime. This was a TDD-caught bug — `pending().await` (the original approach) permanently blocked the select branch after the first timeout.

---

*Last updated: 2026-04-04*
