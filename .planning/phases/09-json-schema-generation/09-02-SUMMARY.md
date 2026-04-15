---
phase: 09-json-schema-generation
plan: 02
status: complete
commits:
  - 913c533  # feat(config): add generate_schema() and commit anyclaw.schema.json
  - 4949f19  # feat(cli): add anyclaw schema subcommand
  - 675aee0  # feat(config): add validate_schema() and schema-first anyclaw validate
---

# Plan 09-02 Summary: Schema Generation, CLI Subcommand, and Schema-First Validate

## What Was Done

Implemented JSCH-03, JSCH-04, and JSCH-05: a `generate_schema()` function, a committed `anyclaw.schema.json`, an `anyclaw schema` CLI subcommand, and schema-first validation in `anyclaw validate`.

### Task 1 — `generate_schema()` and `anyclaw.schema.json`

- Added `pub fn generate_schema() -> serde_json::Value` to `crates/anyclaw-config/src/lib.rs` using `schemars::schema_for!(AnyclawConfig)`.
- Committed `anyclaw.schema.json` at repo root (2-space indent, trailing newline, `$schema` Draft 2020-12 key).
- Added `jsonschema = "0.28"` and `glob = "0.3"` as dev-dependencies for schema drift and example-config tests.
- Added 5 tests: schema key present, title correct, top-level properties, drift detection (committed file matches `generate_schema()` output), and example configs validate against the committed schema.

### Task 2 — `anyclaw schema` CLI subcommand

- Added `Schema` variant to the `Commands` enum in `crates/anyclaw/src/cli.rs` with doc comment.
- Dispatched in `main.rs`: calls `anyclaw_config::generate_schema()` and prints pretty JSON to stdout. No config file required.
- Added 2 tests confirming the subcommand parses correctly and uses the default config path.

### Task 3 — `validate_schema()` and schema-first `anyclaw validate`

- Moved `jsonschema = "0.28"` from `[dev-dependencies]` to `[dependencies]` in `crates/anyclaw-config/Cargo.toml`.
- Added `pub fn validate_schema(yaml_content: &str) -> Vec<String>` to `crates/anyclaw-config/src/validate.rs`. Parses YAML to `serde_json::Value`, compiles the schema with `jsonschema::validator_for`, and maps errors to `"field/path: message"` strings.
- Updated the `Validate` dispatch in `main.rs` to run `validate_schema()` first. Schema errors cause an early exit with `✗ schema: …` lines; semantic validation (binary checks, etc.) only runs when the schema is clean.
- Added 4 tests: valid YAML → no errors; `log_level: 123` → error mentioning `log_level`; unknown top-level keys → no errors (additionalProperties not restricted); `acp_timeout_secs: "not_a_number"` → type mismatch error.

## Files Modified

| File | Change |
|------|--------|
| `crates/anyclaw-config/Cargo.toml` | Added `jsonschema = "0.28"` (dep + dev), `glob = "0.3"` (dev) |
| `crates/anyclaw-config/src/lib.rs` | `generate_schema()` + 5 tests |
| `crates/anyclaw-config/src/validate.rs` | `validate_schema()` + 4 tests |
| `crates/anyclaw/src/cli.rs` | `Schema` variant + 2 tests |
| `crates/anyclaw/src/main.rs` | Schema dispatch + schema-first Validate dispatch |
| `anyclaw.schema.json` | Generated and committed at repo root |

## Test Results

- **`anyclaw-config`**: 154 passed, 0 failed, 1 ignored
- **`anyclaw`**: 30 passed, 0 failed
- **Full workspace** (excluding integration): all green, 0 failures
- **`cargo clippy --workspace`**: clean, no warnings

## Acceptance Criteria

- [x] `anyclaw.schema.json` exists at repo root and is valid JSON Schema (Draft 2020-12)
- [x] `anyclaw schema` prints the JSON Schema to stdout without requiring a config file
- [x] `anyclaw validate` checks config against JSON Schema before binary-path checks
- [x] Schema violations reported with field path and message
- [x] Example configs in `examples/` validate against the committed schema
- [x] Committed schema file matches `generate_schema()` output (drift test)
- [x] All 4 `validate_schema()` tests pass
- [x] `cargo clippy --workspace` clean
