# Technology Stack

**Project:** Anyclaw v1.0.0 Config-Driven Architecture
**Researched:** 2026-04-15

## Recommended Stack

### Core: JSON Schema Generation

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| schemars | 1.2 | Generate JSON Schema from Rust serde types | De facto Rust JSON Schema crate. 1.0 stable since June 2025, now at 1.2.1 (Feb 2026). 27M+ downloads/month, 1.3K stars. Derives alongside serde — reads `#[serde(...)]` attributes to match serialization behavior. MSRV 1.74 (project uses 1.94). Generates Draft 2020-12 schemas. |

### CI: Schema Validation

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| jsonschema | 0.45 | Validate example YAML files against generated schema | High-performance JSON Schema validator. Supports Draft 2020-12 (matches schemars output). Used by Tauri and Apollo Router for config validation. Dev-dependency only — not shipped in the binary. |

### Already Present (No Changes Needed)

| Technology | Version | Purpose | Notes |
|------------|---------|---------|-------|
| figment | 0.10 | Layered config loading | Stays as-is. Defaults externalization is pure refactoring of the YAML layer, not a Figment change. |
| serde | 1 | Serialization/deserialization | schemars reads serde attributes directly — no adapter needed. |
| serde_json | 1 | JSON output for schema files | Already in workspace. Used to serialize the generated schema to JSON. |
| serde_yaml (yaml_serde) | 0.10 | YAML parsing for defaults | Already in workspace. No changes needed. |

## What NOT to Add

| Temptation | Why Avoid |
|------------|-----------|
| `typify` / `schemafy` (schema → Rust types) | Wrong direction. We generate schema FROM types, not types from schema. |
| `valico` / `jsonschema_valid` | Slower alternatives to `jsonschema`. No reason to use them. |
| `schemars` v0.8.x | Legacy branch. v1.x is stable, actively maintained, and generates Draft 2020-12 (v0.8 generates Draft-07). |
| `jsonschema-cli` binary in CI | Adds a binary dependency to CI. Using the `jsonschema` crate as a dev-dep in a test is simpler and more portable. |
| Runtime schema validation | Schema is for IDE/CI validation of YAML files. Figment + serde already validate at load time. Don't add runtime JSON Schema validation to the config loading path. |

## Integration Points with Existing Setup

### schemars + serde (derive co-location)

schemars reads `#[serde(...)]` attributes automatically. Add `#[derive(JsonSchema)]` alongside existing `#[derive(Deserialize, Serialize)]`:

```rust
use schemars::JsonSchema;

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkspaceConfig {
    Local(LocalWorkspaceConfig),
    Docker(DockerWorkspaceConfig),
}
```

Supported serde attributes (verified via Context7, HIGH confidence):
- `rename`, `rename_all`, `rename_all_fields` ✓
- `tag` / `content` / `untagged` ✓
- `default` ✓
- `skip`, `skip_serializing`, `skip_deserializing` ✓
- `flatten` ✓
- `with` (must be a type implementing `JsonSchema`) ✓
- `bound` ✓

### ⚠️ serde(alias) is NOT supported by schemars

**This is the biggest integration concern.** The project uses `#[serde(alias = "agents-manager")]` on three fields in `AnyclawConfig`. schemars does NOT list `alias` in its supported serde attributes. The generated schema will only include the primary field name (`agents_manager`), not the hyphenated alias.

**Mitigation options:**
1. Drop the aliases (breaking config OK per PROJECT.md) — cleanest
2. Use `#[schemars(rename = "...")]` to override if you want the hyphenated form as canonical
3. Accept that the schema won't validate alias usage (document this limitation)

Recommendation: Option 1 — drop aliases. The milestone explicitly allows breaking config changes, and having one canonical name is cleaner.

### schemars + serde_json::Value fields

Several config types use `HashMap<String, serde_json::Value>` for `options` fields. schemars handles `serde_json::Value` natively — it generates `true` (accepts any JSON value) as the schema for those fields. This is correct behavior for freeform options maps.

### schemars + custom Deserialize impls

