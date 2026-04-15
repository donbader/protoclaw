# Architecture Patterns

**Domain:** Config-driven architecture for Rust infrastructure sidecar
**Researched:** 2026-04-15

## Recommended Architecture

Integrate config-driven features into the existing Figment + serde pipeline without replacing any working components. The architecture change is additive: schemars derives layer on top of existing serde derives, defaults consolidation is a refactoring of the YAML layer, and schema generation is a new output artifact — not a new runtime dependency.

The key insight: **nothing changes at runtime**. Figment still loads defaults → user YAML → extract. The new features produce build-time and CI-time artifacts (JSON Schema, validated examples) that improve the developer experience around the same config pipeline.

### Current Architecture (Before)

```
                    ┌─────────────────────────────────────────┐
                    │           Config Loading (lib.rs)        │
                    │                                          │
  defaults.yaml ──►│  Figment::from(Yaml::string(DEFAULTS))   │
  (23 lines,       │      .merge(SubstYaml::file(path))       │
   ~40% coverage)  │      .extract::<AnyclawConfig>()          │
                    └──────────────┬───────────────────────────┘
                                   │
                    ┌──────────────▼───────────────────────────┐
                    │         types.rs Deserialization          │
                    │                                          │
                    │  23 default_*() fns fill remaining ~60%  │
                    │  #[serde(default = "default_foo")]        │
                    │  + 4 DEFAULT_* consts in constants.rs    │
                    └──────────────┬───────────────────────────┘
                                   │
                    ┌──────────────▼───────────────────────────┐
                    │         AnyclawConfig (validated)         │
                    │                                          │
                    │  Consumed by: supervisor, 3 managers,    │
                    │  resolve.rs, validate.rs, init.rs        │
                    └──────────────────────────────────────────┘
```

### Target Architecture (After)

```
                    ┌─────────────────────────────────────────┐
                    │           Config Loading (lib.rs)        │
                    │                                          │
  defaults.yaml ──►│  Figment::from(Yaml::string(DEFAULTS))   │
  (COMPLETE —      │      .merge(ext_defaults...)  [NEW]      │
   all defaults)   │      .merge(SubstYaml::file(path))       │
                    │      .extract::<AnyclawConfig>()          │
                    └──────────────┬───────────────────────────┘
                                   │
                    ┌──────────────▼───────────────────────────┐
                    │         types.rs Deserialization          │
                    │                                          │
                    │  #[serde(default)] (no fn) — values from │
                    │  YAML layer, serde just knows "optional" │
                    │  #[derive(JsonSchema)] on all types [NEW]│
                    └──────────────┬───────────────────────────┘
                                   │
                    ┌──────────────▼───────────────────────────┐
                    │         AnyclawConfig (validated)         │
                    │                                          │
                    │  + schema_for!(AnyclawConfig) [NEW]      │
                    │  → schema/anyclaw.schema.json [NEW]      │
                    │  → CI drift check [NEW]                  │
                    │  → CI example validation [NEW]           │
                    └──────────────────────────────────────────┘
```

### Component Boundaries

#### Modified Components (Existing Files)

| Component | File | What Changes | Risk |
|-----------|------|-------------|------|
| defaults.yaml | `crates/anyclaw-config/src/defaults.yaml` | Grows from 23 lines to ~80+ lines covering ALL defaults | LOW — additive YAML, Figment merge semantics unchanged |
| types.rs | `crates/anyclaw-config/src/types.rs` | (1) Add `#[derive(JsonSchema)]`. (2) Replace `#[serde(default = "fn")]` with `#[serde(default)]`. (3) Remove 23 `default_*()` fns. (4) Remove 3 `#[serde(alias)]`. (5) Manual `JsonSchema` for `StringOrArray`, `PullPolicy`. | MEDIUM — largest surface, but mechanical |
| constants.rs | `crates/anyclaw-core/src/constants.rs` | Remove 4 `DEFAULT_*` consts. Keep 6 internal guard consts. | LOW |
| init.rs | `crates/anyclaw/src/init.rs` | Replace hardcoded supervisor values with defaults/schema values | LOW |
| Cargo.toml (workspace) | `Cargo.toml` | Add `schemars = "1.2"` to workspace deps | LOW |
| Cargo.toml (config) | `crates/anyclaw-config/Cargo.toml` | Add `schemars`, `jsonschema` (dev-dep) | LOW |

