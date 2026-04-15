# Project Research Summary

**Project:** Anyclaw v1.0.0 Config-Driven Architecture
**Domain:** Rust infrastructure sidecar — config system hardening
**Researched:** 2026-04-15
**Confidence:** HIGH

## Executive Summary

Anyclaw's config system works but has a structural debt problem: defaults are split across two mechanisms (23 Rust `default_*()` fns and a partial `defaults.yaml` covering ~40% of fields), there's no JSON Schema for IDE support, and legacy aliases create naming inconsistencies. The milestone goal is to consolidate defaults into a single source of truth, generate a JSON Schema from the Rust types via schemars 1.2, and wire up CI validation — all without changing runtime behavior.

The recommended approach is a strict 4-phase pipeline where each phase unblocks the next: defaults consolidation → config cleanup → schema generation → CI/IDE integration. This ordering is non-negotiable because generating a schema on the current dual-default system would produce incorrect default values, and generating before cleanup means regenerating after cleanup. The stack additions are minimal (schemars 1.2 as a dependency, jsonschema 0.45 as dev-dep only) and both are mature, high-confidence choices.

The primary risk is the defaults migration itself — it touches every config type's deserialization path and has subtle interactions between Figment's merge semantics and serde's `#[serde(default)]` behavior. The existing test suite (40+ config tests) provides a strong safety net, but the migration must be atomic per-struct to avoid a dual-default window where values silently disagree. Two custom-deserialized types (`StringOrArray`, `PullPolicy`) need manual `JsonSchema` impls — deriving will produce schemas that reject valid config.

## Key Findings

### Recommended Stack

Two new dependencies, both well-established. No architectural changes to the runtime config loading path.

**Core technologies:**
- **schemars 1.2**: JSON Schema generation from Rust serde types — 27M+ downloads/month, stable since June 2025, generates Draft 2020-12, reads `#[serde(...)]` attributes directly
- **jsonschema 0.45**: CI validation of example YAML against generated schema — dev-dependency only, supports Draft 2020-12, use `default-features = false` to avoid pulling reqwest/TLS

**Already present (no changes):** Figment 0.10, serde, serde_json, serde_yaml.

**Explicitly avoid:** schemars 0.8.x (legacy Draft-07), runtime schema validation (duplicates serde), typify/schemafy (wrong direction).

### Expected Features

**Must have (table stakes):**
- Single source of truth for defaults — migrate all 23 `default_*` fns into `defaults.yaml`
- JSON Schema generation from config types
- Schema committed to repo at `schema/anyclaw.schema.json`
- Config schema cleanup — remove legacy aliases, enforce snake_case
- Generated YAML template from schema/defaults (fix hardcoded `init.rs`)

**Should have (differentiators):**
- IDE autocomplete via yaml-language-server modeline
- CI schema drift check (`#[test] fn schema_is_up_to_date()`)
- CI config validation of example YAML files
- Doc comments → schema descriptions (free with schemars derive)
- Per-manager defaults in YAML (backoff, crash tracker, WASM sandbox)

**Defer (v2+):**
- Per-extension defaults.yaml — highest complexity, lowest urgency
- Schema validation attributes (garde/validator)
- SchemaStore submission — premature for pre-1.0 project
- Config hot-reloading, GUI editor, env var override layer

### Architecture Approach

The architecture change is purely additive — nothing changes at runtime. Figment still loads `defaults.yaml` → user YAML → extract. The new work produces build-time and CI-time artifacts (JSON Schema, validated examples) that improve developer experience around the same config pipeline. The key transformation is collapsing the two-path default resolution (YAML layer vs. serde `default_*` fns) into a single path where all values flow through the YAML layer.

**Modified components (existing files):**
1. **defaults.yaml** — grows from 23 lines to ~80+ lines covering ALL defaults (LOW risk, additive YAML)
2. **types.rs** — add `#[derive(JsonSchema)]`, replace `#[serde(default = "fn")]` with `#[serde(default)]`, remove 23 fns, remove 3 aliases, add manual `JsonSchema` for 2 types (MEDIUM risk, largest surface)
3. **constants.rs** — remove 4 `DEFAULT_*` consts, keep 6 internal guard consts (LOW risk)
4. **init.rs** — replace hardcoded supervisor values with defaults/schema values (LOW risk)

**New components:**
- `schema/anyclaw.schema.json` — generated, committed to repo
- Schema drift test in `anyclaw-config` tests
- Optional `anyclaw schema` CLI subcommand

**Unchanged:** All manager crates, all SDK crates, supervisor, validate.rs, resolve.rs — they consume `AnyclawConfig` the same way.

