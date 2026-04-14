# Technology Stack

**Analysis Date:** 2026-04-14

## Languages

**Primary:**
- Rust, Edition 2024, MSRV 1.94 — all workspace crates

**Secondary:**
- YAML — configuration (`anyclaw.yaml`, `defaults.yaml`)
- WAT/WASM — sandboxed tool modules

## Runtime

**Environment:**
- Tokio async runtime (multi-threaded `rt-multi-thread`)
- Tokio version: `1.50`
- Feature sets vary per crate; heaviest in `anyclaw-agents`: `fs`, `io-util`, `macros`, `net`, `process`, `rt`, `rt-multi-thread`, `sync`, `time`

**Package Manager:**
- Cargo with workspace resolver v2
- Lockfile: `Cargo.lock` present (workspace)

## Frameworks

**Core:**
- Axum `0.8` — HTTP server (debug-http channel, supervisor health/metrics endpoint, MCP aggregated tool server)
- Teloxide `0.17` — Telegram Bot API (in `ext/channels/telegram`, features: `macros`, `rustls`, `ctrlc_handler`)
- rmcp `1.4` — MCP protocol client/server (in `anyclaw-tools` and `anyclaw-sdk-tool`, features: `client`, `server`, `transport-io`, `transport-child-process`, `transport-streamable-http-server`)

**Testing:**
- rstest `0.26` — parameterized/fixture-based test framework (all crates)
- tokio-test `0.4` — async test utilities
- test-log `0.2` — tracing-aware test logging (integration tests, test-helpers)
- temp-env `0.3` — scoped env var manipulation in tests

**Build/Dev:**
- Cargo workspace — 12 core crates + 5 ext binaries + 1 example tool + 1 integration test package
- mold linker via clang for musl targets (`.cargo/config.toml`)
- Release profile: `strip = true`, `lto = "thin"`

## Key Dependencies

**Critical:**
- `tokio` `1.50` — async runtime, subprocess management, channels, timers
- `serde` `1` (with `derive`) — serialization for all config and wire types
- `serde_json` `1` — JSON parsing/generation throughout
- `agent-client-protocol-schema` `0.11` — ACP wire type definitions (features: `unstable_session_resume`, `unstable_session_fork`)

**Error Handling:**
- `thiserror` `2` — typed error enums in all library crates
- `anyhow` `1` — entry points only (`main.rs`, `supervisor.rs`, `init.rs`, `status.rs`)

**Observability:**
- `tracing` `0.1` — structured logging/spans throughout
- `tracing-subscriber` `0.3` (features: `env-filter`, `json`) — log output formatting
- `metrics` `0.24` — runtime metrics collection
- `metrics-exporter-prometheus` `0.18` — Prometheus metrics endpoint

**Infrastructure:**
- `tokio-util` `0.7` (features: `codec`) — `LinesCodec` for NDJSON framing
- `tokio-stream` `0.1` (features: `sync`) — stream adapters for channel broadcast
- `futures` `0.3` — future combinators
- `bytes` `1` — byte buffer primitives for codec
- `uuid` `1` (features: `v4`) — session and message IDs
- `clap` `4` (features: `derive`, `env`) — CLI argument parsing
- `reqwest` `0.12` (features: `json`, `rustls-tls`, `stream`) — HTTP client (CLI status command, integration tests)
- `bollard` `0.20` — Docker Engine API client (agent Docker workspace support)

**Configuration:**
- `figment` `0.10` (features: `toml`, `yaml`, `env`) — layered config loading
- `yaml_serde` `0.10` (aliased as `serde_yaml`) — YAML parsing
- `subst` `0.3` — `${VAR}` environment variable substitution in YAML

**Storage:**
- `rusqlite` `0.31` (features: `bundled`) — SQLite session persistence (in `anyclaw-core`)

**WASM Sandbox:**
- `wasmtime` `43` — WASM runtime engine
- `wasmtime-wasi` `43` — WASI host implementation for sandboxed tools

**SDK-specific:**
- `schemars` `1` — JSON Schema generation for tool input schemas (in `anyclaw-sdk-tool`)
- `regex` `1` — pattern matching (in `ext/channels/telegram`)

## Configuration

**Environment:**
- Layered: embedded `defaults.yaml` → user `anyclaw.yaml` (with `${VAR}` substitution) → env vars (`ANYCLAW_` prefix, `__` separator)
- Config file: `crates/anyclaw-config/src/defaults.yaml`
- Config types: `crates/anyclaw-config/src/types.rs`
- Config loading: `crates/anyclaw-config/src/lib.rs`

**Build:**
- Workspace root: `Cargo.toml` (all deps centralized via `[workspace.dependencies]`)
- Linker config: `.cargo/config.toml` (mold via clang for `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`)
- Release profile: strip symbols, thin LTO

## Platform Requirements

**Development:**
- Rust 1.94+ (edition 2024)
- No `rust-toolchain.toml` — relies on MSRV in `Cargo.toml`

**Production:**
- Docker deployment (Alpine musl targets with mold linker)
- Dockerfiles: `Dockerfile` (root), `examples/01-fake-agent-telegram-bot/Dockerfile.mock-agent`, `examples/02-real-agents-telegram-bot/Dockerfile`, `tests/integration/Dockerfile.mock-agent`
- SQLite bundled (no external DB dependency)

## Notable Patterns

**Workspace-level dependency management:**
- All shared deps declared in `[workspace.dependencies]` in root `Cargo.toml`
- Crates reference via `{ workspace = true }` with per-crate feature overrides

**SDK crates are publishable:**
- `anyclaw-sdk-types` `0.4.0`, `anyclaw-sdk-channel` `0.2.7`, `anyclaw-sdk-tool` `0.2.5`, `anyclaw-sdk-agent` `0.2.5`
- Licensed MIT OR Apache-2.0, with readme, keywords, categories
- Internal crates have `publish = false`

**Feature flags:**
- `agent-client-protocol-schema`: `unstable_session_resume`, `unstable_session_fork`
- `rmcp`: split across consumer crates — `anyclaw-tools` uses `client`, `server`, `transport-io`, `transport-child-process`, `transport-streamable-http-server`; `anyclaw-sdk-tool` uses `server`, `transport-io`

**No unsafe code:** Zero `unsafe` blocks across the entire workspace.

---

*Stack analysis: 2026-04-14*
