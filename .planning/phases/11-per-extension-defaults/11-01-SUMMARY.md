---
phase: 11-per-extension-defaults
plan: 01
status: complete
started: 2026-04-16
completed: 2026-04-16
---

## What Was Built

Per-extension sidecar defaults loading via `load_extension_defaults()` in `anyclaw-config`. Extensions can ship a `<binary>.defaults.yaml` file that merges into entity `options` maps with user-wins precedence.

## Key Files

### Created
- `crates/anyclaw-config/src/extension_defaults.rs` — `load_extension_defaults()` + `merge_sidecar_into_options()` helper + 9 unit tests

### Modified
- `crates/anyclaw-config/src/lib.rs` — module registration + re-export
- `crates/anyclaw-supervisor/src/lib.rs` — call site after `resolve_all_binary_paths()`
- `crates/anyclaw-config/AGENTS.md` — files table + Extension Defaults section

## Decisions Made

- Used `entry().or_insert()` for user-wins merge semantics (per D-06)
- Extracted `merge_sidecar_into_options` helper to avoid repeating read/parse/merge across three entity types
- Malformed YAML: `tracing::warn!` + skip (per agent discretion)
- Successful load: `tracing::trace!` with key list

## Self-Check

- [x] 9/9 tests pass (`cargo test -p anyclaw-config -- extension_defaults`)
- [x] Full workspace tests pass
- [x] Zero new clippy warnings in extension_defaults.rs
- [x] No unsafe, no bare unwrap in production code
- [x] AGENTS.md updated in same commit
