# Protoclaw

## What This Is

Infrastructure sidecar that connects AI agents to the outside world. Protoclaw wraps any ACP-speaking agent process (opencode, claude-code, gemini-cli) and gives it channels to talk to users and tools to act on the world (MCP servers). The agent is a black box — protoclaw spawns it locally or in Docker containers, keeps the pipes connected, and recovers from crashes. Ships with debug-http and Telegram channels, CLI tooling (init/validate/status), multi-agent routing, message queuing, ack reactions, Docker container workspace support with bollard stream bridging, a first-party opencode wrapper binary, and Docker Compose examples for both fake and real agent setups.

## Core Value

The agent must stay alive, connected to channels, and able to call tools — regardless of crashes, restarts, or network issues. Protoclaw is infrastructure plumbing, not AI logic.

## Requirements

### Validated

- ✓ Supervisor lifecycle management (boot, shutdown, health check, crash recovery with exponential backoff) — v1.0
- ✓ Agents Manager — ACP client that spawns, manages, and communicates with a primary agent process over JSON-RPC 2.0 stdio — v1.0
- ✓ Tools Manager — stands up MCP servers and registers them with the agent at session/new — v1.0
- ✓ ACP protocol implementation — initialize, session/new, session/prompt, session/update, session/cancel, session/request_permission — v1.0
- ✓ Channels Manager — routes inbound/outbound messages between external channels and the agent, handles permission requests — v1.0
- ✓ Channel subprocess protocol — JSON-RPC stdio interface for channel extensions (debug-http as reference impl) — v1.0
- ✓ Message flow: channel → agent (inbound) and agent → channel (outbound streaming) — v1.0
- ✓ Permission flow: agent asks permission → channel shows prompt → user responds → agent continues — v1.0
- ✓ TOML configuration with layered providers (defaults → file → env vars) — v1.0
- ✓ JSON-RPC 2.0 NDJSON framing for ACP and channel subprocess communication — v1.0
- ✓ CLI subcommands: run (default), init, validate, status — v1.0
- ✓ Config semantic validation (binary exists, no duplicate names) without booting — v1.0
- ✓ Startup banner showing agent, channels, MCP servers, and config path — v1.0
- ✓ /health endpoint with component-level status including live agent introspection — v1.0
- ✓ `protoclaw status` queries running instance and displays formatted status — v1.0
- ✓ Channels SDK — Channel trait + ChannelHarness handles JSON-RPC NDJSON stdio framing, initialize handshake, message routing — Phase 5
- ✓ Tools SDK — Tool trait with Value in/out + ToolServer wrapping rmcp ServerHandler for MCP serving — Phase 5
- ✓ Agent adapter layer (Agents SDK) — AgentAdapter trait with per-method hooks, GenericAcpAdapter passthrough default — Phase 5
- ✓ debug-http retrofitted to use Channels SDK (Channel trait + ChannelHarness) — Phase 5
- ✓ Tool extension protocol — real MCP hosting via rmcp, external MCP server subprocess spawning, aggregated tool server — Phase 6
- ✓ WASM sandboxing for custom tools via Wasmtime — fuel metering, epoch interruption, WASI p1 sandbox, configurable limits — Phase 6
- ✓ Telegram channel — teloxide 0.17 with inbound dispatcher, streaming delivery (edit throttling, message splitting), inline keyboard permissions — Phase 7

