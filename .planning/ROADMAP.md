# Roadmap: Protoclaw

## Milestones

- ✅ **Pre-release Development** — Phases 1-76 (shipped 2026-04-12, released as v0.2.x)
- ✅ **v0.3.0 Production Readiness & Protocol Stabilization** — Phases 77-83 (shipped 2026-04-12)
- ✅ **v0.3.1 Config Schema Modernization** — Phases 84-85 (shipped 2026-04-13)
- 🚧 **v0.3.2 DX & Permission Hardening** — Phases 86-89 (in progress)
- 📋 **v0.3.3 Session Persistence & Recovery** — Phases 90-94 (planned)

## Phases

<details>
<summary>✅ Pre-release Development (Phases 1-76) — SHIPPED 2026-04-12</summary>

<details>
<summary>✅ MVP (Phases 1-4) — SHIPPED 2026-04-02</summary>

- [x] Phase 1: Foundation (4/4 plans) — Core types, JSON-RPC framing, config parsing, supervisor lifecycle
- [x] Phase 2: Agent Connection (5/5 plans) — ACP protocol client, agent process management, MCP tool server standup
- [x] Phase 3: Channel Routing (3/3 plans) — Channel subprocess protocol, message routing, permission flow, debug-http channel
- [x] Phase 4: Developer Experience (3/3 plans) — CLI subcommands, config validation, startup banner, health endpoint, status command

</details>

<details>
<summary>✅ SDKs & Extensions (Phases 5-8) — SHIPPED 2026-04-03</summary>

- [x] Phase 5: SDK Crates (5/5 plans) — Three publishable SDK crates with debug-http retrofit and adapter integration
- [x] Phase 6: Tools Infrastructure (3/3 plans) — Real MCP hosting via rmcp and WASM sandboxed tool execution
- [x] Phase 7: Telegram Channel (3/3 plans) — First real messaging channel validating the Channels SDK
- [x] Phase 8: Examples & Onboarding (3/3 plans) — Docker Compose starter example for telegram-bot

</details>

<details>
<summary>✅ Examples & Dockerfile Restructure (Phases 9-12) — SHIPPED 2026-04-04</summary>

- [x] Phase 9: Infrastructure Restructure (5/5 plans) — Multi-stage Dockerfile, mock agent promotion, thinking pipeline, test fixup
- [x] Phase 10: Fake Agent Example (3/3 plans) — 01-fake-agent-telegram-bot with debug-http + telegram, no real AI keys
- [x] Phase 11: Real Agent Examples & Cleanup (4/4 plans) — 02-real-agents-telegram-bot with opencode/claude-code, remove old examples
- [x] Phase 12: Message Debouncing, Ack Reactions & Config Defaults (4/4 plans) — Manager-hierarchy config, debounce buffer, ack reactions

</details>

<details>
<summary>✅ Quality & Infrastructure Hardening (Phases 13-18) — SHIPPED 2026-04-04</summary>

- [x] Phase 13: Test Foundation & Documentation (3/3 plans) — Coverage tooling, test helpers, design principles docs
- [x] Phase 14: Config Format Migration (2/2 plans) — TOML→YAML with first-party Figment support
- [x] Phase 15: Config-Driven Constants (2/2 plans) — All hardcoded values extracted into typed config structs
- [x] Phase 16: Structured Logging (2/2 plans) — ANSI stripping, JSON log format, subprocess log attribution
- [x] Phase 17: Telegram Batch Ack (1/1 plan) — Ack reaction targets only last message in debounced batch
- [x] Phase 18: Example Validation & Flow Tests (2/2 plans) — Config validation, docker-compose checks, self-contained test scripts

</details>

<details>
<summary>✅ Testing & Runtime Config (Phases 19-22) — SHIPPED 2026-04-05</summary>

- [x] Phase 19: Test Infrastructure (3/3 plans) — SSE helper extraction, test-log integration, test suite reorganization
- [x] Phase 20: E2E Test Coverage (2/2 plans) — Multi-agent, thinking, crash recovery, shutdown, health E2E tests
- [x] Phase 21: Runtime Config Loading (1/1 plan) — Config path E2E test, Dockerfile verification, README docs
- [x] Phase 22: SDK Integration Tests (2/2 plans) — sdk-test-channel, sdk-test-tool, E2E integration tests

</details>

<details>
<summary>✅ Agent Containers & First-Party Opencode (Phases 23-35) — SHIPPED 2026-04-08</summary>

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
<summary>✅ System Refactoring & Code Health (Phases 36-44) — SHIPPED 2026-04-09</summary>

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
<summary>✅ Tech Debt & Hardening (Phases 45-55) — SHIPPED 2026-04-10</summary>

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

