# Anyclaw

## What This Is

Infrastructure sidecar connecting AI agents to messaging channels and tools. Manages agent subprocess lifecycle, routes messages via ACP protocol (JSON-RPC 2.0 over stdio), and provides tool access through MCP servers and WASM sandboxes. Fully config-driven with JSON Schema validation, IDE autocomplete, and per-extension defaults. 12 core crates, external binaries, and examples.

## Core Value

Every line of code should be there for a reason, with typed data flowing through typed interfaces — no `serde_json::Value` soup, no bare unwraps, no inconsistent patterns across crates.

## Current State

**Shipped:** v1.0.0 Config-Driven Architecture (2026-04-16)

The config system is now fully driven by `defaults.yaml` with JSON Schema validation. IDEs autocomplete `anyclaw.yaml` via yaml-language-server. Extensions can ship sidecar `defaults.yaml` files. `anyclaw validate` checks schema + unknown keys with `--strict` mode for CI.

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
- ✓ Legacy serde aliases removed from AnyclawConfig — v1.0.0 Phase 7
- ✓ Defaults.yaml covers all fixed-path fields, drift/completeness tests — v1.0.0 Phase 8
- ✓ Init template omits supervisor section (defaults apply automatically) — v1.0.0 Phase 8
- ✓ JSON Schema generation via schemars on all 21 config types — v1.0.0 Phase 9
- ✓ Manual JsonSchema impls for StringOrArray and PullPolicy — v1.0.0 Phase 9
- ✓ Committed anyclaw.schema.json with drift test — v1.0.0 Phase 9
- ✓ `anyclaw schema` CLI subcommand — v1.0.0 Phase 9
- ✓ `anyclaw validate` enhanced with JSON Schema validation — v1.0.0 Phase 9
- ✓ Schema drift test and example validation in CI — v1.0.0 Phase 10
- ✓ YAML modeline in `anyclaw init` for IDE schema association — v1.0.0 Phase 10
- ✓ Unknown key warnings with `--strict` flag on validate — v1.0.0 Phase 10
- ✓ Per-extension sidecar defaults.yaml loading — v1.0.0 Phase 11

### Active

(None — next milestone not yet defined)

### Out of Scope

- Env var override layer (`ANYCLAW_` prefix) — documented but unimplemented, separate concern
- Feature-gating wasmtime/bollard (separate milestone)
- SchemaStore submission for global IDE support
- Nested unknown key detection (top-level only in v1.0.0)
- Extension defaults for Docker workspace types

## Context

Config system uses Figment with layered loading: embedded `defaults.yaml` → per-extension sidecar defaults → user `anyclaw.yaml`. All 21 config types derive `JsonSchema` (schemars 1.x). `anyclaw.schema.json` committed at repo root. `anyclaw validate` runs JSON Schema validation → unknown key detection → binary path checks.

Key files:
- `crates/anyclaw-config/src/types.rs` — all config types with JsonSchema derives
- `crates/anyclaw-config/src/lib.rs` — Figment loading + `generate_schema()`
- `crates/anyclaw-config/src/defaults.yaml` — complete embedded defaults
- `crates/anyclaw-config/src/extension_defaults.rs` — per-extension sidecar loading
- `crates/anyclaw-config/src/validate.rs` — schema validation + unknown key detection
- `crates/anyclaw/src/cli.rs` — CLI with `schema` and `validate --strict` subcommands
- `anyclaw.schema.json` — committed JSON Schema (Draft 2020-12)

## Constraints

- **No unsafe**: Zero unsafe blocks exist. Do not introduce any.
- **No mod.rs**: Flat lib.rs with pub mod + pub use.
- **Manager communication**: tokio::sync::mpsc via ManagerHandle only.
- **Boot order**: tools → agents → channels. Do not change MANAGER_ORDER.
- **Test framework**: rstest 0.23 with BDD naming.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Crate-by-crate approach (v0.x quality) | Keeps each PR reviewable | ✓ Good |
| Breaking changes allowed (v0.x quality) | Quality over backward compat | ✓ Good |
| Entire workspace in scope (v0.x quality) | Consistency requires touching everything | ✓ Good |
| Breaking config format OK (v1.0.0) | Clean over backward compatible | ✓ Good — removed 4 aliases cleanly |
| schemars 1.x for JSON Schema (v1.0.0) | De facto Rust JSON Schema crate, derives alongside serde | ✓ Good — reads serde attrs directly |
| Keep serde default fns as fallback (v1.0.0) | Removing would break 40+ tests that deserialize without Figment | ✓ Good — drift test catches divergence |
| Sidecar file for extension defaults (v1.0.0) | Simplest approach, no protocol changes, fits extensions_dir layout | ✓ Good |
| Unknown keys as warnings not errors (v1.0.0) | Forward-compatible; --strict for CI use | ✓ Good |
| jsonschema 0.28 for runtime validation (v1.0.0) | Validates user config against schema before binary checks | ✓ Good |

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
*Last updated: 2026-04-16 after v1.0.0 milestone completion*
