<!-- GSD:project-start source:PROJECT.md -->
## Project

**Protoclaw**

Infrastructure sidecar that connects AI agents to the outside world. Protoclaw wraps any ACP-speaking agent process (opencode, claude-code, gemini-cli) and gives it channels to talk to users (Telegram, Slack, Discord) and tools to act on the world (MCP servers, WASM-sandboxed tools). The agent is a black box — protoclaw just keeps the pipes connected.

**Core Value:** The agent must stay alive, connected to channels, and able to call tools — regardless of crashes, restarts, or network issues. Protoclaw is infrastructure plumbing, not AI logic.

### Constraints

- **Language**: Rust — systems-level reliability required for infrastructure sidecar
- **Protocol**: ACP (Agent Client Protocol) — JSON-RPC 2.0 over stdio, non-negotiable
- **Architecture**: Three-manager pattern with single Supervisor — established in idea doc
- **Sandboxing**: WASM for custom tools — security boundary for user-defined code
- **Process model**: Channels as subprocesses, agent as long-lived process — one agent per workspace
<!-- GSD:project-end -->

<!-- GSD:stack-start source:research/STACK.md -->
## Technology Stack

## Recommended Stack
### Core Technologies
| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| Rust | 2024 edition | Implementation language | Systems reliability, memory safety without GC, async/await maturity. Non-negotiable per project constraints. |
| Tokio | 1.49+ | Async runtime | De facto standard for async Rust. Built-in subprocess management (`tokio::process`), channels (`mpsc`, `broadcast`, `watch`), timers, signal handling. Every major Rust async library builds on Tokio. 17.9M+ downloads. |
| rmcp | 1.3.0 | MCP protocol SDK | Official Rust SDK from `modelcontextprotocol/rust-sdk`. 6.5M+ downloads, 792 reverse deps. Provides both client and server with stdio transport (`transport-io`, `transport-child-process`), `#[tool]` macros, JSON Schema generation. Handles all MCP JSON-RPC 2.0 framing. |
| Wasmtime | 38+ | WASM sandbox runtime | Bytecode Alliance reference implementation. Best WASI support, fuel metering for CPU limits, epoch-based interruption for timeouts, component model support. Industry standard for embedding WASM in Rust. |
| serde | 1.x | Serialization framework | Universal Rust serialization. Derive macros for zero-boilerplate JSON-RPC message types. Required by rmcp, wasmtime, and virtually every crate in the stack. |
| serde_json | 1.x | JSON serialization | The JSON implementation for serde. Handles all JSON-RPC 2.0 message encoding/decoding. `Value` type for dynamic JSON when needed. |
### Async & Subprocess Management
| Library | Version | Purpose | Why |
|---------|---------|---------|-----|
| tokio | 1.49+ (features: full) | Async runtime + subprocess | `tokio::process::Command` provides async spawn, piped stdin/stdout/stderr, `wait_with_output()`, signal forwarding. Exactly what's needed for agent/channel subprocess lifecycle. |
| tokio-util | 0.7+ | Codec framing | `LinesCodec` or custom codec for JSON-RPC line-delimited framing over stdio pipes. Essential for the ACP protocol layer. |
| futures | 0.3+ | Stream/Sink combinators | `StreamExt`, `SinkExt` for composing async message flows between managers. |
### Protocol & Messaging
| Library | Version | Purpose | Why |
|---------|---------|---------|-----|
| rmcp | 1.3.0 (features: client, server, transport-io, transport-child-process, macros, schemars) | MCP server/client | Handles Tools Manager's MCP servers and connecting to external MCP servers. `TokioChildProcess` transport for spawning MCP server subprocesses. `stdio` transport for serving. |
| serde_json | 1.x | JSON-RPC message encoding | All ACP and channel protocol messages are JSON-RPC 2.0. Custom ACP types derive `Serialize`/`Deserialize`. |
| jsonschema | 0.28+ | JSON Schema validation | Validate tool input schemas, MCP tool definitions. Optional but recommended for runtime safety. |
### WASM Sandboxing
| Library | Version | Purpose | Why |
|---------|---------|---------|-----|
| wasmtime | 38+ | WASM execution engine | Fuel consumption for CPU limits, epoch interruption for timeouts, WASI for filesystem/network access control. Component model for typed interfaces. |
| wasmtime-wasi | 38+ | WASI host implementation | Provides `WasiCtxBuilder` for fine-grained capability control: inherit or deny stdio, filesystem preopens, env vars, network. Essential for sandboxing custom tools. |
### Supporting Libraries
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| thiserror | 2.x | Typed error enums | Library-style error types for each manager (AgentError, ChannelError, ToolError). Derive `Error` + `Display` with zero boilerplate. Use for all public API boundaries. |
| anyhow | 1.x | Ergonomic error propagation | Application-level error handling in main, CLI entry points, and test code. NOT for library boundaries — use thiserror there. |
| tracing | 0.1+ | Structured diagnostics | Spans for request lifecycle (message in → agent → response out), events for state changes. Async-aware, integrates with tokio. Industry standard over `log` crate. |
| tracing-subscriber | 0.3+ | Log output formatting | `fmt` layer for human-readable dev output, `json` layer for production. `EnvFilter` for runtime log level control via `RUST_LOG`. |
| figment | 0.10+ | Configuration management | Layered config: defaults → TOML file → env vars → CLI args. Serde-based, type-safe. Supports workspace-level and global config. 23M+ downloads. |
| toml | 0.8+ | TOML parsing | Config file format. Human-readable, standard for Rust ecosystem (Cargo.toml precedent). Used as figment provider. |
| uuid | 1.x | Unique identifiers | Session IDs, request IDs for JSON-RPC correlation. `v4` feature for random UUIDs. |
| bytes | 1.x | Byte buffer management | Efficient zero-copy byte handling for stdio pipe I/O. Required by tokio-util codecs. |
| dashmap | 6.x | Concurrent hash map | Session registry, channel routing tables. Lock-free reads for hot paths (message routing). |
| notify | 7.x | Filesystem watching | Watch config files for hot-reload. Watch workspace for tool changes. Optional but valuable for long-running sidecar. |
| signal-hook | 0.3+ | Unix signal handling | Graceful shutdown on SIGTERM/SIGINT. Coordinate supervisor teardown sequence (Channels → Agents → Tools). |
### Development Tools
| Tool | Purpose | Notes |
|------|---------|-------|
| cargo-nextest | Fast test runner | Parallel test execution, better output than `cargo test`. Use for CI and local dev. |
| cargo-watch | Auto-rebuild on change | `cargo watch -x check -x test` for rapid feedback loop during development. |
| cargo-deny | Dependency auditing | License compliance, vulnerability scanning, duplicate detection. Run in CI. |
| cargo-clippy | Linting | Catch common mistakes, enforce idioms. Use `#![deny(clippy::all)]` in CI. |
| cargo-llvm-cov | Code coverage | LLVM-based coverage for accurate reporting. Integrates with nextest. |
| just | Task runner | Replaces Makefiles. Define `just test`, `just run`, `just lint` recipes. Rust ecosystem standard. |
## Installation
# Cargo.toml — Core dependencies
# Optional
## Alternatives Considered
| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Async runtime | Tokio | async-std | async-std has stalled development, smaller ecosystem. Tokio has 90%+ market share in async Rust. rmcp and wasmtime-wasi both depend on Tokio — using anything else means fighting the ecosystem. |
| WASM runtime | Wasmtime | Wasmer | Wasmer is more "batteries-included" but less standards-compliant. Wasmtime has better WASI support, is the Bytecode Alliance reference impl, and leads in JIT/AOT benchmarks (2026). Wasmer's advantage is package registry (wapm) which we don't need. |
| WASM runtime | Wasmtime | WasmEdge | WasmEdge targets edge/cloud-native use cases. Weaker Rust embedding API. Wasmtime's fuel/epoch metering is more mature for our sandboxing needs. |
| MCP SDK | rmcp | rust-mcp-sdk | rust-mcp-sdk (v0.9) is community-maintained, fewer downloads (not in millions). rmcp is the official SDK from modelcontextprotocol org, 6.5M+ downloads, 792 reverse deps. Clear winner. |
| MCP SDK | rmcp | Hand-rolled JSON-RPC | MCP protocol has enough surface area (tools, resources, prompts, sampling, subscriptions) that hand-rolling is a maintenance burden. rmcp handles framing, transport, and schema generation. |
| JSON-RPC (ACP layer) | Hand-rolled on serde_json | jsonrpsee | jsonrpsee is designed for HTTP/WebSocket JSON-RPC servers. ACP is stdio-based with custom message types (session/new, session/update). jsonrpsee's transport model doesn't fit. Thin custom layer on serde_json + tokio-util codecs is simpler and more correct. |
| Config | figment | config-rs | config-rs works but figment has better ergonomics, stronger typing, and is more actively maintained (23M downloads). figment's provider model maps cleanly to our layered config needs. |
| Config format | TOML | YAML | TOML is the Rust ecosystem standard (Cargo.toml). Simpler, less error-prone than YAML's implicit typing. Users expect TOML in Rust tools. |
| Error handling | thiserror + anyhow | eyre + color-eyre | eyre is excellent for CLI apps with pretty error reports. But protoclaw is a sidecar/daemon — errors go to logs, not terminals. thiserror's typed enums are better for programmatic error handling between managers. |
| Concurrency map | dashmap | std RwLock<HashMap> | dashmap provides lock-free reads which matter for the hot path (routing messages to channels). RwLock contention under concurrent reads is measurable at scale. |
| Tracing | tracing | log + env_logger | `tracing` provides spans (request lifecycle tracking across async boundaries), structured fields, and layers. `log` is fire-and-forget events only. For a multi-manager system with async message routing, spans are essential for debugging. |
## What NOT to Use
| Avoid | Why | Use Instead |
|-------|-----|-------------|
| async-std | Stalled development, tiny ecosystem share. Forces you to bridge incompatible futures with Tokio-dependent crates (rmcp, wasmtime-wasi). | Tokio |
| jsonrpc-stdio-server (parity) | Last release 2021 (v18). Unmaintained. Based on old parity jsonrpc-core which is archived. | Custom ACP layer on serde_json + tokio-util |
| wasmer | Less standards-compliant WASI, weaker fuel/epoch metering for sandboxing. Package registry focus doesn't match our embedding use case. | Wasmtime |
| tower-lsp / tower-lsp-server | Designed for LSP (Language Server Protocol), not general JSON-RPC. Would require fighting the LSP-specific abstractions to implement ACP. | Custom ACP protocol layer |
| tonic / gRPC | ACP is JSON-RPC 2.0 over stdio, not gRPC. Wrong protocol entirely. Adding protobuf serialization adds complexity for zero benefit. | serde_json for JSON-RPC |
| actix-web / axum (for core) | Protoclaw's primary communication is stdio pipes, not HTTP. Adding a web framework for the core is unnecessary weight. Only consider axum if adding a debug-http channel later. | Tokio TCP/HTTP only where needed |
| slog | Older structured logging crate. tracing has won the ecosystem — better async support, span model, and subscriber ecosystem. | tracing |
| dotenv / dotenvy | Environment-only config is too limited. Protoclaw needs layered config (file + env + defaults). | figment with env provider |
## Stack Patterns by Variant
- Hand-roll JSON-RPC 2.0 types with serde: `Request`, `Response`, `Notification` structs
- Use `tokio-util::codec::LinesCodec` over piped stdin/stdout for line-delimited JSON framing
- Keep it thin — ACP has ~6 methods, not worth a framework dependency
- Pattern: `tokio::process::Command` spawns agent → take stdin/stdout → wrap in codec → `Stream<Item=JsonRpcMessage>` + `Sink<JsonRpcMessage>`
- Same JSON-RPC 2.0 framing as ACP but protoclaw is the server, channel is the client
- Each channel subprocess gets its own `tokio::process::Child` with piped stdio
- Use `tokio::sync::mpsc` channels to bridge subprocess I/O to the Channels Manager's routing logic
- Pattern: one `tokio::spawn` per channel subprocess handling its read loop, one for write loop
- Use rmcp's server features directly — `#[tool]` macro for defining tools, `stdio` transport for serving
- For spawning external MCP servers: rmcp's `TokioChildProcess` client transport
- Pattern: Tools Manager starts its own MCP server(s) via rmcp, then passes connection details to agent at `session/new`
- Wasmtime `Engine` shared across all WASM modules (compilation cache)
- Per-invocation `Store` with fresh fuel budget and WASI context
- Pattern: `WasiCtxBuilder::new().inherit_stdio().preopened_dir(...)` for controlled filesystem access
- Use epoch interruption for wall-clock timeouts, fuel for CPU instruction limits
- Component model for typed tool interfaces (input schema → WASM component → output)
- `tokio::sync::mpsc` for unidirectional message passing (channel → agents manager → channel)
- `tokio::sync::oneshot` for request-response patterns (permission requests)
- `tokio::sync::broadcast` for fan-out (agent streaming updates to multiple channels)
- `tokio::sync::watch` for state (supervisor health status)
- Do NOT use shared mutable state between managers — message passing only
- Exponential backoff: implement manually with `tokio::time::sleep` (100ms → 200ms → 400ms → ... → 30s cap)
- Health checks: `tokio::time::interval` polling each manager
- Graceful shutdown: `tokio::signal` for SIGTERM/SIGINT → ordered teardown via `tokio::sync::watch` channel
## Version Compatibility
| Package | Compatible With | Notes |
|---------|-----------------|-------|
| rmcp 1.3 | tokio 1.x | rmcp uses tokio internally for all transports. No version conflicts expected. |
| wasmtime 38 | tokio 1.x | wasmtime-wasi uses tokio for async WASI operations. Same tokio version tree. |
| rmcp 1.3 | serde 1.x, serde_json 1.x | rmcp re-exports its own JSON-RPC types built on serde. Custom ACP types must use same serde version. |
| wasmtime 38 | Rust 2024 edition | Wasmtime 38 requires recent stable Rust. Pin MSRV to 1.85+ to ensure compatibility with all deps. |
| tracing 0.1 | tracing-subscriber 0.3 | These are the current stable pair. tracing 0.2 is not yet released. |
| figment 0.10 | serde 1.x, toml 0.8 | figment deserializes config via serde. toml 0.8 provider is built-in. |
| dashmap 6.x | Rust 2024 edition | dashmap 6 requires recent Rust. Compatible with our MSRV. |
## Sources
- Context7 `/bytecodealliance/wasmtime` — fuel metering, epoch interruption, WASI embedding, engine configuration (HIGH confidence)
- Context7 `/websites/rs_tokio_1_49_0` — subprocess management, piped stdio, async process lifecycle (HIGH confidence)
- Context7 `/websites/rs_rmcp` — MCP SDK features, transport options, client/server setup, feature flags (HIGH confidence)
- Context7 `/websites/rs_tracing` — structured diagnostics, span model (HIGH confidence)
- Context7 `/serde-rs/serde` — serialization framework (HIGH confidence)
- https://github.com/modelcontextprotocol/rust-sdk — Official MCP Rust SDK, 3.2K stars, rmcp crate (HIGH confidence)
- https://crates.io/crates/rmcp — 6.5M+ downloads, 792 reverse deps, v1.3.0 (HIGH confidence)
- https://github.com/paritytech/jsonrpsee — JSON-RPC library evaluation, 828 stars (HIGH confidence, evaluated and rejected for ACP)
- https://wasmruntime.com/en/compare/wasmtime-vs-wasmer — 2026 runtime comparison, benchmarks (MEDIUM confidence)
- https://reintech.io/blog/wasmtime-vs-wasmer-vs-wasmedge-wasm-runtime-comparison-2026 — Runtime comparison with embedding focus (MEDIUM confidence)
- https://crates.io/crates/figment — 23M+ downloads, config management (HIGH confidence)
- https://crates.io/crates/jsonrpsee — 17.9M downloads, evaluated for JSON-RPC (HIGH confidence, rejected — wrong transport model)
- https://docs.rs/jsonrpc-stdio-server — Evaluated and rejected, last release 2021 (HIGH confidence on rejection)
<!-- GSD:stack-end -->

<!-- GSD:conventions-start source:CONVENTIONS.md -->
## Conventions

Conventions not yet established. Will populate as patterns emerge during development.
<!-- GSD:conventions-end -->

<!-- GSD:architecture-start source:ARCHITECTURE.md -->
## Architecture

Architecture not yet mapped. Follow existing patterns found in the codebase.
<!-- GSD:architecture-end -->

<!-- GSD:workflow-start source:GSD defaults -->
## GSD Workflow Enforcement

Before using Edit, Write, or other file-changing tools, start work through a GSD command so planning artifacts and execution context stay in sync.

Use these entry points:
- `/gsd:quick` for small fixes, doc updates, and ad-hoc tasks
- `/gsd:debug` for investigation and bug fixing
- `/gsd:execute-phase` for planned phase work

Do not make direct repo edits outside a GSD workflow unless the user explicitly asks to bypass it.
<!-- GSD:workflow-end -->



<!-- GSD:profile-start -->
## Developer Profile

> Profile not yet configured. Run `/gsd:profile-user` to generate your developer profile.
> This section is managed by `generate-claude-profile` -- do not edit manually.
<!-- GSD:profile-end -->
