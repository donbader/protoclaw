# Milestones

## v7.0 Tech Debt & Optimization (Shipped: 2026-04-11)

**Phases completed:** 8 phases (64-71), 0 GSD-tracked plans (completed outside GSD planning)
**Timeline:** 1 day (Apr 11, 2026)
**Requirements:** 22/22 satisfied

**Key accomplishments:**

- CI hardening — cargo audit, MSRV 1.85 enforcement, --locked flag on all CI builds
- Error handling & safety — SupervisorError typed enum replacing anyhow in public API, zero unsafe blocks (temp-env for env isolation)
- Dependency modernization — async-trait removed from all 9 crates (native Rust 1.85 async fn in traits), SDK crates bumped to 0.2.0, tokio features trimmed to minimal sets
- Config cleanup — ToolType/LogFormat/ReactionLifecycle enums replacing stringly-typed fields, crash config hierarchy documented
- Architecture deduplication — SubprocessSlot<C> generic in protoclaw-core, CrashRecovery shared helper, pub(crate) visibility tightening
- Code quality — long functions (>100 lines) extracted into named sub-functions, SessionKey cloning evaluated
- Test coverage — SDK crate unit tests (sdk-agent 7 hooks, sdk-tool dispatch routing), backend/command module tests
- Documentation lints — #![warn(missing_docs)] on all 4 SDK crates, comprehensive doc comments on public types

---

## v5.1 Tech Debt & Hardening (Shipped: 2026-04-10)

**Phases completed:** 11 phases (45-55), 25 plans
**Timeline:** 1 day (Apr 10, 2026)
**Requirements:** 25/25 satisfied

**Key accomplishments:**

- Config normalization — snake_case keys with serde alias backward compat, Figment env override removal, hostname/IP validation for tools_server_host
- Config completeness — all hardcoded durations (ACP timeout, Telegram turn/rate-limit, channel exit) wired to config fields, StreamableHttp docs
- Dependency health — replaced deprecated serde_yaml with serde_yml across all workspace crates and SubstYaml provider
- Error handling & resilience — Telegram send retries with backoff, session error forwarding to channels, CrashTracker per agent slot with crash-loop disable, unwrap_or_default audit with logged fallbacks, ACP error forwarding
- Production safety — bare unwrap elimination in telegram/SDK paths, SubstYaml fail-loud on missing env vars
- Observability — tracing::instrument spans on tool discovery, agent session setup, channel initialization
- Test coverage — ExternalMcpServer routing tests, mcp_servers integration test, fs callback tests, dual-channel isolation, WASM E2E integration test
- Hygiene — dead code/clone cleanup, Docker base image digest pinning, AGENTS.md v5.1 changelogs across 9 files

---

## v5.0 System Refactoring & Code Health (Shipped: 2026-04-09)

**Phases completed:** 9 phases (36-44), 12 plans
**Timeline:** 2 days (Apr 8-9, 2026)
**Files modified:** 190 | **Lines:** +7,577 / -19,377 (net -11,800) | **Total LOC:** 17,847 Rust
**Requirements:** 29/30 satisfied, 1 accepted-as-is (DOCK-11)

**Key accomplishments:**

- Enforced crate boundaries — eliminated 3 illegal cross-crate dependencies (channels→agents, agents→tools, test-helpers→binary) via AgentsCommand/ToolsCommand relocation to core and protoclaw-supervisor library extraction
- Relocated ChannelEvent/SessionKey to sdk-types, deleted channels types.rs re-export shim, removed sdk-tool schemars re-export
- Eliminated all code smells — zero bare .unwrap(), zero #[allow(dead_code)], TCP stub removed, InternalMessage/MessageContent deleted, cargo clippy clean
- Restructured Docker builds — core-only root Dockerfile (4 stages), per-example standalone Dockerfiles extending protoclaw-core base image, local build contexts
- Config-driven operation — zero std::env::var in production code, all runtime config via protoclaw.yaml initialize handshake, PROTOCLAW_* as sole sanctioned env override
- Updated all crate-level AGENTS.md files, design-principles.md v5.0 section, documented config-driven principle and CLI entry point exceptions

---

## v4.0 Agent Containers & First-Party Opencode (Shipped: 2026-04-08)