#### New Components

| Component | Location | Purpose |
|-----------|----------|---------|
| Schema file | `schema/anyclaw.schema.json` | Generated JSON Schema, committed to repo |
| Schema drift test | `crates/anyclaw-config/` tests | `#[test] fn schema_is_up_to_date()` |
| Schema CLI subcommand | `crates/anyclaw/src/cli.rs` | `anyclaw schema` — optional, low priority |

#### Unchanged Components

| Component | Why Unchanged |
|-----------|---------------|
| `subst_yaml.rs`, `validate.rs`, `resolve.rs`, `error.rs`, `parse.rs` | Orthogonal to defaults/schema work |
| All manager crates (`anyclaw-agents`, `anyclaw-channels`, `anyclaw-tools`) | Consume `AnyclawConfig` — interface unchanged |
| All SDK crates (`anyclaw-sdk-*`) | Don't depend on `anyclaw-config` |
| `supervisor.rs` | Consumes config the same way |

## Data Flow Changes

### Default Value Resolution (Before → After)

**Before:** Two-path default resolution creates drift risk.

```
Field: agents_manager.acp_timeout_secs

Path A (YAML present):  defaults.yaml has "acp_timeout_secs: 30"
                        → Figment provides value → serde accepts it

Path B (YAML absent):   defaults.yaml missing this field
                        → Figment has no value → serde calls default_acp_timeout_secs() → 30

Problem: Path A and Path B can disagree if someone updates one but not the other.
```

**After:** Single-path resolution. All defaults in YAML, serde just marks fields optional.

```
Field: agents_manager.acp_timeout_secs

Only path:  defaults.yaml has "acp_timeout_secs: 30"
            → Figment ALWAYS provides value → serde accepts it
            → #[serde(default)] still present so partial user YAML works
            → But the default VALUE comes from YAML layer, not Rust fn

Key: #[serde(default)] (no fn) uses Default::default() for the TYPE (0 for u64),
     but Figment's merge means the YAML layer value wins. The serde default is
     only a fallback if Figment somehow has no value — which won't happen because
     defaults.yaml is always the base layer.
```

### Schema Generation Flow (New)

```
types.rs structs                    schema/anyclaw.schema.json
    │                                        ▲
    │ #[derive(JsonSchema)]                  │
    ▼                                        │
schemars::schema_for!(AnyclawConfig)  ──►  serde_json::to_string_pretty()
    │                                        │
    │ Reads:                                 │ Written by:
    │  - struct field names                  │  - #[test] fn schema_is_up_to_date()
    │  - #[serde(...)] attributes            │  - OR: `anyclaw schema` CLI
    │  - /// doc comments → descriptions     │
    │  - Option<T> → nullable                │
    │  - HashMap<K,V> → additionalProperties │
    │  - enum variants → oneOf/anyOf         │
    └────────────────────────────────────────┘
```

### Per-Extension Defaults Flow (Future Phase)

```
Core defaults.yaml          ext/channels/telegram/defaults.yaml
       │                              │
       ▼                              ▼
Figment::from(core_defaults)  .merge(Yaml::string(telegram_defaults))
                                      │
                              .merge(SubstYaml::file(user_yaml))
                                      │
                              .extract::<AnyclawConfig>()

Extension defaults provide sensible values for extension-specific options.
User YAML overrides everything. Core defaults are the base.
Figment merge semantics: nested dicts are unioned, scalars are replaced.
```

## Defaults Migration: The Critical Transformation

This is the highest-risk, highest-value change. Every other feature depends on it.

### What Changes in types.rs

**23 `default_*()` fns to remove.** Each follows the same pattern:

```rust
// BEFORE (current)
#[serde(default = "default_acp_timeout_secs")]
pub acp_timeout_secs: u64,
// ...
fn default_acp_timeout_secs() -> u64 { 30 }

// AFTER (target)
#[serde(default)]
pub acp_timeout_secs: u64,
// No fn needed — Figment's YAML layer provides the value
```

