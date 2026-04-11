# Roadmap: Protoclaw

## Milestones

- ✅ **v1.0 MVP** — Phases 1-4 (shipped 2026-04-02)
- ✅ **v2.0 SDKs & Extensions** — Phases 5-8 (shipped 2026-04-03)
- ✅ **v2.1 Examples & Dockerfile Restructure** — Phases 9-12 (shipped 2026-04-04)
- ✅ **v3.0 Quality & Infrastructure Hardening** — Phases 13-18 (shipped 2026-04-04)
- ✅ **v3.1 Testing & Runtime Config** — Phases 19-22 (shipped 2026-04-05)
- ✅ **v4.0 Agent Containers & First-Party Opencode** — Phases 23-35 (shipped 2026-04-08)
- ✅ **v5.0 System Refactoring & Code Health** — Phases 36-44 (shipped 2026-04-09)
- ✅ **v5.1 Tech Debt & Hardening** — Phases 45-55 (shipped 2026-04-10)
- ✅ **v6.0 Open Source Launch** — Phases 56-63 (shipped 2026-04-11)
- 🚧 **v7.0 Tech Debt & Optimization** — Phases 64-71 (in progress)
- 🚧 **v7.1 Docker Image Optimization** — Phases 72-76 (planned)

## Phases

<details>
<summary>✅ v1.0 MVP (Phases 1-4) — SHIPPED 2026-04-02</summary>

- [x] Phase 1: Foundation (4/4 plans) — Core types, JSON-RPC framing, config parsing, supervisor lifecycle
- [x] Phase 2: Agent Connection (5/5 plans) — ACP protocol client, agent process management, MCP tool server standup
- [x] Phase 3: Channel Routing (3/3 plans) — Channel subprocess protocol, message routing, permission flow, debug-http channel
- [x] Phase 4: Developer Experience (3/3 plans) — CLI subcommands, config validation, startup banner, health endpoint, status command

</details>

<details>
<summary>✅ v2.0 SDKs & Extensions (Phases 5-8) — SHIPPED 2026-04-03</summary>

- [x] Phase 5: SDK Crates (5/5 plans) — Three publishable SDK crates with debug-http retrofit and adapter integration
- [x] Phase 6: Tools Infrastructure (3/3 plans) — Real MCP hosting via rmcp and WASM sandboxed tool execution
- [x] Phase 7: Telegram Channel (3/3 plans) — First real messaging channel validating the Channels SDK
- [x] Phase 8: Examples & Onboarding (3/3 plans) — Docker Compose starter example for telegram-bot

</details>

<details>
<summary>✅ v2.1 Examples & Dockerfile Restructure (Phases 9-12) — SHIPPED 2026-04-04</summary>

- [x] Phase 9: Infrastructure Restructure (5/5 plans) — Multi-stage Dockerfile, mock agent promotion, thinking pipeline, test fixup
- [x] Phase 10: Fake Agent Example (3/3 plans) — 01-fake-agent-telegram-bot with debug-http + telegram, no real AI keys
- [x] Phase 11: Real Agent Examples & Cleanup (4/4 plans) — 02-real-agents-telegram-bot with opencode/claude-code, remove old examples
- [x] Phase 12: Message Debouncing, Ack Reactions & Config Defaults (4/4 plans) — Manager-hierarchy config, debounce buffer, ack reactions

</details>

<details>
<summary>✅ v3.0 Quality & Infrastructure Hardening (Phases 13-18) — SHIPPED 2026-04-04</summary>

- [x] Phase 13: Test Foundation & Documentation (3/3 plans) — Coverage tooling, test helpers, design principles docs
- [x] Phase 14: Config Format Migration (2/2 plans) — TOML→YAML with first-party Figment support
- [x] Phase 15: Config-Driven Constants (2/2 plans) — All hardcoded values extracted into typed config structs
- [x] Phase 16: Structured Logging (2/2 plans) — ANSI stripping, JSON log format, subprocess log attribution
- [x] Phase 17: Telegram Batch Ack (1/1 plan) — Ack reaction targets only last message in debounced batch
- [x] Phase 18: Example Validation & Flow Tests (2/2 plans) — Config validation, docker-compose checks, self-contained test scripts

</details>

<details>
<summary>✅ v3.1 Testing & Runtime Config (Phases 19-22) — SHIPPED 2026-04-05</summary>

- [x] Phase 19: Test Infrastructure (3/3 plans) — SSE helper extraction, test-log integration, test suite reorganization
- [x] Phase 20: E2E Test Coverage (2/2 plans) — Multi-agent, thinking, crash recovery, shutdown, health E2E tests
- [x] Phase 21: Runtime Config Loading (1/1 plan) — Config path E2E test, Dockerfile verification, README docs
- [x] Phase 22: SDK Integration Tests (2/2 plans) — sdk-test-channel, sdk-test-tool, E2E integration tests