**Phases completed:** 15 phases, ~31 plans
**Timeline:** 4 days (Apr 5–8, 2026)
**Files modified:** 229 | **Lines:** +23,717 / -1,641 | **Total LOC:** 20,507 Rust
**Git range:** 193 commits

**Key accomplishments:**

- Docker agent containers via bollard — WorkspaceConfig enum dispatching Local/Docker backends, stream bridging through bollard attach to existing ACP pipeline, stale container cleanup, image auto-pull, resource limits (memory + CPU)
- ProcessBackend trait abstraction — LocalBackend and DockerBackend behind Box<dyn ProcessBackend>, AgentConnection refactored for pluggable backends with MockBackend for testing
- BDD test migration — rstest across all 12 workspace crates (372+ tests), given_/when_/then_ naming, parameterised #[case::label] tests, fixtures as free functions
- Opencode wrapper binary — ext/agents/opencode-wrapper/ thin stdio proxy with config-driven model/provider passthrough, transparent ACP bridging (no escape stripping — intentional)
- Comprehensive E2E test suite — 28 Docker integration tests, 9 session/batch flow tests, 6 tool invocation tests, crash recovery and resilience tests across 8 flow files
- Telegram ChatTurn state machine — replaced 12 per-chat fields with unified state machine, fixed truncation bug, rate-limited editing
- Channel SDK DX — ContentKind enum, content_to_string, PermissionBroker, ChannelTester extracted to SDK crates, new-channel docs expanded

---

## v3.1 Testing & Runtime Config (Shipped: 2026-04-04)

**Phases completed:** 4 phases, 8 plans, 13 tasks

**Key accomplishments:**

- Uniform options HashMap on all subprocess config types, wired through ACP initialize so mock-agent reads thinking from config instead of MOCK_AGENT_THINK env var
- SseCollector with iterator-style SSE parsing and boot_supervisor_with_port moved to protoclaw-test-helpers, plus test-log wired for tracing visibility
- Split monolithic e2e.rs into 6 focused flow test files with test-log tracing and SseCollector where applicable
- 1. [Rule 3 - Blocking] Missing serde_json dependency in protoclaw-test-helpers
- 1. [Rule 1 - Bug] Shutdown test timing with delay_ms
- Phase:
- Plan:
- Plan:

---

## v3.0 Quality & Infrastructure Hardening (Shipped: 2026-04-04)

**Phases completed:** 6 phases, 12 plans, 24 tasks

**Key accomplishments:**

- protoclaw-test-helpers crate with 5 modules (paths, config, ports, handles, timeout) consolidating duplicated test utilities, integration tests migrated to shared helpers
- 7 flow_ integration tests covering all core flows including crash recovery, with cargo-llvm-cov baseline ratchet and GitHub Actions CI/coverage workflows
- Design principles document with architecture rationale and failure modes, plus AGENTS.md for all 6 previously undocumented crates
- SubstYaml provider with env interpolation, defaults.yaml, and full config crate migration from TOML to YAML — 49 tests passing
- Complete TOML→YAML migration across CLI, examples, docker-compose, integration tests, and documentation
- Named constants module with 10 internal guards/defaults in protoclaw-core, plus BackoffConfig/CrashTrackerConfig types and per-entity Option overrides on AgentConfig/ChannelConfig
- All ~15 hardcoded Duration values and channel capacities replaced with config fields or constants module references across agents, channels, tools, supervisor, and status
- Registry-based conditional tracing init with log_format config field supporting JSON and pretty-print output
- ANSI-free ext/ binary output via .with_ansi(false) and span-based subprocess stderr attribution with source field for agent/channel identity
- Moved ack notification from inbound handler to three dispatch sites so batched messages only ack the last message
- Figment::Jail integration tests for example config schema validation + CI docker-compose syntax checking
- Self-contained test.sh scripts for both examples with Docker lifecycle, trap cleanup, tool/batch verification (ex01) and .env validation (ex02)

---

## v2.1 Examples & Dockerfile Restructure (Shipped: 2026-04-04)

