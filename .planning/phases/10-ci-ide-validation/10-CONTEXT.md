# Phase 10: CI, IDE & Validation - Context

**Gathered:** 2026-04-16
**Status:** Ready for planning

<domain>
## Phase Boundary

Add YAML modeline to `anyclaw init` output for IDE schema association. Add unknown top-level key detection to `anyclaw validate` with warning output and `--strict` flag. CI-01 and CI-02 are already satisfied by Phase 9's drift test and example validation test (they run in CI via `cargo nextest run`).

</domain>

<decisions>
## Implementation Decisions

### CI requirements (already satisfied)
- **D-01:** CI-01 (schema drift test) is already implemented — `when_committed_schema_file_exists_then_it_matches_generate_schema_output` in `lib.rs` runs in CI via `cargo nextest run`
- **D-02:** CI-02 (example YAML validation) is already implemented — `when_example_configs_loaded_then_they_validate_against_schema` in `lib.rs` runs in CI via `cargo nextest run`
- **D-03:** No changes to `.github/workflows/ci.yml` needed — existing `cargo nextest run --locked --workspace` already runs these tests

### IDE modeline (IDE-01)
- **D-04:** Add `# yaml-language-server: $schema=./anyclaw.schema.json` as the first line of `anyclaw init` generated YAML output
- **D-05:** The modeline uses a relative path — assumes `anyclaw.schema.json` is in the same directory as `anyclaw.yaml` (repo root). This is the standard convention for yaml-language-server.

### Unknown key warnings (SCHM-02)
- **D-06:** Add `pub fn check_unknown_keys(yaml_content: &str) -> Vec<String>` to `validate.rs` — compares top-level YAML keys against schema `properties` keys, returns list of unknown key names
- **D-07:** `anyclaw validate` calls `check_unknown_keys()` and prints warnings (not errors) for each unknown key
- **D-08:** Add `--strict` flag to `Validate` subcommand in `cli.rs` — when set, unknown keys are treated as errors (exit 1)
- **D-09:** Unknown key detection is validate-time only — not at boot. Figment silently ignores unknown keys at runtime, which is the correct behavior for forward compatibility.

### Agent's Discretion
- Whether to also check nested unknown keys (recommendation: top-level only for v1.0.0)
- Warning message format
- Test approach for unknown key detection

</decisions>

<canonical_refs>
## Canonical References

### Init template
- `crates/anyclaw/src/init.rs` — `generate_config_yaml()` — add modeline as first line

### Validation
- `crates/anyclaw-config/src/validate.rs` — add `check_unknown_keys()`
- `crates/anyclaw-config/src/lib.rs` — `generate_schema()` provides schema properties for comparison
- `crates/anyclaw/src/cli.rs` — add `--strict` flag to `Validate` variant
- `crates/anyclaw/src/main.rs` — wire `--strict` into validate dispatch

### CI
- `.github/workflows/ci.yml` — existing CI, no changes needed

### Prior phases
- `.planning/phases/09-json-schema-generation/09-02-SUMMARY.md` — drift test and example validation already in place

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `generate_schema()` already returns `serde_json::Value` with `properties` key — can extract known keys from it
- `validate_schema()` already exists in validate.rs — `check_unknown_keys()` follows the same pattern
- `anyclaw.schema.json` committed at repo root — modeline path points to it

### Established Patterns
- CLI uses Clap derive with `#[arg]` for flags
- Validate dispatch in main.rs already has schema validation → binary checks pipeline
- Warning output uses `eprintln!("  ⚠ ...")` pattern (from existing validate dispatch)

### Integration Points
- `init.rs` — modeline added to format string
- `cli.rs` — `--strict` flag on Validate variant
- `main.rs` — wire strict flag through validate dispatch
- `validate.rs` — new `check_unknown_keys()` function

</code_context>

<specifics>
## Specific Ideas

No specific requirements beyond the standard implementation.

</specifics>

<deferred>
## Deferred Ideas

- Nested unknown key detection — top-level only for v1.0.0
- SchemaStore submission for global IDE support — future milestone

</deferred>

---

*Phase: 10-ci-ide-validation*
*Context gathered: 2026-04-16*
