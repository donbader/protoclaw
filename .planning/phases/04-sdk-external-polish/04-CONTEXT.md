# Phase 4: SDK & External Polish - Context

**Gathered:** 2026-04-15
**Status:** Ready for planning

<domain>
## Phase Boundary

SDK crates and external binaries have typed JSON, complete doc coverage, round-trip serde tests, and inline limitation comments. After this phase, the public-facing surface is polished.

</domain>

<decisions>
## Implementation Decisions

### Value Replacement in SDK/ext
- **D-01:** Same patterns as Phases 2-3 — typed structs, parse at boundary, `#[serde(flatten)]` for extras. No lighter touch for ext/ binaries.
- **D-02:** Remove all `#[allow(clippy::disallowed_types)]` in SDK crates (6 total) and ext/ (2 in telegram) after replacing Value usages.

### missing_docs Rollout
- **D-03:** Enable `#![warn(missing_docs)]` on ALL crates at once (SDK already have it, add to ~12 internal crates + ext binaries). Fix all warnings in one pass.
- **D-04:** Meaningful doc comments that explain WHY, not just WHAT. No "Returns the foo" stubs.

### Inline Limitation Comments
- **D-05:** Inline ALL known limitations from AGENTS.md anti-patterns (~12) and CONCERNS.md issues at the relevant code locations.
- **D-06:** Full explanation inline — no need to look up AGENTS.md. Self-contained comments at the code site.

### Serde Test Coverage
- **D-07:** Round-trip serialization tests for every public wire type across all SDK crates + ext binaries. Not just changed types.

### Carried Forward
- Parse at boundary, typed internally (Phase 2 D-02)
- `#[serde(flatten)]` with HashMap for extensible fields (Phase 2 D-03)
- Prefer `?` operator (Phase 2 D-06)
- Mix of `#[from]` + manual `From` for error conversions (Phase 2 D-10)

### Agent's Discretion
- Order of SDK crates vs ext binaries
- Exact wording of inline limitation comments
- Which doc comments need detailed explanations vs brief descriptions

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### SDK crate sources (files with Value usages)
- `crates/anyclaw-sdk-agent/src/adapter.rs` — Value usages
- `crates/anyclaw-sdk-agent/src/lib.rs` — 1 allow annotation
- `crates/anyclaw-sdk-channel/src/harness.rs` — Value usages
- `crates/anyclaw-sdk-channel/src/trait_def.rs` — Value in Channel trait
- `crates/anyclaw-sdk-channel/src/content.rs` — Value usages
- `crates/anyclaw-sdk-channel/src/error.rs` — Value usages
- `crates/anyclaw-sdk-tool/src/trait_def.rs` — Value in Tool trait
- `crates/anyclaw-sdk-tool/src/server.rs` — Value usages

### External binary sources
- `ext/agents/mock-agent/src/main.rs` — Value usages
- `ext/channels/debug-http/src/main.rs` — Value usages
- `ext/channels/telegram/src/channel.rs` — Value usages
- `ext/channels/telegram/src/deliver.rs` — Value usages
- `ext/channels/sdk-test-channel/src/main.rs` — Value usages
- `ext/tools/sdk-test-tool/src/main.rs` — Value usages
- `examples/01-fake-agent-telegram-bot/tools/system-info/src/main.rs` — Value usages

### Limitation sources
- `AGENTS.md` §Anti-Patterns — ~12 anti-patterns to inline
- `.planning/codebase/CONCERNS.md` — Known limitations with file references and line numbers

### Conventions
- `AGENTS.md` §Conventions — Doc comment standards, serde patterns

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- Phase 2 typed structs in sdk-types — SDK crates already consume these
- Phase 3 typed JsonRpcMessage pipeline — ext binaries may need to adapt
- Existing `#![warn(missing_docs)]` on 4 SDK crates — extend pattern to all

### Established Patterns
- SDK trait pattern: `Channel`, `Tool`, `AgentAdapter` — Value in trait signatures needs typed replacement
- Harness/server pattern handles JSON-RPC framing — Value at the harness boundary
- `#[non_exhaustive]` on SDK enums — maintain

### Integration Points
- SDK trait signatures are public API — changing Value to typed params is a breaking change (acceptable per PROJECT.md)
- ext/ binaries consume SDK crates — they'll need updating after SDK trait changes
- Channel trait `handle_unknown` returns `Result<Value, _>` — needs typed alternative

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 04-sdk-external-polish*
*Context gathered: 2026-04-15*
