# Phase 6: File Decomposition - Context

**Gathered:** 2026-04-15
**Status:** Ready for planning

<domain>
## Phase Boundary

Break up the two largest files in the workspace: agents manager (3,708 lines) and supervisor (927 lines) into focused sub-modules. Public API unchanged, `pub(crate)` boundaries for extracted modules.

</domain>

<decisions>
## Implementation Decisions

### Agents Manager Decomposition
- **D-01:** Extract logical groups from the 3,708-line manager.rs into separate modules: fs_sandbox, session_recovery, tool_events, and the main run loop.
- **D-02:** Extracted modules use `pub(crate)` visibility — public API surface of the crate stays unchanged.
- **D-03:** No file should exceed ~500 lines after decomposition.

### Supervisor Decomposition
- **D-04:** Extract signal handling, shutdown orchestration, and health monitoring from the 927-line lib.rs into sub-modules.
- **D-05:** Same `pub(crate)` boundary pattern as agents manager.

### Validation
- **D-06:** All existing tests must pass without modification after decomposition — this is purely structural.

### Agent's Discretion
- Exact module boundaries (where to cut)
- Module naming
- Whether to extract helper functions or keep them inline

</decisions>

<canonical_refs>
## Canonical References

### Decomposition targets
- `crates/anyclaw-agents/src/manager.rs` — 3,708 lines, primary target
- `crates/anyclaw-supervisor/src/lib.rs` — 927 lines, secondary target

### Conventions
- `AGENTS.md` §Conventions — Flat lib.rs with pub mod, no mod.rs files
- `AGENTS.md` §Anti-Patterns — Don't change MANAGER_ORDER, don't call run() without start()

</canonical_refs>

<code_context>
## Existing Code Insights

### Established Patterns
- Flat `lib.rs` with `pub mod` + `pub use` — new modules added the same way
- No `mod.rs` files — convention must be maintained

### Integration Points
- agents manager public API used by supervisor and integration tests
- supervisor public API used by main.rs

</code_context>

<specifics>
## Specific Ideas

No specific requirements — standard module extraction.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>

---

*Phase: 06-file-decomposition*
*Context gathered: 2026-04-15*