</details>

<details>
<summary>✅ v4.0 Agent Containers & First-Party Opencode (Phases 23-35) — SHIPPED 2026-04-08</summary>

- [x] Phase 23: Config Schema Extension (1/1 plans) — WorkspaceConfig enum (Local/Docker) + DockerWorkspaceConfig on AgentConfig
- [x] Phase 24.1: Thinking Display Fix & Message Batching (1/1 plans) — Debounced thought streaming in Telegram, system-level merge window for rapid messages
- [x] Phase 24: ConnectionBackend Refactor (2/2 plans) — ProcessBackend trait + LocalBackend/DockerBackend, AgentConnection refactor to Box<dyn ProcessBackend>
- [x] Phase 24.2: BDD Test Coverage & Bug Fixes (7/7 plans) — Enforce BDD-style TDD across all crates, catalog all features with proper test coverage, fix broken functionality
- [x] Phase 25: Docker Module & Stream Bridging (2/2 plans) — docker.rs in protoclaw-agents: spawn, reader task, lifecycle, orphan cleanup, image pull, crash recovery
- [x] Phase 26: Opencode Wrapper Binary (1/1 plans) — ext/agents/opencode-wrapper/ thin binary: spawn+pipe to opencode acp, transparent stdio proxy, explicit env, opencode_config mapping
- [x] Phase 27: Docker Example (2/2 plans) — Update examples 01+02 with Docker workspace, socket proxy, agent Dockerfiles
- [x] Phase 28: Docker Integration Tests (2/2 plans) — Integration tests for Docker agent: spawn, communicate, shutdown, crash recovery
- [x] Phase 29: E2E Session & Batch Flows (2/2 plans) — End-to-end tests for session lifecycle, message batching, and complete response flows
- [x] Phase 30: E2E Tool Invocation & Payload Handling (2/2 plans) — End-to-end tests for MCP tool calls, payload serialization, and error propagation
- [x] Phase 31: E2E Resilience & Recovery (1/1 plans) — End-to-end tests for crash recovery, reconnection, and graceful degradation
- [x] Phase 32: Telegram ChatTurn State Machine (1/1 plans) — Replace 12 per-chat fields with ChatTurn state machine, fix truncation bug
- [x] Phase 33: Channel SDK DX Improvements (3/3 plans) — Extract duplicated patterns into SDK, improve new-channel docs
- [x] Phase 34: Milestone Bookkeeping & Requirements Sync (1/1 plans) — Fix ROADMAP structural issues, cross-verify REQUIREMENTS consistency
- [x] Phase 35: Example 02 End-to-End Fix (1/1 plans) — Fix examples/02-real-agents-telegram-bot to work end-to-end (EXMP-01)

</details>

<details>
<summary>✅ v5.0 System Refactoring & Code Health (Phases 36-44) — SHIPPED 2026-04-09</summary>

- [x] Phase 36: Crate Boundary Foundations (2/2 plans) — Eliminate cross-crate deps, extract protoclaw-supervisor, typed errors
- [x] Phase 37: ChannelEvent Relocation & Typed Config Conversion (2/2 plans) — ChannelEvent to sdk-types, typed AckConfig From impl
- [x] Phase 38: Modularity Cleanup (2/2 plans) — Remove channels types.rs shim, remove sdk-tool schemars re-export
- [x] Phase 39: Code Smell Elimination (1/1 plan) — Zero unwrap, zero dead_code, TCP stub removed, dead types deleted, clippy clean
- [x] Phase 40: Docker Build Efficiency (1/1 plan) — Core-only root Dockerfile, consolidated base image
- [x] Phase 41: Config-Driven Operation (1/1 plan) — Zero std::env::var, all config via initialize handshake
- [x] Phase 42: Documentation & AGENTS.md Hygiene (0/0 plans) — All crate AGENTS.md updated, design-principles v5.0 section
- [x] Phase 43: Code Smell & Modularity Fixes (1/1 plan) — Gap closure: From impl, bare unwrap, clippy
- [x] Phase 44: Docker Build Context Isolation (2/2 plans) — Gap closure: per-example Dockerfiles, local build contexts

</details>

<details>
<summary>✅ v5.1 Tech Debt & Hardening (Phases 45-55) — SHIPPED 2026-04-10</summary>

