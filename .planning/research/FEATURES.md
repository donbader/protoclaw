# Feature Landscape

**Domain:** Config-driven architecture for Rust infrastructure sidecar
**Researched:** 2026-04-15

## Table Stakes

Features users expect from a config-driven Rust project. Missing = config system feels half-baked.

| Feature | Why Expected | Complexity | Dependencies | Notes |
|---------|--------------|------------|--------------|-------|
| Single source of truth for defaults | Dual-default mechanisms (YAML + serde fns) cause drift and confusion. Every mature config system has one canonical default location. | Medium | Existing `defaults.yaml`, 23 `default_*` fns in `types.rs` | Currently ~40% of defaults in YAML, rest in Rust fns. Must migrate all 23 fns + `constants.rs` `DEFAULT_*` consts into `defaults.yaml`. Serde `#[serde(default)]` (no fn) still needed for Figment to know fields are optional, but the *values* come from YAML layer. |
| JSON Schema generation from types | Any project shipping a YAML config file should ship a schema. schemars 1.x is the de facto standard — 27M+ downloads/month, derives alongside serde, respects `#[serde(...)]` attributes. Without this, users guess at field names and valid values. | Medium | `schemars = "1.2"` added to `anyclaw-config`. `#[derive(JsonSchema)]` on all config types. | schemars 1.x (stable since 2025-06-23) generates JSON Schema 2020-12 by default. Handles `#[serde(tag = "type")]`, `#[serde(rename_all)]`, `#[serde(default)]`, `Option<T>`, `HashMap<K,V>` correctly. Custom types like `StringOrArray` and `PullPolicy` need manual `JsonSchema` impls since they have custom `Deserialize`. |
| Schema committed to repo | Generated schema must be version-controlled so users and CI can reference it without building from source. Standard pattern: `schema/anyclaw.schema.json` at repo root. | Low | JSON Schema generation working first | A `cargo test` or dedicated binary generates schema, writes to known path. CI checks committed file matches generated output. |
| Config schema cleanup | Consistent naming (`snake_case` everywhere), flatten structural inconsistencies, remove legacy aliases once breaking changes are acceptable. Users expect a clean, predictable config surface. | Medium | Breaking config format accepted per PROJECT.md | Aliases like `agents-manager` / `session-store` can be removed. Field naming should be uniform. |
| Generated YAML template from schema | `anyclaw init` should produce a YAML template that reflects actual defaults, not hardcoded strings. Currently `init.rs` hardcodes supervisor values. | Low | Single source of truth + JSON Schema | Template generation reads from `defaults.yaml` or schema `default` values instead of duplicating them in Rust code. |

## Differentiators

Features that set the config system apart. Not expected, but valued by power users and extension authors.

| Feature | Value Proposition | Complexity | Dependencies | Notes |
|---------|-------------------|------------|--------------|-------|
| Per-extension defaults.yaml | Each ext binary (mock-agent, telegram, debug-http, system-info) ships its own `defaults.yaml` that layers into Figment before user config. Extension authors define their own defaults without touching core config. | High | Figment layered loading, extension discovery | Figment supports arbitrary provider chaining: `Figment::from(core_defaults).merge(ext_defaults_1).merge(ext_defaults_2).merge(user_yaml)`. Challenge: discovering which extensions exist and where their defaults live at load time. Need a convention like `ext/<type>/<name>/defaults.yaml` or embedding defaults in the binary and extracting via a handshake. |
| IDE autocomplete for anyclaw.yaml | VS Code + Red Hat YAML extension (yaml-language-server) provides autocomplete, hover docs, and validation when a JSON Schema is associated. Works via `yaml.schemas` setting, modeline comment (`# yaml-language-server: $schema=...`), or SchemaStore registration. | Low | Committed JSON Schema file | Three association methods: (1) modeline in YAML file: `# yaml-language-server: $schema=./schema/anyclaw.schema.json`, (2) `.vscode/settings.json` with `yaml.schemas` mapping, (3) eventual SchemaStore submission. Method 1 is zero-config for users. yaml-language-server supports JSON Schema drafts 04, 07, 2019-09, and 2020-12 — schemars 1.x outputs 2020-12 which is supported. |
| CI schema drift check | CI step that regenerates the schema and diffs against committed version. Catches when someone changes config types but forgets to update the schema file. Standard pattern in Rust projects: a `#[test]` that generates and compares. | Low | Committed JSON Schema | Pattern: `#[test] fn schema_is_up_to_date()` generates schema, reads committed file, asserts equality. Fails with clear message: "run `cargo run --bin generate-schema` to update". Alternatively, a CI job runs the generator and `git diff --exit-code`. |
| CI config validation | Example `anyclaw.yaml` files in `examples/` validated against the schema in CI. Catches schema-breaking changes before they ship. | Low | Committed JSON Schema, a JSON Schema validator | Use `jsonschema` crate (Rust) or `lintel` CLI (multi-format validator, Rust-based, SchemaStore-aware). `lintel check` in CI validates YAML files against schema with zero config. |
| Per-manager defaults in YAML | Backoff config, crash tracker config, WASM sandbox defaults, tools server host — all live in `defaults.yaml` instead of scattered `default_*` fns. Makes the full default config visible in one file. | Medium | Single source of truth migration | Currently `BackoffConfig`, `CrashTrackerConfig`, `WasmSandboxConfig` defaults are Rust fns. Move to nested YAML sections. Figment's merge semantics handle partial overrides correctly (nested dicts are unioned). |
| Doc comments → schema descriptions | schemars 1.x automatically converts `///` doc comments on structs and fields into JSON Schema `title` and `description` fields. These surface as hover documentation in IDEs. | Low | `#[derive(JsonSchema)]` on types | Already have doc comments on most public types. schemars picks them up automatically — no extra work beyond deriving `JsonSchema`. Free IDE documentation. |
| Schema validation attributes | schemars respects `#[validate(...)]` attributes (from `validator` or `garde` crates) to add constraints like `min`, `max`, `pattern` to the schema. Makes the schema more precise than just types. | Medium | `schemars` + optionally `garde` | Not required for v1.0 but valuable later. Example: `acp_timeout_secs` could have `#[validate(range(min = 1))]` reflected in schema as `"minimum": 1`. |

