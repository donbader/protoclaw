# Phase 5: Test Coverage & Verification - Context

**Gathered:** 2026-04-15
**Status:** Ready for planning

<domain>
## Phase Boundary

Fill all identified test gaps, add property-based testing for wire types, and verify coverage improvement over the Phase 1 baseline (75.17%).

</domain>

<decisions>
## Implementation Decisions

### Test Gap Coverage
- **D-01:** Write tests for all identified gaps: health.rs, sdk-agent error.rs, sdk-tool error.rs + lib.rs, agents acp_types.rs + backend.rs.
- **D-02:** All new tests use rstest 0.23 with BDD naming (`when_action_then_result` / `given_when_then`).

### Property-Based Testing
- **D-03:** Add proptest as dev-dependency. Write Arbitrary impls for all ACP and MCP wire types.
- **D-04:** Property tests verify: serialize → deserialize round-trip, no panics on arbitrary input, field constraints preserved.

### Coverage
- **D-05:** Run cargo-llvm-cov after all tests added. Compare against 75.17% baseline from Phase 1.

### Agent's Discretion
- Exact test cases per file (cover public API behavior, not implementation details)
- Which wire types get proptest Arbitrary impls (all public types in sdk-types at minimum)
- Test organization within each file

</decisions>

<canonical_refs>
## Canonical References

### Test gap targets (from CONCERNS.md)
- `crates/anyclaw-core/src/health.rs` — HealthSnapshot, HealthStatus untested
- `crates/anyclaw-sdk-agent/src/error.rs` — AgentSdkError untested
- `crates/anyclaw-sdk-tool/src/error.rs` — ToolSdkError untested
- `crates/anyclaw-sdk-tool/src/lib.rs` — untested
- `crates/anyclaw-agents/src/acp_types.rs` — ACP type re-exports untested
- `crates/anyclaw-agents/src/backend.rs` — Backend trait untested

### Conventions
- `AGENTS.md` §Conventions — rstest 0.23, BDD naming, fixture patterns

</canonical_refs>

<code_context>
## Existing Code Insights

### Established Patterns
- rstest fixtures with `fn given_*()` naming
- Parameterized tests with `#[case::label_name]`
- Phase 2 added round-trip serde tests — follow same pattern for proptest

</code_context>

<specifics>
## Specific Ideas

No specific requirements — agent discretion on test design.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>

---

*Phase: 05-test-coverage-verification*
*Context gathered: 2026-04-15*
