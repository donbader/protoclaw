# Anyclaw

## What This Is

Infrastructure sidecar connecting AI agents to messaging channels and tools. Manages agent subprocess lifecycle, routes messages via ACP protocol (JSON-RPC 2.0 over stdio), and provides tool access through MCP servers and WASM sandboxes. 12 core crates, external binaries, and examples.

## Core Value

Every line of code should be there for a reason, with typed data flowing through typed interfaces — no `serde_json::Value` soup, no bare unwraps, no inconsistent patterns across crates.

## Current Milestone: v1.0.0 Config-Driven Architecture

**Goal:** Make anyclaw fully config-driven — all defaults externalized to YAML, JSON Schema for IDE validation, config structure clean and consistent.

**Target features:**
- JSON Schema generation (`schemars`) so IDEs autocomplete and validate `anyclaw.yaml`
- Complete `defaults.yaml` — move all `default_*` fns out of Rust code into YAML, eliminate the dual-default mechanism
- Per-extension defaults: each ext binary can ship its own `defaults.yaml` that layers into Figment
- Per-manager defaults: backoff, crash tracker, WASM sandbox defaults all live in `defaults.yaml`
- Config schema cleanup: consistent naming, flatten inconsistencies, clean structure
- Breaking changes to config format are acceptable

## Requirements

### Validated

- ✓ Three-manager architecture (tools → agents → channels) with supervisor orchestration — v0.x
- ✓ ACP protocol layer with JSON-RPC 2.0 over stdio — v0.x
- ✓ MCP host and WASM sandbox for tool execution — v0.x
- ✓ Channel subprocess routing (Telegram, debug-http) — v0.x
- ✓ Session persistence (SQLite + noop stores) — v0.x
- ✓ Figment-based config loading (defaults.yaml → anyclaw.yaml) — v0.x
- ✓ SDK crates for external channel/tool/agent implementors — v0.x
- ✓ Exponential backoff and crash loop detection — v0.x
- ✓ Filesystem sandboxing for agent operations — v0.x
- ✓ Integration test suite with mock-agent — v0.x
- ✓ Typed JSON across all crates (zero serde_json::Value soup) — code quality milestone
- ✓ Consistent error handling (thiserror in libs, anyhow at entry points) — code quality milestone
- ✓ Zero clippy warnings, dead code removed — code quality milestone
- ✓ Full test coverage with rstest + proptest — code quality milestone
- ✓ File decomposition (agents manager, supervisor) — code quality milestone
- ✓ Consistent serde patterns, doc comments on all public items — code quality milestone
- ✓ Legacy serde aliases removed from AnyclawConfig — Phase 7
- ✓ Defaults.yaml covers all fixed-path fields, drift/completeness tests — Phase 8

### Active

- [ ] JSON Schema generation for `anyclaw.yaml` via `schemars`
- [ ] All defaults externalized to `defaults.yaml` — no dual-default mechanism
- [ ] Per-extension/manager `defaults.yaml` files layered into Figment
- [ ] Config schema cleanup: consistent naming, structural consistency
- [ ] `constants.rs` `DEFAULT_*` consts consolidated with config defaults
- [ ] `init.rs` generated YAML template reads from defaults instead of hardcoding
- [ ] CI: schema drift check (committed schema matches schemars output)
- [ ] CI: example `anyclaw.yaml` files validated against schema

### Out of Scope

- New runtime features or capabilities — this is config architecture work
- Performance optimization
- Env var override layer (`ANYCLAW_` prefix) — documented but unimplemented, separate concern
- Feature-gating wasmtime/bollard (separate milestone)

## Context

The config system uses Figment with two layers: embedded `defaults.yaml` → user `anyclaw.yaml`. A `defaults.yaml` already exists at `crates/anyclaw-config/src/defaults.yaml` but only covers ~40% of actual defaults. The remaining defaults are hardcoded as 23 `#[serde(default = "fn")]` functions in `types.rs`. Additionally, `constants.rs` has `DEFAULT_*` consts that duplicate some config defaults.

Key files:
- `crates/anyclaw-config/src/types.rs` — all config types, 23 `default_*` fns
- `crates/anyclaw-config/src/lib.rs` — Figment loading (defaults.yaml → SubstYaml::file)
- `crates/anyclaw-config/src/defaults.yaml` — embedded defaults (incomplete)
- `crates/anyclaw-core/src/constants.rs` — `DEFAULT_*` consts duplicating config values
- `crates/anyclaw/src/init.rs` — generated YAML template with hardcoded supervisor values

## Constraints

- **No unsafe**: Zero unsafe blocks exist. Do not introduce any.
- **No mod.rs**: Flat lib.rs with pub mod + pub use.
- **Manager communication**: tokio::sync::mpsc via ManagerHandle only.
- **Boot order**: tools → agents → channels. Do not change MANAGER_ORDER.
- **Test framework**: rstest 0.23 with BDD naming.
- **Breaking config OK**: Config format changes are acceptable for this milestone.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Crate-by-crate approach (v0.x quality) | Keeps each PR reviewable | ✓ Good |
| Breaking changes allowed (v0.x quality) | Quality over backward compat | ✓ Good |
| Entire workspace in scope (v0.x quality) | Consistency requires touching everything | ✓ Good |
| Breaking config format OK (v1.0.0) | Clean over backward compatible | — Pending |
| schemars for JSON Schema | De facto Rust JSON Schema crate, derives alongside serde | — Pending |
| Single source of truth in defaults.yaml | Eliminate dual-default mechanism (YAML + serde fns) | — Pending |

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
*Last updated: 2026-04-15 after milestone v1.0.0 initialization*
