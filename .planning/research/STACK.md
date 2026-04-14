# Technology Stack: Rust Code Quality Toolchain

**Project:** Anyclaw — Code Quality Milestone
**Researched:** 2026-04-14
**Overall Confidence:** HIGH (tools are mature, well-documented, widely adopted)

## Current State

The workspace (edition 2024, rust-version 1.94) has:
- `thiserror = "2"` and `anyhow = "1"` — already in workspace deps
- `rstest = "0.26"` — already in workspace deps
- `deny.toml` — licenses only, no advisories/bans/sources checks
- No `clippy.toml` or `.clippy.toml`
- No `rustfmt.toml`
- No `[workspace.lints]` in root `Cargo.toml`
- `#![warn(missing_docs)]` only on 4 SDK crates
- 259 `serde_json::Value` usages across the workspace
- Zero clippy configuration beyond defaults

---

## Recommended Stack

### Static Analysis & Linting

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| clippy (workspace lints) | built-in | Lint enforcement | Workspace-level `[lints]` table (stable since Rust 1.74) centralizes lint policy. No more per-crate `#![warn(...)]` scattered across lib.rs files. Single source of truth. | HIGH |
| clippy.toml | built-in | Clippy configuration | Tunes thresholds (cognitive complexity, too-many-args, large-error-threshold) and enables `disallowed_methods`/`disallowed_types` to ban `serde_json::Value` in new code. | HIGH |
| rustfmt (rustfmt.toml) | built-in | Formatting | Consistent formatting across 12+ crates. Even with defaults, an explicit `rustfmt.toml` signals intent and prevents drift. | HIGH |

#### Workspace Lints Configuration

Add to root `Cargo.toml`:

```toml
[workspace.lints.rust]
unsafe_code = "forbid"
missing_debug_implementations = "warn"
unreachable_pub = "warn"
unused_qualifications = "warn"

[workspace.lints.clippy]
# Correctness & safety
unwrap_used = "warn"           # Enforces .expect("reason") or ? over .unwrap()
panic = "warn"                 # Flags panic!() in library code
# Code quality
clone_on_ref_ptr = "warn"      # Flags .clone() on Arc/Rc — prefer Arc::clone(&x)
needless_pass_by_value = "warn"
redundant_closure_for_method_calls = "warn"
manual_let_else = "warn"
uninlined_format_args = "warn"
# Style consistency
module_name_repetitions = "allow"  # Too noisy for this codebase's naming style
```

Each crate opts in via:
```toml
[lints]
workspace = true
```

**Rationale:** Workspace lints replaced the old approach of `#![warn(...)]` in every lib.rs. They're the standard since Rust 1.74, composable, and overridable per-crate when needed. The SDK crates can layer additional `missing_docs = "warn"` on top.

#### clippy.toml Configuration

```toml
# Workspace root clippy.toml
cognitive-complexity-threshold = 40   # Default 25 is too aggressive for manager.rs during refactor
too-many-arguments-threshold = 8      # Default 7, slight headroom for manager methods
too-many-lines-threshold = 150        # Flag functions over 150 lines
large-error-threshold = 256           # Default 128, some error enums carry context strings
avoid-breaking-exported-api = false   # We're allowing breaking changes this milestone

# Ban serde_json::Value in new code (existing uses get cleaned up manually)
disallowed-types = [
    { path = "serde_json::Value", reason = "Use typed structs instead. See ARCHITECTURE.md for patterns." }
]

# Ban bare unwrap in production
disallowed-methods = [
    { path = "core::result::Result::unwrap", reason = "Use .expect(\"reason\") or ? operator" },
    { path = "core::option::Option::unwrap", reason = "Use .expect(\"reason\") or ? operator" },
]

# Allow unwrap/expect in tests
allow-unwrap-in-tests = true
allow-expect-in-tests = true
allow-dbg-in-tests = true
```

**Rationale:** `disallowed-types` is the enforcement mechanism for the "no Value soup" goal. It catches new introductions at lint time. `disallowed-methods` enforces the existing `.expect("reason")` convention mechanically rather than by code review alone.

### Dependency Auditing & Supply Chain

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| cargo-deny | 0.19+ | License, advisory, ban, source checks | Already partially configured (licenses only). Needs advisories and bans sections. The standard tool — 2.3k stars, actively maintained by Embark Studios. Covers what cargo-audit does plus license/ban/source checks in one tool. | HIGH |
| cargo-audit | 0.21+ | Security advisory scanning (alternative) | Overlaps with cargo-deny's advisories check. Only needed if you want a standalone audit step separate from deny. For this project, cargo-deny covers it. Skip cargo-audit. | HIGH |

#### Expand deny.toml

The current `deny.toml` only checks licenses. Add:

