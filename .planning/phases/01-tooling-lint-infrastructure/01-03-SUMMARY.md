---
phase: 01-tooling-lint-infrastructure
plan: 03
subsystem: infra
tags: [cargo-llvm-cov, coverage, llvm, ci]

requires:
  - phase: none
    provides: n/a
provides:
  - Baseline coverage report with per-file line coverage percentages
  - Recommended CI coverage floor (70%)
  - cargo-llvm-cov verified working with Homebrew LLVM toolchain
affects: [ci-enforcement, test-coverage-improvements]

tech-stack:
  added: [cargo-llvm-cov]
  patterns: [LLVM_COV/LLVM_PROFDATA env vars for Homebrew Rust installs]

key-files:
  created:
    - .planning/phases/01-tooling-lint-infrastructure/coverage-baseline.txt
  modified: []

key-decisions:
  - "Set CI floor at 70% (5% below measured 75.17%) to allow refactoring headroom"
  - "Exclude integration tests from coverage (require pre-built binaries, hang under instrumentation)"
  - "Use Homebrew LLVM tools via env vars since Rust is installed via Homebrew not rustup"

patterns-established:
  - "Coverage measurement: LLVM_COV and LLVM_PROFDATA env vars required for Homebrew Rust"

requirements-completed: [TOOL-05]

duration: 29min
completed: 2026-04-14
---

# Phase 01 Plan 03: Coverage Baseline Summary

**cargo-llvm-cov baseline at 75.17% line coverage across 82 source files, CI floor set at 70%**

## Performance

- **Duration:** 29 min
- **Started:** 2026-04-14T06:56:51Z
- **Completed:** 2026-04-14T07:25:51Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- Verified cargo-llvm-cov 0.8.5 installed and working with Homebrew LLVM 21
- Captured per-file coverage report: 75.17% total line coverage, 77.34% function coverage
- Documented recommended CI floor at 70% with rationale
- Verified HTML report generation works (target/llvm-cov-html/)

## Task Commits

Each task was committed atomically:

1. **Task 1: Install cargo-llvm-cov and capture baseline coverage** - `d3f7111` (chore)

## Files Created/Modified
- `.planning/phases/01-tooling-lint-infrastructure/coverage-baseline.txt` - Per-file coverage report with TOTAL row and CI floor recommendation header

## Decisions Made
- Set CI floor at 70% (5% below measured 75.17%) — provides headroom for refactoring that may temporarily reduce coverage
- Excluded integration tests from coverage measurement — they require pre-built binaries and hang under LLVM instrumentation
- Documented Homebrew LLVM env var requirement — Rust installed via Homebrew lacks rustup, so llvm-tools-preview component unavailable

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Homebrew LLVM tools required instead of rustup component**
- **Found during:** Task 1 (running cargo llvm-cov)
- **Issue:** `rustup component add llvm-tools-preview` failed because Rust is installed via Homebrew, not rustup
- **Fix:** Set LLVM_COV and LLVM_PROFDATA env vars pointing to Homebrew's llvm@21 binaries
- **Files modified:** coverage-baseline.txt (documented in header comments)
- **Verification:** cargo llvm-cov report ran successfully
- **Committed in:** d3f7111

**2. [Rule 3 - Blocking] Integration tests hanging under coverage instrumentation**
- **Found during:** Task 1 (first coverage run)
- **Issue:** Integration tests (flows_acp_wire) hung indefinitely under llvm-cov instrumentation, even with --exclude flag using wrong package name
- **Fix:** Used correct package name `anyclaw-integration-tests` with `--exclude`, and used `cargo llvm-cov report` subcommand for summary output
- **Files modified:** None (command-line adjustment only)
- **Verification:** Coverage report completed successfully with all unit tests passing
- **Committed in:** d3f7111

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both fixes necessary to produce the coverage report. No scope creep.

## Issues Encountered
None beyond the deviations documented above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 01 complete (3/3 plans done), ready for next phase
- Coverage baseline established for CI enforcement in future phases

---
*Phase: 01-tooling-lint-infrastructure*
*Completed: 2026-04-14*
