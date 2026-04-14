---
phase: 01-tooling-lint-infrastructure
plan: 01
subsystem: infra
tags: [clippy, rustfmt, cargo-deny, lints, workspace-config]

requires: []
provides:
  - Workspace-level clippy lint configuration with serde_json::Value ban
  - rustfmt.toml with edition 2024
  - Expanded deny.toml with advisories, bans, sources enforcement
  - Lint inheritance across all 19 workspace crates
affects: [01-tooling-lint-infrastructure, 02-leaf-crate-cleanup, 03-manager-crate-cleanup, 04-sdk-crate-cleanup]

tech-stack:
  added: [clippy.toml, rustfmt.toml]
  patterns: [workspace-lint-inheritance, disallowed-types-enforcement]

key-files:
  created: [clippy.toml, rustfmt.toml]
  modified: [Cargo.toml, deny.toml, crates/*/Cargo.toml, ext/*/Cargo.toml]

key-decisions:
  - "All lints set to warn (not deny) — CI promotes to errors with -D warnings"
  - "deny.toml advisories uses cargo-deny 0.19.x defaults instead of per-severity fields"

patterns-established:
  - "Workspace lint inheritance: all crates use [lints] workspace = true"
  - "serde_json::Value banned via clippy disallowed-types — requires explicit #[allow] with justification"

requirements-completed: [TOOL-01, TOOL-02, TOOL-03, TOOL-04]

duration: 4min
completed: 2026-04-14
---

# Phase 01 Plan 01: Lint Config & Workspace Lints Summary

**Workspace-wide clippy/rustfmt/cargo-deny config with serde_json::Value ban and lint inheritance across all 19 crates**

## Performance

- **Duration:** 4 min
- **Started:** 2026-04-14T06:50:55Z
- **Completed:** 2026-04-14T06:54:39Z
- **Tasks:** 2
- **Files modified:** 23

## Accomplishments
- Created clippy.toml banning serde_json::Value via disallowed-types
- Created rustfmt.toml with edition 2024
- Expanded deny.toml with advisories, bans, and sources enforcement
- Added workspace-level clippy and rust lint sections to root Cargo.toml
- Propagated lint inheritance to all 19 workspace crates

## Task Commits

Each task was committed atomically:

1. **Task 1: Create lint config files and workspace lints** - `db818ee` (chore)
2. **Task 2: Propagate lint inheritance to all workspace crates** - `e002e9e` (chore)

## Files Created/Modified
- `clippy.toml` - Disallowed types config banning serde_json::Value
- `rustfmt.toml` - Edition 2024 formatting config
- `deny.toml` - Expanded with advisories, bans, sources sections
- `Cargo.toml` - Workspace-level [workspace.lints.clippy] and [workspace.lints.rust]
- `crates/*/Cargo.toml` (13 files) - Added [lints] workspace = true
- `ext/*/Cargo.toml` (4 files) - Added [lints] workspace = true
- `examples/01-fake-agent-telegram-bot/tools/system-info/Cargo.toml` - Added [lints] workspace = true

## Decisions Made
- All lints set to `warn` level per plan guidance — CI would promote to errors with `-- -D warnings`
- Used cargo-deny 0.19.x compatible config format (simplified advisories section, removed unsupported `allow-build-scripts` string value)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed deny.toml for cargo-deny 0.19.x compatibility**
- **Found during:** Task 1 verification
- **Issue:** Plan specified `unmaintained = "warn"` and `allow-build-scripts = "allow"` which are invalid in cargo-deny 0.19.x (expects different value types)
- **Fix:** Simplified [advisories] to use defaults with `ignore = []`, removed `allow-build-scripts` string value
- **Files modified:** deny.toml
- **Verification:** `cargo deny check` passes — advisories ok, bans ok, licenses ok, sources ok
- **Committed in:** a9b60cf

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Necessary for cargo-deny compatibility. Same security enforcement, correct config format.

## Issues Encountered
None beyond the deny.toml format deviation above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Lint infrastructure is active — all subsequent code changes will trigger workspace clippy warnings
- Plan 02 (clippy warning fixes) can proceed immediately
- Plan 03 (CI enforcement) can wire `-- -D warnings` to promote warns to errors

## Self-Check: PASSED

All files exist, all commits verified.

---
*Phase: 01-tooling-lint-infrastructure*
*Completed: 2026-04-14*
