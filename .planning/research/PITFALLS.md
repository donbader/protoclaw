# Domain Pitfalls

**Domain:** Adding schemars JSON Schema generation + defaults consolidation to existing Figment-based Rust config
**Researched:** 2026-04-15

## Critical Pitfalls

Mistakes that cause compile failures, silent schema incorrectness, or broken config loading for existing users.

### Pitfall 1: Deriving JsonSchema on Types with Custom Deserialize Impls

**What goes wrong:** Adding `#[derive(JsonSchema)]` to `StringOrArray` or `PullPolicy` fails to compile or generates a schema that doesn't match what serde actually accepts. schemars derives schema from the *type structure*, but these types have custom `Deserialize` impls that accept different input shapes than the struct definition implies.

**Why it happens:** `StringOrArray` accepts either a bare string `"opencode"` or an array `["opencode", "acp"]` via a custom untagged deserializer. `PullPolicy` accepts lowercase strings (`"always"`, `"never"`, `"if_not_present"`) and maps `None`/`""` to `IfNotPresent`. The derive macro doesn't see this logic — it sees `pub struct StringOrArray(pub Vec<String>)` and generates a schema for an array only, missing the string form entirely.

**Consequences:**
- `StringOrArray` schema says "array of strings" — IDE rejects `binary: "opencode"` (the most common form)
- `PullPolicy` schema says enum with PascalCase variants — IDE rejects `pull_policy: always`
- Users see red squiggles on valid config, lose trust in the schema, stop using it

**Prevention:** Implement `JsonSchema` manually for both types. For `StringOrArray`, use `anyOf: [string, array of strings]`. For `PullPolicy`, use `enum: ["always", "never", "if_not_present"]`. Write a validation test that checks known-good YAML snippets against the generated schema — if the schema rejects valid config, the test fails.

**Detection:** A test that validates `binary: "opencode"`, `binary: ["opencode", "acp"]`, and `pull_policy: always` against the generated schema. If any fail, the manual impl is wrong or missing.

**Phase:** JSON Schema generation phase. Must be done before schema is committed.

### Pitfall 2: Removing serde(alias) Is a Silent Breaking Change

**What goes wrong:** Dropping `#[serde(alias = "agents-manager")]` (and the other 3 aliases on `AnyclawConfig`) means existing `anyclaw.yaml` files using the hyphenated form silently fail to populate those fields. Figment doesn't error on unknown keys by default — it just ignores them. The field falls back to its `#[serde(default)]` value, producing an empty manager config with no agents, channels, or tools.

**Why it happens:** schemars doesn't support `#[serde(alias)]` — the schema will only show the primary field name. The natural instinct is to drop aliases to make the schema accurate. But Figment's permissive parsing means the old key becomes an ignored unknown key rather than a hard error.

**Consequences:** Users upgrade, their config "loads successfully" but all their agents/channels/tools vanish. No error message. The supervisor boots with empty managers. Extremely confusing to debug — the config file looks correct but nothing works.

**Prevention:**
1. Drop aliases AND add `#[serde(deny_unknown_fields)]` on `AnyclawConfig` so the old hyphenated keys produce a hard error at load time with a clear message
2. OR: keep aliases in serde but accept the schema won't validate alias usage (document this)
3. If dropping: add a migration note to the changelog and a startup warning that detects common old key names in the raw YAML before parsing

**Detection:** Integration test that loads a config with hyphenated keys (`agents-manager:`) and asserts it either (a) works via alias or (b) fails with a clear error — never silently ignores.

**Phase:** Config schema cleanup phase. Must be decided before schema generation since it affects field names in the schema.

### Pitfall 3: Dual-Default Desync During Incremental Migration

**What goes wrong:** During the migration from `#[serde(default = "default_fn")]` to `defaults.yaml`, there's a window where some defaults live in YAML and some in Rust fns. If a value is in both places with different values, the behavior depends on whether Figment's YAML layer or serde's default fn "wins" — and the answer changes depending on whether the user's YAML file includes that section at all.

**Why it happens:** Figment `merge()` semantics: the user YAML layer wins over the defaults YAML layer for any key present in both. But `#[serde(default = "fn")]` only fires when a key is *completely absent* from the merged Figment result. So: if `defaults.yaml` sets `acp_timeout_secs: 30` and the serde fn returns `60`, the YAML value (30) wins when the key exists in defaults.yaml. But if you remove the key from defaults.yaml without removing the serde fn, the fn's value (60) takes over. During migration, it's easy to update one but not the other.

