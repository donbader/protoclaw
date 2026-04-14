# Feature Landscape

**Domain:** Rust workspace code quality improvement (12 crates + ext binaries)
**Researched:** 2026-04-14

## Table Stakes

Features users (contributors, reviewers, future-you) expect from a quality pass. Missing any of these and the effort feels incomplete.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Typed JSON replacement | `serde_json::Value` manipulation is the antithesis of Rust's type system. Untyped JSON hides bugs at compile time that surface at runtime. | High | Touches every crate that handles ACP/MCP wire data. Must define structs, update serialization, and fix all call sites. Highest-effort item. |
| Error handling consistency | Mixed `thiserror`/`anyhow` boundaries erode trust in error propagation. Convention exists but may not be uniformly enforced. | Medium | Audit every crate for `anyhow` leaking into library code. Verify each manager crate has a proper error enum. Mechanical but wide-reaching. |
| Dead code removal | Dead code signals abandonment. Unused imports, stale modules, unreachable branches — all noise that slows comprehension. | Low | `cargo clippy --workspace` + `#[warn(dead_code)]` catches most. Manual review for conditionally-dead paths. Quick wins. |
| Zero clippy warnings | Clippy clean is the baseline for any Rust project that takes quality seriously. Non-negotiable. | Low | Already reportedly clean per CONVENTIONS.md, but verify after all other changes. May surface new warnings from refactoring. |
| Consistent serde patterns | SDK types use `camelCase` rename, but internal crates may not follow a uniform pattern. Inconsistency causes subtle serialization bugs. | Medium | Audit all `#[serde(...)]` attributes. Ensure SDK wire types are `camelCase`, config types are `snake_case`, and round-trip tests exist. |
| Remove unnecessary `.clone()` calls | 103 clones in one file signals structural issues. Unnecessary clones waste allocations and obscure ownership semantics. | Medium | Requires understanding each clone's purpose. Some are necessary (Arc, async moves). Target: eliminate clones where borrowing or `&str` suffices. |
| Missing doc comments on public items | Public API without docs is incomplete API. SDK crates already enforce `#![warn(missing_docs)]`; internal crates should match. | Medium | Add `#![warn(missing_docs)]` to all crates. Write meaningful doc comments (not just "Returns the foo"). Tedious but straightforward. |
| Test coverage for untested files | Files identified in CONCERNS.md with zero coverage: `health.rs`, SDK error types, `acp_types.rs`, `backend.rs`. Untested code is untrustworthy code. | Medium | Write rstest-based tests following BDD naming. Focus on public API behavior, not implementation details. |

## Differentiators

Features that go beyond basic cleanup. Not expected in a minimal quality pass, but elevate the codebase significantly.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| File decomposition (agents manager) | Breaking the 3,708-line `manager.rs` into focused modules (`fs_sandbox.rs`, `session_recovery.rs`, `tool_events.rs`, run loop) makes each concern independently testable and reviewable. | High | Highest-risk item. Must preserve public API surface. Extract modules behind `pub(crate)` boundaries. Requires careful integration test validation. |
| Arc<Mutex> replacement in connection crates | Replace `Arc<Mutex<HashMap<u64, oneshot::Sender>>>` with `DashMap` or a lock-free pattern. Reduces contention risk and modernizes the pattern. | Medium | Low urgency per CONCERNS.md (locks are short-lived), but cleaner code. `DashMap` is the standard Rust ecosystem answer here. |
| Inline documentation of known limitations | Zero TODO/FIXME comments means known limitations live only in AGENTS.md. Adding targeted inline comments (`// LIMITATION: single-agent routing`) helps developers encounter context where they need it. | Low | Quick pass through CONCERNS.md items, adding inline comments at the relevant code locations. |
| Property-based testing for serialization | Round-trip serde tests with `proptest` or `quickcheck` for all wire types. Catches edge cases that hand-written tests miss (empty strings, unicode, large payloads). | Medium | Add `proptest` as dev-dependency. Write `Arbitrary` impls for key types. High value for ACP/MCP wire types specifically. |
| Workspace-wide `#![warn(missing_docs)]` | Enforcing missing_docs on internal crates (not just SDK crates) prevents documentation rot going forward. | Low | Flip the lint, then fix all warnings. Mechanical but creates a quality ratchet. |
| Supervisor file decomposition | `supervisor/src/lib.rs` at 927 lines is the second-largest file. Extracting signal handling, shutdown orchestration, and health monitoring into sub-modules improves navigability. | Medium | Lower priority than agents manager decomposition. Same approach: extract behind `pub(crate)`. |