### Critical Pitfalls

1. **Custom Deserialize types break schemars derive** — `StringOrArray` and `PullPolicy` have custom `Deserialize` impls. Deriving `JsonSchema` generates schemas that reject valid config (e.g., `binary: "opencode"` rejected because schema says "array only"). Prevention: manual `JsonSchema` impls with `oneOf` for StringOrArray, string enum for PullPolicy. Validate known-good YAML snippets against generated schema.

2. **Dual-default desync during migration** — While migrating from `default_*` fns to YAML, having the same default in both places with different values causes silent behavior changes. Figment's YAML layer wins when present, serde fn wins when absent. Prevention: migrate atomically per struct — move ALL defaults for a struct in one commit, then remove the fns. Snapshot test asserting every default value.

3. **Removing serde(alias) is a silent breaking change** — Dropping `#[serde(alias = "agents-manager")]` means existing configs using hyphenated keys silently fail (Figment ignores unknown keys, fields fall back to empty defaults). Prevention: drop aliases AND add `#[serde(deny_unknown_fields)]` so old keys produce hard errors, not silent ignoring.

4. **schemars default property shows Rust fn value, not YAML value** — After migration, if `#[serde(default = "fn")]` survives, schemars serializes the fn's return value into the schema's `"default"` field — which may disagree with the actual Figment-resolved default from YAML. Prevention: change to plain `#[serde(default)]` (no fn) after migration. Accept schema won't show default values, or use `#[schemars(transform)]` to inject from YAML.

5. **Figment merge/join order breaks extension defaults** — `merge()` = later wins, `join()` = earlier wins. Extension defaults must use `join()` (fill gaps), user config must use `merge()` (override everything). Getting this backwards means extension defaults silently override user config. Prevention: test with conflicting values at each layer.

## Implications for Roadmap

Based on research, suggested phase structure. The ordering is driven by strict dependencies — building out of order creates rework.

### Phase 1: Defaults Consolidation
**Rationale:** Everything downstream assumes a single source of truth. Schema generation on the current dual-default system produces incorrect default values.
**Delivers:** All config defaults in `defaults.yaml`, zero `default_*()` fns (except 3 per-entity survivors: `default_true`, `default_agent`, `default_reaction_emoji`), `DEFAULT_*` consts removed from `constants.rs`, `Default` impls updated with inlined literals.
**Addresses:** Single source of truth (table stakes), per-manager defaults in YAML
**Avoids:** Dual-default desync (Pitfall 3), removing fns breaks Default impls (Pitfall 9), constants.rs drift (Pitfall 8)
**Risk:** MEDIUM — touches every config type's deserialization path, but existing 40+ test suite catches regressions immediately. Migrate atomically per struct.

### Phase 2: Config Schema Cleanup
**Rationale:** Clean the schema surface BEFORE generating the schema. Generating first then cleaning means regenerating and re-reviewing.
**Delivers:** 3 `#[serde(alias)]` removed, `#[serde(deny_unknown_fields)]` added, snake_case enforced everywhere, example YAML files updated.
**Addresses:** Config schema cleanup (table stakes)
**Avoids:** Silent alias removal (Pitfall 2)
**Risk:** LOW — breaking change but explicitly allowed per PROJECT.md. Mechanical find-replace.

### Phase 3: JSON Schema Generation
**Rationale:** Depends on Phase 1 (correct defaults) and Phase 2 (clean field names). This is the payoff phase.
**Delivers:** `schemars = "1.2"` added, `#[derive(JsonSchema)]` on all 23 config types, manual `JsonSchema` for `StringOrArray` and `PullPolicy`, `schema/anyclaw.schema.json` committed, `#[test] fn schema_is_up_to_date()` drift check.
**Addresses:** JSON Schema generation (table stakes), schema committed to repo (table stakes), doc comments → descriptions (differentiator)
**Avoids:** Custom Deserialize schema mismatch (Pitfall 1), schema default value divergence (Pitfall 4), tagged enum IDE issues (Pitfall 6)
**Risk:** LOW — additive (new derive, new file, new test). No runtime changes.

### Phase 4: CI Integration + IDE Support
**Rationale:** Pure consumption of the schema artifact from Phase 3.
**Delivers:** `jsonschema` dev-dep for example validation, CI test validating example YAML files, modeline in `anyclaw init` output, `init.rs` reads from defaults instead of hardcoding, `.vscode/settings.json` documentation.
**Addresses:** CI schema drift check, CI config validation, IDE autocomplete, generated YAML template (all differentiators)
**Avoids:** Flaky CI comparison (Pitfall 7) — use semantic JSON comparison, not byte-for-byte
**Risk:** LOW