**Phases completed:** 4 phases, 16 plans, 33 tasks
**Timeline:** 2 days (Apr 3-4, 2026)
**Files modified:** 122 | **Lines:** +14,866 / -1,112 | **Total LOC:** 12,700 Rust
**Git range:** feat(phase-9) → fix(agents)

**Key accomplishments:**

- Mock agent promoted from tests/mock-agent/ to ext/agents/mock-agent/ with git history preserved, workspace updated, all integration tests passing
- Mock agent emits 2 agent_thought_chunk notifications with 200ms delays before echo response when MOCK_AGENT_THINK=1
- debug-http emits agent thoughts as named "thought" SSE events via SsePayload; Telegram sends 🧠-prefixed thinking messages that collapse to elapsed time on result
- cargo-chef multi-stage Dockerfile with 4 per-binary runtime targets replacing fingerprint hack
- Explicit thought chunk tracing in AgentsManager and ThoughtContent SDK helper for channel authors to deserialize thought payloads from DeliverMessage
- Config fields (log_level, enabled, extensions_dir) with @built-in/ binary resolution wired into managers and tracing
- MCP tool binary using protoclaw-sdk-tool returning hostname, OS, arch, and version over stdio
- Turnkey fake agent bot with Dockerfile, compose, config, and debug-http-first README — zero AI API keys needed
- Multi-agent [[agents]] config with backward-compat normalize_agents() shim, ChannelConfig agent routing field, and legacy examples/telegram-bot/ deletion
- AgentSlot per-agent lifecycle with Vec<AgentSlot> in AgentsManager, agent_name routing on CreateSession/PromptSession, and multi-agent health/status
- Channel-based agent routing via config agent field, per-agent MCP tool filtering on GetMcpUrls, and multi-agent status display with legacy backward compat
- Complete examples/02-real-agents-telegram-bot/ with dual-target Dockerfile (opencode/claude-code), config mounts, multi-agent [[agents]] TOML, and agent-switching README
- Manager-hierarchy config with named HashMaps, AckConfig/DebounceConfig structs, embedded defaults.toml, and ChannelEvent::AckMessage variant
- Wired manager-hierarchy config through Figment loading, supervisor, all three managers, example TOML, and integration tests
- Per-session sliding window debounce with configurable merging, mid-response queueing, and deadline-driven dispatch in ChannelsManager
- Ack reactions with emoji + typing on message receipt, configurable lifecycle (remove/keep/replace), and immediate JSON ack for debug-http

---

## v1.0 MVP (Shipped: 2026-04-02)

**Phases completed:** 4 phases, 15 plans, 30 tasks

**Key accomplishments:**

- Cargo workspace with 4 crates, core ID newtypes, InternalMessage format, Manager trait, ExponentialBackoff + CrashTracker, and typed error enums — 25 tests passing
- JSON-RPC 2.0 types with Content-Length framing codec, byte-accurate UTF-8 encoding, and 24 TDD tests covering partial reads, multi-byte payloads, and oversized frame rejection
- TOML config parsing via figment with layered providers (defaults → file → env), typed config structs, and example protoclaw.toml
- Supervisor select loop with ordered boot/shutdown, SIGTERM/SIGINT handling, crash restart with exponential backoff, and crash-loop protection using stub managers
- NDJSON codec replacing ContentLengthCodec — splits on byte 0x0A, encodes compact JSON + newline, 14 TDD tests
- Mock ACP agent binary with echo/crash/permission simulation, plus ToolsManager with TCP-based MCP server stubs replacing StubToolsManager
- AgentsManager with full ACP lifecycle (initialize → session/new → prompt → update), crash recovery via session/load, and bidirectional NDJSON subprocess I/O replacing StubAgentsManager
- Debug-http channel with axum serving message/cancel/permissions/SSE endpoints, validated by 4 E2E integration tests through the full pipeline
- Channel subprocess protocol types with NdJsonCodec stdio framing and ChannelsManager with per-channel crash isolation
- SessionKey-based routing table wiring bidirectional message flow between channels and agent with per-peer ACP sessions
- debug-http extracted to standalone subprocess binary with ChannelsManager wired into supervisor and E2E tests proving full message/permission pipeline through channel subprocess
- Startup banner, enriched /health endpoint with live agent status via oneshot introspection, and `protoclaw status` CLI command

---
