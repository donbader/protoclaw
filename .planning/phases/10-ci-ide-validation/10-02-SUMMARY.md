---
phase: 10-ci-ide-validation
plan: 02
status: complete
started: 2026-04-16
completed: 2026-04-16
---

# Plan 10-02 Summary: Unknown key warnings + --strict flag

## What Was Built

Added `check_unknown_keys()` to validate.rs — compares top-level YAML keys against JSON Schema properties. Added `--strict` flag to `anyclaw validate` CLI. Unknown keys produce warnings by default (exit 0), errors with `--strict` (exit 1).

## Key Files

### Modified
- `crates/anyclaw-config/src/validate.rs` — Added `check_unknown_keys()` + 4 tests
- `crates/anyclaw/src/cli.rs` — Added `strict: bool` to Validate variant + 2 tests
- `crates/anyclaw/src/main.rs` — Wired unknown key checking into validate dispatch

## Verification

- `cargo test -p anyclaw-config`: 158 passed, 0 failed
- `cargo test -p anyclaw`: 33 passed, 0 failed
- `cargo clippy --workspace`: clean

## Commit

- `3faa33b` — feat(validate): add unknown key warnings and --strict flag