```toml
[advisories]
vulnerability = "deny"
unmaintained = "warn"
yanked = "warn"
notice = "warn"

[bans]
multiple-versions = "warn"
wildcards = "deny"
highlight = "all"
# Allow known duplicates that are hard to resolve
allow-wildcard-paths = []

[sources]
unknown-registry = "deny"
unknown-git = "deny"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
allow-git = []
```

**Rationale:** `cargo deny check` should be a single command that validates licenses, known vulnerabilities, duplicate crate versions, and crate sources. The existing config only does 25% of what cargo-deny offers. The advisories check uses the RustSec advisory database — same as cargo-audit but integrated.

### Typed JSON Patterns (Replacing serde_json::Value)

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| serde + derive | 1.x | Typed deserialization | Already a dependency. The fix isn't a new tool — it's using what's already there correctly. Every `serde_json::Value` should become a `#[derive(Deserialize, Serialize)]` struct. | HIGH |
| serde(flatten) | built-in | Catch-all for unknown fields | For protocol extensibility (ACP `extra` fields), use `#[serde(flatten)] extra: HashMap<String, serde_json::Value>` on a typed struct. This preserves forward-compat while typing known fields. | HIGH |
| serde(deny_unknown_fields) | built-in | Strict parsing for internal types | For internal config/message types where unknown fields indicate bugs, not extensibility. | HIGH |

#### Pattern: Replace Value Soup with Typed Structs

Before (current — 259 occurrences):
```rust
fn handle_session_update(&mut self, params: serde_json::Value) {
    let session_id = params.get("sessionId").and_then(|v| v.as_str()).unwrap_or("");
    let status = params.get("status").and_then(|v| v.as_str());
    // ... more .get().and_then() chains
}
```

After:
```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionUpdateParams {
    session_id: String,
    #[serde(default)]
    status: Option<String>,
}

fn handle_session_update(&mut self, params: SessionUpdateParams) {
    // Fields are typed, validated at deserialization boundary
}
```

#### Pattern: Protocol Messages with Extension Points

For ACP wire types where the protocol may add fields:
```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AcpRequest {
    method: String,
    params: AcpParams,  // typed enum
    #[serde(flatten)]
    extra: HashMap<String, serde_json::Value>,  // forward-compat
}
```

#### Pattern: JSON-RPC Request/Response Typing

The JSON-RPC layer should parse into typed envelopes early:
```rust
#[derive(Debug, Deserialize)]
struct JsonRpcRequest<P> {
    jsonrpc: String,
    id: Option<JsonRpcId>,
    method: String,
    params: P,
}
```

Where `P` is deserialized based on `method` using a dispatch enum or `serde_json::from_value` at the boundary — not passed around as `Value`.

**Key principle:** Parse `Value` into typed structs at system boundaries (connection layer). Internal code never touches `Value`. The 259 usages should collapse to ~20-30 at deserialization boundaries only.

**Confidence:** HIGH — this is standard serde practice, well-documented, zero new dependencies.

### Error Handling Patterns

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| thiserror | 2.x | Typed error enums in library crates | Already used. Version 2 (current) dropped MSRV constraints and simplified the derive. The project convention is correct — enforce it consistently. | HIGH |
| anyhow | 1.x | Ergonomic errors at entry points | Already used. Correctly scoped to main.rs, supervisor, init, status. No changes needed to the boundary rule. | HIGH |

#### Pattern: Error Composition Across Crate Boundaries

The project already has per-crate error enums (`AgentsError`, `ChannelsError`, etc.). The pattern to enforce:

```rust
// In crates/anyclaw-agents/src/error.rs
#[derive(Debug, thiserror::Error)]
pub enum AgentsError {
    #[error("ACP protocol error: {0}")]
    Protocol(#[from] AcpError),

    #[error("connection error: {0}")]
    Connection(#[from] ConnectionError),

    #[error("session store error: {0}")]
    SessionStore(#[from] SessionStoreError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    // Context-carrying variants for domain-specific failures
    #[error("agent {agent} failed to initialize: {reason}")]
    InitFailed { agent: String, reason: String },
}
```

#### Pattern: Error Context Without anyhow in Libraries

When library code needs to add context to errors:
```rust
// Use map_err with thiserror variants, not .context()
store.load(key).map_err(|e| AgentsError::SessionStore(e))?;

// Or for string context on IO errors:
std::fs::read(&path).map_err(|e| AgentsError::FsError {
    path: path.display().to_string(),
    source: e,
})?;
```

#### Anti-Pattern: Don't Use `Box<dyn Error>`

The codebase correctly avoids this. Keep it that way. `Box<dyn Error>` loses type information and makes matching impossible. Every crate boundary should have a concrete error enum.

**Confidence:** HIGH — the project already follows this pattern, just needs consistent enforcement.