## Anti-Features

Things to deliberately NOT do during this quality pass. Tempting but counterproductive.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Performance optimization | Quality pass is about correctness and clarity, not speed. Premature optimization obscures intent and introduces risk. | Note performance opportunities in comments/issues. Address in a dedicated performance milestone. |
| Dependency version upgrades | Upgrading deps during a refactoring pass doubles the risk surface. Version bumps can introduce breaking changes that mask quality regressions. | Pin current versions. Upgrade in a separate milestone unless a refactor strictly requires it. |
| Feature-gating wasmtime/bollard | Explicitly out of scope per PROJECT.md. Cargo feature flags are a feature change, not a quality change. | Document as a future milestone. Don't mix concerns. |
| Rewriting the polling workaround | `poll_channels()` with 1ms timeout is a known architectural limitation. Replacing it with `FuturesUnordered`/`StreamMap` is a behavioral change, not a quality fix. | Document the limitation inline. Address in an architecture milestone. |
| Adding rate limiting | New capability, not quality improvement. Mixing new features into a quality pass muddies the scope. | File as a separate issue/milestone. |
| Changing public API signatures for aesthetics | Renaming types or restructuring public APIs "because it reads better" breaks downstream consumers for no functional gain. | Only change APIs where the quality fix requires it (e.g., replacing `Value` with a typed struct). |
| Blanket `#[allow(...)]` suppressions | Suppressing warnings instead of fixing them defeats the purpose. Each `#[allow]` should have a justification comment. | Fix the underlying issue. If suppression is truly needed, add a `// REASON:` comment. |
| Refactoring cross-manager communication | Changing mpsc patterns or ManagerHandle design is architectural work. Quality pass should enforce the existing pattern consistently, not redesign it. | Verify all managers follow the convention. Fix deviations. Don't redesign. |

## Feature Dependencies

```
Dead code removal → (unblocks) Clippy zero warnings
    (removing dead code may resolve clippy warnings)

Typed JSON replacement → (unblocks) Consistent serde patterns
    (new typed structs need correct serde attributes from the start)

Typed JSON replacement → (unblocks) Property-based testing for serialization
    (can't property-test serde_json::Value — need typed structs first)

Error handling consistency → (unblocks) Doc comments on public items
    (error types may change during consistency pass — document after stabilized)

File decomposition (agents manager) → (unblocks) Test coverage for untested files
    (extracted modules are easier to test in isolation)

File decomposition (agents manager) → (unblocks) Clone reduction
    (smaller modules make ownership flow visible — easier to spot unnecessary clones)
```

## MVP Recommendation

A quality pass that stops before these items is incomplete:

1. **Dead code removal** — lowest effort, highest signal-to-noise improvement, unblocks clippy
2. **Zero clippy warnings** — baseline hygiene, catches issues early
3. **Error handling consistency** — enforces the existing convention uniformly
4. **Typed JSON replacement** — the single highest-value change; makes the type system work for you
5. **Consistent serde patterns** — natural follow-on from typed JSON; ensures wire format correctness
6. **Remove unnecessary clones** — mechanical cleanup that improves clarity
7. **Test coverage for untested files** — validates everything above actually works
8. **Doc comments on public items** — documents the now-stable API surface

Defer to differentiator phase:
- **File decomposition**: High risk, high reward, but the quality pass delivers value without it. Do it when the codebase is otherwise clean so the diff is purely structural.
- **Property-based testing**: Valuable but not blocking. Add after typed structs are stable.
- **Arc<Mutex> replacement**: Low urgency per CONCERNS.md. Nice-to-have.

## Complexity Budget

| Category | Items | Estimated Effort | Risk |
|----------|-------|-----------------|------|
| Table stakes (all 8) | Dead code, clippy, errors, typed JSON, serde, clones, tests, docs | ~70% of milestone effort | Medium — typed JSON replacement is the riskiest |
| Differentiators (all 6) | File decomposition ×2, Arc replacement, inline docs, proptest, workspace lints | ~30% of milestone effort | High — file decomposition carries regression risk |

## Sources

- `.planning/codebase/CONCERNS.md` — specific issues with file references and line numbers
- `.planning/codebase/CONVENTIONS.md` — established patterns to enforce
- `.planning/PROJECT.md` — project scope and constraints
- `AGENTS.md` — anti-patterns and conventions

---

*Feature landscape analysis: 2026-04-14*