**Consequences:** Default values silently change between commits. A field that defaulted to 30 suddenly defaults to 60 (or vice versa). No test catches it unless you explicitly assert every default value. Existing users who relied on the old default get different behavior without changing their config.

**Prevention:**
- Migrate atomically per struct: move ALL defaults for a given struct from fns to YAML in one commit, then remove the fns
- Keep the `#[serde(default)]` attribute (no fn) on fields so Figment knows they're optional, but let the YAML layer provide the actual value
- Write a "defaults snapshot test" that loads config from only `defaults.yaml` (no user file) and asserts every field matches expected values. Run this test after every migration step.
- Never have the same default value defined in both a `default_*` fn and `defaults.yaml` simultaneously — that's the desync window

**Detection:** The snapshot test. Also: `grep -c 'default = "default_' types.rs` should monotonically decrease to zero across migration commits.

**Phase:** Defaults consolidation phase. The very first thing to get right — everything else builds on it.

### Pitfall 4: schemars default Property Serializes via the Rust fn, Not the YAML Value

**What goes wrong:** After migrating defaults to YAML, you keep `#[serde(default = "default_acp_timeout_secs")]` on fields so serde knows they're optional. schemars sees this attribute and serializes the fn's return value into the schema's `"default": 30` property. If you later change the default in `defaults.yaml` to 60 but forget to update the Rust fn (or remove it), the schema advertises `"default": 30` while the actual runtime default is 60.

**Why it happens:** schemars calls the `default = "path"` function at schema generation time and serializes its return value. It has no knowledge of Figment's YAML layering. The schema's `default` property and the actual Figment-resolved default are two independent code paths.

**Consequences:** IDE shows wrong default values in hover documentation. Users think a field defaults to X when it actually defaults to Y. Subtle misconfiguration.

**Prevention:**
- After migration, change `#[serde(default = "default_fn")]` to plain `#[serde(default)]` (which calls `Default::default()` on the field type). The actual default value comes from the YAML layer, not from serde.
- Use `#[schemars(default)]` or a `transform` to inject the correct default from the YAML source if you want the schema to show accurate defaults
- OR: accept that the schema won't show default values (fields are just marked as not-required) and document defaults separately
- The cleanest approach: remove all `default_*` fns, use `#[serde(default)]` on fields, let Figment YAML provide values, and use a `#[schemars(transform)]` to inject defaults from the parsed `defaults.yaml` into the schema

**Detection:** A test that generates the schema, parses `defaults.yaml`, and asserts that every schema `default` value matches the corresponding YAML value. If they diverge, the test fails.

**Phase:** Spans both defaults consolidation and schema generation phases. Design the approach during defaults consolidation; implement during schema generation.

### Pitfall 5: Figment Merge Order Breaks Per-Extension Defaults Layering

**What goes wrong:** Extension defaults are layered in the wrong order, causing core defaults to override extension-specific values or user config to be overridden by extension defaults. The Figment chain `core_defaults.merge(ext_defaults).merge(user_yaml)` looks right, but if extension defaults are added with `join()` instead of `merge()`, or if multiple extensions' defaults conflict on shared keys, the result is unpredictable.

**Why it happens:** Figment's `merge()` and `join()` have opposite precedence: `merge()` = later wins, `join()` = earlier wins. The correct chain is `Figment::from(core_defaults).join(ext_a_defaults).join(ext_b_defaults).merge(user_yaml)` — extensions fill gaps in core defaults (join), user config overrides everything (merge). Getting this backwards means extension defaults silently override user config.

**Consequences:** User sets `acp_timeout_secs: 120` in their YAML, but an extension's defaults.yaml also sets `acp_timeout_secs: 30`. If the extension layer uses `merge()` instead of `join()`, the extension's value wins and the user's explicit config is silently ignored. Extremely hard to debug — the user's file is correct but the runtime value is wrong.

**Prevention:**
- Establish a clear layering convention and document it: `core_defaults (from) → ext_defaults (join each) → user_yaml (merge) → env_vars (merge)`
- Write a test that sets a value in user YAML, sets a different value in extension defaults, and asserts the user value wins
- Write a test that omits a value from user YAML, sets it in extension defaults, and asserts the extension value is used
- Never use `merge()` for extension defaults — always `join()`

**Detection:** Test with conflicting values at each layer. If user config doesn't win over extension defaults, the merge order is wrong.

