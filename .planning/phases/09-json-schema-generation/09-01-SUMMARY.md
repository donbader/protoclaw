---
phase: 09-json-schema-generation
plan: 01
status: complete
commit: d8b3f20
---

# Plan 09-01 Summary: JsonSchema Derives and Manual Impls

## What Was Done

Added `schemars = "1"` to `crates/anyclaw-config/Cargo.toml` and implemented `JsonSchema` for all 21 config types in `crates/anyclaw-config/src/types.rs`.

### Approach

- **19 standard types**: Added `JsonSchema` to derive list. Schemars 1.x reads serde attributes directly (`tag`, `rename_all`, etc.), so tagged enums and renamed fields generate correct schemas automatically.
- **StringOrArray**: Manual impl using `schemars::json_schema!` macro — produces `oneOf: [{ type: "string" }, { type: "array", items: { type: "string" } }]`.
- **PullPolicy**: Manual impl — produces `{ type: "string", enum: [...], default: "if_not_present" }`. Manual impl required because the `Deserialize` impl is custom (accepts `None`/empty as `IfNotPresent`).

### API Note

The plan suggested `serde_json::json!({...}).try_into()` for manual impls. Schemars 1.x does not implement `TryFrom<serde_json::Value>` for `Schema`. The correct 1.x API is the `schemars::json_schema!({ ... })` macro.

## Files Modified

| File | Change |
|------|--------|
| `crates/anyclaw-config/Cargo.toml` | Added `schemars = "1"` |
| `crates/anyclaw-config/src/types.rs` | Added `use schemars::JsonSchema`, derives on 19 types, 2 manual impls, 8 tests |

## Test Results

- **145 tests pass** (137 existing + 8 new schema structure tests)
- `cargo clippy -p anyclaw-config` — clean, no warnings

## Schema Tests Added

1. `when_anyclaw_config_schema_generated_then_has_2020_12_dialect`
2. `when_anyclaw_config_schema_generated_then_has_expected_property_keys`
3. `when_session_store_config_schema_generated_then_has_type_discriminator`
4. `when_workspace_config_schema_generated_then_has_type_discriminator`
5. `when_string_or_array_schema_generated_then_has_one_of_with_string_and_array`
6. `when_pull_policy_schema_generated_then_is_string_type_with_enum_variants`
7. `when_pull_policy_schema_generated_then_has_default_if_not_present`
8. `when_local_workspace_config_schema_generated_then_binary_field_references_string_or_array`

## Acceptance Criteria

- [x] `crates/anyclaw-config/Cargo.toml` contains `schemars = "1"`
- [x] `types.rs` contains `use schemars::JsonSchema`
- [x] `JsonSchema` derived on all 19 standard types
- [x] Manual `impl JsonSchema for StringOrArray` — oneOf [string, array<string>]
- [x] Manual `impl JsonSchema for PullPolicy` — string enum + default
- [x] 8 schema structure tests pass
- [x] All 137 existing tests pass unchanged
- [x] `cargo clippy -p anyclaw-config` clean
