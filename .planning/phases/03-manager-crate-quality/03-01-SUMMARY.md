---
phase: 03-manager-crate-quality
plan: 01
subsystem: tools
tags: [serde_json, dashmap, clone-audit, d-03-boundaries]

requires:
  - phase: 02-leaf-crate-quality
    provides: Typed SDK tool trait, typed ToolsCommand in anyclaw-core
provides:
  - D-03 documented Value boundaries across all tools crate modules
  - dashmap workspace dependency for Plans 02/03
  - Clone audit complete — all Arc clones use explicit Arc::clone()
affects: [03-02, 03-03, 03-04]

tech-stack:
  added: [dashmap 6]
  patterns: [D-03 justification comments on Value boundaries, explicit Arc::clone()]

key-files:
  created: []
  modified:
    - Cargo.toml
    - crates/anyclaw-tools/src/lib.rs
    - crates/anyclaw-tools/src/manager.rs
    - crates/anyclaw-tools/src/mcp_host.rs
    - crates/anyclaw-tools/src/external.rs
    - crates/anyclaw-tools/src/wasm_runner.rs
    - crates/anyclaw-tools/src/wasm_tool.rs

key-decisions:
  - "All serde_json::Value usages in tools crate are D-03 extensible boundaries — documented, not replaced"
  - "dashmap added to workspace deps only, not to anyclaw-tools Cargo.toml (reserved for Plans 02/03)"

patterns-established:
  - "D-03 comment pattern: inline comment explaining why Value cannot be typed at this layer"
  - "Explicit Arc::clone() for clarity over implicit .clone() on Arc handles"

requirements-completed: [JSON-06, CLON-02, CLON-03, BUGF-01, BUGF-02]

duration: 6min
completed: 2026-04-14
---

# Phase 03 Plan 01: Type anyclaw-tools Crate Summary

**D-03 documented Value boundaries across all tools modules, dashmap workspace dep added, clone audit with explicit Arc::clone()**

## Performance

- **Duration:** 6 min
- **Started:** 2026-04-14T15:11:21Z
- **Completed:** 2026-04-14T15:16:54Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- All 5 grandfathered `#[allow(clippy::disallowed_types)]` in lib.rs updated with specific D-03 justifications
- D-03 justification comments added to every serde_json::Value usage across manager.rs, mcp_host.rs, external.rs, wasm_runner.rs, wasm_tool.rs
- Clone audit complete — replaced implicit .clone() with Arc::clone() in manager.rs and wasm_runner.rs, replaced s.clone() with s.to_owned() in external.rs and wasm_runner.rs
- dashmap = "6" added to workspace dependencies for downstream plans

## Task Commits

Each task was committed atomically:

1. **Task 1: Add dashmap workspace dep + type tools manager Value usages** - `0c8ff89` (chore)
2. **Task 2: Type external.rs + wasm files, update lib.rs allows, clone audit** - `c9d04ab` (refactor)

## Files Created/Modified
- `Cargo.toml` - Added dashmap = "6" to [workspace.dependencies]
- `crates/anyclaw-tools/src/lib.rs` - Updated all 5 allow comments from "Grandfathered" to D-03 justifications
- `crates/anyclaw-tools/src/manager.rs` - D-03 comments on route_call/dispatch_tool_inner args, Arc::clone(), test mock allow
- `crates/anyclaw-tools/src/mcp_host.rs` - D-03 comments on dispatch_tool args, test mock allow
- `crates/anyclaw-tools/src/external.rs` - D-03 comment on serialize_option_value, s.to_owned() instead of s.clone()
- `crates/anyclaw-tools/src/wasm_runner.rs` - D-03 comments on options params, Arc::clone(), s.to_owned()
- `crates/anyclaw-tools/src/wasm_tool.rs` - D-03 comments on Tool impl and WASM output parsing

## Decisions Made
- All serde_json::Value usages in the tools crate are legitimate D-03 extensible boundaries (Tool trait input_schema/execute, arbitrary config options, tool call args) — documented rather than replaced
- dashmap added at workspace level only, not wired into anyclaw-tools yet (reserved for Plans 02/03 DashMap migration)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Tools crate fully documented with D-03 boundaries, ready for Plans 02/03 (agents, channels managers)
- dashmap available as workspace dependency for DashMap migration in connection.rs files

## Self-Check: PASSED

- SUMMARY.md: FOUND
- Commit 0c8ff89: FOUND
- Commit c9d04ab: FOUND

---
*Phase: 03-manager-crate-quality*
*Completed: 2026-04-14*