<details>
<summary>✅ Open Source Launch (Phases 56-63) — SHIPPED 2026-04-11</summary>

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

</details>

<details>
<summary>✅ Tech Debt & Optimization (Phases 64-71) — SHIPPED 2026-04-11</summary>

- [x] Phase 64: CI Hardening (CI-01, CI-02, CI-03)
- [x] Phase 65: Error Handling & Safety (ERR-01, ERR-02)
- [x] Phase 66: Dependency Modernization (DEPS-01, DEPS-02, DEPS-03)
- [x] Phase 67: Config Cleanup (CFG-01, CFG-02)
- [x] Phase 68: Architecture Deduplication (ARCH-01, ARCH-02, ARCH-03)
- [x] Phase 69: Code Quality (QUAL-01, QUAL-02, QUAL-03, QUAL-04)
- [x] Phase 70: Test Coverage (TEST-01, TEST-02, TEST-03)
- [x] Phase 71: Documentation Lints (DOCS-01, DOCS-02)

</details>

<details>
<summary>✅ Docker Image Optimization (Phases 72-76) — SHIPPED 2026-04-12</summary>

- [x] Phase 72: Core Image Optimization (IMG-01, IMG-02, IMG-03, IMG-04) — **Plans:** 1 plan (completed 2026-04-11)
  Plans:
  - [x] 72-01-PLAN.md — Strip+LTO release profile, distroless core image, protoclaw-only
- [x] Phase 73: Extension Path Normalization (EXT-01) — **Plans:** 1 plan (completed 2026-04-12)
  Plans:
  - [x] 73-01-PLAN.md — Categorized @built-in/ paths with legacy alias support
- [x] Phase 74: Dockerfile Restructure & Builder Image (EXT-02, EXT-03, EXT-04) — **Plans:** 1 plan (completed 2026-04-12)
  Plans:
  - [x] 74-01-PLAN.md — Builder-export stage, CI workflow, example Dockerfile updates
- [x] Phase 75: Extension Image Optimization (OPT-01, OPT-02, OPT-03) — **Plans:** 1 plan (completed 2026-04-12)
  Plans:
  - [x] 75-01-PLAN.md — Multi-stage npm install with --omit=dev and node_modules pruning
- [x] Phase 76: ghcr.io Lifecycle Management (GHCR-01, GHCR-02, GHCR-03, GHCR-04, GHCR-05) — **Plans:** 1 plan (completed 2026-04-12)
  Plans:
  - [x] 76-01-PLAN.md — Trivy scanning, multi-arch verification, retention workflow, tagging docs

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

</details>

</details>

<details>
<summary>✅ v0.3.0 Production Readiness & Protocol Stabilization (Phases 77-83) — SHIPPED 2026-04-12</summary>

- [ ] Phase 77: CI Docker Build Optimization (deferred — addressed ad-hoc via CI commits)
- [x] Phase 78: WASM Sandbox Wiring (1/1 plans) — Enforce memory limits and filesystem access in wasmtime Store
- [x] Phase 79: Config Wiring & Supervisor Hardening (1/1 plans) — ToolConfig.options env wiring + circuit breaker tracing
- [x] Phase 80: Agent Capabilities & Cancel (1/1 plans) — Capability-gated fork/list, AvailableCommandsUpdate, per-session cancel
- [x] Phase 81: Security Hardening (1/1 plans) — Path sandboxing for fs operations, debug-http bearer token auth
- [x] Phase 82: Production Observability (1/1 plans) — Admin HTTP server with /health and /metrics, audit logging
- [x] Phase 83: ACP Protocol Stabilization (1/1 plans) — ACP version negotiation, extension types, SDK #[non_exhaustive]

### Phase 77: CI Docker Build Optimization
**Goal**: GitHub Actions Docker builds are significantly faster through better caching or parallelism
**Depends on**: Nothing
**Requirements**: CI-01
**Status**: Deferred — partially addressed by ad-hoc CI commits (rust-cache, cargo-nextest, cargo-chef)
**Success Criteria** (what must be TRUE):
  1. Docker build step in CI completes at least 30% faster than current baseline (measure before/after)
  2. Cache hit rate on dependency layers is above 80% for non-Cargo.lock-changing PRs
  3. No regression in image correctness — existing Docker integration tests pass

### Phase 78: WASM Sandbox Wiring
**Goal**: WASM tools run within configured resource and filesystem boundaries
**Depends on**: Nothing (independent)
**Requirements**: CFG-01, CFG-02
**Success Criteria** (what must be TRUE):
  1. A WASM tool configured with `memory_limit_bytes: 16777216` is killed when it tries to allocate beyond 16MB — wasmtime Store enforces the limit
  2. A WASM tool configured with `preopened_dirs: ["/tmp/sandbox"]` can read/write files under that path but cannot access `/etc/passwd` or other paths outside the allowlist
  3. A WASM tool with no `preopened_dirs` configured has zero filesystem access — any file operation returns an error
  4. Existing WASM E2E integration test still passes with default config (backward compatible)