## Anti-Features

Features to explicitly NOT build for this milestone.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Runtime schema validation of user YAML | Validating user config against JSON Schema at runtime adds a dependency (`jsonschema` crate) and duplicates what serde already does during deserialization. Serde + Figment already reject invalid types/unknown fields (when configured). | Keep using serde deserialization as the validation layer. Schema is for IDE/CI, not runtime. |
| SchemaStore submission (now) | SchemaStore requires a stable schema URL and mature project. anyclaw is pre-1.0 with breaking config changes expected. Premature submission creates maintenance burden. | Ship schema in repo. Add modeline support to `anyclaw init`. Submit to SchemaStore after config format stabilizes post-v1.0. |
| Config hot-reloading | Watching config file for changes and reloading at runtime. Massive complexity (partial updates, manager restarts, state consistency). Out of scope per PROJECT.md. | Require restart for config changes. Standard for infrastructure sidecars. |
| GUI config editor | Visual config editor or web UI. Overkill for an infrastructure sidecar. | IDE autocomplete via JSON Schema covers the UX need. |
| Env var override layer implementation | `ANYCLAW_` prefix env var overrides are documented but unimplemented. Explicitly out of scope per PROJECT.md. | Keep as future milestone. Figment already has the `.merge(Env::prefixed("ANYCLAW_").split("__"))` call ready in `lib.rs` — just not wired up yet. |
| schemars 0.8.x | The 0.8 line is legacy. schemars 1.x has been stable since June 2025 with significant improvements (JSON Schema 2020-12, better serde compat, `transform` attribute, `no_std` support). | Use schemars 1.2.x exclusively. The API changed significantly from 0.8 — don't reference 0.8 patterns. |
| Custom Figment provider for extension defaults discovery | Building a complex provider that auto-discovers extension defaults from filesystem at load time. Over-engineered for the current extension count. | Start with explicit extension default paths in the Figment chain. Iterate toward discovery if extension ecosystem grows. |

## Feature Dependencies

```
Single source of truth (defaults.yaml migration)
  → JSON Schema generation (schemars derives)
    → Schema committed to repo
      → CI schema drift check
      → CI config validation (examples/)
      → IDE autocomplete (modeline + yaml-language-server)
    → Generated YAML template from schema (init.rs)
  → Per-manager defaults in YAML

Config schema cleanup (naming, structure)
  → JSON Schema generation (clean schema output)

Per-extension defaults.yaml
  → Single source of truth (core defaults pattern established first)
  → Figment layered loading (provider chain design)
```

## MVP Recommendation

Prioritize (in order):

1. **Single source of truth** — migrate all 23 `default_*` fns and `DEFAULT_*` consts into `defaults.yaml`. This is the foundation everything else builds on. Unblocks clean schema generation.
2. **Config schema cleanup** — flatten inconsistencies, remove legacy aliases. Do this before generating the schema so the schema reflects the clean structure.
3. **JSON Schema generation** — add `#[derive(JsonSchema)]` to all config types, implement `JsonSchema` for custom types (`StringOrArray`, `PullPolicy`), generate schema file.
4. **Schema committed + CI drift check** — commit `schema/anyclaw.schema.json`, add `#[test]` for drift detection.
5. **IDE autocomplete** — add modeline to `anyclaw init` output, document `.vscode/settings.json` setup.
6. **Per-extension defaults** — design the layering convention, implement for existing extensions.

Defer:
- **Schema validation attributes** (`garde`/`validate`): nice-to-have, not blocking. Add incrementally after core schema works.
- **CI config validation**: trivial to add once schema exists, but lower priority than the generation pipeline.
- **Per-extension defaults**: highest complexity, lowest urgency. Current extension count is small. Design the convention in v1.0, implement in a follow-up.

## Sources

- schemars 1.x docs (Context7, HIGH confidence): `#[derive(JsonSchema)]` with serde compat, `transform` attribute, doc comment extraction
- schemars changelog (GitHub, HIGH confidence): 1.2.1 released 2026-02-01, stable since 1.0.0 (2025-06-23), JSON Schema 2020-12 default
- Figment docs (Context7, HIGH confidence): `merge()` / `join()` provider chaining, `Yaml::file()`, `Serialized::defaults()`
- yaml-language-server (GitHub, HIGH confidence): supports JSON Schema 2020-12, modeline `# yaml-language-server: $schema=...`, `yaml.schemas` setting
- lintel (GitHub, MEDIUM confidence): Rust-based multi-format JSON Schema linter, SchemaStore-aware, CI-friendly
- Existing codebase: `types.rs` (23 `default_*` fns), `defaults.yaml` (23 lines, ~40% coverage), `lib.rs` (Figment loading chain)