- ✓ Starter examples — examples/telegram-bot/ with Docker Compose, protoclaw.toml, .env.example, README for copy-paste bot setup — Phase 8
- ✓ Fake agent example — examples/01-fake-agent-telegram-bot/ with mock agent, debug-http, system-info MCP tool, @built-in/ config, debug-http-first workflow — Phase 10
- ✓ Real agent examples — examples/02-real-agents-telegram-bot/ with opencode/claude-code, user-mountable config directories, agent-switching README — Phase 11
- ✓ Dockerfile restructure — cargo-chef multi-stage build with core-only base, per-binary runtime targets for ext/ binaries — Phase 9
- ✓ Mock agent promotion — ext/agents/mock-agent/ with thinking simulation (AgentThoughtChunk streaming) — Phase 9
- ✓ Thinking pipeline — end-to-end agent thought delivery through channels (SSE named events, Telegram 🧠 messages) — Phase 9
- ✓ Multi-agent support — AgentSlot per-agent lifecycle, channel-based agent routing, per-agent tool filtering — Phase 11
- ✓ Manager-hierarchy config — named HashMaps with embedded defaults.toml, Figment layered loading — Phase 12
- ✓ Per-session FIFO message queue — each message processed individually in order, ack on every push, typing indicator on dispatch — Phase 12 (originally debounce, replaced with FIFO queue)
- ✓ Ack reactions — emoji + typing on receipt, configurable lifecycle (remove/keep/replace), instant JSON ack for debug-http — Phase 12
- ✓ Test infrastructure — shared test-helpers crate, integration test matrix (7 flow tests), coverage baseline ratchet, GitHub Actions CI — Phase 13
- ✓ Test infrastructure v2 — config-driven options passthrough, SseCollector helper, boot helper extraction, test-log tracing, e2e.rs split into 6 focused flow files — Phase 19
- ✓ AGENTS.md & design principles — design-principles.md, per-crate AGENTS.md for all 12 crates, architecture rationale — Phase 13
- ✓ Config format migration — TOML→YAML with figment first-party support, SubstYaml env interpolation, example configs converted — Phase 14
- ✓ Example validation — integration tests for config schema parsing, docker-compose syntax validation, self-contained test.sh scripts with full Docker lifecycle — Phase 18
- ✓ Config-driven constants — all ~15 hardcoded Duration values extracted into typed config structs, named constants module, per-entity overrides — Phase 15
- ✓ Telegram batch ack — ack reaction fires only on last message in debounced batch, not every message — Phase 17
- ✓ Structured logging — ANSI stripping, JSON log format via config, subprocess source attribution — Phase 16
- ✓ E2E test coverage — 8 integration flow tests across 7 files covering boot, ack, thinking, crash recovery, shutdown, multi-agent, health, config path — Phase 19-20
- ✓ Runtime config loading — `--config` CLI flag / `PROTOCLAW_CONFIG` env var, Dockerfile externalizes config via volume mount, edit-restart workflow documented — Phase 21
- ✓ SDK integration tests — sdk-test-channel (Channel trait echo), sdk-test-tool (Tool trait echo), E2E tests proving round-trip message through SDK channel and MCP serving through SDK tool — Phase 22
- ✓ Docker integration tests — 8 tests across 2 files (spawn, communicate, shutdown, crash recovery, stale cleanup, resource limits, mixed mode, pull policy) + CI docker-tests job — Phase 28
- ✓ E2E session & batch flow tests — 9 tests across 3 files (session lifecycle, streaming response ordering, multi-turn, large payload, high-volume FIFO, interleaved timing, queue drain) — Phase 29
- ✓ E2E tool invocation & payload tests — 6 E2E tests + 5 unit tests across 2 files (multi-tool boot, large payload, JSON preservation, sequential messages, invalid tool graceful degradation, disabled tool) — Phase 30
- ✓ Docker workspace for agents — WorkspaceConfig enum (Local/Docker), DockerWorkspaceConfig with image/memory/cpu/volumes/env, bollard stream bridging to existing ACP pipeline — v4.0 (Phase 23, 25)
- ✓ ProcessBackend trait abstraction — LocalBackend and DockerBackend behind Box<dyn ProcessBackend>, pluggable agent connection backends with MockBackend for testing — v4.0 (Phase 24)
- ✓ BDD test migration — rstest across all 12 workspace crates (372+ tests), given_/when_/then_ naming, parameterised #[case::label] tests, fixtures as free functions — v4.0 (Phase 24.2)
- ✓ Docker agent lifecycle — container create/start/stop/remove, stale container cleanup on startup, image auto-pull, crash recovery with backoff, resource limits (memory + CPU) — v4.0 (Phase 25)
- ✓ Opencode wrapper binary — ext/agents/opencode-wrapper/ thin stdio proxy with config-driven model/provider passthrough, transparent ACP bridging — v4.0 (Phase 26)
- ✓ Docker examples updated — examples 01+02 with Docker workspace support, socket proxy, agent Dockerfiles — v4.0 (Phase 27)
- ✓ Docker integration tests — 8 tests across 2 files (spawn, communicate, shutdown, crash recovery, stale cleanup, resource limits, mixed mode, pull policy) + CI docker-tests job — v4.0 (Phase 28)
- ✓ E2E resilience & recovery tests — crash recovery, reconnection, and graceful degradation E2E tests — v4.0 (Phase 31)
- ✓ Telegram ChatTurn state machine — replaced 12 per-chat fields with unified state machine, fixed truncation bug, rate-limited editing — v4.0 (Phase 32)
- ✓ Channel SDK DX improvements — ContentKind enum, content_to_string, PermissionBroker, ChannelTester extracted to SDK crates, new-channel docs expanded — v4.0 (Phase 33)
- ✓ Example 02 end-to-end fix — standard Anthropic provider, removed hardcoded secrets, API key marked required — v4.0 (Phase 35)

