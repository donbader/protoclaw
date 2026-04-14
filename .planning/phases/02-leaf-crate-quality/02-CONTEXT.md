# Phase 2: Leaf Crate Quality - Context

**Gathered:** 2026-04-14
**Status:** Ready for planning

<domain>
## Phase Boundary

Foundation crates (anyclaw-sdk-types, anyclaw-jsonrpc, anyclaw-core) use typed structs everywhere, have consistent error enums, and follow serde conventions. After this phase, manager crates can build on solid typed foundations.

</domain>

<decisions>
## Implementation Decisions

### Typed JSON Strategy
- **D-01:** Reuse `agent-client-protocol-schema` (v0.11) types where they exist (AgentCapabilities, McpCapabilities, PromptCapabilities, SessionCapabilities, etc.), write new typed structs for everything else (channel events, JSON-RPC params, tool results).
- **D-02:** Parse JSON into typed structs at the codec/connection boundary — internal code never touches `serde_json::Value`.
- **D-03:** For extensible protocol messages (ACP params, tool results), use `#[serde(flatten)]` with `HashMap<String, serde_json::Value>` for unknown fields — typed core + extensible extras.
- **D-04:** Remove all `#[allow(clippy::disallowed_types)]` annotations in leaf crates after replacing Value usages. 6 annotations across sdk-types (3), jsonrpc (2), core (1).

### Unwrap Cleanup
- **D-05:** Replace ALL ~96 bare `.unwrap()` calls in non-test leaf crate production code. No exceptions.
- **D-06:** Prefer `?` operator with proper error propagation wherever possible. Use `.expect("reason")` only for true invariants (e.g., regex compilation, static data).

### Serde Conventions
- **D-07:** Full audit and fix of ALL serde attributes in leaf crates — ensure `#[serde(rename_all = "camelCase")]` on wire types, `snake_case` on config types. Not just types being modified.
- **D-08:** Add round-trip serde tests for every public wire type in leaf crates during this phase (don't defer to Phase 5). Types are changing now — tests should validate immediately.

### Error Enum Approach
- **D-09:** Restructure error enums from scratch in all three leaf crates. Clean hierarchy, consistent patterns, every fallible path has a typed variant.
- **D-10:** Mix of `#[from]` for automatic conversion where it makes sense + manual `From` impls for conversions that need to add context.

### Agent's Discretion
- Exact struct field names for new typed structs (follow existing naming conventions)
- Which specific ACP schema types to reuse vs write fresh (based on what the crate covers)
- Internal organization of error enum variants
- Whether to use `#[serde(default)]` on optional fields or `Option<T>`

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Leaf crate sources (files with Value usages to replace)
- `crates/anyclaw-sdk-types/src/acp.rs` — ACP wire types, 8 unwraps, Value usages
- `crates/anyclaw-sdk-types/src/channel.rs` — Channel types, 32 unwraps, Value usages
- `crates/anyclaw-sdk-types/src/channel_event.rs` — Channel event types, 8 unwraps, Value usages
- `crates/anyclaw-sdk-types/src/permission.rs` — Permission types, 10 unwraps
- `crates/anyclaw-sdk-types/src/session_key.rs` — Session key, 1 unwrap
- `crates/anyclaw-jsonrpc/src/codec.rs` — JSON-RPC codec, 31 unwraps, Value usages
- `crates/anyclaw-jsonrpc/src/types.rs` — JSON-RPC types, 10 unwraps, Value usages
- `crates/anyclaw-jsonrpc/src/error.rs` — Framing error, Value usages
- `crates/anyclaw-core/src/agents_command.rs` — Agent commands, Value usages
- `crates/anyclaw-core/src/session_store.rs` — Session store, 2 unwraps
- `crates/anyclaw-core/src/manager.rs` — Manager trait, 2 unwraps

### Error enums to restructure
- `crates/anyclaw-sdk-types/src/lib.rs` — SDK types error (if exists)
- `crates/anyclaw-jsonrpc/src/error.rs` — FramingError
- `crates/anyclaw-core/src/error.rs` — ManagerError

### External type source
- `agent-client-protocol-schema` crate (v0.11) — ACP wire type definitions to reuse

### Conventions
- `AGENTS.md` §Conventions — Error handling boundary, unwrap rules
- `.planning/codebase/CONVENTIONS.md` — Serde patterns, naming conventions

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `agent-client-protocol-schema` (v0.11): Already provides `AgentCapabilities`, `McpCapabilities`, `PromptCapabilities`, `SessionCapabilities` — reuse these instead of redefining
- Existing error enums in all three crates: `FramingError` (jsonrpc), `ManagerError` (core), SDK type errors — restructure rather than extend

### Established Patterns
- Flat `lib.rs` with `pub mod` + `pub use` re-exports — maintain this during restructuring
- `#[serde(rename_all = "camelCase")]` on SDK wire types — enforce consistently
- `#[non_exhaustive]` on SDK enums — maintain for forward compatibility
- `impl_id_type!` macro for ID newtypes — reuse for any new ID types

### Integration Points
- `anyclaw-sdk-types` is depended on by sdk-agent, sdk-channel, sdk-tool, and core (re-exports) — type changes here cascade
- `anyclaw-jsonrpc` is depended on by agents and channels — codec changes affect message framing
- `anyclaw-core` re-exports from sdk-types — keep re-exports in sync after type changes

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard Rust serde/thiserror patterns for the typed replacements.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 02-leaf-crate-quality*
*Context gathered: 2026-04-14*
