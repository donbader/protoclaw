# Roadmap: Anyclaw Code Quality

## Overview

A crate-by-crate quality pass across the anyclaw workspace. Starts with tooling enforcement (so every subsequent phase benefits from automated checks), works through leaf crates → managers → SDK following the dependency graph, then fills test gaps and decomposes oversized files last (when the code they cover is in its final shape). Bug fixes (BUGF-01, BUGF-02) are opportunistic — fixed in whichever phase discovers them.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Tooling & Lint Infrastructure** - Workspace lints, clippy.toml, rustfmt.toml, deny.toml, coverage setup, dead code removal (completed 2026-04-14)
- [x] **Phase 2: Leaf Crate Quality** - Typed JSON in sdk-types/jsonrpc/core, error enum audit, serde consistency (completed 2026-04-14)
- [ ] **Phase 3: Manager Crate Quality** - Typed JSON in agents/channels/tools, clone reduction, DashMap migration
- [ ] **Phase 4: SDK & External Polish** - Typed JSON in SDK + ext binaries, docs enforcement, inline limitation comments
- [ ] **Phase 5: Test Coverage & Verification** - Fill test gaps, coverage baseline, property-based testing for wire types
- [ ] **Phase 6: File Decomposition** - Break up agents manager (3,708 lines) and supervisor (927 lines)

## Phase Details

### Phase 1: Tooling & Lint Infrastructure
**Goal**: Automated quality enforcement exists across the entire workspace — no code change can regress lint, format, or dependency policy
**Depends on**: Nothing (first phase)
**Requirements**: TOOL-01, TOOL-02, TOOL-03, TOOL-04, TOOL-05, HYGN-01, HYGN-02, HYGN-03, BUGF-01, BUGF-02
**Success Criteria** (what must be TRUE):
  1. `cargo clippy --workspace` produces zero warnings with the new workspace lint config
  2. `cargo fmt --check` passes across all crates with the new rustfmt.toml
  3. `cargo deny check` validates advisories, bans, and sources (not just licenses)
  4. `cargo llvm-cov` runs successfully and produces a baseline coverage report
  5. No unused imports or stale modules remain anywhere in the workspace
**Plans:** 3/3 plans complete
Plans:
- [x] 01-01-PLAN.md — Lint config files (clippy.toml, rustfmt.toml, deny.toml, workspace lints) + propagation to all crates
- [x] 01-02-PLAN.md — Fix all clippy warnings, dead code removal, unused imports cleanup
- [x] 01-03-PLAN.md — Coverage baseline with cargo-llvm-cov

### Phase 2: Leaf Crate Quality
**Goal**: Foundation crates (sdk-types, jsonrpc, core) use typed structs everywhere, have consistent error enums, and follow serde conventions — so manager crates can build on solid types
**Depends on**: Phase 1
**Requirements**: JSON-01, JSON-02, JSON-03, ERRH-01, ERRH-02, ERRH-03, SERD-01, SERD-02, BUGF-01, BUGF-02
**Success Criteria** (what must be TRUE):
  1. Zero `serde_json::Value` usage remains in anyclaw-sdk-types, anyclaw-jsonrpc, and anyclaw-core
  2. Every library crate uses thiserror with a typed error enum — no anyhow in library code
  3. Zero bare `.unwrap()` calls exist in production code (all replaced with `.expect("reason")` or `?`)
  4. All SDK wire types use `#[serde(rename_all = "camelCase")]` and all config types use `snake_case`
**Plans:** 3/3 plans complete
Plans:
- [x] 02-01-PLAN.md — Type anyclaw-sdk-types: replace Value with typed structs, fix unwraps, serde consistency
- [x] 02-02-PLAN.md — Type anyclaw-jsonrpc: typed RequestId, typed codec, error audit
- [x] 02-03-PLAN.md — Type anyclaw-core + fix downstream compilation for workspace build