- [x] Phase 45: Config Normalization & Validation (2/2 plans) — Snake_case keys, env override removal, hostname validation
- [x] Phase 46: Config Completeness (2/2 plans) — ACP timeout, Telegram turn/rate-limit delays, channel exit timeout, StreamableHttp docs
- [x] Phase 47: serde_yaml Replacement (1/1 plan) — Replaced deprecated serde_yaml with serde_yml across all crates
- [x] Phase 48: Error Handling & Resilience (5/5 plans) — Telegram retries, session error forwarding, crash escalation, unwrap audit, ACP error forwarding
- [x] Phase 49: Production Safety & Observability (3/3 plans) — Bare unwrap elimination, SubstYaml fail-loud, tracing instrument spans
- [x] Phase 50: Test Coverage Gaps (4/4 plans) — ExternalMcpServer tests, mcp_servers integration test, fs callback tests, dual-channel isolation
- [x] Phase 51: Hygiene & Documentation (3/3 plans) — Dead code/clone cleanup, AGENTS.md updates
- [x] Phase 52: Config Gap Closure (2/2 plans) — Snake_case normalization with serde alias backward compat, env override removal
- [x] Phase 53: Error Handling Gap Closure (3/3 plans) — Telegram retry coverage, CrashTracker per agent slot, unwrap_or_default logged fallbacks
- [x] Phase 54: Observability, Test & Docs Gap Closure (2/2 plans) — Tracing instrument spans, ExternalMcpServer routing tests, AGENTS.md v5.1 changelogs
- [x] Phase 55: Final Gap Closure (2/2 plans) — WASM E2E integration test, Docker base image digest pinning

</details>

### 🚧 v6.0 Open Source Launch (Phases 56-63) — SHIPPED 2026-04-11

- [x] Phase 56: Hygiene & Pre-flight (HYG-01, HYG-02, HYG-03, HYG-04, CI-05)
- [x] Phase 57: Governance Files (GOV-01, GOV-02, GOV-03, GOV-04)
- [x] Phase 58: Cargo.toml Metadata & SDK Packaging (PKG-01, PKG-02, PKG-03, CI-04)
- [x] Phase 59: Documentation (DOCS-01, DOCS-02, DOCS-03)
- [x] Phase 60: GitHub Templates & Settings (GH-01, GH-02, GH-03, GH-04, GH-05)
- [x] Phase 61: CI/CD — Release Automation (CI-01, CI-02)
- [x] Phase 62: CI/CD — Container Images (CI-03)
- [x] Phase 63: First Release & Verification (PKG-04)

### Phase 56: Hygiene & Pre-flight

**Requirements:** HYG-01, HYG-02, HYG-03, HYG-04, CI-05

**Rationale:** Clean the repo before any public visibility. `.planning/` removal from history, secret scanning, gitignore hardening, and the CI test fix must all land first — before governance files or packaging that might trigger CI runs.

**Success criteria:**
1. `.gitignore` contains entries for `.planning/`, `.DS_Store`, `.idea/`, `.vscode/`, and `*.wasm`; `git check-ignore` confirms all are ignored
2. `gitleaks detect --source . --no-git` exits 0 with no findings on the current working tree
3. `.planning/` directory does not appear in any commit reachable from `HEAD` after `git filter-repo` rewrite
4. No scratch files (`idea.md`, `prompt.md`, or similar) exist at repo root or in tracked paths
5. `cargo test -p protoclaw-tools` passes green, including `flows_sdk_tool`, on a clean CI runner without the `echo` binary workaround

### Phase 57: Governance Files

**Requirements:** GOV-01, GOV-02, GOV-03, GOV-04

**Rationale:** LICENSE files are required by crates.io before any publish attempt. CONTRIBUTING.md and CODE_OF_CONDUCT.md set community expectations before the repo is promoted publicly.

**Success criteria:**
1. `LICENSE-MIT` and `LICENSE-APACHE-2.0` exist at repo root with correct year and copyright holder text
2. `CONTRIBUTING.md` documents the PR workflow, required test commands (`cargo test`, `cargo clippy --workspace`), and commit conventions matching AGENTS.md
3. `CODE_OF_CONDUCT.md` references the Rust Code of Conduct URL and provides a working enforcement contact email
4. `SECURITY.md` documents the private email reporting path and references RustSec advisory DB submission

### Phase 58: Cargo.toml Metadata & SDK Packaging

**Requirements:** PKG-01, PKG-02, PKG-03, CI-04

**Rationale:** Metadata and `publish = false` guards must be verified before CI automation can safely run `cargo publish`. `cargo-deny` belongs here because it validates the license metadata just set.

**Success criteria:**
1. All 4 SDK crates (`protoclaw-sdk-types`, `protoclaw-sdk-agent`, `protoclaw-sdk-channel`, `protoclaw-sdk-tool`) have `license`, `description`, `repository`, `homepage`, `readme`, `keywords`, and `categories` fields in `Cargo.toml`
2. All non-SDK workspace crates have `publish = false` in their `Cargo.toml`; `cargo metadata --no-deps` confirms no unwanted `publish = true` values
3. `cargo publish --dry-run -p protoclaw-sdk-types` (and the three other SDK crates) all exit 0 with no errors
4. `deny.toml` created with MIT/Apache-2.0/BSD/ISC allowlist and GPL denylist; `cargo deny check licenses` passes green on a fresh clone

