---
phase: 07-config-schema-cleanup
plan: 01
status: complete
started: 2026-04-15
completed: 2026-04-15
---

# Plan 07-01 Summary: Remove legacy serde aliases from AnyclawConfig

## What Was Built

Removed all 4 `#[serde(alias)]` attributes from `AnyclawConfig` (`agents-manager`, `channels-manager`, `tools-manager`, `session-store`). Updated the backward-compat test to assert hyphenated keys are silently ignored (fields get defaults) rather than parsed via aliases.

## Key Files

### Modified
- `crates/anyclaw-config/src/types.rs` — Removed 4 alias attributes, updated test

## Verification

- `cargo test -p anyclaw-config`: 135 passed, 0 failed
- `cargo clippy -p anyclaw-config`: 0 warnings
- `cargo build --workspace`: success
- `grep 'serde(alias' crates/anyclaw-config/src/types.rs`: 0 matches

## Commit

- `909a7db` — refactor(config): remove legacy serde alias attributes from AnyclawConfig