**Phase:** Per-extension defaults phase. Must be designed carefully before implementation.

## Moderate Pitfalls

### Pitfall 6: Internally-Tagged Enum Schema Confuses YAML Validators

**What goes wrong:** `WorkspaceConfig` and `SessionStoreConfig` use `#[serde(tag = "type", rename_all = "snake_case")]`. schemars generates a correct JSON Schema for internally-tagged enums using `oneOf` with a `const` discriminator on the `type` property. However, yaml-language-server (the Red Hat YAML extension) has historically had issues with complex `oneOf` schemas — it may show false errors, fail to provide autocomplete for variant-specific fields, or show all variants' fields as valid for any variant.

**Why it happens:** JSON Schema `oneOf` with a discriminator property is well-specified in Draft 2020-12, but yaml-language-server's schema resolution doesn't always handle it perfectly. The `type` property name also collides with JSON Schema's own `type` keyword in error messages, making diagnostics confusing.

**Consequences:** IDE shows false validation errors on valid `workspace: { type: local, binary: "..." }` config. Users disable schema validation or ignore real errors because of noise. The schema is technically correct but practically unhelpful for the most complex parts of the config.

**Prevention:**
- Test the generated schema with yaml-language-server manually before committing — paste a sample config and verify autocomplete works for both `local` and `docker` workspace variants
- If yaml-language-server struggles, consider using `#[schemars(transform)]` to simplify the generated schema for these enums (e.g., using `if/then/else` instead of `oneOf`)
- Add `description` fields to each variant to help users understand which properties apply to which type

**Detection:** Manual testing with VS Code + YAML extension. Automated: validate both `type: local` and `type: docker` YAML snippets against the schema with `jsonschema` crate.

**Phase:** JSON Schema generation phase. Test after initial schema generation, before committing.

### Pitfall 7: CI Schema Drift Check Is Flaky Across Platforms

**What goes wrong:** The `#[test] fn schema_is_up_to_date()` test generates the schema at test time and compares it byte-for-byte against the committed `anyclaw.schema.json`. The test fails on CI even though no config types changed, because schemars' output differs slightly between Rust compiler versions, schemars patch versions, or due to HashMap iteration order affecting property ordering in the JSON output.

**Why it happens:** JSON Schema generation involves serializing Rust types to JSON. HashMap key ordering is non-deterministic in Rust (randomized per process by default). schemars uses `BTreeMap` internally for properties (so ordering is stable), but if any part of the pipeline touches a `HashMap` — or if a schemars patch release changes formatting — the output changes without a semantic difference.

**Consequences:** CI fails on unrelated PRs. Developers learn to blindly regenerate and commit the schema without reviewing changes. The drift check becomes a rubber-stamp step rather than a meaningful guard.

**Prevention:**
- Use semantic comparison, not byte-for-byte: parse both JSON files into `serde_json::Value` and compare the Values, not the strings
- Pin `schemars` version exactly in `Cargo.toml` (not `"1.2"` but `"=1.2.1"`) to prevent patch-level output changes
- If using string comparison, normalize: generate with `serde_json::to_string_pretty()` with sorted keys (schemars already sorts, but verify)
- The test error message should include a diff and the regeneration command: `"Schema out of date. Run: cargo run -- schema > schema/anyclaw.schema.json"`

**Detection:** CI failures on PRs that don't touch config types. If this happens more than once, switch to semantic comparison.

**Phase:** CI schema drift check phase. Design the comparison strategy upfront.

## Minor Pitfalls

### Pitfall 8: constants.rs DEFAULT_* Consts Drift from defaults.yaml After Migration

**What goes wrong:** `constants.rs` has `DEFAULT_BACKOFF_BASE_MS`, `DEFAULT_BACKOFF_MAX_SECS`, `DEFAULT_CRASH_MAX`, `DEFAULT_CRASH_WINDOW_SECS`. These are used by `ExponentialBackoff::default()` and `CrashTracker::default()` in `anyclaw-core`. After migrating config defaults to `defaults.yaml`, the YAML values and the constants are two independent sources of truth. Someone changes the YAML default without updating the constant (or vice versa), and the backoff/crash tracker behavior diverges from what the config system advertises.