### Phase 59: Documentation

**Requirements:** DOCS-01, DOCS-02, DOCS-03

**Rationale:** README is the project's front door — it must be ready before GitHub templates or CI badges reference it. CHANGELOG seeds release-plz automation. The `docs/` content finalizes the public developer story.

**Success criteria:**
1. `README.md` renders correctly on GitHub with project description, architecture diagram or overview, quickstart section, SDK crate table with links, and CI + license badges
2. `CHANGELOG.md` exists with a `[Unreleased]` section and a seeded `[0.1.0]` entry summarizing v5.1 in Keep a Changelog format
3. `docs/` contains at minimum: `architecture.md` (system overview, crate dependency flow), `design-principles.md` (core invariants, anti-patterns), and `project-structure.md` (workspace layout, where to find things)
4. All links in README.md and docs/ resolve — no 404s on a fresh clone

### Phase 60: GitHub Templates & Settings

**Requirements:** GH-01, GH-02, GH-03, GH-04, GH-05

**Rationale:** Community infrastructure (issue templates, PR template, Dependabot) should be live before the release automation pipeline is enabled — so incoming contributions land in a well-structured repo from day one.

**Success criteria:**
1. `.github/ISSUE_TEMPLATE/bug_report.yml` renders a structured form on GitHub with required fields (version, OS, reproduction steps, expected/actual behavior)
2. `.github/ISSUE_TEMPLATE/feature_request.yml` renders a structured form with motivation + proposed solution fields
3. `.github/pull_request_template.md` contains Motivation, Solution, and Testing sections; new PRs pre-populate with this template
4. `.github/dependabot.yml` configures weekly updates for both `cargo` and `github-actions` ecosystems
5. GitHub repo has description and relevant topics set, and branch protection on `main` requires at least the `ci` status check to pass before merge

### Phase 61: CI/CD — Release Automation

**Requirements:** CI-01, CI-02

**Rationale:** release-plz automates the changelog + version bump + publish cycle. It must be configured and tested before the first real `cargo publish` in Phase 63.

**Success criteria:**
1. `.github/workflows/release-plz.yml` exists and triggers on push to `main`; on a test push it opens a release PR with correct version bumps and changelog entries
2. The release-plz workflow publishes SDK crates in dependency order (sdk-types first, then sdk-agent/sdk-channel/sdk-tool) when a release PR is merged
3. `release-plz.toml` (or equivalent config) explicitly excludes all non-SDK crates from publication
4. The release PR generated by release-plz includes accurate changelog entries derived from conventional commit messages

### Phase 62: CI/CD — Container Images

**Requirements:** CI-03

**Rationale:** Docker image builds are triggered by version tags created by release-plz in Phase 61. The workflow must be defined before the first release tag is pushed.

**Success criteria:**
1. `.github/workflows/docker.yml` exists and triggers on semver tags (`v*.*.*`)
2. Multi-platform build (linux/amd64 + linux/arm64) completes successfully for both the protoclaw binary image and the mock-agent image
3. Images are pushed to `ghcr.io/protoclaw/protoclaw` with `latest`, semver (`v1.2.3`), and short-SHA tags
4. A test workflow run against a pre-release tag confirms ghcr.io images are publicly pullable without authentication

### Phase 63: First Release & Verification

**Requirements:** PKG-04

**Rationale:** This is the integration test for the entire v6.0 pipeline. Every prior phase feeds into this one — hygiene, governance, metadata, docs, templates, CI automation, and container images all must work end-to-end for the first public release to succeed.

**Success criteria:**
1. SDK crates published to crates.io at version 0.1.0; `cargo add protoclaw-sdk-types` resolves from the public registry
2. Trusted Publishing (OIDC) configured so no long-lived API tokens are stored in GitHub secrets for subsequent releases
3. `ghcr.io/protoclaw/protoclaw:0.1.0` and `:latest` are publicly pullable after the release tag triggers the Docker workflow
4. GitHub release page for `v0.1.0` is created automatically with changelog content by release-plz
5. All CI status checks on `main` show green after the 0.1.0 release commit

### 🚧 v7.0 Tech Debt & Optimization (Phases 64-71) — IN PROGRESS

- [x] Phase 64: CI Hardening (CI-01, CI-02, CI-03)
- [x] Phase 65: Error Handling & Safety (ERR-01, ERR-02)
- [x] Phase 66: Dependency Modernization (DEPS-01, DEPS-02, DEPS-03)
- [x] Phase 67: Config Cleanup (CFG-01, CFG-02)
- [x] Phase 68: Architecture Deduplication (ARCH-01, ARCH-02, ARCH-03)
- [x] Phase 69: Code Quality (QUAL-01, QUAL-02, QUAL-03, QUAL-04)
- [x] Phase 70: Test Coverage (TEST-01, TEST-02, TEST-03)
- [x] Phase 71: Documentation Lints (DOCS-01, DOCS-02)