### Phase 3: Manager Crate Quality
**Goal**: The three manager crates (agents, channels, tools) use typed data throughout, have reduced clone overhead, and use lock-free concurrent maps — the heaviest crates are clean
**Depends on**: Phase 2
**Requirements**: JSON-04, JSON-05, JSON-06, CLON-01, CLON-02, CLON-03, ADVN-01, BUGF-01, BUGF-02
**Success Criteria** (what must be TRUE):
  1. Zero `serde_json::Value` usage remains in anyclaw-agents, anyclaw-channels, and anyclaw-tools
  2. Clone count in anyclaw-agents manager is measurably reduced from the 103-clone baseline
  3. Borrowing (`&str`, references) is used instead of ownership transfer where ownership isn't needed
  4. `Arc<Mutex<HashMap<u64, oneshot::Sender>>>` in connection crates is replaced with DashMap
**Plans:** 4 plans
Plans:
- [x] 03-01-PLAN.md — Type anyclaw-tools: Value replacement, clone reduction, add dashmap workspace dep
- [x] 03-02-PLAN.md — Type anyclaw-channels: DashMap migration, typed codec pipeline, clone reduction
- [x] 03-03-PLAN.md — Type anyclaw-agents support files: DashMap in connection.rs, typed platform_commands + slot
- [x] 03-04-PLAN.md — Type anyclaw-agents manager.rs: Value replacement, 107-clone audit, error audit

### Phase 4: SDK & External Polish
**Goal**: SDK crates and external binaries have typed JSON, complete doc coverage, and inline limitation comments — the public-facing surface is polished
**Depends on**: Phase 3
**Requirements**: JSON-07, JSON-08, SERD-03, DOCS-01, DOCS-02, DOCS-03, ADVN-03, BUGF-01, BUGF-02
**Success Criteria** (what must be TRUE):
  1. Zero `serde_json::Value` usage remains in SDK crates (sdk-agent, sdk-channel, sdk-tool) and ext/ binaries
  2. `#![warn(missing_docs)]` is enabled on all crates and produces zero warnings
  3. Round-trip serialization tests exist for all wire types
  4. Known limitations from AGENTS.md are documented as inline comments at the relevant code locations
**Plans**: TBD

### Phase 5: Test Coverage & Verification
**Goal**: Every identified test gap is filled, coverage baseline is established, and wire types have property-based tests — the codebase is verifiably correct
**Depends on**: Phase 4
**Requirements**: TEST-01, TEST-02, TEST-03, TEST-04, TEST-05, ADVN-02, BUGF-01, BUGF-02
**Success Criteria** (what must be TRUE):
  1. Tests exist for health.rs, sdk-agent error.rs, sdk-tool error.rs + lib.rs, agents acp_types.rs + backend.rs
  2. All new tests use rstest 0.23 with BDD naming (`when_action_then_result` / `given_when_then`)
  3. Property-based tests (proptest) exist for all ACP and MCP wire types
  4. Coverage report shows improvement over the Phase 1 baseline
**Plans**: TBD

### Phase 6: File Decomposition
**Goal**: Oversized files are broken into focused modules with clear boundaries — the codebase is navigable and each module has a single responsibility
**Depends on**: Phase 5
**Requirements**: DECO-01, DECO-02, DECO-03, BUGF-01, BUGF-02
**Success Criteria** (what must be TRUE):
  1. `anyclaw-agents/src/manager.rs` is decomposed into focused modules (fs_sandbox, session_recovery, tool_events, run loop) — no single file exceeds ~500 lines
  2. `anyclaw-supervisor/src/lib.rs` is decomposed into sub-modules (signal handling, shutdown orchestration, health monitoring)
  3. All extracted modules use `pub(crate)` boundaries — the public API surface of each crate is unchanged
  4. All existing tests pass without modification (decomposition is purely structural)
**Plans**: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5 → 6

**Cross-cutting:** BUGF-01 and BUGF-02 are opportunistic — bugs and code smells are fixed in whichever phase discovers them.

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Tooling & Lint Infrastructure | 3/3 | Complete    | 2026-04-14 |
| 2. Leaf Crate Quality | 3/3 | Complete    | 2026-04-14 |
| 3. Manager Crate Quality | 0/0 | Not started | - |
| 4. SDK & External Polish | 0/0 | Not started | - |
| 5. Test Coverage & Verification | 0/0 | Not started | - |
| 6. File Decomposition | 0/0 | Not started | - |