### Testing Tools

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| rstest | 0.26 | Test framework with fixtures & parameterization | Already used. BDD naming convention established. No change needed — just enforce consistently across all crates. | HIGH |
| cargo-llvm-cov | 0.8.5 | LLVM source-based code coverage | The standard Rust coverage tool. 1.3k stars, actively maintained. Uses rustc's `-C instrument-coverage` — precise line/region coverage. Supports workspace-level reports, lcov output for IDE integration, and `--fail-under-lines` for CI gates. Preferred over cargo-tarpaulin (which uses ptrace, less accurate, Linux-only). | HIGH |
| cargo-nextest | latest | Parallel test runner | Faster test execution via per-test process isolation. Not required for this milestone but worth noting — cargo-llvm-cov integrates with it via `cargo llvm-cov nextest`. Consider for CI speed later. | MEDIUM |

#### Coverage Strategy

```bash
# Generate HTML report locally
cargo llvm-cov --workspace --html --open

# Generate lcov for IDE integration (VS Code Coverage Gutters)
cargo llvm-cov --workspace --lcov --output-path lcov.info

# CI gate: fail if line coverage drops below threshold
cargo llvm-cov --workspace --fail-under-lines 70

# Exclude test code and integration tests from coverage report
cargo llvm-cov --workspace --ignore-filename-regex '(tests/|test_helpers/|mock[-_])'
```

#### Coverage Exclusion for Untestable Code

For code that legitimately can't be covered (e.g., signal handlers, panic paths):
```rust
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

#[cfg_attr(coverage_nightly, coverage(off))]
fn handle_signal() { /* ... */ }
```

Add to workspace `Cargo.toml` to suppress the `unexpected_cfgs` warning:
```toml
[workspace.lints.rust]
unexpected_cfgs = { level = "warn", check-cfg = ['cfg(coverage,coverage_nightly)'] }
```

#### Testing Patterns to Enforce

The project uses rstest with BDD naming. Enforce these consistently:

```rust
// Fixture pattern — reusable test setup
#[fixture]
fn given_agent_config() -> AgentConfig { /* ... */ }

// Parameterized — test multiple cases
#[rstest]
#[case::empty_params("{}")]
#[case::null_session(r#"{"sessionId": null}"#)]
fn when_parsing_invalid_params_then_returns_error(#[case] input: &str) { /* ... */ }

// Async test
#[rstest]
#[tokio::test]
async fn when_agent_crashes_then_session_recovers(given_agent_config: AgentConfig) { /* ... */ }
```

**Property testing (proptest/quickcheck):** Not recommended for this milestone. The codebase is protocol-heavy with specific message shapes — parameterized rstest cases are more readable and maintainable than property generators for JSON-RPC messages. Revisit if adding fuzzing later.

**Confidence:** HIGH for rstest (already used), HIGH for cargo-llvm-cov (standard tool, verified current).

### Code Formatting

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| rustfmt | built-in | Consistent formatting | No config exists today. An explicit `rustfmt.toml` prevents drift and documents style decisions. Use stable options only — nightly-only rustfmt options are fragile across toolchain updates. | HIGH |

#### rustfmt.toml Configuration

```toml
edition = "2024"
max_width = 100
use_field_init_shorthand = true
use_try_shorthand = true
```

Keep it minimal. The defaults are good. The main value is having the file exist so `cargo fmt --check` in CI has a consistent baseline. Avoid nightly-only options like `imports_granularity` or `group_imports` — they require `rustfmt +nightly` and break on toolchain updates.

**Confidence:** HIGH — rustfmt is built-in, stable, universally adopted.

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Coverage | cargo-llvm-cov | cargo-tarpaulin | Tarpaulin uses ptrace (Linux-only, less accurate). llvm-cov uses rustc's native instrumentation — works on macOS, more precise line/region/branch coverage. |
| Coverage | cargo-llvm-cov | grcov | grcov is Mozilla's tool, also LLVM-based, but cargo-llvm-cov wraps the same LLVM tooling with better cargo integration and simpler CLI. |
| Dep auditing | cargo-deny | cargo-audit standalone | cargo-deny subsumes cargo-audit's advisory checking and adds license/ban/source checks. One tool instead of two. |
| Linting | workspace [lints] table | Per-crate #![warn(...)] | Workspace lints are centralized, composable, and the standard since Rust 1.74. Per-crate attributes scatter policy across 12+ lib.rs files. |
| Error handling | thiserror 2 | error-stack (by hash) | error-stack adds report-style error chains with SpanTrace. Overkill for this project — thiserror's `#[from]` and `#[source]` provide sufficient error chaining. error-stack also has a heavier API surface. |
| Error handling | thiserror 2 | snafu | snafu is a valid alternative with context selectors, but thiserror is already established in this codebase and has 5x the adoption (574k dependents). Switching would be churn for no gain. |
| Test framework | rstest | test-case | test-case is lighter but rstest's fixtures + parameterization + async support are already in use. Switching would break all existing tests. |
| Property testing | Skip for now | proptest / quickcheck | Protocol message shapes are better tested with explicit parameterized cases. Property testing shines for algorithmic code — this codebase is mostly I/O orchestration. |
| JSON typing | serde derive | typify / schemars | Code generation from JSON Schema is useful for API-first design. This project's types are Rust-first — hand-written serde structs are simpler and more maintainable. |