- ✓ Crate boundary enforcement — eliminated channels→agents, agents→tools, test-helpers→binary dependencies via type relocation and protoclaw-supervisor extraction — v5.0
- ✓ ChannelEvent/SessionKey relocated to protoclaw-sdk-types, re-export shims removed — v5.0
- ✓ Code smell elimination — zero bare .unwrap(), zero #[allow(dead_code)], TCP stub removed, dead types deleted, clippy clean — v5.0
- ✓ Docker build efficiency — core-only root Dockerfile, per-example standalone Dockerfiles extending base image, local build contexts — v5.0
- ✓ Config-driven operation — zero std::env::var in production code, all runtime config via protoclaw.yaml initialize handshake — v5.0
- ✓ Documentation hygiene — all crate-level AGENTS.md updated, design-principles.md v5.0 section, config-driven principle documented — v5.0
- ✓ Typed AckConfig conversion — From<AckConfig> for ChannelAckConfig replacing manual JSON construction — v5.0

- ✓ Config normalization — snake_case keys with serde alias backward compat, Figment env override removal, hostname/IP validation for tools_server_host — v5.1
- ✓ Config completeness — all hardcoded durations (ACP timeout, Telegram turn/rate-limit, channel exit) wired to config fields, StreamableHttp docs — v5.1
- ✓ Dependency health — replaced deprecated serde_yaml with serde_yml across all workspace crates and SubstYaml provider — v5.1
- ✓ Error handling & resilience — Telegram send retries with backoff, session error forwarding, crash escalation with CrashTracker per agent slot, unwrap_or_default audit, ACP error forwarding — v5.1
- ✓ Production safety — bare unwrap elimination in telegram/SDK paths, SubstYaml fail-loud on missing env vars — v5.1
- ✓ Observability — tracing::instrument spans on tool discovery, agent session setup, channel initialization — v5.1
- ✓ Test coverage gaps closed — ExternalMcpServer routing tests, mcp_servers integration test, fs callback tests, dual-channel isolation, WASM E2E integration test — v5.1
- ✓ Hygiene — dead code/clone cleanup, Docker base image digest pinning, AGENTS.md v5.1 changelogs across 9 files — v5.1

## Current Milestone: v7.1 Docker Image Optimization

**Goal:** Shrink protoclaw Docker images to competitive sizes, align image architecture vision, and establish ghcr.io management practices.

**Target features:**
- Switch core image base from debian:bookworm-slim to distroless (~90MB savings)
- Add `strip = true` and `lto = true` to `[profile.release]` (~20-30MB savings on binaries)
- Establish extension image strategy (core vs agent/channel images, layering approach)
- ghcr.io image management — tagging conventions, cleanup policies, multi-arch verification
- Audit example Dockerfiles for unnecessary bloat

**Key context:**
- Core image reduced from 174MB to 58.7MB in Phase 72 (strip+LTO + distroless)
- Example images (861MB) dominated by Node.js + opencode-ai — separate from core optimization
- 6 Rust binaries in core image (protoclaw, telegram-channel, debug-http, mock-agent, system-info, opencode-wrapper)

## Previous Milestone: v7.0 Tech Debt & Optimization

**Goal:** Tech debt reduction, dependency modernization, architecture deduplication, and CI hardening identified by full codebase audit.

**Shipped:** 2026-04-11. All 8 phases (64-71) complete. async-trait removed, SubprocessSlot<C> unified, SDK crates at 0.2.0, CI hardened with cargo audit + MSRV + --locked, all SDK crates fully documented.

## Previous Milestone: v6.0 Open Source Launch

**Goal:** Make protoclaw a credible, contributor-friendly open source project — public docs, governance, crate publishing, container images, automated releases, and CI visibility.