### Phase 79: Config Wiring & Supervisor Hardening
**Goal**: Tool options flow through to MCP/WASM processes and supervisor enforces restart limits
**Depends on**: Nothing (independent)
**Requirements**: CFG-03, CFG-04
**Success Criteria** (what must be TRUE):
  1. `ToolConfig.options` map entries appear in the MCP server subprocess environment or initialize params — an MCP server can read a configured option value
  2. `ToolConfig.options` map entries are passed as input to WASM tool invocations — a WASM tool can read a configured option value
  3. When a manager crashes more than `max_restarts` times within `restart_window_secs`, the supervisor stops restarting it and logs an escalation event
  4. Default behavior (no max_restarts configured) preserves current unlimited restart behavior

### Phase 80: Agent Capabilities & Cancel
**Goal**: Agents that advertise fork/list capabilities get those features dispatched, commands registration flows through to channels, and users can cancel in-progress responses
**Depends on**: Nothing (independent)
**Requirements**: CFG-05, CFG-06, SEC-03
**Success Criteria** (what must be TRUE):
  1. When an agent's initialize response includes `capabilities.session.fork`, the supervisor dispatches fork requests to that agent
  2. When an agent's initialize response includes `capabilities.session.list`, the supervisor dispatches list requests to that agent
  3. Agent `AvailableCommandsUpdate` notifications are deserialized into typed `AcpCommand { name, description }` structs and forwarded to channels via `ContentKind::AvailableCommandsUpdate`
  4. Telegram channel calls `setMyCommands` when it receives an `AvailableCommandsUpdate` — commands appear in Telegram's `/` menu
  5. A channel can send a cancel command that terminates an in-progress agent response — the agent receives `session/cancel` and stops streaming
  6. Cancel from one channel session does not affect other sessions

### Phase 81: Security Hardening
**Goal**: File system operations are sandboxed and debug-http requires authentication
**Depends on**: Nothing (independent)
**Requirements**: SEC-01, SEC-02
**Success Criteria** (what must be TRUE):
  1. `fs/read_text_file` with a path outside the agent's working directory returns a permission error — no file content is leaked
  2. `fs/write_text_file` with a path containing `../` traversal above the working directory is rejected
  3. A configurable allowlist can grant access to additional directories beyond the working directory
  4. HTTP requests to debug-http without a valid API key or bearer token receive 401 Unauthorized — no message is processed
  5. HTTP requests to debug-http with a valid token are processed normally

### Phase 82: Production Observability
**Goal**: Operators can monitor protoclaw health and audit tool usage via standard interfaces
**Depends on**: Nothing (independent)
**Requirements**: OBS-01, OBS-02, OBS-03
**Success Criteria** (what must be TRUE):
  1. `GET /metrics` returns Prometheus-format text with request counts, error rates, and agent/channel health gauges
  2. `GET /health` returns JSON with component-level status (supervisor, each agent, each channel, tools manager) — compatible with load balancer health checks
  3. Every tool invocation emits a structured tracing event containing agent name, tool name, input summary, output summary, duration, and success/failure status
  4. Health endpoint returns appropriate HTTP status codes — 200 when healthy, 503 when degraded

### Phase 83: ACP Protocol Stabilization
**Goal**: ACP protocol is version-negotiated, schema is clean of agent-specific types, and SDK APIs are reviewed for semver stability
**Depends on**: Phase 80 (capabilities work informs protocol surface)
**Requirements**: PROTO-01, PROTO-02, PROTO-03
**Success Criteria** (what must be TRUE):
  1. ACP initialize handshake includes protocol version negotiation — client and server agree on a version, replacing the hardcoded `protocol_version: 1`
  2. `SessionUpdateType` variants specific to opencode (e.g., opencode-specific update kinds) are moved to an extension enum or removed from the shared `acp_types.rs`
  3. SDK public API audit completed — each SDK crate's public types, traits, and functions reviewed for semver compatibility, with breaking changes executed or documented
  4. A mock agent advertising protocol version 2 can negotiate down to version 1 with the current supervisor

</details>

### 🚧 v0.3.2 DX & Permission Hardening

**Milestone Goal:** Close developer experience gaps, add permission E2E test, ensure dev tooling doesn't leak into production.

