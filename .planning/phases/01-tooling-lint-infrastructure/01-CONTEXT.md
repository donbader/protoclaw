# Phase 1: Tooling & Lint Infrastructure - Context

**Gathered:** 2026-04-14
**Status:** Ready for planning

<domain>
## Phase Boundary

Set up automated quality enforcement across the entire anyclaw workspace. Workspace lints, clippy.toml, rustfmt.toml, expanded deny.toml, coverage baseline, and dead code removal. After this phase, no code change can regress lint, format, or dependency policy.

</domain>

<decisions>
## Implementation Decisions

### Clippy Configuration
- **D-01:** Moderate strictness — default clippy warnings plus workspace-level warn on key groups (clippy::unwrap_used, a pedantic subset). Not full pedantic.
- **D-02:** Warn locally, deny in CI — developers can iterate without blocking, CI enforces compliance.
- **D-03:** `clippy.toml` with `disallowed-types` banning `serde_json::Value` — new code cannot introduce untyped JSON. Existing usages are grandfathered with `#[allow]` until their phase cleans them up.

### Rustfmt Configuration
- **D-04:** Edition 2024 defaults — create `rustfmt.toml` to make it explicit and consistent. No custom rules beyond what the edition provides.

### deny.toml Expansion
- **D-05:** Deny known vulnerabilities, warn on unmaintained crates — security advisories are blocking, unmaintained is advisory.
- **D-06:** Ban duplicate versions of key dependencies (serde, tokio, etc.) to keep the dep tree clean. Not a blanket ban on all duplicates.
- **D-07:** Add `[sources]` section to restrict crate sources to crates.io.

### Coverage Strategy
- **D-08:** cargo-llvm-cov as the coverage tool — standard Rust source-based instrumentation.
- **D-09:** Enforce a coverage floor — set a minimum percentage and fail CI if it drops below. Exact floor to be determined after the baseline report (researcher/planner should propose a reasonable number based on the baseline).

### Dead Code Removal
- **D-10:** Remove all unused imports, stale modules, and unreachable branches. Standard `cargo clippy` + manual review.

### Agent's Discretion
- Exact clippy pedantic lint subset (which specific pedantic lints to enable)
- Exact coverage floor percentage (based on baseline measurement)
- Which key dependencies to include in deny.toml duplicate ban list
- `[sources]` section specifics in deny.toml

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing config
- `deny.toml` — Current deny config (licenses only, 20 lines). Expand in place.
- `Cargo.toml` (workspace root) — Add `[workspace.lints]` section. Currently has no lint config.

### Conventions
- `AGENTS.md` §Conventions — Error handling boundary, unwrap rules, tracing rules
- `.planning/codebase/CONVENTIONS.md` — Full convention analysis with code examples
- `.planning/codebase/CONCERNS.md` — Known issues including dead code and testing gaps

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- None — this phase creates new config files from scratch

### Established Patterns
- `deny.toml` exists with license checking — expand rather than replace
- `Cargo.toml` workspace root manages all deps centrally — add `[workspace.lints]` alongside
- No existing rustfmt.toml or clippy.toml — greenfield for both

### Integration Points
- `[workspace.lints]` in root `Cargo.toml` — each crate's `Cargo.toml` needs `[lints] workspace = true`
- CI pipeline (if exists) — add `cargo deny check`, `cargo fmt --check`, `cargo clippy` steps

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches for Rust 2024 workspace tooling.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 01-tooling-lint-infrastructure*
*Context gathered: 2026-04-14*
