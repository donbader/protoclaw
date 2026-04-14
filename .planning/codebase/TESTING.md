# Testing Patterns

**Analysis Date:** 2026-04-14

## Test Framework

**Runner:** Built-in `cargo test` with `rstest = "0.23"` for fixtures and parameterized tests.

**Assertion Library:** Standard `assert!`, `assert_eq!`, `assert_ne!`, `assert!(matches!(...))`. No external assertion crate.

**Logging in tests:** `test_log` crate for integration tests — `#[test_log::test(tokio::test)]` enables tracing output during test runs.

**Run Commands:**
```bash
cargo test                                     # All unit tests (all crates)
cargo build --bin mock-agent --bin debug-http   # Required BEFORE integration tests
cargo test -p integration                      # E2E tests (needs binaries built first)
cargo clippy --workspace                       # Lint all crates
```

## Test File Organization

**Location:** Co-located `#[cfg(test)] mod tests` at the bottom of each source file. Every source file with logic has its own test module — 72 `#[cfg(test)]` blocks across the workspace.

**Integration tests:** Separate `tests/integration/` crate with per-flow test files.

**Structure:**
```
crates/*/src/*.rs          → inline #[cfg(test)] mod tests { ... }
tests/integration/
├── src/lib.rs             → re-exports from anyclaw-test-helpers
├── tests/
│   ├── flows_boot.rs      → supervisor boot + health
│   ├── flows_message.rs   → message posting + SSE echo
│   ├── flows_session.rs   → ACP session lifecycle
│   ├── flows_crash.rs     → agent crash recovery
│   ├── flows_resilience.rs → multi-crash, health during recovery
│   ├── flows_shutdown.rs  → graceful shutdown with inflight messages
│   ├── flows_queue.rs     → session queue FIFO behavior
│   ├── flows_thinking.rs  → thought event streaming
│   ├── flows_health.rs    → health endpoint
│   ├── flows_cancel.rs    → cancellation flows
│   ├── flows_permission.rs → permission request/response
│   ├── flows_multi_agent.rs → multi-agent routing
│   ├── flows_dual_channel.rs → dual channel routing
│   ├── flows_acp_wire.rs  → ACP wire format
│   ├── flows_response.rs  → response delivery
│   ├── flows_error_cases.rs → error handling paths
│   ├── flows_sdk_channel.rs → SDK channel integration
│   ├── flows_sdk_tool.rs  → SDK tool integration
│   ├── flows_wasm_e2e.rs  → WASM tool end-to-end
│   ├── flows_docker.rs    → Docker workspace tests
│   ├── flows_docker_advanced.rs → Docker advanced scenarios
│   ├── flows_config_path.rs → config file loading
│   ├── flows_batch_advanced.rs → batch message merging
│   └── example_config*.rs → example config validation
```

## Test Structure

**Unit test pattern:** `#[cfg(test)] mod tests` with `use super::*;` at the top. Use `#[rstest]` for most tests, plain `#[test]` or `#[tokio::test]` also acceptable.

```rust
// crates/anyclaw-agents/src/error.rs — typical unit test
#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn assert_std_error<E: std::error::Error>(_: &E) {}

    #[rstest]
    fn when_spawn_failed_displayed_then_shows_binary_name() {
        let err = AgentsError::SpawnFailed("claude-code".into());
        assert_eq!(err.to_string(), "Failed to spawn agent process: claude-code");
    }

    #[rstest]
    fn when_agents_error_checked_then_implements_std_error() {
        let err = AgentsError::ConnectionClosed;
        assert_std_error(&err);
    }
}
```

**Integration test pattern:** Boot real supervisor with mock-agent, interact via HTTP, assert via SSE events.

```rust
// tests/integration/tests/flows_message.rs
#[test_log::test(tokio::test)]
async fn when_message_posted_then_agent_echoes_back_via_sse() {
    let config = mock_agent_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let mut sse = SseCollector::connect(port).await;

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "ping"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let events = sse.collect_events(Duration::from_secs(10)).await;
    let saw_echo = events.iter().any(|e| e.data.contains("Echo: ") && e.data.contains("ping"));
    assert!(saw_echo, "should have received echo chunk via SSE");

    cancel.cancel();
    let _ = with_timeout(5, handle).await;
}
```

**Error type tests:** Every error enum has tests verifying `Display` output and `std::Error` trait implementation. Pattern: `assert_std_error()` helper function + per-variant display assertions.

## Test Helpers

**Shared crate:** `crates/anyclaw-test-helpers/` — dev-dependency for all crates and integration tests. Re-exported via `tests/integration/src/lib.rs`.

**Key utilities:**

| Module | Function | Purpose |
|--------|----------|---------|
| `config.rs` | `mock_agent_config()` | Standard config with mock-agent + debug-http |
| `config.rs` | `mock_agent_config_with_options(opts)` | Config with custom agent options (e.g., `exit_after`) |
| `config.rs` | `sdk_channel_config()` | Config with mock-agent + debug-http + sdk-test-channel |
| `config.rs` | `sdk_tool_config()` | Config with mock-agent + debug-http + sdk-test-tool |
| `config.rs` | `wasm_tool_config()` | Config with mock-agent + debug-http + WASM echo tool (compiled from WAT at runtime) |
| `config.rs` | `multi_tool_config()` | Config with two tool instances |
| `config.rs` | `invalid_tool_config()` | Config with intentionally bad tool binary for degradation tests |
| `config.rs` | `docker_agent_config()` | Config with Docker workspace agent |
| `config.rs` | `build_mock_agent_docker_image()` | Builds `anyclaw-mock-agent:test` Docker image |
| `config.rs` | `cleanup_test_containers()` | Removes containers with `anyclaw.managed=true` label |
| `supervisor.rs` | `boot_supervisor_with_port(config)` | Boots supervisor, waits for debug-http port, returns `(cancel, handle, port)` |
| `handles.rs` | `make_handle::<C>(buffer)` | Creates `(ManagerHandle<C>, mpsc::Receiver<C>)` pair |
| `poll.rs` | `wait_for_condition(timeout_ms, closure)` | Polls async condition at 100ms intervals until satisfied or timeout |
| `ports.rs` | `wait_for_port(port_rx, timeout_ms)` | Waits for non-zero port on a `watch::Receiver<u16>` |
| `timeout.rs` | `with_timeout(secs, future)` | Wraps future with timeout, panics on expiry |
| `sse.rs` | `SseCollector::connect(port)` | Connects to SSE endpoint, collects/parses events |
| `paths.rs` | `mock_agent_path()`, `debug_http_path()`, etc. | Resolves binary paths relative to workspace root |

