# Phase 3: Manager Crate Quality - Context

**Gathered:** 2026-04-14
**Status:** Ready for planning

<domain>
## Phase Boundary

The three manager crates (anyclaw-agents, anyclaw-channels, anyclaw-tools) use typed data throughout, have reduced clone overhead, and use lock-free concurrent maps. After this phase, the heaviest crates in the workspace are clean.

</domain>

<decisions>
## Implementation Decisions

### Value Replacement Approach
- **D-01:** Crate by crate — each manager crate fully typed before moving to the next. Easier to verify, cleaner commits.
- **D-02:** Replace Phase 2 codec boundary shims with fully typed pipelines — managers consume `JsonRpcMessage` directly from the codec. No intermediate Value conversion.
- **D-03:** Remove all `#[allow(clippy::disallowed_types)]` annotations in manager crates after replacing Value usages. 12 annotations across agents (4), channels (3), tools (5).

### Clone Reduction Strategy
- **D-04:** Full systematic audit of all 107 clones in agents manager — categorize each as necessary (Arc, async move) vs unnecessary, eliminate the unnecessary ones.
- **D-05:** Maximum cleanup — eliminate every clone that isn't strictly necessary. Not just obvious ones.
- **D-06:** Apply same audit to channels and tools managers (CLON-02).

### DashMap Migration
- **D-07:** Replace `Arc<Mutex<HashMap<u64, oneshot::Sender>>>` with `DashMap` in both agents and channels connection.rs.
- **D-08:** Add `dashmap` as a workspace dependency.

### Error Handling
- **D-09:** Restructure error enums from scratch in all three manager crates — same treatment as Phase 2 leaf crates. Clean hierarchy, every fallible path typed.
- **D-10:** Same conversion pattern as Phase 2: mix of `#[from]` + manual `From` impls (carried forward from Phase 2 D-10).

### Carried Forward from Phase 2
- Parse at boundary, typed internally (Phase 2 D-02)
- `#[serde(flatten)]` with HashMap for extensible fields (Phase 2 D-03)
- Prefer `?` operator over `.expect()` (Phase 2 D-06)

### Agent's Discretion
- Order of the three manager crates (agents, channels, tools — likely dependency order)
- Which specific clones are necessary vs unnecessary (ownership analysis)
- Internal error enum variant organization

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Manager crate sources (files with Value usages)
- `crates/anyclaw-agents/src/manager.rs` — 107 clones, Value usages, largest file (3,708 lines)
- `crates/anyclaw-agents/src/connection.rs` — Arc<Mutex<HashMap>> pending requests, Value usages
- `crates/anyclaw-agents/src/slot.rs` — Value usages
- `crates/anyclaw-agents/src/platform_commands.rs` — Value usages
- `crates/anyclaw-channels/src/connection.rs` — Arc<Mutex<HashMap>> pending requests, Value usages
- `crates/anyclaw-channels/src/manager.rs` — Value usages
- `crates/anyclaw-channels/src/debug_http.rs` — Value usages
- `crates/anyclaw-tools/src/manager.rs` — Value usages
- `crates/anyclaw-tools/src/mcp_host.rs` — Value usages
- `crates/anyclaw-tools/src/external.rs` — Value usages
- `crates/anyclaw-tools/src/wasm_runner.rs` — Value usages
- `crates/anyclaw-tools/src/wasm_tool.rs` — Value usages

### Phase 2 outputs (typed foundations to build on)
- `crates/anyclaw-jsonrpc/src/types.rs` — JsonRpcMessage, RequestId, typed codec
- `crates/anyclaw-sdk-types/src/acp.rs` — Typed ACP wire types
- `crates/anyclaw-sdk-types/src/channel_event.rs` — Typed channel events
- `crates/anyclaw-core/src/agents_command.rs` — Typed agent commands

### Conventions
- `AGENTS.md` §Conventions — Error handling, unwrap rules, manager communication
- `AGENTS.md` §Anti-Patterns — No shared mutable state between managers, no cross-manager crate imports

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `JsonRpcMessage` from Phase 2 — managers should consume this directly instead of raw Value
- `RequestId` enum — replaces `Option<Value>` for JSON-RPC id fields
- Typed ACP structs from sdk-types — reuse for agent communication
- `dashmap` crate (to be added) — drop-in replacement for Arc<Mutex<HashMap>>

### Established Patterns
- Phase 2 codec shims in manager crates — these are the integration points to replace
- `ManagerHandle<C>` for inter-manager communication — don't change this pattern
- `CancellationToken` for shutdown — maintain existing pattern

### Integration Points
- Codec boundary: managers read from `NdJsonCodec` which now produces `JsonRpcMessage`
- Connection crates: pending_requests maps are the DashMap migration targets
- Error enums: `AgentsError`, `ChannelsError`, `ToolsError` — restructure in place

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard Rust patterns for the typed replacements and clone reduction.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 03-manager-crate-quality*
*Context gathered: 2026-04-14*