**Target features:**
- Public README with architecture overview, quickstart, badges
- Dual license (MIT + Apache-2.0), CONTRIBUTING.md, CODE_OF_CONDUCT.md
- GitHub issue templates, PR template, SECURITY.md
- Publish SDK crates to crates.io (sdk-types, sdk-channel, sdk-tool, sdk-agent)
- Container images on ghcr.io (GitHub Actions workflow)
- Automated releases via release-please (git tags, GitHub Releases, changelogs)
- Public GitHub Actions CI with status badges
- Developer docs (getting-started, writing-a-channel, writing-a-tool)
- Cargo.toml metadata across all crates (description, license, repository, keywords)
- CHANGELOG.md (generated/maintained by release-please)
- .planning/ added to .gitignore, useful content extracted to docs/

**Key decisions:**
- .planning/ → gitignored, GSD stays local-only
- Contribution model → fully open, maintainer reviews all PRs
- Versioning → automated via release-please with semver
- Container registry → ghcr.io
- Crate publishing → SDK crates only (4 crates)
- License → MIT + Apache-2.0 dual license

### Active

(v6.0 complete — all phases shipped. See v7.0 milestone above for active work.)

### Future

- [ ] Slack channel via slack-morphism with Socket Mode transport
- [ ] Discord channel via serenity with Gateway WebSocket transport
- [ ] WASM tool hot-loading — watch directory for `.wasm` files, register dynamically
- [ ] `protoclaw init-channel <name>` scaffolds a new channel project
- [ ] `protoclaw init-tool <name>` scaffolds a new tool project

## Current State

Shipped v7.0 (Apr 11, 2026). Tech Debt & Optimization complete — async-trait removed from all crates, SubprocessSlot<C> unified, SupervisorError typed enum, SDK crates at 0.2.0 with full doc coverage, CI hardened. Starting v7.1 Docker Image Optimization — Phase 72 (core image distroless + strip+LTO) already shipped, reducing core image from 174MB to 58.7MB. ~17,847 lines of Rust across 13 crates + 7 binaries.

### Out of Scope

- Agent internals (thinking, subagents, context windows) — protoclaw doesn't manage what happens inside the agent
- Streaming protocol ownership — the agent owns its own streaming
- Building agents — protoclaw connects to existing agents, doesn't create them
- AI logic of any kind — protoclaw is pure infrastructure
- Web UI / dashboard — debug-http is sufficient; production monitoring via OTel exports
- Plugin marketplace / registry — premature; ecosystem doesn't exist yet

## Context

- ~17,800 lines of Rust across 13 workspace crates (protoclaw-core, protoclaw-supervisor, protoclaw-agents, protoclaw-channels, protoclaw-tools, protoclaw-config, protoclaw-jsonrpc, protoclaw-sdk-types, protoclaw-sdk-channel, protoclaw-sdk-tool, protoclaw-sdk-agent, protoclaw-test-helpers) plus 7 binaries (protoclaw, debug-http, telegram-channel, mock-agent, opencode-wrapper, system-info, sdk-test-channel/sdk-test-tool)
- Speaks ACP (Agent Client Protocol) — JSON-RPC 2.0 over NDJSON stdio
- Protoclaw is the ACP client, the agent is the server
- Three-manager architecture: Agents Manager, Channels Manager, Tools Manager — coordinated by Supervisor
- Each channel is a subprocess communicating over JSON-RPC NDJSON stdio
- Tools are MCP servers — agent connects directly after introduction via session/new
- Debug-http channel ships as reference Channel trait implementation with SSE streaming
- Agents can run locally (LocalBackend) or in Docker containers (DockerBackend) — configured per-agent via WorkspaceConfig enum in protoclaw.yaml
- Telegram channel with inline keyboard permissions, streaming delivery, ack reactions, thinking message rendering, ChatTurn state machine for per-chat response lifecycle
- Four SDK crates enable extension authors to build channels, tools, and agent adapters without touching internals
- Multi-agent support with per-agent lifecycle (AgentSlot), channel-based routing, per-agent tool filtering
- YAML config with Figment layering (embedded defaults.yaml → file → env vars), SubstYaml env interpolation
- All tunable values config-driven — typed config structs with serde defaults, named constants module for internal guards
- Structured logging — JSON or pretty-print via config, ANSI stripping on subprocess output, source attribution spans
- Message debouncing with per-session sliding window, batch ack on last message only → replaced with per-session FIFO queue (ack on every push, typing on dispatch)
- CLI supports run/init/validate/status subcommands
- Two Docker Compose examples: fake agent (zero AI keys) and real agents (opencode/claude-code), both with self-contained test.sh
- Channel SDK includes ContentKind typed dispatch, content_to_string helper, PermissionBroker for async permission bridging, and ChannelTester for integration testing
- Test infrastructure: shared test-helpers crate (SseCollector, boot helper, port waiter, timeout, SDK config builders, Docker config builders), 10 integration flow tests across 9 files + 8 Docker integration tests across 2 files + 9 E2E session/batch/response tests across 3 files + 6 tool invocation tests + resilience tests, test-log for tracing visibility, config-driven test options, dedicated Docker CI job, rstest BDD across all crates (372+ tests)
- Shipped v1.0 (Apr 2), v2.0 (Apr 2-3), v2.1 (Apr 3-4), v3.0 (Apr 4), v3.1 (Apr 4-5), v4.0 (Apr 5-8), v5.0 (Apr 8-9), v5.1 (Apr 10)
- Target agents: opencode, claude-code, gemini-cli (any ACP-speaking process)