### Phase 64: CI Hardening

**Requirements:** CI-01, CI-02, CI-03

**Rationale:** Catches issues in all subsequent phases. Quick wins with no code changes — security auditing, MSRV enforcement, and Cargo.lock integrity land first so every following phase runs against a hardened baseline.

**Success criteria:**
1. `cargo audit` step added to CI workflow; `cargo audit` exits 0 on current dependency tree
2. MSRV enforcement job added — `cargo build` with `rust-version = "1.85"` toolchain passes on current codebase
3. `--locked` flag added to all `cargo build` and `cargo test` invocations in CI; pipeline passes green

### Phase 65: Error Handling & Safety

**Requirements:** ERR-01, ERR-02

**Rationale:** Clean error boundaries before architecture refactoring. The `SupervisorError` typed enum is needed before slot unification in Phase 68. Eliminating `unsafe` blocks removes the only safety footgun in the codebase.

**Success criteria:**
1. `protoclaw-supervisor` public API (`run()`, `run_with_cancel()`, `boot_managers()`) returns `SupervisorError` typed enum; `anyhow` removed from the public API surface
2. Zero `unsafe` blocks anywhere in the workspace; `crates/protoclaw/src/init.rs` test code uses `temp-env` crate or `serial_test` for env isolation

### Phase 66: Dependency Modernization

**Requirements:** DEPS-01, DEPS-02, DEPS-03

**Rationale:** `async-trait` removal simplifies trait definitions and reduces macro expansion overhead before the slot unification in Phase 68. Patch updates include security-adjacent `rustls-webpki`. Trimming `tokio` features reduces compile time and makes feature dependencies explicit.

**Success criteria:**
1. `async-trait` crate removed from all 9 workspace crates; all async traits use native Rust 1.85 syntax
2. SDK crates (`sdk-types`, `sdk-agent`, `sdk-channel`, `sdk-tool`) bumped to 0.2.0 in Cargo.toml reflecting the breaking API change
3. All patch updates applied: rustls-webpki, wasm-bindgen ecosystem, rand, cc, js-sys/web-sys; `cargo tree` shows updated versions
4. Library crates replace `tokio = { features = ["full"] }` with minimal required feature lists; `cargo build` passes

### Phase 67: Config Cleanup

**Requirements:** CFG-01, CFG-02

**Rationale:** Enum types clarify the domain model and eliminate stringly-typed comparison bugs before the architecture dedup in Phase 68. Crash config hierarchy documentation prevents future confusion when the slot pattern is unified.

**Success criteria:**
1. `ToolConfig.tool_type` uses `ToolType::Mcp | ToolType::Wasm` enum; `log_format` uses `LogFormat::Pretty | LogFormat::Json`; `reaction_lifecycle` uses `ReactionLifecycle::Remove | ReactionLifecycle::ReplaceDone`
2. Serde deserialization for all three fields still works with existing YAML configs (string values round-trip correctly)
3. `SupervisorConfig.max_restarts`/`restart_window_secs` vs per-entity `CrashTrackerConfig` documented in config types with doc comments; `CFG-02` concern addressed

### Phase 68: Architecture Deduplication

**Requirements:** ARCH-01, ARCH-02, ARCH-03

**Rationale:** The big refactor — unifies the slot pattern and crash recovery loop across all three managers. Requires the clean foundation from phases 64-67: typed errors (ERR-01), no unsafe (ERR-02), async-trait gone (DEPS-01), and enum config types (CFG-01).

**Success criteria:**
1. `SubprocessSlot<C>` generic struct in `protoclaw-core` holds `cancel_token`, `backoff`, `crash_tracker`, `disabled`; AgentSlot, ChannelSlot, and ManagerSlot all use it
2. `CrashRecovery` helper in `protoclaw-core` implements the record-crash → check-loop → backoff → respawn state machine; agents, channels, and supervisor managers all use it
3. `PendingPermission` changed to `pub(crate)`; `AgentSlot` fields changed to `pub(crate)`; `with_log_level()` duplication removed
4. All existing unit and integration tests pass; `cargo clippy --workspace` clean

### Phase 69: Code Quality

**Requirements:** QUAL-01, QUAL-02, QUAL-03, QUAL-04

**Rationale:** Extract long functions after architecture changes settle. The slot unification in Phase 68 likely reshuffles function boundaries; extracting before that would be rework. `SessionKey` optimization targets the hot message-routing path.

**Success criteria:**
1. `collect_channel_message` in `protoclaw-channels/src/manager.rs` split into named sub-functions; no single function in the file exceeds 100 lines
2. `spawn_inner` and `spawn` in agents connection/docker_backend split into smaller named functions; each under 100 lines
3. `SessionKey` cloning in message-routing hot paths evaluated; either `Arc<str>` adopted or a justified decision documented in code comment
4. `handle_incoming`, `handle_crash`, `handle_channel_event`, `start` (tools manager), `validate_config`, and custom `deserialize<D>` each under 100 lines; all existing tests pass

