# Domain Pitfalls

**Domain:** Rust workspace code quality improvement
**Researched:** 2026-04-14

## Critical Pitfalls

Mistakes that cause rewrites, regressions, or wasted effort.

### Pitfall 1: Typed JSON Replacement Breaks Wire Compatibility

**What goes wrong:** Replacing `serde_json::Value` with typed structs changes serialization behavior. Fields that were silently ignored now cause deserialization errors. Optional fields that defaulted to `null` now require explicit `#[serde(default)]` or `Option<T>`. Field ordering or casing changes break existing channel/agent subprocesses.

**Why it happens:** `Value` is maximally permissive — it accepts anything. Typed structs are strict by default. The gap between "what the code accepted" and "what the protocol spec says" only becomes visible when you add types.

**Consequences:** Agent or channel subprocesses that worked before now fail to communicate. Integration tests pass (they use the same types) but real deployments break because external binaries send slightly different JSON.

**Prevention:**
- Add `#[serde(deny_unknown_fields)]` only on types where you control both ends. Use `#[serde(default)]` liberally on incoming wire types.
- Write round-trip tests: serialize a struct, deserialize it back, assert equality.
- Write backward-compat tests: deserialize known JSON strings (captured from real traffic or existing tests) into the new types.
- Run integration tests with the actual ext/ binaries (mock-agent, debug-http, telegram) after every wire type change.

**Detection:** Integration test failures. Deserialization errors in logs. Agent/channel subprocesses failing to initialize.

### Pitfall 2: File Decomposition Breaks Test Isolation

**What goes wrong:** Extracting modules from the 3,708-line agents manager moves `#[cfg(test)] mod tests` blocks. Tests that relied on `pub(crate)` or private access to the parent module's internals now can't compile. Test helpers that were defined inline become inaccessible.

**Why it happens:** Rust's visibility rules are module-scoped. A test inside `manager.rs` can access private items in `manager.rs`. A test inside `fs_sandbox.rs` cannot access private items in `manager.rs`.