**Critical subtlety:** `#[serde(default)]` without a function calls `Default::default()` for the field type (`0` for `u64`, `""` for `String`, `false` for `bool`). This is fine because Figment's base layer (defaults.yaml) always provides the real value. The serde `default` attribute just tells Figment "this field is optional in the user's YAML" — it doesn't determine the actual default value.

**Exception — `default_true()` fields:** Three fields use `default_true()` → `true`: `AgentConfig.enabled`, `ChannelConfig.enabled`, `ToolConfig.enabled`, plus `PreopenedDir.readonly`. These can't use bare `#[serde(default)]` because `bool::default()` is `false`. Two options:

1. Keep `default_true()` as the only surviving default fn (pragmatic)
2. Move these to defaults.yaml too — but `enabled` lives on per-entity configs inside HashMaps, so there's no YAML path to set a default for "every future agent's enabled field"

**Recommendation:** Keep `default_true()` for `enabled` and `readonly` fields. These are per-entity defaults that can't be expressed in a top-level YAML file. Document why they survive the migration.

**Exception — `default_agent()` → `"default"`:** The `ChannelConfig.agent` field defaults to the string `"default"`. `String::default()` is `""`, not `"default"`. Same situation as `default_true()` — this is a per-entity default inside a HashMap. Keep `default_agent()`.

**Exception — `default_reaction_emoji()` → `"👀"`:** Same pattern. Keep it.

### What Changes in defaults.yaml

Grow from 23 lines to cover every field that currently has a `default_*()` fn:

```yaml
# Current (23 lines, ~40% coverage)
log_level: "info,hyper=warn,..."
log_format: "pretty"
extensions_dir: "/usr/local/bin"
agents_manager:
  acp_timeout_secs: 30
  shutdown_grace_ms: 100
channels_manager:
  init_timeout_secs: 10
  exit_timeout_secs: 5
tools_manager: {}
supervisor:
  shutdown_timeout_secs: 30
  health_check_interval_secs: 5
  max_restarts: 5
  restart_window_secs: 60
session_store:
  type: none

# NEW additions needed:
supervisor:
  admin_port: 3000              # was default_admin_port()
tools_manager:
  tools_server_host: "127.0.0.1"  # was default_tools_server_host()
# Plus nested defaults for BackoffConfig, CrashTrackerConfig, WasmSandboxConfig
# when used at the manager level (not per-entity overrides)
```

### What Changes in constants.rs

Remove 4 `DEFAULT_*` consts that duplicate config defaults:

| Const | Value | Used By | Migration |
|-------|-------|---------|-----------|
| `DEFAULT_BACKOFF_BASE_MS` | 100 | `ExponentialBackoff::default()` | Inline the literal, or accept `BackoffConfig` from caller |
| `DEFAULT_BACKOFF_MAX_SECS` | 30 | `ExponentialBackoff::default()` | Same |
| `DEFAULT_CRASH_MAX` | 5 | `CrashTracker::default()` | Same |
| `DEFAULT_CRASH_WINDOW_SECS` | 60 | `CrashTracker::default()` | Same |

Keep 6 internal guard consts (`POLL_TIMEOUT_MS`, `POLL_INTERVAL_MS`, `CMD_CHANNEL_CAPACITY`, `EVENT_CHANNEL_CAPACITY`, `EPOCH_TICK_INTERVAL_SECS`, `STATUS_HTTP_TIMEOUT_SECS`) — these are NOT user-configurable and don't belong in defaults.yaml.

### Impact on Default impls

Six structs have `impl Default` that call `default_*()` fns:

- `BackoffConfig::default()` → uses `default_backoff_base_ms()`, `default_backoff_max_secs()`
- `CrashTrackerConfig::default()` → uses `default_crash_max()`, `default_crash_window_secs()`
- `WasmSandboxConfig::default()` → uses `default_fuel_limit()`, `default_epoch_timeout()`, `default_memory_limit()`
- `SupervisorConfig::default()` → uses 5 default fns
- `AgentsManagerConfig::default()` → uses 2 default fns
- `ChannelsManagerConfig::default()` → uses 2 default fns
- `AckConfig::default()` → uses `default_reaction_emoji()`
- `ToolsManagerConfig::default()` → uses `default_tools_server_host()`
- `SqliteStoreConfig::default()` → uses `default_ttl_days()`