### Phase 70: Test Coverage

**Requirements:** TEST-01, TEST-02, TEST-03

**Rationale:** Test the final state after all refactoring is complete. Writing tests before Phase 68 would require rewriting them after the slot unification changes function signatures.

**Success criteria:**
1. `protoclaw-sdk-agent` has `#[rstest]` unit tests covering all 7 default `AgentAdapter` trait method hooks and `GenericAcpAdapter` passthrough behavior
2. `protoclaw-sdk-tool` has `#[rstest]` unit tests covering the `Tool` trait contract, `ToolServer` dispatch routing, and error handling paths
3. `backend.rs`, `agents_command.rs`, `tools_command.rs`, and `generic.rs` each have `#[cfg(test)]` modules with at least one meaningful test; `cargo test` passes green

### Phase 71: Documentation Lints

**Requirements:** DOCS-01, DOCS-02

**Rationale:** Document the final API surface after all changes have settled. Running missing_docs lint before architecture dedup would generate warnings on types that get renamed or moved.

**Success criteria:**
1. `#![warn(missing_docs)]` added to `lib.rs` of all 4 SDK crates; `cargo build -p protoclaw-sdk-types -p protoclaw-sdk-agent -p protoclaw-sdk-channel -p protoclaw-sdk-tool` produces zero `missing_docs` warnings
2. All public types in `protoclaw-sdk-types` (ChannelEvent, SessionKey, PermissionOption, ChannelCapabilities, ContentKind, ThoughtContent) have doc comments explaining their purpose and field semantics

### 🚧 v7.1 Docker Image Optimization (Phases 72-76) — PLANNED

- [x] Phase 72: Core Image Optimization (IMG-01, IMG-02, IMG-03, IMG-04) — **Plans:** 1 plan (completed 2026-04-11)
  Plans:
  - [x] 72-01-PLAN.md — Strip+LTO release profile, distroless core image, protoclaw-only
- [ ] Phase 73: Extension Path Normalization (EXT-01)
- [ ] Phase 74: Dockerfile Restructure & Builder Image (EXT-02, EXT-03, EXT-04)
- [ ] Phase 75: Extension Image Optimization (OPT-01, OPT-02, OPT-03)
- [ ] Phase 76: ghcr.io Lifecycle Management (GHCR-01, GHCR-02, GHCR-03, GHCR-04, GHCR-05)

### Phase 72: Core Image Optimization

**Requirements:** IMG-01, IMG-02, IMG-03, IMG-04

**Rationale:** Biggest bang for the buck — distroless base saves ~90MB, strip+LTO saves ~20-30MB on binaries. Core image drops from 174MB to <30MB. Must land before Dockerfile restructure (Phase 74) since the core stage changes fundamentally.

**Success criteria:**
1. `[profile.release]` has `strip = true` and `lto = true`; `cargo build --release` produces stripped binaries
2. Core Dockerfile uses `gcr.io/distroless/cc-debian12:nonroot` as runtime base (no debian:bookworm-slim)
3. Core image contains only `protoclaw` binary — no ext/ binaries (telegram-channel, debug-http, mock-agent, system-info, opencode-wrapper removed from core stage)
4. `docker images` shows core image under 30MB; Dockerfile includes `stat`-based size gate rejecting binaries under 1MB

### Phase 73: Extension Path Normalization

**Requirements:** EXT-01

**Rationale:** The `@built-in/` path convention must be consistent before restructuring Dockerfiles and builder images — otherwise the binary layout in images won't match config expectations. Current paths (`@built-in/telegram-channel`, `@built-in/agents/opencode`) are inconsistent with the ext/ directory structure.

**Success criteria:**
1. `@built-in/` resolves to `extensions_dir/{agents,channels,tools}/<name>` — e.g. `@built-in/agents/opencode-wrapper`, `@built-in/channels/telegram`
2. Old paths (`@built-in/telegram-channel`, `@built-in/agents/opencode`) still work via backward-compat aliases or migration
3. All example `protoclaw.yaml` configs updated to new path convention

### Phase 74: Dockerfile Restructure & Builder Image

**Requirements:** EXT-02, EXT-03, EXT-04

**Rationale:** With core image slim and paths normalized, restructure the Dockerfile into core (protoclaw-only) and builder (all ext/ binaries) stages. The builder image enables users to compose custom images via `COPY --from=`. Depends on Phase 72 (core stage) and Phase 73 (path layout).

