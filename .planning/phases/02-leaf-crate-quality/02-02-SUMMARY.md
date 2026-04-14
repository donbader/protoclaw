---
phase: 02-leaf-crate-quality
plan: 02
subsystem: jsonrpc
tags: [serde, typed-json, codec, ndjson, tokio-util, thiserror]

requires:
  - phase: 01-tooling-ci
    provides: clippy disallowed_types lint, CI pipeline
provides:
  - Typed NdJsonCodec decoding/encoding JsonRpcMessage instead of raw Value
  - RequestId enum replacing Option<Value> for JSON-RPC id fields
  - Convenience Encoder impls for JsonRpcRequest and JsonRpcResponse
  - Non-JSON-RPC lines skipped at codec boundary (typed deserialization rejects them)
affects: [03-manager-crate-quality]

tech-stack:
  added: []
  patterns: [struct-level clippy::disallowed_types with D-03 justification, convenience Encoder impls for sub-types]

key-files:
  created: []
  modified:
    - crates/anyclaw-jsonrpc/src/types.rs
    - crates/anyclaw-jsonrpc/src/codec.rs
    - crates/anyclaw-jsonrpc/src/lib.rs
    - crates/anyclaw-jsonrpc/AGENTS.md

key-decisions:
  - "Struct-level #[allow(clippy::disallowed_types)] required — clippy fires on inner type expressions, not suppressible at field level"
  - "params/result/data stay as serde_json::Value — D-03 extensible boundaries where typed core meets arbitrary method-specific payloads"
  - "Module-level allow kept on types module only — codec module is now Value-free"

patterns-established:
  - "Convenience Encoder impls: add Encoder<SubType> that wraps into the enum, avoiding forced wrapping at call sites"
  - "Non-JSON-RPC lines silently skipped by typed deserialization — same skip behavior as invalid JSON"

requirements-completed: [JSON-02, ERRH-01, ERRH-02, ERRH-03, SERD-01, SERD-02, BUGF-01, BUGF-02]

duration: 7min
completed: 2026-04-14
---

# Phase 2 Plan 2: jsonrpc Typed Codec Summary

**Typed NdJsonCodec decoding JsonRpcMessage with RequestId enum, convenience encoders for Request/Response, and non-JSON-RPC line rejection at codec boundary — 39 tests, zero clippy warnings**

## Performance

- **Duration:** 7 min
- **Started:** 2026-04-14T10:01:46Z
- **Completed:** 2026-04-14T10:08:29Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Replaced `Decoder::Item` from `serde_json::Value` to `JsonRpcMessage` — upstream crates now receive typed messages
- Added `RequestId` enum (Number/String) replacing `Option<Value>` for all id fields
- Added convenience `Encoder<JsonRpcRequest>` and `Encoder<JsonRpcResponse>` impls
- Non-JSON-RPC lines (valid JSON but not request/response) now silently skipped at codec boundary
- Removed grandfathered allow from codec module — codec is completely Value-free
- 39 total tests (20 types + 19 codec) with rstest, BDD naming, round-trip coverage

## Task Commits

Each task was committed atomically:

1. **Task 1: Type JSON-RPC types — RequestId enum, typed id fields** - `461ce51` (feat)
2. **Task 2: Type codec — decode/encode JsonRpcMessage, remove lib.rs grandfathered allows** - `5f203f1` (feat)

## Files Created/Modified
- `crates/anyclaw-jsonrpc/src/types.rs` - Added RequestId enum, typed id fields, struct-level D-03 allows, 20 rstest tests
- `crates/anyclaw-jsonrpc/src/codec.rs` - Decoder::Item=JsonRpcMessage, Encoder<JsonRpcMessage/Request/Response>, 19 rstest tests
- `crates/anyclaw-jsonrpc/src/lib.rs` - Removed grandfathered allow on codec, kept types module allow with D-03 justification
- `crates/anyclaw-jsonrpc/AGENTS.md` - Updated key types and framing docs to reflect typed API

## Decisions Made
- Struct-level `#[allow(clippy::disallowed_types)]` required on JsonRpcRequest, JsonRpcResponse, JsonRpcError — clippy's disallowed_types lint fires on inner type expressions and cannot be suppressed at field level. Same finding as plan 02-01.
- params/result/data fields stay as `serde_json::Value` — these are D-03 extensible boundaries. Typing them would require the framing layer to know about every JSON-RPC method's schema, violating the crate's "pure framing" anti-pattern.
- Module-level allow kept on `types` module only — codec module is now completely Value-free, so its grandfathered allow was removed.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Moved field-level allows to struct-level**
- **Found during:** Task 2
- **Issue:** Plan specified field-level `#[allow(clippy::disallowed_types)]` on params/result/data fields, but clippy fires on inner type expressions and ignores field-level allows
- **Fix:** Moved allows to struct level with D-03 justification comments, kept module-level allow on types module
- **Files modified:** crates/anyclaw-jsonrpc/src/types.rs, crates/anyclaw-jsonrpc/src/lib.rs
- **Verification:** `cargo clippy -p anyclaw-jsonrpc -- -D warnings` produces zero warnings
- **Committed in:** 5f203f1

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Allow placement is a clippy limitation, not a design change. The intent (D-03 extensible fields documented) is preserved.

## Issues Encountered
None — plan executed cleanly after the allow placement adjustment.

## Next Phase Readiness
- jsonrpc crate fully typed — codec delivers `JsonRpcMessage` to consumers
- Breaking change: downstream crates (anyclaw-agents, anyclaw-channels, anyclaw-sdk-agent, anyclaw-sdk-channel) will need updating in Phase 3 to consume `JsonRpcMessage` instead of `Value`
- FramingError enum already complete — no new variants needed

## Self-Check: PASSED

- SUMMARY.md: FOUND
- Commit 461ce51: FOUND
- Commit 5f203f1: FOUND

---
*Phase: 02-leaf-crate-quality*
*Completed: 2026-04-14*