After migration, these `Default` impls either:
1. Inline the literal values (simplest — the values are stable)
2. Get removed entirely if the type is only ever constructed via serde deserialization from the YAML layer

**Recommendation:** Inline literals in `Default` impls. They're used in tests and in `valid_config()` test helpers. Removing them would break test ergonomics.

## schemars Integration Details

### Derive Placement

Add `#[derive(JsonSchema)]` to every config struct that appears in `AnyclawConfig`'s type tree. Full list:

| Struct/Enum | Custom Deserialize? | JsonSchema Strategy |
|-------------|--------------------|--------------------|
| `AnyclawConfig` | No | `#[derive(JsonSchema)]` |
| `AgentsManagerConfig` | No | `#[derive(JsonSchema)]` |
| `ChannelsManagerConfig` | No | `#[derive(JsonSchema)]` |
| `ToolsManagerConfig` | No | `#[derive(JsonSchema)]` |
| `SupervisorConfig` | No | `#[derive(JsonSchema)]` |
| `AgentConfig` | No | `#[derive(JsonSchema)]` |
| `ChannelConfig` | No | `#[derive(JsonSchema)]` |
| `ToolConfig` | No | `#[derive(JsonSchema)]` |
| `BackoffConfig` | No | `#[derive(JsonSchema)]` |
| `CrashTrackerConfig` | No | `#[derive(JsonSchema)]` |
| `WasmSandboxConfig` | No | `#[derive(JsonSchema)]` |
| `PreopenedDir` | No | `#[derive(JsonSchema)]` |
| `AckConfig` | No | `#[derive(JsonSchema)]` |
| `WorkspaceConfig` | No | `#[derive(JsonSchema)]` — schemars handles `#[serde(tag = "type")]` |
| `LocalWorkspaceConfig` | No | `#[derive(JsonSchema)]` |
| `DockerWorkspaceConfig` | No | `#[derive(JsonSchema)]` |
| `SessionStoreConfig` | No | `#[derive(JsonSchema)]` — schemars handles `#[serde(tag = "type")]` |
| `SqliteStoreConfig` | No | `#[derive(JsonSchema)]` |
| `LogFormat` | No | `#[derive(JsonSchema)]` — schemars handles `#[serde(rename_all)]` |
| `ReactionLifecycle` | No | `#[derive(JsonSchema)]` |
| `ToolType` | No | `#[derive(JsonSchema)]` |
| `StringOrArray` | **Yes** — custom `Deserialize` | **Manual `impl JsonSchema`** |
| `PullPolicy` | **Yes** — custom `Deserialize` | **Manual `impl JsonSchema`** |

### Manual JsonSchema Implementations

**`StringOrArray`** — accepts either a string or array of strings:

```rust
impl JsonSchema for StringOrArray {
    fn schema_name() -> Cow<'static, str> {
        "StringOrArray".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "oneOf": [
                generator.subschema_for::<String>(),
                generator.subschema_for::<Vec<String>>(),
            ],
            "description": "A string or array of strings. Single string is equivalent to a one-element array."
        })
    }
}
```

**`PullPolicy`** — string enum with custom deserialization:

```rust
impl JsonSchema for PullPolicy {
    fn schema_name() -> Cow<'static, str> {
        "PullPolicy".into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "enum": ["always", "never", "if_not_present"],
            "default": "if_not_present",
            "description": "Docker image pull policy."
        })
    }
}
```

### serde(alias) Removal

Three fields on `AnyclawConfig` use `#[serde(alias = "...")]`:

```rust
#[serde(alias = "agents-manager")]   pub agents_manager: ...
#[serde(alias = "channels-manager")] pub channels_manager: ...
#[serde(alias = "session-store")]    pub session_store: ...
```

schemars does NOT support `alias`. The schema would only show the primary name. Since breaking config changes are acceptable (per PROJECT.md), remove the aliases entirely. Users must use `agents_manager` (snake_case) going forward. This is also the config cleanup goal.

Note: `tools_manager` has no alias — it was already snake_case only. This inconsistency is another reason to clean up.

### Doc Comments → Schema Descriptions

schemars 1.x automatically extracts `///` doc comments as JSON Schema `title` and `description`. Most config types already have doc comments. After adding `#[derive(JsonSchema)]`, these become IDE hover documentation for free. No extra work needed — just verify the comments are user-facing quality (not internal implementation notes).