**Success criteria:**
1. Root Dockerfile has two publishable stages: `core` (protoclaw-only distroless) and `builder` (all ext/ binaries in known paths)
2. Builder image pushed to `ghcr.io/protoclaw/protoclaw-builder` with ext/ binaries at `/usr/local/bin/{agents,channels,tools}/<name>`
3. Example Dockerfiles use `COPY --from=ghcr.io/protoclaw/protoclaw-builder` to compose custom images
4. Example 01 and 02 Dockerfiles updated and working end-to-end

### Phase 75: Extension Image Optimization

**Requirements:** OPT-01, OPT-02, OPT-03

**Rationale:** Agent images (opencode 802MB, claude-code similar) are dominated by Node.js + npm install. Multi-stage builds with production-only deps and pruning can significantly reduce size. Lands after Dockerfile restructure so examples use the new composition pattern.

**Success criteria:**
1. opencode agent image uses multi-stage npm install with `--omit=dev` and node_modules pruning
2. claude-code agent image uses same optimization approach
3. Size reduction documented — baseline vs optimized for both images

### Phase 76: ghcr.io Lifecycle Management

**Requirements:** GHCR-01, GHCR-02, GHCR-03, GHCR-04, GHCR-05

**Rationale:** Final phase — production-grade registry management for the optimized images. Tagging, retention, scanning, and multi-arch verification. Depends on all prior phases since it manages the images they produce.

