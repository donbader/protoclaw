---
phase: 04-sdk-external-polish
plan: 04
subsystem: ext
tags: [typed-json, serde, d-03, channel, tool, mock-agent, clippy, disallowed-types]

requires:
  - phase: 04-sdk-external-polish
    provides: Typed SDK trait signatures from plan 01

provides:
  - All ext/ binaries and examples compile against updated SDK traits
  - Zero undocumented serde_json::Value — every usage has D-03 justification
  - Zero grandfathered #[allow(clippy::disallowed_types)] annotations
  - PendingPermission.options typed as Vec<PermissionOption> in debug-http

affects: [05-testing, 06-decomposition]

tech-stack:
  added: []
  patterns: [D-03 allow annotation pattern for ext/ binaries with Value at trait boundaries]

key-files:
  created: []
  modified:
    - ext/channels/debug-http/src/main.rs
    - ext/channels/telegram/src/channel.rs
    - ext/channels/telegram/src/deliver.rs
    - ext/channels/telegram/src/main.rs
    - ext/channels/sdk-test-channel/src/main.rs
    - ext/agents/mock-agent/src/main.rs
    - ext/tools/sdk-test-tool/src/main.rs
    - examples/01-fake-agent-telegram-bot/tools/system-info/src/main.rs

key-decisions:
  - "mock-agent keeps crate-level D-03 allow — it builds raw JSON-RPC messages by design"
  - "Tool binaries keep crate-level D-03 allow — Tool trait is Value-based per Plan 01 Task 3"
  - "debug-http PendingPermission.options typed as Vec<PermissionOption> — eliminates one Value usage"
  - "debug-http ad-hoc JSON handler returns get per-function D-03 allows instead of crate-level"

patterns-established:
  - "D-03 allow pattern: comment explaining why + #[allow(clippy::disallowed_types)] at narrowest scope"

requirements-completed: [JSON-08, BUGF-01, BUGF-02]

duration: 12min
completed: 2026-04-15
---

# Phase 4 Plan 4: Ext Binary Typing Summary

**Typed PendingPermission.options in debug-http, D-03 documented all Value usages across 8 ext/example binaries, zero grandfathered allows remaining**

## Performance

- **Duration:** 12 min
- **Started:** 2026-04-15T00:06:14Z
- **Completed:** 2026-04-15T00:18:20Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments
- All ext/ channel binaries compile against updated Channel trait (show_permission_prompt, typed on_ready)
- PendingPermission.options typed as Vec<PermissionOption> in debug-http (was serde_json::Value)
- handle_permissions_pending returns Json<Vec<PendingPermission>> instead of Json<Value>
- All grandfathered #[allow(clippy::disallowed_types)] replaced with D-03 justified allows
- Zero clippy warnings across full workspace

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix ext/ channel binaries** - `f87313d` (feat)
2. **Task 2: Type ext/ agent + tool binaries and examples** - `6ba5357` (feat)

## Files Created/Modified
- `ext/channels/debug-http/src/main.rs` - Typed PendingPermission.options, typed pending endpoint return, D-03 allows on handlers
- `ext/channels/telegram/src/channel.rs` - D-03 allows on on_initialize and handle_unknown
- `ext/channels/telegram/src/deliver.rs` - D-03 allow on deliver_to_chat content param
- `ext/channels/telegram/src/main.rs` - Replaced grandfathered allows with D-03 comments
- `ext/channels/sdk-test-channel/src/main.rs` - D-03 comment on DeliverMessage.content usage
- `ext/agents/mock-agent/src/main.rs` - Crate-level D-03 allow for raw JSON-RPC construction
- `ext/tools/sdk-test-tool/src/main.rs` - Crate-level D-03 allow for Tool trait Value I/O
- `examples/01-fake-agent-telegram-bot/tools/system-info/src/main.rs` - Crate-level D-03 allow for Tool trait Value I/O

## Decisions Made
- mock-agent keeps crate-level D-03 allow — it intentionally builds raw JSON-RPC to exercise the wire format
- Tool binaries (sdk-test-tool, system-info) keep crate-level D-03 allow — Tool trait is Value-based by design
- debug-http uses per-function D-03 allows on ad-hoc JSON handlers — narrower scope than crate-level
- telegram uses per-method D-03 allows on on_initialize (options HashMap) and handle_unknown (no schema)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- JSON-08 complete — all ext/ binaries and examples use typed SDK traits
- BUGF-01, BUGF-02 complete — zero clippy warnings, zero undocumented Value
- Phase 04 fully complete — ready for Phase 05 (testing) or Phase 06 (decomposition)

---
*Phase: 04-sdk-external-polish*
*Completed: 2026-04-15*

## Self-Check: PASSED

- SUMMARY.md: FOUND
- Commit f87313d: FOUND
- Commit 6ba5357: FOUND