- [x] **Phase 86: Dev Iteration Infrastructure** (1 plan) — ITER-01, ITER-02 (completed 2026-04-13)
  - Depends on: Nothing
  Plans:
  - [x] 86-01-PLAN.md — Debug profile for dev.sh + Dockerfile.dev-builder
- [x] **Phase 87: Agent Error Visibility & Logging** (3 plans) — LOG-03, VIS-02, VIS-03 (completed 2026-04-13)
  - Depends on: Nothing
  Plans:
  - [x] 87-01-PLAN.md — Docker container name visibility + bollard stdin bridge warn logging
  - [x] 87-02-PLAN.md — Permission response timeout with auto-deny and warn logging
  - [x] 87-03-PLAN.md — Gap closure: fix permission timeout in production route_permission_event() path
- [ ] **Phase 88: Permission E2E Integration Test** (2 plans) — TEST-01, TEST-02
  - Depends on: Nothing (mock-agent already supports permission commands; tests are additive)
- [ ] **Phase 89: Dev/Prod Separation & Documentation** (3 plans) — SEP-02, SEP-03, SEP-04, DOC-01, DOC-02
  - Depends on: Phase 86 (dev.sh must be finalized before documenting it)

### 📋 v0.3.3 Session Persistence & Recovery

**Milestone Goal:** Sessions survive agent crashes and protoclaw restarts — users never see "connection closed" errors.

- [ ] **Phase 90: SessionStore Trait & Config** — STORE-01, STORE-02, STORE-04, STORE-05, CFG-01, CFG-02, CFG-03
  - Depends on: Nothing (foundation layer, no runtime integration)
- [ ] **Phase 91: SQLite Implementation** — STORE-03
  - Depends on: Phase 90 (implements the trait defined there)
- [ ] **Phase 92: Agent Crash Recovery & Session Loading** — RECV-01, RECV-02, RECV-03, RECV-04, RECV-05
  - Depends on: Phase 90 (uses SessionStore trait and stale_sessions concept)
- [ ] **Phase 93: Persistence Lifecycle Wiring** (2 plans) — LIFE-01, LIFE-02, LIFE-03, LIFE-04
  - Depends on: Phase 91, Phase 92 (wires SQLite store into agents manager recovery paths)
  Plans:
  - [ ] 93-01-PLAN.md — Supervisor store construction + boot cleanup + shutdown persistence
  - [ ] 93-02-PLAN.md — Persist session on create + update last_active on prompt
- [ ] **Phase 94: Error Delivery & Audit** — ERR-01, ERR-02
  - Depends on: Phase 92 (error delivery triggers on recovery failure paths)

<details>
<summary>✅ v0.3.1 Config Schema Modernization & Example 02 Simplification — SHIPPED 2026-04-13</summary>

**Milestone Goal:** Replace `AgentConfig.args` with `StringOrArray` on binary/entrypoint fields, relocate ACP wire types to SDK, remove `opencode-wrapper`, simplify Example 02 for direct ACP spawn.

- [x] **Phase 84: ACP Type Relocation + opencode-wrapper Removal** (completed 2026-04-13) — PROTO-04, PROTO-05, PROTO-06, MIG-01, MIG-04
  - ACP wire types relocated to protoclaw-sdk-types/acp.rs
  - Legacy config aliases updated (opencode → agents/acp-bridge)
  - opencode-wrapper removed from workspace
  - mock-agent sends available_commands_update after initialize
- [x] **Phase 85: Config Schema + Example 02 Update** (2 plans, completed 2026-04-13) — CFG-05, CFG-06, CFG-07, CFG-08, MIG-03, EXMP-01–05
  Plans:
  - [x] 85-01-PLAN.md — StringOrArray type, args removal, spawn logic update
  - [x] 85-02-PLAN.md — Example 02 Dockerfile/config/test/README simplification

</details>

## Phase Details

### Phase 84: ACP Type Relocation + opencode-wrapper Removal
**Goal**: Relocate ACP wire types to SDK crate, remove opencode-wrapper, update legacy aliases
**Depends on**: Nothing
**Requirements**: PROTO-04, PROTO-05, PROTO-06, MIG-01, MIG-04
**Success Criteria:**
1. ACP wire types canonical in `protoclaw-sdk-types/src/acp.rs`
2. `protoclaw-agents/acp_types.rs` re-exports for backward compat
3. `ext/agents/opencode-wrapper/` removed from workspace
4. mock-agent sends `available_commands_update` after initialize