Two types have custom `Deserialize` impls:
- `StringOrArray` — custom untagged string-or-array deserialization
- `PullPolicy` — custom string-to-enum deserialization

These need manual `JsonSchema` implementations since schemars can't derive from custom deserializers. Use `#[schemars(with = "Type")]` or implement `JsonSchema` directly.

### Schema generation binary/test

Add a test or binary that generates the schema and writes it to a file:

```rust
use schemars::schema_for;
let schema = schema_for!(AnyclawConfig);
let json = serde_json::to_string_pretty(&schema).unwrap();
```

This can be:
- A `#[test]` that generates and compares against committed `anyclaw.schema.json` (schema drift check)
- A CLI subcommand (`anyclaw schema`) that prints the schema to stdout

### jsonschema for CI validation

Dev-dependency only. Used in integration tests to validate example `anyclaw.yaml` files:

```rust
let schema: serde_json::Value = serde_json::from_str(SCHEMA_JSON)?;
let instance: serde_json::Value = serde_yaml::from_str(yaml_content)?;
assert!(jsonschema::is_valid(&schema, &instance));
```

Note: `jsonschema` works on `serde_json::Value`, so YAML files must be parsed to JSON Value first (serde_yaml handles this naturally since YAML is a JSON superset).

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Schema generation | schemars 1.2 | Manual schema writing | Unmaintainable. 20+ config types with nested structures. Derive macro keeps schema in sync with types automatically. |
| Schema generation | schemars 1.2 | schemars 0.8.x | v0.8 generates Draft-07, v1.x generates Draft 2020-12. v1 is stable, actively maintained, and the migration path forward. |
| CI validation | jsonschema 0.45 | No validation | Defeats the purpose. Schema without validation is documentation that drifts. |
| CI validation | jsonschema 0.45 (dev-dep) | jsonschema-cli (binary) | External binary adds CI complexity. A Rust test using the crate is self-contained and runs with `cargo test`. |
| Defaults externalization | Figment YAML layering (existing) | New config crate | Overkill. The existing Figment setup already supports layered YAML. This is a refactoring task, not a new dependency. |

## Installation

```toml
# In workspace Cargo.toml [workspace.dependencies]
schemars = "1.2"

# In crates/anyclaw-config/Cargo.toml [dependencies]
schemars = { workspace = true }

# In crates/anyclaw-config/Cargo.toml OR tests/integration/Cargo.toml [dev-dependencies]
jsonschema = { version = "0.45", default-features = false }
```

### jsonschema feature flags

Use `default-features = false` for `jsonschema` to avoid pulling in HTTP resolution (`reqwest`, TLS) which is unnecessary for local file validation. The default features include `resolve-http` and `tls-aws-lc-rs` which add significant compile-time deps that aren't needed for validating a local schema against local YAML files.

## Version Confidence

| Dependency | Version | Confidence | Verification |
|------------|---------|------------|--------------|
| schemars | 1.2.1 | HIGH | Context7 docs + GitHub releases (latest: v1.2.1, 2026-02-01) + crates.io (27M downloads/month) |
| jsonschema | 0.45.1 | HIGH | crates.io page (latest: v0.45.1, 2026-04-06) + 14M downloads/90d |

## Sources

- Context7: schemars documentation and attribute reference (HIGH confidence)
- GitHub: [GREsau/schemars releases](https://github.com/GREsau/schemars/releases) — v1.2.1 latest (2026-02-01)
- GitHub: [GREsau/schemars CHANGELOG](https://github.com/GREsau/schemars/blob/master/CHANGELOG.md)
- crates.io: [schemars](https://crates.io/crates/schemars) — 1.2.1, MSRV 1.74
- crates.io: [jsonschema](https://crates.io/crates/jsonschema) — 0.45.1, MSRV 1.83
- Reddit: [Schemars v1 release announcement](https://www.reddit.com/r/rust/comments/1lkcl0m/schemars_v1_is_now_released/) (June 2025)
- docs.rs: [schemars](https://docs.rs/schemars/latest/schemars/) — API documentation
- docs.rs: [jsonschema](https://docs.rs/jsonschema/0.30.0/jsonschema/) — API documentation
