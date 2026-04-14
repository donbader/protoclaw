# Architecture Patterns

**Domain:** Rust workspace code quality improvement
**Researched:** 2026-04-14

## Current Architecture (Preserve)

The quality pass must not alter the system architecture. Documenting the patterns to preserve and the patterns to apply during refactoring.

### Component Boundaries (Existing — Do Not Change)

| Component | Responsibility | Communicates With |
|-----------|---------------|-------------------|
| Supervisor | Boot order, shutdown, health monitoring | All managers (via ManagerHandle) |
| AgentsManager | ACP protocol, session lifecycle, crash recovery, fs sandbox | ToolsManager (via command), ChannelsManager (via command) |
| ChannelsManager | Channel subprocess routing, session queuing | AgentsManager (via command) |
| ToolsManager | MCP host, WASM sandbox, external tool processes | AgentsManager (via command) |
| SDK crates | External implementor interfaces | JSON-RPC over stdio |

### Data Flow (Existing — Do Not Change)

```
Channel subprocess → ChannelsManager → AgentsManager → Agent subprocess
                                                      ↕
                                              ToolsManager → Tool subprocess
```

All cross-manager communication via `tokio::sync::mpsc` through `ManagerHandle<C>`. This is load-bearing. Do not introduce shared state.

## Patterns to Follow During Quality Pass

### Pattern 1: Module Extraction (for file decomposition)

**What:** Extract logical concerns from large files into sibling modules, keeping the public API on the original module.
**When:** File exceeds ~500 lines with multiple distinct concerns.
**Why:** Preserves the flat `lib.rs` convention while reducing per-file complexity.

```rust
// BEFORE: crates/anyclaw-agents/src/manager.rs (3,708 lines)
// Contains: session lifecycle, crash recovery, fs sandbox, tool events, run loop

// AFTER: crates/anyclaw-agents/src/lib.rs
pub mod manager;          // Public API, run loop (~800 lines)
pub mod fs_sandbox;       // validate_fs_path, validate_fs_write_path
pub mod session_recovery; // session/resume → session/load → fresh fallback
pub mod tool_events;      // Tool event normalization

// manager.rs uses:
use crate::fs_sandbox::validate_fs_path;
use crate::session_recovery::recover_session;
```

**Rules:**
- New modules are `pub(crate)` unless they define public API
- No `mod.rs` files — flat sibling modules
- Tests move with their code into the new module
- Integration tests should pass unchanged (API surface preserved)

### Pattern 2: Typed JSON Replacement

**What:** Replace `serde_json::Value` manipulation with `#[derive(Serialize, Deserialize)]` structs.
**When:** Any code that does `.get("field")`, `.as_str()`, `.as_object()` on a `Value`.
**Why:** Compile-time type checking, better error messages, self-documenting code.

```rust
// BEFORE
let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
let tools: Vec<Value> = params.get("tools").and_then(|v| v.as_array()).cloned().unwrap_or_default();

// AFTER
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitializeParams {
    name: String,
    #[serde(default)]
    tools: Vec<ToolDefinition>,
}

let params: InitializeParams = serde_json::from_value(raw_params)?;
```

**Rules:**
- New types go in the crate that owns the protocol (e.g., `anyclaw-sdk-types` for ACP wire types)
- Use `#[serde(default)]` for optional fields, not `Option<T>` unless absence is semantically meaningful
- Add round-trip serialization tests for every new type
- Use `#[serde(rename_all = "camelCase")]` for wire types, `snake_case` for config types

### Pattern 3: Error Enum Hygiene

**What:** Ensure every library crate has a focused error enum with `#[from]` conversions for its dependencies.
**When:** Auditing error handling consistency.
**Why:** Typed errors enable callers to match on specific failure modes. `anyhow` erases this.

```rust
// Each crate's error.rs should follow this shape:
#[derive(Debug, thiserror::Error)]
pub enum CrateNameError {
    #[error("specific failure: {0}")]
    SpecificCase(String),
    
    #[error(transparent)]
    Io(#[from] std::io::Error),
    
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
```

**Rules:**
- One error enum per crate (or per major subsystem if the crate is large)
- `#[error(transparent)]` for wrapped errors that should display their inner message
- `#[error("context: {0}")]` for errors that add context
- No `anyhow::Error` in any `From` impl in library crates

## Anti-Patterns to Avoid During Quality Pass

### Anti-Pattern 1: Over-Extraction
**What:** Breaking files into too many tiny modules (one function per file).
**Why bad:** Trades one navigation problem for another. 20 files with 50 lines each is worse than 3 files with 300 lines.
**Instead:** Extract when there's a clear conceptual boundary (fs sandbox, session recovery), not just because a function is long.

### Anti-Pattern 2: Type Proliferation
**What:** Creating a new struct for every JSON object, including internal intermediaries.
**Why bad:** Dozens of single-use types clutter the namespace and make the code harder to follow than the `Value` it replaced.
**Instead:** Type the protocol boundaries (wire types in/out). Internal transformations can use tuples or local types.

### Anti-Pattern 3: Changing Behavior While Refactoring
**What:** "While I'm in here, let me also fix this edge case / add this feature."
**Why bad:** Mixes behavioral changes with structural changes. If tests break, you can't tell if it's the refactor or the behavior change.
**Instead:** Pure structural refactors in one commit. Behavioral fixes in a separate commit with their own tests.

### Anti-Pattern 4: Blanket `#[allow(unused)]` During Transition
**What:** Suppressing warnings temporarily "until the refactor is done."
**Why bad:** Temporary suppressions become permanent. They hide real issues introduced during the refactor.
**Instead:** Fix warnings as you go. If something is genuinely unused after refactoring, delete it.

## Sources

- `AGENTS.md` — anti-patterns, module conventions, manager communication rules
- `.planning/codebase/CONVENTIONS.md` — flat lib.rs pattern, serde conventions
- `.planning/codebase/CONCERNS.md` — file sizes, clone density, Arc<Mutex> patterns

---

*Architecture patterns analysis: 2026-04-14*