## What NOT to Use

| Tool/Practice | Why Not |
|---------------|---------|
| `cargo-tarpaulin` | Linux-only (ptrace-based), less accurate than LLVM instrumentation, doesn't work on macOS where this project develops. |
| `cargo-audit` standalone | Redundant with cargo-deny's advisories check. One tool is better than two. |
| `proptest` / `quickcheck` | Not this milestone. The codebase is protocol orchestration, not algorithmic — parameterized rstest is more readable. |
| Nightly-only rustfmt options | `imports_granularity`, `group_imports`, `wrap_comments` require nightly rustfmt. They break when switching toolchains and add CI complexity. Stick to stable. |
| `#![deny(...)]` in source | Use `[workspace.lints]` in Cargo.toml instead. Source-level deny attributes can't be overridden per-crate without `#![allow(...)]` which defeats the purpose. Cargo.toml lints are composable. |
| `clippy::pedantic` as a group | Too noisy. Cherry-pick useful pedantic lints individually (like `uninlined_format_args`, `manual_let_else`) rather than enabling the whole group and suppressing 30+ false positives. |
| `error-stack` | Adds report-style error chains with SpanTrace. Heavy API, overkill for this project's error handling needs. thiserror + anyhow boundary is sufficient. |
| `snafu` | Valid crate but switching from thiserror would be pure churn. thiserror has 5x adoption and is already established here. |
| `Box<dyn Error>` | Loses type information. The project correctly uses typed error enums — don't regress. |
| `Arc<Mutex<>>` across managers | Already an anti-pattern in the codebase. The quality pass should audit existing `Arc<Mutex<>>` in connection crates and consider `dashmap` or lock-free alternatives only if contention is measured. Don't add new shared mutable state. |

## Installation

```bash
# Coverage tool (one-time install)
cargo install cargo-llvm-cov --locked

# Dependency auditing (one-time install, if not already present)
cargo install cargo-deny --locked

# No install needed for:
# - clippy (ships with rustup)
# - rustfmt (ships with rustup)
# - rstest (cargo dependency, already in workspace)
# - thiserror/anyhow (cargo dependencies, already in workspace)
```

### Workspace Config Files to Create

| File | Purpose |
|------|---------|
| `clippy.toml` | Clippy thresholds, disallowed types/methods |
| `rustfmt.toml` | Formatting rules (minimal, stable-only) |
| Root `Cargo.toml` `[workspace.lints]` | Centralized lint policy |
| Expand `deny.toml` | Add advisories, bans, sources sections |

### Per-Crate Changes

Every crate's `Cargo.toml` needs:
```toml
[lints]
workspace = true
```

SDK crates additionally keep their existing `#![warn(missing_docs)]` or move it to a per-crate lint override:
```toml
[lints]
workspace = true

[lints.rust]
missing_docs = "warn"
```

## Sources

| Source | What | Confidence |
|--------|------|------------|
| https://github.com/taiki-e/cargo-llvm-cov (v0.8.5, Mar 2026) | Coverage tool — features, CLI, CI integration, exclusion patterns | HIGH |
| https://github.com/EmbarkStudios/cargo-deny (v0.19.1, Apr 2026) | Dependency linting — licenses, advisories, bans, sources | HIGH |
| https://github.com/dtolnay/thiserror (v2.0.18, Jan 2026) | Error derive macro — patterns, #[from], #[source], transparent | HIGH |
| https://doc.rust-lang.org/clippy/lint_configuration.html | Clippy config options — disallowed-types, disallowed-methods, thresholds | HIGH |
| Workspace Cargo.toml (local) | Current dependency versions, edition 2024, rust-version 1.94 | HIGH |
| deny.toml (local) | Current config — licenses only, no advisories/bans/sources | HIGH |
| Codebase grep: 259 serde_json::Value usages | Scope of typed JSON migration work | HIGH |
| Codebase grep: 4 crates with #![warn(missing_docs)] | Current lint coverage gaps | HIGH |
| Rust Reference: workspace lints (stable since 1.74) | [workspace.lints] table specification | HIGH |

---

*Research completed: 2026-04-14*
