---
phase: 02-leaf-crate-quality
plan: 03
subsystem: core
tags: [typed-json, thiserror, session-list, codec-boundary, tokio]

requires:
  - phase: 02-leaf-crate-quality
    provides: Typed sdk-types wire types (Plan 01), typed NdJsonCodec (Plan 02)
provides:
  - Zero serde_json::Value in anyclaw-core production code
  - Typed SessionListResult flowing through AgentsCommand::ListSessions
  - Workspace compiles with all leaf crate breaking changes resolved
  - Mock-agent ContentPart wire format aligned with typed SessionUpdateType
affects: [03-manager-crate-quality]

tech-stack:
  added: []
  patterns: [Value↔JsonRpcMessage conversion at codec boundary for minimal downstream fix]

key-files:
  created: []
  modified:
    - crates/anyclaw-core/src/agents_command.rs
    - crates/anyclaw-core/src/lib.rs
    - crates/anyclaw-core/Cargo.toml
    - crates/anyclaw-agents/src/connection.rs
    - crates/anyclaw-agents/src/manager.rs
    - crates/anyclaw-channels/src/connection.rs
    - ext/agents/mock-agent/src/main.rs

key-decisions:
  - "Error enums (SupervisorError, ManagerError, SessionStoreError) confirmed complete — no restructuring needed"
  - "Value↔JsonRpcMessage conversion at codec boundary — minimal fix for Phase 2, full pipeline typing deferred to Phase 3"
  - "tokio rt feature added to anyclaw-core dependencies — spawn_blocking requires it"

patterns-established:
  - "Codec boundary conversion: serde_json::from_value/to_value at FramedRead/FramedWrite boundaries bridges typed codec with Value-based pipelines"

requirements-completed: [JSON-03, ERRH-01, ERRH-02, ERRH-03, SERD-01, SERD-02, D-08, D-09, D-10, BUGF-01, BUGF-02]

duration: 24min
completed: 2026-04-14
---

# Phase 2 Plan 3: core Typed Commands + Workspace Compilation Summary

**Zero Value in anyclaw-core with typed SessionListResult, error enums audited, and full workspace compiling after leaf crate breaking changes — codec boundary conversions bridge typed codec with Value-based manager pipelines**

## Performance

- **Duration:** 24 min
- **Started:** 2026-04-14T10:11:02Z
- **Completed:** 2026-04-14T10:35:07Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Replaced the single `serde_json::Value` usage in `agents_command.rs` with typed `SessionListResult`
- Removed grandfathered `#[allow(clippy::disallowed_types)]` from `lib.rs` — core is now Value-free
- Fixed downstream compilation in agents/channels connection.rs for NdJsonCodec breaking change (Decoder::Item = JsonRpcMessage)
- Updated mock-agent to emit ContentPart wire format `{"type":"text","text":"..."}` instead of bare strings
- Audited all error enums — confirmed complete, no restructuring needed
- Zero bare `.unwrap()` in production code (all existing unwraps are in test modules)

## Task Commits

Each task was committed atomically:

1. **Task 1: Type agents_command.rs, audit error enums, fix unwraps, remove grandfathered allows** - `3bda246` (feat)
2. **Task 2: Fix downstream compilation — update consumers of changed types so workspace builds** - `a0c1d7f` (fix)

## Files Created/Modified
- `crates/anyclaw-core/src/agents_command.rs` - ListSessions reply typed as SessionListResult, added oneshot round-trip test
- `crates/anyclaw-core/src/lib.rs` - Removed grandfathered allow on agents_command module
- `crates/anyclaw-core/Cargo.toml` - Added tokio `rt` feature (spawn_blocking requires it)
- `crates/anyclaw-agents/src/connection.rs` - Value↔JsonRpcMessage conversion at codec read/write boundaries
- `crates/anyclaw-agents/src/manager.rs` - list_sessions() returns SessionListResult, deserializes from Value
- `crates/anyclaw-channels/src/connection.rs` - Value↔JsonRpcMessage conversion at codec read/write boundaries
- `ext/agents/mock-agent/src/main.rs` - ContentPart wire format for thought/message chunks, updated tests

## Decisions Made
- Error enums (`SupervisorError`, `ManagerError`, `SessionStoreError`) audited and confirmed complete — all variants have context, `thiserror` used consistently, no restructuring needed.
- `Config(String)` kept on `SupervisorError` — `anyclaw-core` doesn't depend on `anyclaw-config`, so wrapping `ConfigError` directly isn't possible.
- `Backend(String)` kept on `SessionStoreError` — consistent with existing `.map_err(|e| e.to_string())` pattern throughout sqlite_session_store.
- Value↔JsonRpcMessage conversion at codec boundary is the minimal fix — Phase 3 will type the full manager pipelines.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added tokio `rt` feature to anyclaw-core dependencies**
- **Found during:** Task 1
- **Issue:** `sqlite_session_store.rs` uses `tokio::task::spawn_blocking` but the crate only declared `features = ["sync"]`. The `rt` feature was only in dev-dependencies, causing clippy to fail on the lib target.
- **Fix:** Added `"rt"` to tokio features in `[dependencies]`
- **Files modified:** crates/anyclaw-core/Cargo.toml
- **Verification:** `cargo clippy -p anyclaw-core -- -D warnings` produces zero warnings
- **Committed in:** 3bda246

**2. [Rule 3 - Blocking] Fixed mock-agent ContentPart wire format**
- **Found during:** Task 2
- **Issue:** Plan 01 typed `SessionUpdateType` content fields as `ContentPart` (internally tagged `{"type":"text","text":"..."}`) but mock-agent still emitted bare strings. Integration test `when_message_sent_then_streaming_chunks_arrive_before_result` failed with deserialization errors.
- **Fix:** Updated mock-agent to emit ContentPart-formatted JSON for thought chunks and message chunks. Updated mock-agent unit tests.
- **Files modified:** ext/agents/mock-agent/src/main.rs
- **Verification:** Integration tests pass (except pre-existing `when_two_tools_configured` failure unrelated to this plan)
- **Committed in:** a0c1d7f

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both fixes necessary for compilation and test correctness. No scope creep.

## Issues Encountered
- Integration test `when_two_tools_configured_and_message_sent_then_agent_echoes_back` fails — pre-existing issue where platform context injection replaces user message content. Unrelated to codec/type changes. Logged to deferred items.
- Integration test `when_request_permission_received_then_harness_calls_channel_and_sends_response` is flaky — passes in isolation, fails intermittently in full workspace run. Pre-existing.

## Next Phase Readiness
- All three leaf crates (sdk-types, jsonrpc, core) are now typed with zero Value in production code
- Workspace compiles and all unit tests pass
- Manager crates have Value↔JsonRpcMessage conversion shims at codec boundaries — Phase 3 will type the full pipelines and remove these shims
- Breaking change: manager crates still use `serde_json::Value` internally with grandfathered allows — Phase 3 scope

## Self-Check: PASSED

- SUMMARY.md: FOUND
- Commit 3bda246: FOUND
- Commit a0c1d7f: FOUND

---
*Phase: 02-leaf-crate-quality*
*Completed: 2026-04-14*
