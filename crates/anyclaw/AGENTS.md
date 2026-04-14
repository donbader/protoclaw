# anyclaw — Binary Crate

Main entry point for the anyclaw sidecar. CLI parsing, config loading, and the Supervisor that orchestrates all three managers.

## Files

| File | Purpose |
|------|---------|
| `main.rs` | Tracing init, CLI dispatch |
| `cli.rs` | Clap derive: `run`, `init`, `validate`, `status` subcommands. `--config` flag / `ANYCLAW_CONFIG` env |
| `supervisor.rs` | Three-manager boot/health/restart/shutdown loop |
| `init.rs` | `anyclaw init` — scaffold `anyclaw.yaml` |
| `status.rs` | `anyclaw status` — runtime health check |
| `banner.rs` | ASCII banner for startup |
| `lib.rs` | Re-exports supervisor for test access |

## Supervisor Lifecycle

1. `Supervisor::new(config)` — creates mpsc channels, watch channels for port discovery
2. `run()` — installs SIGTERM/SIGINT handler, delegates to `run_with_cancel()`
3. `boot_managers()` — iterates `MANAGER_ORDER` (tools → agents → channels), calls `start()` then spawns `run()` as tokio task
4. Health loop — `tokio::select!` on cancel signal + health interval tick
5. `check_and_restart_managers()` — detects finished join handles, applies backoff + crash tracking
6. `shutdown_ordered()` — reverse iteration, per-manager timeout, abort on timeout

## Key Implementation Details

- `MANAGER_ORDER: [&str; 3] = ["tools", "agents", "channels"]` — DO NOT CHANGE
- `ManagerSlot` holds: name, cancel token (child of root), join handle, backoff, crash tracker, `disabled: bool`
- `disabled: bool` on `ManagerSlot` — set `true` when a manager exceeds its crash loop threshold. A disabled manager is not restarted. If the disabled manager is considered critical (currently: any manager), the root cancellation token is also cancelled, triggering full supervisor shutdown.
- `ManagerKind` enum wraps all three manager types for dynamic dispatch
- `create_manager()` factory wires mpsc channels based on manager name
- `agents_cmd_tx` is captured AFTER agents manager `start()` — channels manager needs it
- `debug_http_port_rx` watch channel forwards port from channel subprocess to supervisor
- `boot_notify` (test-only) signals when all managers are up

## Anti-Patterns (this crate)

- Do not add managers without updating `MANAGER_ORDER`, `ManagerKind`, and `create_manager()`
- Do not use `anyhow` outside `supervisor.rs`, `init.rs`, `status.rs`, `main.rs`
- `cmd_rx` fields are `Option<Receiver>` consumed via `.take()` — never accessed twice
- Do not remove the `disabled` flag check in the health loop — it prevents crash-looping managers from being restarted indefinitely
- When a manager is disabled due to crash loop, the root cancellation token is cancelled — this is intentional escalation. Do not downgrade this to a per-manager cancel only.