### Phase 85: Config Schema + Example 02 Update
**Goal**: Replace `AgentConfig.args` with `StringOrArray`, simplify Example 02 for direct ACP spawn
**Depends on**: Phase 84
**Requirements**: CFG-05, CFG-06, CFG-07, CFG-08, MIG-03, EXMP-01–05
**Success Criteria:**
1. `StringOrArray` type with custom serde, Display, From impls
2. `AgentConfig.args` field removed (breaking change)
3. `connection.rs` and `docker_backend.rs` use `command_and_args()` API
4. Example 02 Dockerfile has 3 stages, ENTRYPOINT `["opencode", "acp"]`
5. `cargo build --workspace` + `cargo test` + `cargo clippy` all pass

### Phase 86: Dev Iteration Infrastructure
**Goal**: dev.sh uses debug profile for fast incremental rebuilds and persists cargo artifacts across restarts
**Depends on**: Nothing
**Requirements**: ITER-01, ITER-02
**Success Criteria:**
1. `dev.sh rebuild` invokes `cargo build` with `--profile dev` (not release) — confirmed by `docker inspect` or compose env showing no `--release` flag
2. `CARGO_TARGET_DIR` is set in the dev container environment and points to a named Docker volume — `docker volume ls` shows the volume; consecutive rebuilds skip unchanged crates
3. Two consecutive `dev.sh rebuild` runs after a single-file edit complete in under 30 seconds (measured wall-clock inside container)
4. Production `docker compose up` (without dev override) does not mount the target volume — verified by absence of volume mount in default compose.yaml

### Phase 87: Agent Error Visibility & Logging
**Goal**: Agent container name is discoverable at spawn and bollard/permission failures are surfaced in logs
**Depends on**: Nothing
**Requirements**: LOG-03, VIS-02, VIS-03
**Success Criteria:**
1. When a Docker-workspace agent starts, a `tracing::info!` event with the container name appears in the supervisor log before the first ACP message is sent
2. A bollard stdin write or flush failure on the agent's stdin bridge emits `tracing::warn!` with the error and context — verified by unit test injecting a broken writer
3. When the supervisor routes `channel/requestPermission` and the agent produces no `session/request_permission` response within the configured timeout, a `tracing::warn!` with the request ID and elapsed duration is emitted — verified by integration test using mock-agent with a deliberate response delay

### Phase 88: Permission E2E Integration Test
**Goal**: Full permission round-trip (mock agent request → supervisor routes → mock channel responds → ACP response back to agent) is covered by an automated integration test
**Depends on**: Nothing
**Requirements**: TEST-01, TEST-02
**Success Criteria:**
1. `cargo test -p integration` includes a test `when_agent_requests_permission_then_response_reaches_agent_stdin` that passes green on CI
2. Test asserts the ACP wire format of the permission response sent to agent stdin: JSON object with `result.outcome` (string `"allow"` or `"deny"`) and `result.optionId` (string) — asserted via `serde_json::Value` inspection
3. Test verifies the supervisor correctly routes the permission request from the agent through the channels manager and back — no mocking of internal manager communication
4. Test runs without real Telegram or external network access — uses mock-agent + debug-http channel only

### Phase 89: Dev/Prod Separation & Documentation
**Goal**: Dev tooling is clearly documented as contributor-only, production image is free of dev artifacts, and AGENTS.md reflects new v0.3.2 patterns
**Depends on**: Phase 86 (dev.sh finalized before documenting)
**Requirements**: SEP-02, SEP-03, SEP-04, DOC-01, DOC-02
**Success Criteria:**
1. Example 02 README contains a "Development" section explaining `dev.sh` and `Dockerfile.dev-builder` as contributor tools — not part of production quickstart
2. The "Getting Started" / production quickstart section in Example 02 README references only `docker compose up` (not `dev.sh`)
3. `docker compose up` in Example 02 without any dev override produces images with no cargo target volume mounts and no `cargo-watch` or dev-only binaries present
4. Root `AGENTS.md` (or `crates/protoclaw-agents/AGENTS.md`) documents: ACP permission wire format (`result.outcome`/`result.optionId`), `send_raw` bypass pattern, bollard stdin flush behavior, and `protoclaw-test-helpers` `test_support` module
5. `dev.sh` file (or its header comment) and the Example 02 README both contain an explicit "contributor-only" notice distinguishing it from the production workflow

### Phase 90: SessionStore Trait & Config
**Goal**: A pluggable session persistence interface exists with config-driven backend selection and a no-op default
**Depends on**: Nothing (foundation layer, no runtime integration)
**Requirements**: STORE-01, STORE-02, STORE-04, STORE-05, CFG-01, CFG-02, CFG-03
**Success Criteria** (what must be TRUE):
  1. `SessionStore` trait compiles with async methods: `load_open_sessions`, `upsert_session`, `mark_closed`, `update_last_active`, `delete_expired`
  2. `PersistedSession` struct holds `session_key`, `agent_name`, `acp_session_id`, `created_at`, `last_active_at`, and `closed` flag — round-trips through serde
  3. `NoopSessionStore` implements `SessionStore` and all methods return success without side effects — existing behavior unchanged when no store configured
  4. `SessionStoreConfig` enum in protoclaw-config accepts `type: sqlite` (with optional `path` and `ttl_days`) and `type: none` (default) via serde tagged enum
  5. `cargo build --workspace` and `cargo test` pass with the new types and config — no runtime wiring yet
