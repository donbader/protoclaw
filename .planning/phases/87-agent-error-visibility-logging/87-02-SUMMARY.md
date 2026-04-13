---
phase: 87-agent-error-visibility-logging
plan: 02
subsystem: channels, config
tags: [permission-timeout, auto-deny, observability]
dependency_graph:
  requires: []
  provides: [permission-timeout-config, permission-auto-deny]
  affects: [channels-manager, supervisor-config, agents-manager]
tech_stack:
  added: []
  patterns: [tokio-time-timeout, builder-pattern-config-wiring]
key_files:
  created: []
  modified:
    - crates/protoclaw-config/src/types.rs
    - crates/protoclaw-channels/src/manager.rs
    - crates/protoclaw-supervisor/src/lib.rs
    - crates/protoclaw-agents/src/manager.rs
    - crates/protoclaw-test-helpers/src/config.rs
decisions:
  - "permission_timeout_secs as Option<u64> on SupervisorConfig — None preserves block-forever default"
  - "Auto-deny uses option_id 'denied' — consistent with existing permission vocabulary"
  - "Timeout wired via builder method on ChannelsManager rather than full SupervisorConfig passthrough"
metrics:
  duration: 14m
  completed: "2026-04-13"
---

# Phase 87 Plan 02: Permission Response Timeout Summary

Configurable permission timeout with auto-deny and warn logging — unblocks agents when users ignore permission prompts.

## What Was Done

### Task 1: Config + Rename (08f3870)
Added `permission_timeout_secs: Option<u64>` to `SupervisorConfig` with `#[serde(default)]` (defaults to `None`). Renamed `PendingPermission._received_at` to `received_at` in agents manager for future timeout elapsed-time logging. Added rstest tests for YAML parsing.

### Task 2: Timeout Implementation (88c3d95)
Wrapped the permission response `rx.await` in `tokio::time::timeout` when `permission_timeout_secs` is `Some`. On timeout: emits `tracing::warn!` with channel, request_id, elapsed_secs, then sends `AgentsCommand::RespondPermission { option_id: "denied" }`. When `None`, preserves current block-forever behavior. Wired config from `SupervisorConfig` through supervisor to `ChannelsManager` via `with_permission_timeout()` builder. Updated all 7 test-helper config builders.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed test-helpers SupervisorConfig constructors**
- Found during: Task 2
- Issue: 7 explicit `SupervisorConfig` constructors in test-helpers lacked the new field
- Fix: Added `permission_timeout_secs: None` to all 7 constructors
- Files modified: crates/protoclaw-test-helpers/src/config.rs
- Commit: 88c3d95

**2. [Rule 1 - Bug] Added #[allow(dead_code)] on received_at field**
- Found during: Task 1
- Issue: Renaming `_received_at` to `received_at` triggered clippy dead_code warning (field not yet read)
- Fix: Added targeted `#[allow(dead_code)]` with explanation comment
- Files modified: crates/protoclaw-agents/src/manager.rs
- Commit: 08f3870

## Commits

| Task | Hash | Message |
|------|------|---------|
| 1 | 08f3870 | feat(87-02): add permission_timeout_secs to SupervisorConfig and rename _received_at |
| 2 | 7bfebce | feat(87-02): implement permission timeout with auto-deny in channels manager |

## Verification

- `cargo build --workspace` — clean
- `cargo clippy --workspace -- -D warnings` — clean
- `cargo test -p protoclaw-config` — 135 passed
- `cargo test -p protoclaw-agents` — 111 passed
- `cargo test -p protoclaw-channels` — 65 passed

## Self-Check: PASSED
