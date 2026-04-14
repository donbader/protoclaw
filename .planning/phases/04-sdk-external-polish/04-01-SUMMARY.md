---
phase: 04-sdk-external-polish
plan: 01
subsystem: sdk
tags: [typed-json, serde, acp, mcp, channel, agent-adapter, tool-trait, jsonrpc]

requires:
  - phase: 02-leaf-crate-cleanup
    provides: Typed ACP structs in anyclaw-sdk-types
  - phase: 03-manager-deep-cleanup
    provides: Typed JsonRpcRequest/Response in anyclaw-jsonrpc

provides:
  - Typed AgentAdapter trait with ACP struct signatures (zero Value)
  - Typed ChannelHarness dispatch using JsonRpcRequest/Response
  - D-03 documented boundaries in all three SDK crates

affects: [04-sdk-external-polish, ext-binaries]

tech-stack:
  added: []
  patterns: [D-03 boundary documentation for intentional Value usages]

key-files:
  created: []
  modified:
    - crates/anyclaw-sdk-agent/src/adapter.rs
    - crates/anyclaw-sdk-agent/src/generic.rs
    - crates/anyclaw-sdk-agent/src/lib.rs
    - crates/anyclaw-sdk-channel/src/harness.rs
    - crates/anyclaw-sdk-channel/src/trait_def.rs
    - crates/anyclaw-sdk-channel/src/content.rs
    - crates/anyclaw-sdk-channel/src/lib.rs
    - crates/anyclaw-sdk-tool/src/trait_def.rs
    - crates/anyclaw-sdk-tool/src/server.rs
    - crates/anyclaw-sdk-tool/src/lib.rs

key-decisions:
  - "AgentAdapter hooks use typed ACP structs — zero serde_json::Value in sdk-agent"
  - "ChannelHarness parses stdin as JsonRpcRequest, uses typed RequestId for pending permissions"
  - "Tool I/O stays Value with D-03 justification — JSON Schema input has no fixed Rust type"
  - "handle_unknown stays Value with D-03 justification — unknown methods have no schema"

patterns-established:
  - "D-03 boundary pattern: inline comment explaining why Value is intentional, not grandfathered"

requirements-completed: [JSON-07]

duration: 10min
completed: 2026-04-15
---

# Phase 4 Plan 1: SDK Typing Summary

**Typed ACP structs in AgentAdapter, JsonRpcRequest dispatch in ChannelHarness, D-03 documented boundaries in all SDK crates**

## Performance

- **Duration:** 10 min
- **Started:** 2026-04-14T22:59:33Z
- **Completed:** 2026-04-14T23:09:33Z
- **Tasks:** 3
- **Files modified:** 12

## Accomplishments
- All 7 AgentAdapter hooks now accept/return typed ACP structs instead of serde_json::Value
- ChannelHarness parses stdin as JsonRpcRequest and uses typed RequestId for pending permissions map
- All remaining Value usages across 3 SDK crates have inline D-03 justification comments
- All grandfathered `#[allow(clippy::disallowed_types)]` annotations replaced with D-03 explanations
- 70 tests pass across all 3 SDK crates, workspace compiles clean

## Task Commits

Each task was committed atomically:

1. **Task 1: Type anyclaw-sdk-agent** - `c88bfb3` (feat)
2. **Task 2: Type anyclaw-sdk-channel** - `1fedf88` (feat)
3. **Task 3: Type anyclaw-sdk-tool** - `febcfe3` (docs)

## Files Created/Modified
- `crates/anyclaw-sdk-agent/src/adapter.rs` - Typed AgentAdapter/DynAgentAdapter trait signatures
- `crates/anyclaw-sdk-agent/src/generic.rs` - Updated tests for typed params
- `crates/anyclaw-sdk-agent/src/lib.rs` - Removed grandfathered allow, updated tests
- `crates/anyclaw-sdk-agent/Cargo.toml` - Added tokio macros+rt features for tests
- `crates/anyclaw-sdk-channel/src/harness.rs` - JsonRpcRequest parsing, typed RequestId map
- `crates/anyclaw-sdk-channel/src/trait_def.rs` - D-03 comment on handle_unknown
- `crates/anyclaw-sdk-channel/src/content.rs` - D-03 comment on content_to_string
- `crates/anyclaw-sdk-channel/src/lib.rs` - D-03 allow annotations replacing grandfathered
- `crates/anyclaw-sdk-channel/Cargo.toml` - Added anyclaw-jsonrpc dependency
- `crates/anyclaw-sdk-tool/src/trait_def.rs` - D-03 comments on Tool/DynTool Value signatures
- `crates/anyclaw-sdk-tool/src/server.rs` - D-03 comments on build_tool_list and dispatch_tool
- `crates/anyclaw-sdk-tool/src/lib.rs` - D-03 allow annotations replacing grandfathered

## Decisions Made
- AgentAdapter hooks use typed ACP structs — zero serde_json::Value in sdk-agent
- ChannelHarness parses stdin as JsonRpcRequest, uses typed RequestId for pending permissions
- Tool I/O stays Value with D-03 justification — JSON Schema input has no fixed Rust type
- handle_unknown stays Value with D-03 justification — unknown methods have no schema
- content_to_string stays Value with D-03 justification — DeliverMessage.content is agent-defined

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed tokio dev-dependency missing macros feature**
- **Found during:** Task 1 (AgentAdapter typing)
- **Issue:** `#[tokio::test]` failed to resolve — tokio dev-dependency lacked `macros` and `rt-multi-thread` features
- **Fix:** Added `features = ["macros", "rt-multi-thread"]` to sdk-agent Cargo.toml dev-dependencies
- **Files modified:** crates/anyclaw-sdk-agent/Cargo.toml
- **Verification:** All 25 sdk-agent tests pass
- **Committed in:** c88bfb3 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Necessary for test compilation. No scope creep.

## Issues Encountered
None — the Phase 3 compilation errors mentioned in critical_context were already resolved before this plan started.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- SDK crate typing complete — ext/ binaries (Plan 04) can now be updated to match
- All downstream crates (anyclaw-agents, anyclaw-channels, anyclaw-tools) compile clean
- D-03 boundary pattern established for remaining Value usages across the codebase

---
*Phase: 04-sdk-external-polish*
*Completed: 2026-04-15*

## Self-Check: PASSED

- SUMMARY.md: FOUND
- Commit c88bfb3: FOUND
- Commit 1fedf88: FOUND
- Commit febcfe3: FOUND