## Constraints

- **Language**: Rust — systems-level reliability required for infrastructure sidecar
- **Protocol**: ACP (Agent Client Protocol) — JSON-RPC 2.0 over stdio, non-negotiable
- **Architecture**: Three-manager pattern with single Supervisor — proven in v1.0
- **Sandboxing**: WASM for custom tools — security boundary for user-defined code (v2)
- **Process model**: Channels as subprocesses, agent as long-lived process — multi-agent support with per-agent routing

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust as implementation language | Systems reliability, performance, safety for infrastructure sidecar | ✓ Good — 12.7K LOC, strong type safety caught bugs at compile time |
| ACP as agent protocol | Industry standard for IDE↔agent communication, JSON-RPC 2.0 | ✓ Good — clean protocol boundary, mock agent easy to build |
| Three-manager architecture | Clean separation of concerns: agents, channels, tools | ✓ Good — independent crash recovery, clean message passing |
| WASM for tool sandboxing | Security isolation for user-defined tools with resource limits | ✓ Good — Wasmtime with fuel metering + epoch interruption (v2.0) |
| Channels as subprocesses | Isolation, independent crash recovery, language-agnostic extensions | ✓ Good — crash isolation proven in E2E tests |
| NDJSON over Content-Length framing | ACP uses NDJSON, simpler than Content-Length for stdio | ✓ Good — replaced ContentLengthCodec in Phase 2 |
| RPITIT instead of async_trait | Rust 2024 edition native support, no macro overhead | ✓ Good — cleaner code, but makes Manager trait non-object-safe (ManagerKind enum dispatch) |
| figment for config | Layered providers (defaults → file → env), type-safe, serde-based | ✓ Good — clean TOML + env var layering, extended to manager-hierarchy in v2.1 |
| TCP stub for MCP servers (v1) | Proves wiring without heavy rmcp dependency | ✓ Good — sufficient for v1, replaced with real rmcp in v2.0 |
| Per-channel ChannelSlot | Independent CancellationToken, backoff, crash tracker per channel | ✓ Good — crash isolation without affecting other channels |
| SessionKey routing | channel_name:kind:peer_id format for multi-session routing | ✓ Good — clean O(1) lookup for outbound routing |
| ChannelEvent in protoclaw-core | Avoids circular dependency between agents and channels crates | ✓ Good — clean crate boundary |
| debug-http as subprocess | Reference channel implementation, extracted from in-process to subprocess | ✓ Good — proves the channel subprocess protocol works end-to-end |
| protoclaw-sdk-types as shared leaf | Zero protoclaw-* deps, shared by all SDK crates | ✓ Good — clean dependency graph, no circular deps |
| Inlined ACP types instead of official SDK | Official agent-client-protocol crate has ?Send constraint incompatible with tokio | ✓ Good — wire-compatible types, deferred official SDK adoption |
| ChannelHarness with tokio::select | Inline bidirectional I/O instead of spawned tasks | ✓ Good — avoids 'static bounds, fully testable with mock readers |
| oneshot channels for permission bridging | Bridges async Channel::request_permission with HTTP handler responses | ✓ Good — clean async coordination without polling |
| cargo-chef multi-stage Dockerfile | Dependency caching without fingerprint hacks, per-binary runtime targets | ✓ Good — clean build caching, modular image composition (v2.1) |
| Manager-hierarchy config with named HashMaps | Entity names as map keys, embedded defaults.toml per component | ✓ Good — eliminates name duplication, clean Figment merge (v2.1) |
| AgentSlot per-agent lifecycle | Independent backoff, crash tracker, session management per agent | ✓ Good — multi-agent without shared state (v2.1) |
| DebounceBuffer → SessionQueue | Per-session FIFO queue replaces sliding-window debounce for better UX (users can queue messages like opencode) | ✓ Good — cleaner model, ack on every push, typing on dispatch only |
| Ack via direct channel notification | AckNotification sent to channel subprocess, not via ChannelEvent round-trip | ✓ Good — lower latency ack delivery (v2.1) |
| 50ms sleep replacing pending() in agent poll | pending().await permanently blocked select loop after 1ms timeout | ✓ Good — TDD-caught bug, matches ChannelsManager pattern (v2.1) |
| YAML over TOML for config | First-party Figment support, natural hierarchy, comment support, SubstYaml env interpolation | ✓ Good — cleaner config, 49 tests passing (v3.0) |
| Conservative 50% coverage baseline | Real baseline set on first CI run, ratchet prevents regression | ✓ Good — catches regressions without blocking development (v3.0) |
| Serde default functions duplicated in types.rs | Avoids adding protoclaw-core dependency to protoclaw-config | ✓ Good — keeps config crate leaf-level (v3.0) |
| Registry-based conditional tracing layers | Match-based layer selection, not boxed dynamic dispatch | ✓ Good — clean JSON/pretty-print switching (v3.0) |
| Ack at dispatch time, not inbound time | Prevents ack spam on batched messages — fires on last message only | ✓ Good — clean batch behavior (v3.0) |
| Config load before tracing init | Config parse errors print via anyhow to stderr before tracing is set up | ✓ Good — no silent failures on bad config (v3.0) |
| ProcessBackend trait for agent backends | Pluggable Local/Docker backends behind Box<dyn ProcessBackend> | ✓ Good — clean abstraction, MockBackend enables unit testing (v4.0) |
| bollard for Docker container management | Pure Rust Docker client, async, well-maintained | ✓ Good — stream bridging to existing ACP pipeline, no shelling out to docker CLI (v4.0) |
| Transparent opencode proxy (no escape stripping) | Opencode output is already ACP-formatted, stripping would corrupt data | ✓ Good — simpler implementation, correct behavior (v4.0) |
| rstest BDD migration across all crates | Consistent test naming (given_/when_/then_), fixtures, parameterised cases | ✓ Good — 372+ tests migrated, readable test names, reusable fixtures (v4.0) |
| ChatTurn state machine replacing per-chat fields | 12 independent fields → unified state machine with explicit transitions | ✓ Good — fixed truncation bug, cleaner state management (v4.0) |
| SDK extraction of ContentKind + PermissionBroker | Duplicated patterns across channels → shared SDK helpers | ✓ Good — new channels get correct behavior for free (v4.0) |
| protoclaw-supervisor library extraction | Binary crate can't be a library dependency; test-helpers needed Supervisor | ✓ Good — clean crate boundary, thin re-export in binary crate (v5.0) |
| Cross-crate type relocation to core | AgentsCommand/ToolsCommand/McpServerUrl crossed manager boundaries | ✓ Good — follows ChannelEvent pattern, original crates re-export for stability (v5.0) |
| Config-driven channel operation | std::env::var in channel binaries violated infrastructure-as-config principle | ✓ Good — all config via initialize handshake options, zero env reads in production (v5.0) |
| Per-example standalone Dockerfiles | Monorepo build context sent entire repo for example builds | ✓ Good — examples extend protoclaw-core base, local context, zero core recompilation (v5.0) |
| DOCK-11 accepted-as-is | Unified Cargo workspace vs separate workspace for examples | ✓ Good — Docker layer architecture satisfies intent without workspace split (v5.0) |

| MSRV 1.85 for native async fn in traits | async-trait removal requires Rust 1.85 minimum | ✓ Good — cleaner trait definitions, no macro overhead (v7.0) |
| SDK crates bumped to 0.2.0 | async-trait removal is a breaking API change | ✓ Good — clear semver signal to consumers (v7.0) |
| SubprocessSlot<C> generic unification | AgentSlot/ChannelSlot/ManagerSlot shared identical fields | ✓ Good — single implementation, consistent behavior (v7.0) |
| Distroless cc-debian12 for core image | Smaller attack surface, no shell, ~90MB savings vs debian-slim | ✓ Good — 174MB→58.7MB, security hardening (v7.1) |
| Strip+LTO in release profile | Binary size reduction without behavioral changes | ✓ Good — protoclaw binary 24MB stripped (v7.1) |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-04-11 after v7.0 milestone*
