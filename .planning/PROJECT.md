# Anyclaw — Code Quality Milestone

## What This Is

A comprehensive code quality improvement pass across the entire anyclaw workspace — 12 core crates, external binaries, and examples. The goal is to make every crate feel intentional: typed JSON everywhere, consistent error handling, dead code removed, full test coverage, zero clippy warnings. Crate-by-crate, breaking changes allowed.

## Core Value

Every line of code should be there for a reason, with typed data flowing through typed interfaces — no `serde_json::Value` soup, no bare unwraps, no inconsistent patterns across crates.

## Requirements

### Validated

<!-- Existing capabilities inferred from codebase map -->

- ✓ Three-manager architecture (tools → agents → channels) with supervisor orchestration — existing
- ✓ ACP protocol layer with JSON-RPC 2.0 over stdio — existing
- ✓ MCP host and WASM sandbox for tool execution — existing
- ✓ Channel subprocess routing (Telegram, debug-http) — existing
- ✓ Session persistence (SQLite + noop stores) — existing
- ✓ Figment-based config loading with env var overlay — existing
- ✓ SDK crates for external channel/tool/agent implementors — existing
- ✓ Exponential backoff and crash loop detection — existing
- ✓ Filesystem sandboxing for agent operations — existing
- ✓ Integration test suite with mock-agent — existing

### Active

<!-- Quality improvements to make -->

- [ ] Replace all `serde_json::Value` manipulation with typed structs
- [ ] Consistent error handling: thiserror in libraries, anyhow at entry points only
- [ ] Remove dead code, unused imports, stale modules
- [ ] Zero clippy warnings across entire workspace
- [ ] Meaningful test coverage for every public function and type
- [ ] Break up oversized files (agents manager at 3,708 lines)
- [ ] Eliminate unnecessary `.clone()` calls
- [ ] Consistent serde patterns across all crates
- [ ] Clean up `Arc<Mutex<>>` patterns where alternatives exist
- [ ] Add missing doc comments on public items

### Out of Scope

- New features or capabilities — this is purely quality work
- Performance optimization beyond what falls out of cleanup naturally
- Dependency version upgrades (unless required by refactoring)
- CI/CD pipeline changes
- Feature-gating wasmtime/bollard (separate milestone)

## Context

Anyclaw is a Rust workspace (12 core crates + ext binaries + examples) acting as an infrastructure sidecar connecting AI agents to channels and tools. The codebase is functional but has accumulated inconsistencies: arbitrary JSON handling via `serde_json::Value`, a 3,708-line agents manager, 103 clone() calls in one file, files without test coverage, and patterns that vary across crates.

The codebase map (`.planning/codebase/`) documents current conventions, architecture, and known concerns in detail. Key reference files:
- `CONVENTIONS.md` — established patterns to enforce consistently
- `CONCERNS.md` — specific issues identified with file references and line numbers

## Constraints

- **No unsafe**: Zero unsafe blocks exist. Do not introduce any.
- **No mod.rs**: Flat lib.rs with pub mod + pub use. Convention must be maintained.
- **Manager communication**: tokio::sync::mpsc via ManagerHandle only. No shared mutable state across managers.
- **Boot order**: tools → agents → channels. Do not change MANAGER_ORDER.
- **Test framework**: rstest 0.23 with BDD naming. All new tests must follow this.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Crate-by-crate approach | Keeps each PR reviewable, allows incremental progress | — Pending |
| Breaking changes allowed | Quality over backward compat for this milestone | — Pending |
| Entire workspace in scope | Consistency requires touching everything | — Pending |

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
*Last updated: 2026-04-14 after initialization*