**Success criteria:**
1. Tagging conventions documented in CONTRIBUTING.md or docs/ — semver, latest, short-SHA applied on every release
2. GitHub Actions workflow deletes untagged manifests older than 30 days
3. Trivy vulnerability scan runs on every image push, blocks release on critical/high findings
4. Multi-arch manifest (amd64 + arm64) verified for core and builder images — `docker manifest inspect` shows both platforms
5. Retention policy keeps last 10 semver tags + latest; older tags eligible for cleanup

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Foundation | v1.0 | 4/4 | Complete | 2026-04-02 |
| 2. Agent Connection | v1.0 | 5/5 | Complete | 2026-04-02 |
| 3. Channel Routing | v1.0 | 3/3 | Complete | 2026-04-02 |
| 4. Developer Experience | v1.0 | 3/3 | Complete | 2026-04-02 |
| 5. SDK Crates | v2.0 | 5/5 | Complete | 2026-04-02 |
| 6. Tools Infrastructure | v2.0 | 3/3 | Complete | 2026-04-03 |
| 7. Telegram Channel | v2.0 | 3/3 | Complete | 2026-04-03 |
| 8. Examples & Onboarding | v2.0 | 3/3 | Complete | 2026-04-03 |
| 9. Infrastructure Restructure | v2.1 | 5/5 | Complete | 2026-04-03 |
| 10. Fake Agent Example | v2.1 | 3/3 | Complete | 2026-04-03 |
| 11. Real Agent Examples & Cleanup | v2.1 | 4/4 | Complete | 2026-04-03 |
| 12. Message Debouncing, Ack Reactions & Config Defaults | v2.1 | 4/4 | Complete | 2026-04-04 |
| 13. Test Foundation & Documentation | v3.0 | 3/3 | Complete | 2026-04-04 |
| 14. Config Format Migration | v3.0 | 2/2 | Complete | 2026-04-04 |
| 15. Config-Driven Constants | v3.0 | 2/2 | Complete | 2026-04-04 |
| 16. Structured Logging | v3.0 | 2/2 | Complete | 2026-04-04 |
| 17. Telegram Batch Ack | v3.0 | 1/1 | Complete | 2026-04-04 |
| 18. Example Validation & Flow Tests | v3.0 | 2/2 | Complete | 2026-04-04 |
| 19. Test Infrastructure | v3.1 | 3/3 | Complete | 2026-04-04 |
| 20. E2E Test Coverage | v3.1 | 2/2 | Complete | 2026-04-04 |
| 21. Runtime Config Loading | v3.1 | 1/1 | Complete | 2026-04-05 |
| 22. SDK Integration Tests | v3.1 | 2/2 | Complete | 2026-04-05 |
| 23. Config Schema Extension | v4.0 | 1/1 | Complete | 2026-04-05 |
| 24.1. Thinking Display Fix & Message Batching | v4.0 | 1/1 | Complete | 2026-04-05 |
| 24. ConnectionBackend Refactor | v4.0 | 2/2 | Complete | 2026-04-06 |
| 24.2. BDD Test Coverage & Bug Fixes | v4.0 | 7/7 | Complete | 2026-04-06 |
| 25. Docker Module & Stream Bridging | v4.0 | 2/2 | Complete | 2026-04-06 |
| 26. Opencode Wrapper Binary | v4.0 | 1/1 | Complete | 2026-04-07 |
| 27. Docker Example | v4.0 | 2/2 | Complete | 2026-04-07 |
| 28. Docker Integration Tests | v4.0 | 2/2 | Complete | 2026-04-07 |
| 29. E2E Session & Batch Flows | v4.0 | 2/2 | Complete | 2026-04-07 |
| 30. E2E Tool Invocation & Payload Handling | v4.0 | 2/2 | Complete | 2026-04-07 |
| 31. E2E Resilience & Recovery | v4.0 | 1/1 | Complete | 2026-04-07 |
| 32. Telegram ChatTurn State Machine | v4.0 | 1/1 | Complete | 2026-04-08 |
| 33. Channel SDK DX Improvements | v4.0 | 3/3 | Complete | 2026-04-08 |
| 34. Milestone Bookkeeping & Requirements Sync | v4.0 | 1/1 | Complete | 2026-04-08 |
| 35. Example 02 End-to-End Fix | v4.0 | 1/1 | Complete | 2026-04-08 |
| 36. Crate Boundary Foundations | v5.0 | 2/2 | Complete | 2026-04-08 |
| 37. ChannelEvent Relocation & Typed Config Conversion | v5.0 | 2/2 | Complete | 2026-04-09 |
| 38. Modularity Cleanup | v5.0 | 2/2 | Complete | 2026-04-09 |
| 39. Code Smell Elimination | v5.0 | 1/1 | Complete | 2026-04-09 |
| 40. Docker Build Efficiency | v5.0 | 1/1 | Complete | 2026-04-09 |
| 41. Config-Driven Operation | v5.0 | 1/1 | Complete | 2026-04-09 |
| 42. Documentation & AGENTS.md Hygiene | v5.0 | 0/0 | Complete | 2026-04-09 |
| 43. Code Smell & Modularity Fixes (Gap Closure) | v5.0 | 1/1 | Complete    | 2026-04-09 |
| 44. Docker Build Context Isolation (Gap Closure) | v5.0 | 2/2 | Complete    | 2026-04-09 |
| 45. Config Normalization & Validation | v5.1 | 2/2 | Complete | 2026-04-10 |
| 46. Config Completeness | v5.1 | 2/2 | Complete | 2026-04-10 |
| 47. serde_yaml Replacement | v5.1 | 1/1 | Complete | 2026-04-10 |
| 48. Error Handling & Resilience | v5.1 | 5/5 | Complete | 2026-04-10 |
| 49. Production Safety & Observability | v5.1 | 3/3 | Complete | 2026-04-10 |
| 50. Test Coverage Gaps | v5.1 | 4/5 | Complete | 2026-04-10 |
| 51. Hygiene & Documentation | v5.1 | 2/3 | Complete | 2026-04-10 |
| 52. Config Gap Closure | v5.1 | 2/2 | Complete | 2026-04-10 |
| 53. Error Handling Gap Closure | v5.1 | 3/3 | Complete | 2026-04-10 |
| 54. Observability, Test & Docs Gap Closure | v5.1 | 2/2 | Complete | 2026-04-10 |
| 55. Final Gap Closure (TEST-04, HYG-03) | v5.1 | 2/2 | Complete | 2026-04-10 |
| 56. Hygiene & Pre-flight | v6.0 | 0/— | Complete | 2026-04-11 |
| 57. Governance Files | v6.0 | 0/— | Complete | 2026-04-11 |
| 58. Cargo.toml Metadata & SDK Packaging | v6.0 | 0/— | Complete | 2026-04-11 |
| 59. Documentation | v6.0 | 0/— | Complete | 2026-04-11 |
| 60. GitHub Templates & Settings | v6.0 | 0/— | Complete | 2026-04-11 |
| 61. CI/CD — Release Automation | v6.0 | 0/— | Complete | 2026-04-11 |
| 62. CI/CD — Container Images | v6.0 | 0/— | Complete | 2026-04-11 |
| 63. First Release & Verification | v6.0 | 0/— | Complete | 2026-04-11 |
| 64. CI Hardening | v7.0 | 0/— | Pending | — |
| 65. Error Handling & Safety | v7.0 | 0/— | Pending | — |
| 66. Dependency Modernization | v7.0 | 0/— | Pending | — |
| 67. Config Cleanup | v7.0 | 0/— | Pending | — |
| 68. Architecture Deduplication | v7.0 | 0/— | Pending | — |
| 69. Code Quality | v7.0 | 0/— | Pending | — |
| 70. Test Coverage | v7.0 | 0/— | Pending | — |
| 71. Documentation Lints | v7.0 | 0/— | Pending | — |
| 72. Core Image Optimization | v7.1 | 1/1 | Complete    | 2026-04-11 |
| 73. Extension Path Normalization | v7.1 | 0/— | Pending | — |
| 74. Dockerfile Restructure & Builder Image | v7.1 | 0/— | Pending | — |
| 75. Extension Image Optimization | v7.1 | 0/— | Pending | — |
| 76. ghcr.io Lifecycle Management | v7.1 | 0/— | Pending | — |
