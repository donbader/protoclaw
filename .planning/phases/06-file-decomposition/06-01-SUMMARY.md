---
phase: 06-file-decomposition
plan: 01
subsystem: agents
tags: [rust, module-extraction, file-decomposition, agents-manager]

requires:
  - phase: 03-manager-crate-quality
    provides: typed pipeline and D-03 Value boundary documentation in agents crate
provides:
  - agents manager decomposed into 5 focused modules (manager, commands, incoming, session_recovery, fs_sandbox)
  - all production files under ~540 lines
  - AGENTS.md updated with new module structure
affects: [06-file-decomposition]

tech-stack:
  added: []
  patterns: [cross-file impl blocks for same struct, pub(crate) module boundaries]

key-files:
  created:
    - crates/anyclaw-agents/src/fs_sandbox.rs
    - crates/anyclaw-agents/src/session_recovery.rs
    - crates/anyclaw-agents/src/incoming.rs
    - crates/anyclaw-agents/src/commands.rs
  modified:
    - crates/anyclaw-agents/src/manager.rs
    - crates/anyclaw-agents/src/lib.rs
    - crates/anyclaw-agents/AGENTS.md

key-decisions:
  - "Extracted 4 modules instead of 3 — commands.rs added to bring manager.rs under ~500 lines"
  - "Used cross-file impl blocks (Rust allows multiple impl blocks for same struct across crate modules)"
  - "Made AgentsManager fields pub(crate) where needed for cross-module access"

patterns-established:
  - "Cross-file impl blocks: extracted modules define `impl AgentsManager` blocks for their domain"
  - "pub(crate) module boundaries: new modules are pub(crate) in lib.rs, not part of public API"

requirements-completed: [DECO-01, DECO-03, BUGF-01, BUGF-02]

duration: 43min
completed: 2026-04-15
---

# Phase 06 Plan 01: Agents Manager Decomposition Summary

**3,885-line agents manager decomposed into 5 focused modules using cross-file impl blocks — all production files under ~540 lines, 134 tests pass unchanged**

## Performance

- **Duration:** 43 min
- **Started:** 2026-04-15T01:47:37Z
- **Completed:** 2026-04-15T02:30:28Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Decomposed manager.rs from 3,885 lines to 526 production lines + 1,841 test lines
- Created 4 new modules: fs_sandbox.rs (156), session_recovery.rs (510), incoming.rs (379), commands.rs (540)
- Public API of anyclaw-agents crate unchanged — same exports from lib.rs
- All 134 existing tests pass without modification

## Task Commits

Each task was committed atomically:

1. **Task 1: Extract fs_sandbox, session_recovery, and incoming modules** - `0a849f8` (refactor)
2. **Task 2: Extract commands module, verify, update AGENTS.md** - `9daf7eb` (refactor)

## Files Created/Modified
- `crates/anyclaw-agents/src/fs_sandbox.rs` — path validation, FS read/write handlers (156 lines)
- `crates/anyclaw-agents/src/session_recovery.rs` — crash recovery, session restore, container cleanup (510 lines)
- `crates/anyclaw-agents/src/incoming.rs` — message dispatch, session updates, permissions, tool normalization (379 lines)
- `crates/anyclaw-agents/src/commands.rs` — command dispatch, session CRUD, prompt handling (540 lines)
- `crates/anyclaw-agents/src/manager.rs` — struct, constructor, ACP handshake, Manager trait impl (526 prod + 1841 test lines)
- `crates/anyclaw-agents/src/lib.rs` — added 4 new pub(crate) mod declarations
- `crates/anyclaw-agents/AGENTS.md` — updated Files table, D-03 boundaries, Anti-Patterns with new module locations

## Decisions Made
- Extracted 4 modules instead of the planned 3 — manager.rs was still 1,048 production lines after the first extraction, so `commands.rs` was added (Rule 2 deviation) to bring all files under ~540 lines
- Used Rust's cross-file `impl` blocks pattern — each extracted module defines its own `impl AgentsManager` block
- Made struct fields `pub(crate)` where needed for cross-module access rather than adding accessor methods

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Extracted commands.rs to meet ~500 line target**
- **Found during:** Task 2 (line count verification)
- **Issue:** After extracting 3 modules, manager.rs production code was still 1,048 lines — over the ~500 target
- **Fix:** Extracted `handle_command`, `create_session`, `prompt_session`, `fork_session`, `list_sessions`, `cancel_session`, `handle_platform_command`, `send_prompt_to_slot` into `commands.rs`
- **Files modified:** manager.rs, commands.rs, lib.rs
- **Verification:** `wc -l` confirms manager.rs at 526 production lines, commands.rs at 540 lines
- **Committed in:** 9daf7eb (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 missing critical functionality)
**Impact on plan:** Necessary to meet the ~500 line target. No scope creep — same extraction pattern as planned modules.

## Issues Encountered
None — straightforward module extraction with visibility adjustments.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Agents manager fully decomposed, ready for 06-02 (supervisor decomposition)
- Pattern established: cross-file impl blocks with pub(crate) boundaries

---
*Phase: 06-file-decomposition*
*Completed: 2026-04-15*

## Self-Check: PASSED

All created files exist, all commit hashes verified.