## Build Order (Phase Dependencies)

The features have strict dependencies. Building out of order creates rework.

```
Phase 1: Defaults Consolidation
  ├── Migrate all default_*() fn values into defaults.yaml
  ├── Replace #[serde(default = "fn")] with #[serde(default)] where possible
  ├── Keep default_true(), default_agent(), default_reaction_emoji() (per-entity)
  ├── Inline literals in Default impls (replace fn calls)
  ├── Remove DEFAULT_* consts from constants.rs
  └── Update ExponentialBackoff::default() / CrashTracker::default()
  
  WHY FIRST: Everything downstream assumes a single source of truth.
  Schema generation on the current dual-default system would produce
  a schema with incorrect/missing default values.
  
  TESTS: All existing tests must pass unchanged. The observable behavior
  is identical — only the mechanism changes.

Phase 2: Config Schema Cleanup
  ├── Remove 3 #[serde(alias)] attributes (breaking change)
  ├── Audit field naming for snake_case consistency
  ├── Flatten any structural inconsistencies
  └── Update example YAML files to use canonical names
  
  WHY SECOND: Clean the schema surface BEFORE generating the schema.
  Generating first then cleaning means regenerating and re-reviewing.
  
  TESTS: Tests using hyphenated keys (agents-manager) must be updated
  or removed. Add tests confirming old aliases are rejected.

Phase 3: JSON Schema Generation
  ├── Add schemars = "1.2" to workspace deps
  ├── Add #[derive(JsonSchema)] to all 23 config types
  ├── Implement manual JsonSchema for StringOrArray, PullPolicy
  ├── Generate schema/anyclaw.schema.json
  ├── Add #[test] fn schema_is_up_to_date() drift check
  └── Review generated schema for correctness
  
  WHY THIRD: Depends on Phase 1 (correct defaults in schema) and
  Phase 2 (clean field names in schema). This is the payoff phase.
  
  TESTS: Schema drift test. Manual review of generated schema against
  actual config structure. Round-trip: generate schema → validate
  example YAML against it.

Phase 4: CI Integration + IDE Support
  ├── Add jsonschema dev-dep for example validation
  ├── Add test validating example anyclaw.yaml files against schema
  ├── Add modeline to anyclaw init output
  ├── Update init.rs to read from defaults instead of hardcoding
  └── Document .vscode/settings.json setup
  
  WHY FOURTH: Depends on Phase 3 (schema exists). Pure consumption
  of the schema artifact.
  
  TESTS: CI validation test. init.rs output test.

Phase 5: Per-Extension Defaults (Optional/Future)
  ├── Design convention for ext/<type>/<name>/defaults.yaml
  ├── Add extension defaults to Figment chain in lib.rs
  ├── Create defaults.yaml for existing extensions
  └── Document extension defaults authoring
  
  WHY LAST: Highest complexity, lowest urgency. Depends on Phase 1
  establishing the pattern. Current extension count is small enough
  that this can wait.
  
  TESTS: Integration test with mock extension defaults.
```

### Dependency Graph (Visual)

```
[Phase 1: Defaults]──────►[Phase 2: Cleanup]──────►[Phase 3: Schema Gen]──────►[Phase 4: CI/IDE]
                                                                                       │
                                                           [Phase 5: Ext Defaults]◄────┘
                                                           (optional, can also start
                                                            after Phase 1 alone)
```

### Risk Assessment Per Phase

| Phase | Risk | Reason | Mitigation |
|-------|------|--------|------------|
| 1. Defaults | MEDIUM | Touches every config type's deserialization path | Existing test suite is comprehensive (40+ tests in types.rs). Run full suite after each struct migration. |
| 2. Cleanup | LOW | Breaking change but explicitly allowed. Mechanical find-replace. | grep for alias usage in examples/, tests/, docs/ |
| 3. Schema Gen | LOW | Additive — new derive, new file, new test. No runtime changes. | Review generated schema manually. Validate against real configs. |
| 4. CI/IDE | LOW | Pure consumption of existing artifact. | Test in CI before merging. |
| 5. Ext Defaults | HIGH | Design decision with long-term implications. Discovery mechanism unclear. | Defer until extension ecosystem grows. Start with explicit paths. |