## Test Coverage Assessment

**Well-tested areas:**
- Error types — every error enum has per-variant display + `std::Error` trait tests across all crates
- Backoff/crash tracker — `crates/anyclaw-core/src/backoff.rs` has thorough unit tests for delay doubling, cap, reset, crash loop detection
- Constants — `crates/anyclaw-core/src/constants.rs` pins every constant value
- ID newtypes — `crates/anyclaw-core/src/types.rs` tests Display, From, round-trip
- Config loading — `crates/anyclaw-config/src/lib.rs` tests valid/invalid/missing configs, defaults, unknown keys
- Config resolution — `crates/anyclaw-config/src/resolve.rs` tests `@built-in/` path expansion
- Config validation — `crates/anyclaw-config/src/validate.rs` tests binary existence, Docker config parsing
- JSON-RPC codec — `crates/anyclaw-jsonrpc/src/codec.rs` tests encode/decode, empty lines, oversized lines
- SDK types — `crates/anyclaw-sdk-types/src/` tests serde round-trip for all wire types
- SDK agent adapter — `crates/anyclaw-sdk-agent/src/lib.rs` tests passthrough + custom override behavior
- SDK channel harness — `crates/anyclaw-sdk-channel/src/harness.rs` tests initialize handshake, message routing
- SDK tool server — `crates/anyclaw-sdk-tool/src/server.rs` tests tool dispatch
- Test helpers themselves — every helper module has its own `#[cfg(test)]` block
- E2E flows — 27 integration test files covering boot, messaging, sessions, crash recovery, shutdown, permissions, multi-agent, Docker, WASM, SDK channel/tool

**Under-tested areas:**
- Manager `run()` loops — `crates/anyclaw-agents/src/manager.rs` and `crates/anyclaw-channels/src/manager.rs` have large `#[cfg(test)]` blocks but complex state machines are primarily tested via integration tests
- WASM runner internals — `crates/anyclaw-tools/src/wasm_runner.rs` has tests but sandbox edge cases (fuel exhaustion, epoch timeout) rely on integration tests
- Docker backend — `crates/anyclaw-agents/src/docker_backend.rs` has unit tests but real Docker interaction tested only in integration

**Requirements:** No formal coverage target enforced. No coverage tool configured in CI.

## Common Patterns

**Async testing:**
```rust
// Use #[rstest] + #[tokio::test] for async unit tests
#[rstest]
#[tokio::test]
async fn when_condition_met_immediately_then_returns_value() {
    let result = wait_for_condition(1000, || async { Some(42_u32) }).await;
    assert_eq!(result, Some(42));
}

// Integration tests use #[test_log::test(tokio::test)] for tracing output
#[rstest]
#[test_log::test(tokio::test)]
async fn when_supervisor_boots_then_health_endpoint_responds_and_clean_shutdown() {
    let config = mock_agent_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;
    // ... test logic ...
    cancel.cancel();
    let result = with_timeout(5, handle).await.expect("supervisor task panicked");
    assert!(result.is_ok());
}
```

**Integration test lifecycle:** Every E2E test follows this pattern:
1. Build config via helper (`mock_agent_config()`, etc.)
2. Boot supervisor: `let (cancel, handle, port) = boot_supervisor_with_port(config).await;`
3. Optionally connect SSE: `let mut sse = SseCollector::connect(port).await;`
4. Interact via HTTP (`reqwest::Client`)
5. Assert SSE events or HTTP responses
6. Cancel and verify clean shutdown: `cancel.cancel(); let result = with_timeout(5, handle).await;`

**Error testing:**
```rust
// Verify error Display output
#[rstest]
fn when_timeout_displayed_then_shows_duration() {
    let err = AgentsError::Timeout(std::time::Duration::from_secs(30));
    assert_eq!(err.to_string(), "Request timed out after 30s");
}

// Verify std::Error trait implementation
fn assert_std_error<E: std::error::Error>(_: &E) {}
#[rstest]
fn when_agents_error_checked_then_implements_std_error() {
    let err = AgentsError::ConnectionClosed;
    assert_std_error(&err);
}
```

**Config testing with Figment Jail:**
```rust
// crates/anyclaw-config/src/lib.rs — isolated filesystem for config tests
#[test]
fn when_valid_config_file_exists_then_loads_all_sections() {
    Jail::expect_with(|jail| {
        jail.create_file("anyclaw.yaml", r#"..."#)?;
        let config = AnyclawConfig::load(Some("anyclaw.yaml")).unwrap();
        assert_eq!(config.agents_manager.agents.len(), 1);
        Ok(())
    });
}
```

**Mock agent options:** The `mock-agent` binary (`ext/agents/mock-agent/`) supports options like `exit_after` to simulate crashes after N prompts. Pass via `mock_agent_config_with_options()`.

---

*Testing analysis: 2026-04-14*