**Plans**: TBD

### Phase 91: SQLite Implementation
**Goal**: Sessions can be persisted to and loaded from a SQLite database file
**Depends on**: Phase 90 (implements the SessionStore trait)
**Requirements**: STORE-03
**Success Criteria** (what must be TRUE):
  1. `SqliteSessionStore` implements `SessionStore` using rusqlite with the `bundled` feature — no system SQLite dependency
  2. `upsert_session` + `load_open_sessions` round-trip: insert a session, load it back, fields match
  3. `delete_expired` removes sessions older than the configured TTL — verified by inserting an old session and confirming it's gone after cleanup
  4. Schema is created automatically via `CREATE TABLE IF NOT EXISTS` on store construction — no manual migration step
**Plans**: TBD

### Phase 92: Agent Crash Recovery & Session Loading
**Goal**: Agent crashes preserve session state and prompt_session() self-heals instead of returning ConnectionClosed
**Depends on**: Phase 90 (uses SessionStore trait and PersistedSession)
**Requirements**: RECV-01, RECV-02, RECV-03, RECV-04, RECV-05
**Success Criteria** (what must be TRUE):
  1. When an agent crashes, its session_map entries are drained into `stale_sessions` on the AgentSlot — not discarded
  2. When `prompt_session()` encounters a missing session_key, it attempts `load_session()` from stale_sessions (if agent supports it), then falls back to `create_session()` — user's next message recovers transparently
  3. On boot, open sessions from the store (closed=false) are loaded into `stale_sessions` on matching AgentSlots — sessions survive protoclaw restarts
  4. `prompt_session()` signature is `&mut self` — the handle_command() call site compiles with the new borrow pattern
  5. A unit test confirms the load → create fallback chain: mock agent rejects load → create succeeds → session is usable
**Plans**: TBD

### Phase 93: Persistence Lifecycle Wiring
**Goal**: Session state flows through the store at every lifecycle boundary — create, prompt, shutdown, and boot cleanup
**Depends on**: Phase 91, Phase 92 (wires SQLite store into agents manager recovery paths)
**Requirements**: LIFE-01, LIFE-02, LIFE-03, LIFE-04
**Success Criteria** (what must be TRUE):
  1. After `create_session()` succeeds, the session appears in the store via `upsert_session(closed=false)` — verified by querying the SQLite file
  2. After `prompt_session()` succeeds, `last_active_at` is updated in the store — verified by timestamp comparison
  3. During graceful shutdown (`shutdown_all`), every active session is marked `closed=true` in the store before `session/close` is sent to the agent
  4. At boot, `delete_expired()` removes sessions older than `ttl_days` — stale data doesn't accumulate across restarts
  5. Supervisor creates the store from config and passes `Arc<dyn SessionStore>` to the agents manager — no store construction inside the manager
**Plans:** 2 plans
Plans:
- [ ] 93-01-PLAN.md — Supervisor store construction + boot cleanup + shutdown persistence
- [ ] 93-02-PLAN.md — Persist session on create + update last_active on prompt

### Phase 94: Error Delivery & Audit
**Goal**: Users see meaningful error messages when session recovery fails, and operators can trace recovery attempts
**Depends on**: Phase 92 (error delivery triggers on recovery failure paths)
**Requirements**: ERR-01, ERR-02
**Success Criteria** (what must be TRUE):
  1. When `prompt_session()` fails after all recovery attempts (load + create both fail), the channels manager delivers an error message to the originating channel — user sees "session recovery failed" (or similar) instead of silence
  2. Each recovery attempt emits a structured `tracing::info!` event: recovery started, load attempted (success/failure), create attempted (success/failure), final outcome
  3. A unit test confirms error delivery: mock agent rejects both load and create → channel receives error content