## Anti-Patterns to Avoid

### Anti-Pattern 1: Removing Default impls Entirely
**What:** Deleting `impl Default for BackoffConfig` etc. because "defaults live in YAML now."
**Why bad:** `Default` impls are used extensively in tests (`valid_config()`, direct struct construction) and in `Option<BackoffConfig>` fields where `None` means "use default." Removing them breaks test ergonomics and forces every test to construct full configs from YAML.
**Instead:** Keep `Default` impls with inlined literal values. They serve a different purpose (programmatic construction) than the YAML defaults (config loading).

### Anti-Pattern 2: Making schemars a Runtime Dependency
**What:** Using `schema_for!()` at runtime (e.g., in `anyclaw run`) or adding `jsonschema` validation to the config loading path.
**Why bad:** Adds compile-time cost and binary size for something that's only needed at build/CI time. Figment + serde already validate config at load time.
**Instead:** Schema generation lives in tests or a dedicated CLI subcommand. `jsonschema` is a dev-dependency only.

### Anti-Pattern 3: Migrating Defaults Piecemeal Across Multiple PRs
**What:** Moving 5 defaults to YAML in one PR, 5 more in the next, etc.
**Why bad:** Creates an intermediate state where some defaults are in YAML and some in Rust fns — exactly the dual-default problem we're solving. Tests become harder to reason about during the transition.
**Instead:** Migrate ALL defaults in a single phase. The change is mechanical and the test suite catches regressions immediately.

### Anti-Pattern 4: Using schemars `transform` to Paper Over Structural Issues
**What:** Using `#[schemars(transform = ...)]` to rename fields or restructure the schema output instead of fixing the underlying type structure.
**Why bad:** Creates divergence between the Rust types and the schema. The schema should be a faithful reflection of the types.
**Instead:** Fix the types first (Phase 2: Cleanup), then generate the schema (Phase 3). The schema should need zero transforms.

### Anti-Pattern 5: Generating Schema in CI Instead of Committing It
**What:** Having CI generate the schema on every build instead of committing `schema/anyclaw.schema.json` to the repo.
**Why bad:** Users can't reference the schema without building from source. IDE autocomplete requires a file path or URL. The schema is a build artifact that should be version-controlled.
**Instead:** Commit the schema. CI verifies the committed file matches the generated output (drift check).

## Scalability Considerations

| Concern | Current (5 extensions) | At 20 extensions | At 100+ extensions |
|---------|----------------------|-------------------|-------------------|
| defaults.yaml size | ~80 lines, single file | Still manageable as single file | Consider splitting into `defaults/core.yaml` + `defaults/extensions/` |
| Schema size | ~500 lines JSON, fast generation | ~2K lines, still fast | May need schema composition (`$ref` to sub-schemas) |
| Figment chain length | 2 providers (defaults + user) | 22 providers (defaults + 20 ext + user) | Figment merge is O(n) providers — may need benchmarking |
| Extension discovery | Explicit paths in Figment chain | Convention-based glob (`ext/*/defaults.yaml`) | Registry or manifest file listing extensions |
| CI validation time | Milliseconds | Still milliseconds | Still milliseconds — jsonschema is fast |

The current design (explicit Figment provider chain) scales to ~20 extensions without architectural changes. Beyond that, a discovery mechanism or manifest would be needed — but that's well beyond the current extension count.

## Sources

- Existing codebase analysis (HIGH confidence): `types.rs` (23 `default_*` fns, 1497 lines), `defaults.yaml` (23 lines), `lib.rs` (Figment loading chain), `constants.rs` (4 `DEFAULT_*` consts + 6 guard consts), `init.rs` (hardcoded supervisor values), `validate.rs` (runtime validation)
- schemars 1.x docs (Context7, HIGH confidence): `#[derive(JsonSchema)]` with serde compat, `json_schema!` macro for manual impls, doc comment extraction
- Figment docs (Context7, HIGH confidence): `merge()` provider chaining, `Yaml::string()` for embedded defaults
- STACK.md research (HIGH confidence): schemars 1.2.1, jsonschema 0.45.1 versions and capabilities
- FEATURES.md research (HIGH confidence): feature dependencies, MVP ordering, anti-features

---

*Architecture analysis for config-driven milestone: 2026-04-15*
