---
phase: 03-manager-crate-quality
plan: 03
subsystem: agents
tags: [serde_json, dashmap, typed-codec, d-03-boundaries, clone-audit]

requires:
  - phase: 02-leaf-crate-quality
    provides: Typed JsonRpcMessage codec, typed JsonRpcRequest/JsonRpcResponse, RequestId
  - phase: 03-manager-crate-quality
    provides: dashmap workspace dependency (Plan 01)
provides:
  - DashMap for pending_requests in agents connection.rs
  - Typed JsonRpcMessage pipeline — no codec boundary shims
  - Typed IncomingMessage variants (JsonRpcRequest instead of Value)
  - Typed send_raw (JsonRpcResponse instead of Value)
  - Typed PlatformCommand with Serialize derive
  - D-03 documented Value boundaries across agents crate modules
affects: [03-04]

tech-stack:
  added: [dashmap 6 (wired into anyclaw-agents)]
  patterns: [typed JsonRpcMessage pipeline, DashMap for pending requests, typed JsonRpcResponse for send_raw, D-03 justification comments]

key-files:
  created: []
  modified:
    - Cargo.lock
    - crates/anyclaw-agents/Cargo.toml
    - crates/anyclaw-agents/src/connection.rs
    - crates/anyclaw-agents/src/manager.rs
    - crates/anyclaw-agents/src/platform_commands.rs
    - crates/anyclaw-agents/src/slot.rs
    - crates/anyclaw-agents/src/lib.rs

key-decisions:
  - "send_raw accepts JsonRpcResponse — permission responses and fs responses are typed, not raw Value"
  - "PendingPermission.request changed from Value to JsonRpcRequest — typed throughout permission flow"
  - "last_available_commands stays Option<Value> with D-03 comment — stores arbitrary agent-reported commands"
  - "send_request/send_notification params stay Value with D-03 allows — method-specific schemas"

patterns-established:
  - "Typed response extraction: resp.result.unwrap_or_default() instead of resp[\"result\"]"
  - "Typed error checking: resp.error.is_some() instead of resp.get(\"__jsonrpc_error\")"
  - "DashMap for pending_requests: lock-free concurrent access replaces Arc<Mutex<HashMap>>"

requirements-completed: [JSON-04, ADVN-01, CLON-02, CLON-03, D-07, BUGF-01, BUGF-02]

duration: 19min
completed: 2026-04-14
---

# Phase 03 Plan 03: Type Agents Support Files Summary

**DashMap for pending_requests, typed JsonRpcMessage codec pipeline with no shims, typed permission/fs response flow, Serialize on PlatformCommand**

## Performance

- **Duration:** 19 min
- **Started:** 2026-04-14T15:31:36Z
- **Completed:** 2026-04-14T15:50:18Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Replaced Arc<Mutex<HashMap>> with DashMap for lock-free pending_requests in connection.rs
- Removed codec boundary shims (from_value/to_value) — typed JsonRpcMessage pipeline end-to-end
- IncomingMessage variants now carry typed JsonRpcRequest instead of Value
- send_raw accepts JsonRpcResponse, send_request returns JsonRpcResponse
- PendingPermission.request typed as JsonRpcRequest, send_success_response/send_error_response build typed JsonRpcResponse
- PlatformCommand gets Serialize derive and typed platform_commands() accessor
- All lib.rs allow comments updated from "Grandfathered" to D-03 justifications, connection allow removed

## Task Commits

Each task was committed atomically:

1. **Task 1: DashMap migration + typed codec pipeline in agents connection.rs** - `ef8f0f2` (refactor)
2. **Task 2: Type platform_commands.rs + slot.rs, update lib.rs allows** - `3fc99ad` (refactor)

## Files Created/Modified
- `Cargo.lock` - Added dashmap 6.1.0 to anyclaw-agents
- `crates/anyclaw-agents/Cargo.toml` - Added dashmap workspace dependency
- `crates/anyclaw-agents/src/connection.rs` - DashMap, typed pipeline, IncomingMessage uses JsonRpcRequest, send_raw accepts JsonRpcResponse
- `crates/anyclaw-agents/src/manager.rs` - Typed response extraction, typed permission/fs flows, PendingPermission.request as JsonRpcRequest
- `crates/anyclaw-agents/src/platform_commands.rs` - Serialize derive, typed platform_commands() accessor, simplified platform_commands_json()
- `crates/anyclaw-agents/src/slot.rs` - D-03 comment on last_available_commands
- `crates/anyclaw-agents/src/lib.rs` - Removed connection allow, updated remaining to D-03 justifications

## Decisions Made
- send_raw changed from Value to JsonRpcResponse — all callers (permission responses, fs responses, error responses) now build typed responses
- PendingPermission.request changed from Value to JsonRpcRequest — eliminates Value indexing in permission flow
- last_available_commands stays Option<Value> — stores arbitrary agent-reported availableCommands payload (D-03 boundary)
- send_request/send_notification params stay Value — method-specific schemas cannot be typed at connection layer (D-03)
- Eliminated __jsonrpc_error sentinel — typed JsonRpcResponse.error field replaces the Value-based sentinel pattern

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed downstream manager.rs compilation after connection.rs type changes**
- **Found during:** Task 1
- **Issue:** Changing IncomingMessage, send_raw, send_request return types broke ~30 call sites in manager.rs
- **Fix:** Updated handle_incoming to use typed JsonRpcRequest fields, updated all send_raw callers to build JsonRpcResponse, updated response extraction to use .result.unwrap_or_default(), updated all test constructions
- **Files modified:** crates/anyclaw-agents/src/manager.rs
- **Committed in:** ef8f0f2 (part of Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Necessary cascade from typed pipeline change — same pattern as Plan 02. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Agents support files fully typed with D-03 boundaries, DashMap integrated
- Ready for Plan 04 (manager.rs error enum restructuring and remaining typing)

## Self-Check: PASSED

- SUMMARY.md: FOUND
- Commit ef8f0f2: FOUND
- Commit 3fc99ad: FOUND

---
*Phase: 03-manager-crate-quality*
*Completed: 2026-04-14*