**Consequences:** Tests either break (won't compile) or must be rewritten to use only the public/pub(crate) API, which may require exposing internals that shouldn't be public.

**Prevention:**
- Before extracting, audit which tests access private items. Decide: make those items `pub(crate)`, or restructure the test to use the public API.
- Extract tests alongside their code. If `validate_fs_path` moves to `fs_sandbox.rs`, its tests move too.
- Use `#[cfg(test)]` test modules within each new file, not a separate test file.
- Run `cargo test -p anyclaw-agents` after every extraction step, not just at the end.

**Detection:** Compilation errors in test modules. `private item` visibility errors.

### Pitfall 3: Clone Removal Introduces Use-After-Move

**What goes wrong:** Removing a `.clone()` that seemed unnecessary causes a use-after-move error elsewhere in the function, or worse, changes the semantics (the original needed its own copy because the clone was mutated independently).

**Why it happens:** In async Rust, values are moved into futures/closures. A `.clone()` before an `async move` block is often the only way to use the value both inside and outside the block. The clone looks unnecessary when reading linearly but is required by the borrow checker for the async control flow.

**Consequences:** Compilation errors (best case). Subtle logic bugs if a shared reference replaces an independent copy that was being mutated (worst case — rare but possible).

**Prevention:**
- Never batch-remove clones. Remove one at a time, compile, run tests.
- For each clone, ask: "Is this value used after this point? Is it moved into an async block? Is the clone mutated independently?"
- `Arc::clone()` is almost always intentional — it's reference counting, not data copying. Leave these alone unless the Arc itself is unnecessary.
- `String::clone()` where a `&str` would suffice is the primary target. Look for functions that take `String` but could take `&str` or `impl AsRef<str>`.

**Detection:** Compiler errors (use-after-move). Test failures showing unexpected shared state.

## Moderate Pitfalls

### Pitfall 4: Error Enum Explosion

**What goes wrong:** Consolidating error handling leads to error enums with 15+ variants, many of which are only used in one function. The enum becomes a dumping ground rather than a meaningful type.

**Prevention:** One error enum per crate is the starting point. If a crate has distinct subsystems (e.g., agents manager has session lifecycle, fs sandbox, ACP protocol), consider one enum per subsystem with `#[from]` conversions at the crate boundary. But don't go below two uses per variant — if a variant is used once, it might be better as a `.map_err()` context string.

### Pitfall 5: Doc Comment Busywork

**What goes wrong:** Adding `#![warn(missing_docs)]` to every crate creates hundreds of warnings. The temptation is to write low-value docs like `/// The name` on a field called `name`, just to silence the lint.

**Prevention:** Write docs that answer "why" or "when", not "what". If the type/field name is self-explanatory, a one-liner is fine: `/// Agent's display name, used in logs and status output.` Skip the obvious, explain the non-obvious. For truly self-documenting items, `#[allow(missing_docs)]` with a reason is better than a useless doc comment.

### Pitfall 6: Refactoring Without a Safety Net

**What goes wrong:** Making structural changes to a crate that has poor test coverage. The refactor introduces a subtle bug that isn't caught because no test exercises that path.

**Prevention:** For each crate, check test coverage BEFORE refactoring. If coverage is thin, write characterization tests first (tests that capture current behavior, even if you're not sure it's correct). Then refactor. Then verify characterization tests still pass. This is especially important for the agents manager — it has integration test coverage but the unit test coverage for internal functions may be sparse.

### Pitfall 7: Serde Attribute Inconsistency Across Crate Boundaries

**What goes wrong:** A type defined in `anyclaw-sdk-types` uses `#[serde(rename_all = "camelCase")]`. A consuming crate manually constructs JSON with `snake_case` keys. After typing, the manual construction breaks because the struct expects `camelCase`.

**Prevention:** All wire types live in `anyclaw-sdk-types`. All config types live in `anyclaw-config`. Don't define serializable types in manager crates unless they're purely internal. When replacing `Value` construction with typed structs, always check the serde attributes on the target type.

## Minor Pitfalls

### Pitfall 8: Forgetting to Update AGENTS.md

**What goes wrong:** Module structure changes, new conventions are established, but AGENTS.md files aren't updated. Future contributors (or AI agents) work from stale context.

**Prevention:** Convention per AGENTS.md: "When code changes affect module structure, public APIs, conventions, or anti-patterns, update the relevant AGENTS.md file(s) in the same commit." Treat AGENTS.md updates as part of the definition of done for each phase.

### Pitfall 9: Over-Scoping Individual PRs

**What goes wrong:** A PR that touches 8 crates with typed JSON replacement, error handling fixes, dead code removal, and doc comments. Impossible to review meaningfully. Reviewer fatigue leads to rubber-stamping.

**Prevention:** One concern per PR, one crate per PR (or a small group of tightly coupled crates). "Dead code removal across workspace" is one PR. "Typed JSON in anyclaw-agents" is one PR. "Error handling in anyclaw-channels" is one PR.

### Pitfall 10: Treating `cargo clippy` as Sufficient

**What goes wrong:** Clippy catches syntactic issues but not semantic ones. Code passes clippy but has logic errors, missing error handling branches, or incorrect serde attributes.

**Prevention:** Clippy is necessary but not sufficient. Pair it with: tests (unit + integration), `cargo doc` (catches doc issues), manual review of each changed file. Don't treat "clippy clean" as "quality pass complete."

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| Dead code removal | Removing code that's used conditionally (cfg flags, feature gates) | Check all `#[cfg(...)]` attributes before deleting. Run tests on all feature combinations. |
| Typed JSON replacement | Wire compatibility breakage (Pitfall 1) | Backward-compat deserialization tests. Integration tests with real binaries. |
| Error handling consistency | Error enum explosion (Pitfall 4) | One enum per crate, max. Split only if crate has distinct subsystems. |
| File decomposition | Test visibility breakage (Pitfall 2) | Audit private access in tests before extracting. Move tests with their code. |
| Clone reduction | Use-after-move in async code (Pitfall 3) | One clone at a time. Compile after each removal. |
| Doc comments | Busywork docs (Pitfall 5) | "Why/when" not "what". Allow missing_docs on truly self-documenting items. |
| Test coverage | Refactoring untested code (Pitfall 6) | Write characterization tests before refactoring. |
| Serde patterns | Cross-crate attribute mismatch (Pitfall 7) | Wire types in sdk-types only. Check serde attrs on target types. |

## Sources

- `.planning/codebase/CONCERNS.md` — specific file sizes, clone counts, test gaps
- `.planning/codebase/CONVENTIONS.md` — serde patterns, module organization, error handling rules
- `AGENTS.md` — anti-patterns, AGENTS.md maintenance convention
- `.planning/PROJECT.md` — scope constraints (no behavioral changes)

---

*Pitfalls analysis: 2026-04-14*