**Plans**: TBD

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Foundation | pre-release | 4/4 | Complete | 2026-04-02 |
| 2. Agent Connection | pre-release | 5/5 | Complete | 2026-04-02 |
| 3. Channel Routing | pre-release | 3/3 | Complete | 2026-04-02 |
| 4. Developer Experience | pre-release | 3/3 | Complete | 2026-04-02 |
| 5. SDK Crates | pre-release | 5/5 | Complete | 2026-04-02 |
| 6. Tools Infrastructure | pre-release | 3/3 | Complete | 2026-04-03 |
| 7. Telegram Channel | pre-release | 3/3 | Complete | 2026-04-03 |
| 8. Examples & Onboarding | pre-release | 3/3 | Complete | 2026-04-03 |
| 9. Infrastructure Restructure | pre-release | 5/5 | Complete | 2026-04-03 |
| 10. Fake Agent Example | pre-release | 3/3 | Complete | 2026-04-03 |
| 11. Real Agent Examples & Cleanup | pre-release | 4/4 | Complete | 2026-04-03 |
| 12. Message Debouncing, Ack Reactions & Config Defaults | pre-release | 4/4 | Complete | 2026-04-04 |
| 13. Test Foundation & Documentation | pre-release | 3/3 | Complete | 2026-04-04 |
| 14. Config Format Migration | pre-release | 2/2 | Complete | 2026-04-04 |
| 15. Config-Driven Constants | pre-release | 2/2 | Complete | 2026-04-04 |
| 16. Structured Logging | pre-release | 2/2 | Complete | 2026-04-04 |
| 17. Telegram Batch Ack | pre-release | 1/1 | Complete | 2026-04-04 |
| 18. Example Validation & Flow Tests | pre-release | 2/2 | Complete | 2026-04-04 |
| 19. Test Infrastructure | pre-release | 3/3 | Complete | 2026-04-04 |
| 20. E2E Test Coverage | pre-release | 2/2 | Complete | 2026-04-04 |
| 21. Runtime Config Loading | pre-release | 1/1 | Complete | 2026-04-05 |
| 22. SDK Integration Tests | pre-release | 2/2 | Complete | 2026-04-05 |
| 23. Config Schema Extension | pre-release | 1/1 | Complete | 2026-04-05 |
| 24.1. Thinking Display Fix & Message Batching | pre-release | 1/1 | Complete | 2026-04-05 |
| 24. ConnectionBackend Refactor | pre-release | 2/2 | Complete | 2026-04-06 |
| 24.2. BDD Test Coverage & Bug Fixes | pre-release | 7/7 | Complete | 2026-04-06 |
| 25. Docker Module & Stream Bridging | pre-release | 2/2 | Complete | 2026-04-06 |
| 26. Opencode Wrapper Binary | pre-release | 1/1 | Complete | 2026-04-07 |
| 27. Docker Example | pre-release | 2/2 | Complete | 2026-04-07 |
| 28. Docker Integration Tests | pre-release | 2/2 | Complete | 2026-04-07 |
| 29. E2E Session & Batch Flows | pre-release | 2/2 | Complete | 2026-04-07 |
| 30. E2E Tool Invocation & Payload Handling | pre-release | 2/2 | Complete | 2026-04-07 |
| 31. E2E Resilience & Recovery | pre-release | 1/1 | Complete | 2026-04-07 |
| 32. Telegram ChatTurn State Machine | pre-release | 1/1 | Complete | 2026-04-08 |
| 33. Channel SDK DX Improvements | pre-release | 3/3 | Complete | 2026-04-08 |
| 34. Milestone Bookkeeping & Requirements Sync | pre-release | 1/1 | Complete | 2026-04-08 |
| 35. Example 02 End-to-End Fix | pre-release | 1/1 | Complete | 2026-04-08 |
| 36. Crate Boundary Foundations | pre-release | 2/2 | Complete | 2026-04-08 |
| 37. ChannelEvent Relocation & Typed Config Conversion | pre-release | 2/2 | Complete | 2026-04-09 |
| 38. Modularity Cleanup | pre-release | 2/2 | Complete | 2026-04-09 |
| 39. Code Smell Elimination | pre-release | 1/1 | Complete | 2026-04-09 |
| 40. Docker Build Efficiency | pre-release | 1/1 | Complete | 2026-04-09 |
| 41. Config-Driven Operation | pre-release | 1/1 | Complete | 2026-04-09 |
| 42. Documentation & AGENTS.md Hygiene | pre-release | 0/0 | Complete | 2026-04-09 |
| 43. Code Smell & Modularity Fixes (Gap Closure) | pre-release | 1/1 | Complete    | 2026-04-09 |
| 44. Docker Build Context Isolation (Gap Closure) | pre-release | 2/2 | Complete    | 2026-04-09 |
| 45. Config Normalization & Validation | pre-release | 2/2 | Complete | 2026-04-10 |
| 46. Config Completeness | pre-release | 2/2 | Complete | 2026-04-10 |
| 47. serde_yaml Replacement | pre-release | 1/1 | Complete | 2026-04-10 |
| 48. Error Handling & Resilience | pre-release | 5/5 | Complete | 2026-04-10 |
| 49. Production Safety & Observability | pre-release | 3/3 | Complete | 2026-04-10 |
| 50. Test Coverage Gaps | pre-release | 4/5 | Complete | 2026-04-10 |
| 51. Hygiene & Documentation | pre-release | 2/3 | Complete | 2026-04-10 |
| 52. Config Gap Closure | pre-release | 2/2 | Complete | 2026-04-10 |
| 53. Error Handling Gap Closure | pre-release | 3/3 | Complete | 2026-04-10 |
| 54. Observability, Test & Docs Gap Closure | pre-release | 2/2 | Complete | 2026-04-10 |
| 55. Final Gap Closure (TEST-04, HYG-03) | pre-release | 2/2 | Complete | 2026-04-10 |
| 56. Hygiene & Pre-flight | pre-release | 0/— | Complete | 2026-04-11 |
| 57. Governance Files | pre-release | 0/— | Complete | 2026-04-11 |
| 58. Cargo.toml Metadata & SDK Packaging | pre-release | 0/— | Complete | 2026-04-11 |
| 59. Documentation | pre-release | 0/— | Complete | 2026-04-11 |
| 60. GitHub Templates & Settings | pre-release | 0/— | Complete | 2026-04-11 |
| 61. CI/CD — Release Automation | pre-release | 0/— | Complete | 2026-04-11 |
| 62. CI/CD — Container Images | pre-release | 0/— | Complete | 2026-04-11 |
| 63. First Release & Verification | pre-release | 0/— | Complete | 2026-04-11 |
| 64. CI Hardening | pre-release | 0/— | Complete | 2026-04-11 |
| 65. Error Handling & Safety | pre-release | 0/— | Complete | 2026-04-11 |
| 66. Dependency Modernization | pre-release | 0/— | Complete | 2026-04-11 |
| 67. Config Cleanup | pre-release | 0/— | Complete | 2026-04-11 |
| 68. Architecture Deduplication | pre-release | 0/— | Complete | 2026-04-11 |
| 69. Code Quality | pre-release | 0/— | Complete | 2026-04-11 |
| 70. Test Coverage | pre-release | 0/— | Complete | 2026-04-11 |
| 71. Documentation Lints | pre-release | 0/— | Complete | 2026-04-11 |
| 72. Core Image Optimization | pre-release | 1/1 | Complete    | 2026-04-11 |
| 73. Extension Path Normalization | pre-release | 1/1 | Complete    | 2026-04-12 |
| 74. Dockerfile Restructure & Builder Image | pre-release | 1/1 | Complete    | 2026-04-12 |
| 75. Extension Image Optimization | pre-release | 1/1 | Complete    | 2026-04-12 |
| 76. ghcr.io Lifecycle Management | pre-release | 1/1 | Complete    | 2026-04-12 |
| 77. CI Docker Build Optimization | v0.3.0 | 0/— | Not started | - |
| 78. WASM Sandbox Wiring | v0.3.0 | 1/1 | Complete    | 2026-04-12 |
| 79. Config Wiring & Supervisor Hardening | v0.3.0 | 1/1 | Complete    | 2026-04-12 |
| 80. Agent Capabilities & Cancel | v0.3.0 | 1/1 | Complete    | 2026-04-12 |
| 81. Security Hardening | v0.3.0 | 1/1 | Complete    | 2026-04-12 |
| 82. Production Observability | v0.3.0 | 1/1 | Complete    | 2026-04-12 |
| 83. ACP Protocol Stabilization | v0.3.0 | 1/1 | Complete    | 2026-04-12 |
| 84. ACP Type Relocation + opencode-wrapper Removal | v0.3.1 | 0/— | Complete    | 2026-04-13 |
| 85. Config Schema + Example 02 Update | v0.3.1 | 2/2 | Complete    | 2026-04-13 |
| 86. Dev Iteration Infrastructure | v0.3.2 | 1/1 | Complete    | 2026-04-13 |
| 87. Agent Error Visibility & Logging | v0.3.2 | 3/3 | Complete    | 2026-04-13 |
| 88. Permission E2E Integration Test | v0.3.2 | 0/— | Not started | - |
| 89. Dev/Prod Separation & Documentation | v0.3.2 | 0/— | Not started | - |
| 90. SessionStore Trait & Config | v0.3.3 | 0/— | Not started | - |
| 91. SQLite Implementation | v0.3.3 | 0/— | Not started | - |
| 92. Agent Crash Recovery & Session Loading | v0.3.3 | 0/— | Not started | - |
| 93. Persistence Lifecycle Wiring | v0.3.3 | 0/— | Not started | - |
| 94. Error Delivery & Audit | v0.3.3 | 0/— | Not started | - |
