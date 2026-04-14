---
phase: 02-leaf-crate-quality
plan: 01
subsystem: sdk-types
tags: [serde, typed-json, thiserror, acp, wire-types]

requires:
  - phase: 01-tooling-ci
    provides: clippy disallowed_types lint, CI pipeline
provides:
  - Typed ACP wire types in acp.rs (ContentPart, HashMap<String, Value> for extensible fields)
  - Typed ChannelEvent::RoutePermission.options as Vec<PermissionOption>
  - Default impl for ContentPart
  - Module-level allow annotations with D-03/pass-through justifications
affects: [02-leaf-crate-quality, 03-manager-crate-quality]

tech-stack:
  added: []
  patterns: [D-03 extensible HashMap pattern for agent-defined schemas, module-level clippy::disallowed_types with justification comments]

key-files:
  created: []
  modified:
    - crates/anyclaw-sdk-types/src/acp.rs
    - crates/anyclaw-sdk-types/src/channel.rs
    - crates/anyclaw-sdk-types/src/channel_event.rs
    - crates/anyclaw-sdk-types/src/lib.rs
    - crates/anyclaw-agents/src/manager.rs
    - crates/anyclaw-channels/src/manager.rs

key-decisions:
  - "DeliverMessage.content stays serde_json::Value — agents manager mutates raw JSON (timestamps, normalization, command injection)"
  - "Module-level #[allow(clippy::disallowed_types)] required — clippy disallowed_types fires on inner type expressions, not suppressible at field/item level"
  - "ConfigOptionUpdate/SessionInfoUpdate.extra changed from serde_json::Map to HashMap<String, Value> for consistency"

patterns-established:
  - "D-03 extensible fields: use HashMap<String, serde_json::Value> with module-level allow + comment"
  - "Pass-through Value fields: document why typing is not possible (mutation, normalization)"

requirements-completed: [JSON-01, ERRH-01, ERRH-03, SERD-01, SERD-02, BUGF-01, BUGF-02]

duration: 21min
completed: 2026-04-14
---

# Phase 2 Plan 1: sdk-types Typed Wire Types Summary

**Typed ACP wire types with ContentPart, HashMap extensible fields, and Vec<PermissionOption> for RoutePermission — zero bare unwraps in production code**

## Performance

- **Duration:** 21 min
- **Started:** 2026-04-14T09:37:38Z
- **Completed:** 2026-04-14T09:58:59Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- Replaced all bare `serde_json::Value` fields in acp.rs with typed alternatives (ContentPart, HashMap, Vec)
- Typed `ChannelEvent::RoutePermission.options` as `Vec<PermissionOption>`, eliminating unnecessary serialize/deserialize round-trip in agents manager
- Added `Default` impl for `ContentPart` (empty text)
- Added 17 new round-trip serde tests (72 total in sdk-types)
- Zero bare `.unwrap()` in production code across all sdk-types files

## Task Commits

Each task was committed atomically:

1. **Task 1: Type acp.rs** - `8072d5d` (feat)
2. **Task 2: Type channel_event.rs + lib.rs allows + downstream fixes** - `449bf42` (feat)

## Files Created/Modified
- `crates/anyclaw-sdk-types/src/acp.rs` - Typed Value fields → ContentPart, HashMap, Vec; added Default for ContentPart
- `crates/anyclaw-sdk-types/src/channel.rs` - Added D-03 comments on remaining Value fields
- `crates/anyclaw-sdk-types/src/channel_event.rs` - Typed RoutePermission.options as Vec<PermissionOption>
- `crates/anyclaw-sdk-types/src/lib.rs` - Updated module-level allows with descriptive comments
- `crates/anyclaw-agents/src/manager.rs` - Removed unnecessary serde round-trip for permission options; fixed test
- `crates/anyclaw-channels/src/manager.rs` - Updated route_permission_event signature to accept Vec<PermissionOption>

## Decisions Made
- DeliverMessage.content stays as serde_json::Value — agents manager mutates raw JSON with timestamps, tool event normalization, and command injection. Typing would break this mutation pattern.
- Module-level `#[allow(clippy::disallowed_types)]` kept on acp/channel/channel_event modules — clippy's disallowed_types lint fires on each type expression site and cannot be suppressed at field or item level. Comments updated to document remaining Value reasons.
- ConfigOptionUpdate/SessionInfoUpdate.extra changed from `serde_json::Map` to `HashMap<String, Value>` for consistency with other extensible fields.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed downstream compilation after RoutePermission.options typing**
- **Found during:** Task 2
- **Issue:** Changing RoutePermission.options from Value to Vec<PermissionOption> broke anyclaw-agents and anyclaw-channels
- **Fix:** Updated agents manager to pass Vec directly (removing unnecessary serde round-trip), updated channels manager signature, added .clone() for moved value
- **Files modified:** crates/anyclaw-agents/src/manager.rs, crates/anyclaw-channels/src/manager.rs
- **Verification:** Full workspace compiles and passes clippy
- **Committed in:** 449bf42

**2. [Rule 2 - Scope adjustment] DeliverMessage.content kept as Value**
- **Found during:** Task 2 analysis
- **Issue:** Plan specified typing DeliverMessage.content as SessionUpdateEvent, but agents manager extensively mutates raw JSON (adds _received_at_ms timestamps, normalizes tool event fields, injects platform commands into availableCommands arrays)
- **Fix:** Kept as Value with module-level allow and descriptive comment. ContentKind dispatch helper continues to work on raw JSON.
- **Impact:** No functional change — the mutation pattern requires raw JSON access

---

**Total deviations:** 2 (1 blocking fix, 1 scope adjustment)
**Impact on plan:** Blocking fix was necessary for compilation. Scope adjustment preserves correctness — typing DeliverMessage.content would break the agents manager's JSON mutation pipeline.

## Issues Encountered
- clippy `disallowed_types` lint cannot be suppressed at field or item level — it fires on each `serde_json::Value` usage site within type expressions. Required keeping module-level allows with updated comments instead of removing them per D-04.

## Next Phase Readiness
- sdk-types typed foundations ready for jsonrpc (plan 02) and core (plan 03) to build on
- Downstream crates (agents, channels) already updated for RoutePermission typing

---
*Phase: 02-leaf-crate-quality*
*Completed: 2026-04-14*