### Phase 5: Per-Extension Defaults (Optional/Future)
**Rationale:** Highest complexity, lowest urgency. Depends on Phase 1 establishing the pattern. Current extension count (~5) doesn't justify the complexity.
**Delivers:** Convention for `ext/<type>/<name>/defaults.yaml`, extension defaults in Figment chain, defaults for existing extensions.
**Addresses:** Per-extension defaults.yaml (differentiator)
**Avoids:** Figment merge/join order confusion (Pitfall 5)
**Risk:** HIGH — design decision with long-term implications. Defer until extension ecosystem grows.

### Phase Ordering Rationale

- Phases 1→2→3→4 are strictly sequential — each unblocks the next
- Phase 1 is the foundation: without single-source defaults, schema generation produces wrong values
- Phase 2 before 3 avoids generating a schema with dirty field names then regenerating
- Phase 5 is optional and can start after Phase 1 alone if needed
- All phases share a property: zero runtime behavior changes. The config loading path is identical before and after.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 1 (Defaults Consolidation):** Needs careful analysis of which `default_*` fns can be removed vs. which must survive (per-entity defaults in HashMaps). The 3 survivors are identified but verify during planning.
- **Phase 5 (Per-Extension Defaults):** Needs design research on extension discovery mechanism and Figment provider chain ordering.

Phases with standard patterns (skip research-phase):
- **Phase 2 (Config Cleanup):** Mechanical find-replace of aliases. Well-understood.
- **Phase 3 (Schema Generation):** schemars derive is well-documented. Manual impls for 2 types are straightforward.
- **Phase 4 (CI/IDE):** Standard CI patterns. jsonschema crate usage is trivial.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | schemars 1.2 and jsonschema 0.45 verified via Context7 docs, crates.io, GitHub releases. Both mature, high-download crates. |
| Features | HIGH | Feature list derived from PROJECT.md requirements, existing codebase analysis, and established Rust config patterns. |
| Architecture | HIGH | Additive changes to existing Figment pipeline. No novel patterns — schemars derive is standard, defaults migration is mechanical. |
| Pitfalls | HIGH | Pitfalls derived from schemars docs (alias unsupported), Figment docs (merge/join semantics), and direct codebase analysis (23 fns, 4 consts, 2 custom Deserialize impls). |

**Overall confidence:** HIGH

### Gaps to Address

- **Per-entity defaults in HashMaps:** `enabled`, `agent`, `reaction_emoji` fields live on per-entity configs inside `HashMap<String, AgentConfig>` etc. These can't be expressed in top-level YAML. The 3 surviving `default_*` fns are identified, but verify no others fall in this category during Phase 1 planning.
- **yaml-language-server behavior with tagged enums:** schemars generates correct `oneOf` schemas for `WorkspaceConfig` and `SessionStoreConfig`, but yaml-language-server may struggle with complex `oneOf` + discriminator patterns. Needs manual testing during Phase 3 — may require `if/then/else` schema simplification.
- **Schema default value strategy:** Must decide during Phase 1 whether the schema will show default values (requires `#[schemars(transform)]` to inject from YAML) or just mark fields as not-required (simpler but less helpful for IDE hover). This decision spans Phases 1 and 3.
- **`init.rs` template generation:** Currently hardcodes supervisor values. After migration, should read from `defaults.yaml` or schema `default` values — but the exact mechanism needs design during Phase 4 planning.

## Sources

### Primary (HIGH confidence)
- schemars 1.x documentation (Context7) — derive behavior, serde attribute support, `json_schema!` macro, doc comment extraction
- schemars GitHub releases — v1.2.1 (2026-02-01), stable since v1.0.0 (2025-06-23)
- Figment documentation (Context7) — `merge()`/`join()` semantics, `Yaml::string()`, provider chaining
- crates.io — schemars 1.2.1 (27M+ downloads/month), jsonschema 0.45.1 (14M downloads/90d)
- Existing codebase analysis — `types.rs` (23 `default_*` fns, 1497 lines), `defaults.yaml` (23 lines), `constants.rs` (4+6 consts), `init.rs`, `lib.rs`

### Secondary (MEDIUM confidence)
- yaml-language-server GitHub — JSON Schema 2020-12 support confirmed, modeline syntax
- lintel (Rust JSON Schema linter) — CI-friendly alternative for config validation

### Tertiary (LOW confidence)
- yaml-language-server behavior with complex `oneOf` schemas — needs manual validation during Phase 3

---
*Research completed: 2026-04-15*
*Ready for roadmap: yes*