**Why it happens:** The constants serve `anyclaw-core` (which doesn't depend on `anyclaw-config`), while `defaults.yaml` serves the config loading path. They're in different crates with no compile-time link between them.

**Prevention:**
- Either: make `anyclaw-core`'s `Default` impls read from a shared source (but this creates a dependency cycle — core can't depend on config)
- Or: remove the `DEFAULT_*` consts from `constants.rs` and have `ExponentialBackoff::default()` / `CrashTracker::default()` use inline literals that match the YAML. Add a cross-crate test that loads `defaults.yaml` and asserts the values match the `Default` impls.
- Or: keep the consts but add a test in `anyclaw-config` that asserts `DEFAULT_BACKOFF_BASE_MS == BackoffConfig::default().base_delay_ms` etc.

**Detection:** Cross-crate assertion test. If constants and YAML diverge, the test fails.

**Phase:** Defaults consolidation phase. Address alongside the `default_*` fn migration.

### Pitfall 9: Removing default_* Fns Breaks Struct Default Impls

**What goes wrong:** Each config struct has a manual `impl Default` that calls the `default_*` fns (e.g., `BackoffConfig::default()` calls `default_backoff_base_ms()`). Removing the fns without updating the `Default` impls causes compile errors. But more subtly: if you change `#[serde(default = "default_fn")]` to `#[serde(default)]` (no fn), serde now calls `Default::default()` on the *field type* — which for `u64` is `0`, not `30`. The struct's `Default` impl still returns 30, but serde's field-level default returns 0.

**Why it happens:** `#[serde(default)]` on a field means "use `Default::default()` for this field's type." For `u64`, that's `0`. `#[serde(default)]` on a *struct* means "use `Default::default()` for the whole struct." These are different behaviors and easy to confuse.

**Prevention:**
- Use `#[serde(default)]` at the *struct level* (not field level) for structs that have a meaningful `Default` impl. This calls the struct's `Default::default()` for any missing fields, which preserves the correct values.
- OR: keep `#[serde(default = "default_fn")]` on fields during migration, just move the fn's body to return the value from a single source
- Test: deserialize an empty YAML string into each config struct and assert all fields match expected defaults

**Detection:** Deserialize `""` into each config struct type and assert field values. If any field is `0` when it should be `30`, the default mechanism is broken.

**Phase:** Defaults consolidation phase. Must understand the serde default semantics before removing any fns.

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| Defaults consolidation (migrate fns → YAML) | Dual-default desync (Pitfall 3), removing fns breaks Default impls (Pitfall 9), constants.rs drift (Pitfall 8) | Migrate atomically per struct. Snapshot test for all defaults. Cross-crate assertion test for constants. Use struct-level `#[serde(default)]`. |
| Config schema cleanup (aliases, naming) | Silent alias removal (Pitfall 2) | Add `deny_unknown_fields` when dropping aliases, or keep aliases and accept schema limitation. Integration test with old-format config. |
| JSON Schema generation (schemars derives) | Custom Deserialize types (Pitfall 1), schema default values wrong (Pitfall 4), tagged enum IDE issues (Pitfall 6) | Manual `JsonSchema` impls for `StringOrArray` and `PullPolicy`. Decide schema default strategy before deriving. Test with yaml-language-server. |
| CI schema drift check | Flaky cross-platform comparison (Pitfall 7) | Semantic JSON comparison (parse to Value), pin schemars version exactly, clear regeneration instructions in error message. |
| Per-extension defaults | Figment merge order (Pitfall 5) | Use `join()` for extension defaults, `merge()` for user config. Test with conflicting values at each layer. |
| CI config validation | False confidence from incomplete validation | Validate ALL example YAML files, not just one. Include both minimal and full configs. Test both valid and intentionally-invalid files. |
| init.rs template generation | Template hardcodes values that should come from defaults | Read supervisor defaults from `defaults.yaml` or schema `default` values instead of duplicating in `generate_config_yaml()`. |

## Sources

- schemars 1.x docs (Context7, HIGH confidence): `#[serde(default = "path")]` serializes default value into schema `default` property; `#[serde(tag = "type")]` generates internally-tagged schema; `alias` not listed in supported serde attributes
- schemars attribute reference (Context7, HIGH confidence): `default` excludes field from `required` and populates schema `default` property
- Figment docs (Context7, HIGH confidence): `merge()` = later values win for scalars, union for maps; `join()` = earlier values win
- Existing codebase: `types.rs` (23 `default_*` fns, 4 `#[serde(alias)]` usages, 2 custom `Deserialize` impls), `defaults.yaml` (23 lines, ~40% coverage), `constants.rs` (4 `DEFAULT_*` consts), `init.rs` (hardcoded supervisor values)
