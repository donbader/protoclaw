# ACP Schema Migration & Session Persistence

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Adopt the official `agent-client-protocol-schema` crate as the canonical ACP wire types and fix session persistence across container restarts (including replay suppression).

**Architecture:** Replace hand-rolled ACP types in `anyclaw-sdk-types/src/acp.rs` with re-exports from the official schema crate, adding a thin compatibility layer where types diverge (e.g. `McpServer` enum vs flat `McpServerInfo`). Fix session restore by tracking "prompted" state per-session so replay events from `session/load` are suppressed until the first real `session/prompt`.

**Tech Stack:** `agent-client-protocol-schema` v0.11.4, Rust 2024 edition, serde, tokio

---

## Pre-flight

Before starting, revert the uncommitted WIP changes from the debugging session:

```bash
git checkout -- crates/anyclaw-agents/src/manager.rs crates/anyclaw-agents/src/slot.rs crates/anyclaw-sdk-types/src/acp.rs
```

Keep the committed changes (Dockerfile fixes, docker-compose volume/VOLUMES, session_store config, `has_session_capability` helper, `shutdown_all` cleanup).

---

## Task 1: Add `agent-client-protocol-schema` dependency

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Modify: `crates/anyclaw-sdk-types/Cargo.toml`

- [ ] **Step 1: Add workspace dependency**

In the root `Cargo.toml` `[workspace.dependencies]` section, add:

```toml
agent-client-protocol-schema = "0.11"
```

- [ ] **Step 2: Add crate dependency**

In `crates/anyclaw-sdk-types/Cargo.toml`, add under `[dependencies]`:

```toml
agent-client-protocol-schema = { workspace = true }
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p anyclaw-sdk-types`
Expected: compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock crates/anyclaw-sdk-types/Cargo.toml
git commit -m "deps: add agent-client-protocol-schema v0.11"
```

---

## Task 2: Create compatibility layer in `anyclaw-sdk-types`

**Files:**
- Rewrite: `crates/anyclaw-sdk-types/src/acp.rs`

The official schema crate types differ from anyclaw's in several ways:
- `McpServer` is a tagged enum (`Http`/`Sse`/`Stdio`) vs anyclaw's flat `McpServerInfo`
- `LoadSessionRequest` uses `PathBuf` for `cwd` vs `String`
- `InitializeResponse` has `agent_capabilities: AgentCapabilities` (not `Option`)
- Anyclaw has extension `SessionUpdateType` variants (`CurrentModeUpdate`, `ConfigOptionUpdate`, `SessionInfoUpdate`) not in the spec

Strategy: Re-export official types where compatible, define thin wrappers where they diverge, keep extension types local.

- [ ] **Step 1: Write compatibility test for InitializeResponse deserialization**

This is the bug that started the investigation — `agentCapabilities` was flat instead of nested. Write a test using real OpenCode wire JSON:

```rust
#[test]
fn when_opencode_initialize_response_deserialized_then_capabilities_parsed() {
    let wire = serde_json::json!({
        "protocolVersion": 1,
        "agentCapabilities": {
            "loadSession": true,
            "sessionCapabilities": { "fork": {}, "list": {}, "resume": {} },
            "mcpCapabilities": { "http": true },
            "promptCapabilities": { "embeddedContext": true }
        }
    });
    let result: InitializeResult = serde_json::from_value(wire).unwrap();
    let caps = result.agent_capabilities.expect("should have capabilities");
    assert_eq!(caps.load_session, Some(true));
    assert!(caps.session_capabilities.is_some());
}
```

- [ ] **Step 2: Run test to verify it fails with current types**

Run: `cargo test -p anyclaw-sdk-types -- when_opencode_initialize_response`
Expected: FAIL (current `InitializeResult` doesn't have nested `agent_capabilities`)

- [ ] **Step 3: Rewrite `acp.rs` with official schema re-exports + compatibility layer**

Replace the entire `acp.rs` with:
1. Re-export official types that are wire-compatible
2. Define `InitializeResult` wrapping the official `InitializeResponse` (field name mapping)
3. Keep `McpServerInfo` as anyclaw's flat struct with `From<McpServerInfo> for McpServer` conversion
4. Keep `SessionLoadParams` with `cwd: Option<String>` and `mcp_servers: Option<Vec<McpServerInfo>>` (anyclaw needs optional fields since `session/load` may or may not include them)
5. Keep extension `SessionUpdateType` variants locally
6. Keep `ContentPart` locally (official uses `ContentChunk` with different structure)

The exact code for this step is large — the implementor should:
- Start with `pub use agent_client_protocol_schema::agent::{AgentCapabilities, SessionCapabilities, McpCapabilities, PromptCapabilities};`
- Keep `InitializeResult`, `SessionNewParams`, `SessionPromptParams`, `SessionLoadParams`, `SessionUpdateType`, `SessionUpdateEvent`, `ContentPart`, `McpServerInfo` as local types
- Add `impl From<McpServerInfo> for agent_client_protocol_schema::agent::McpServer` for the HTTP transport case
- Verify all existing tests still pass

- [ ] **Step 4: Run all tests**

Run: `cargo test -p anyclaw-sdk-types`
Expected: all tests pass including the new one

- [ ] **Step 5: Commit**

```bash
git add crates/anyclaw-sdk-types/src/acp.rs
git commit -m "refactor(sdk-types): adopt official ACP schema for capability types"
```

---

## Task 3: Fix `InitializeResult` deserialization in agents manager

**Files:**
- Modify: `crates/anyclaw-agents/src/manager.rs`
- Modify: `crates/anyclaw-agents/src/slot.rs`

After Task 2, `InitializeResult` has `agent_capabilities: Option<AgentCapabilities>` (nested). All code that accessed capabilities directly on `InitializeResult` needs updating.

- [ ] **Step 1: Update `has_session_capability` in slot.rs**

The helper currently does:
```rust
self.agent_capabilities.as_ref().and_then(|r| r.session_capabilities.as_ref())
```

After the schema change, `AgentCapabilities` has `session_capabilities: SessionCapabilities` (not `Option`). Update the chain:

```rust
pub(crate) fn has_session_capability(&self, check: fn(&SessionCapabilities) -> bool) -> bool {
    self.agent_capabilities
        .as_ref()
        .and_then(|r| r.agent_capabilities.as_ref())
        .map(|a| check(&a.session_capabilities))
        .unwrap_or(false)
}
```

Note: The exact field access depends on whether `SessionCapabilities` fields are `Option` or not in the official schema. Check the schema crate's `SessionCapabilities` struct and adjust accordingly.

- [ ] **Step 2: Update `load_session` capability checks in manager.rs**

Find all `and_then(|c| c.load_session)` chains and update to access through `agent_capabilities`:

```rust
let supports_load = self.slots[slot_idx]
    .agent_capabilities
    .as_ref()
    .and_then(|r| r.agent_capabilities.as_ref())
    .map(|a| a.load_session)
    .unwrap_or(false);
```

There are two occurrences: one in `heal_session` (~line 623) and one in `try_restore_session` (~line 1284).

- [ ] **Step 3: Run diagnostics and tests**

Run: `cargo clippy -p anyclaw-agents --no-deps -- -D warnings && cargo test -p anyclaw-agents`
Expected: clean

- [ ] **Step 4: Commit**

```bash
git add crates/anyclaw-agents/src/manager.rs crates/anyclaw-agents/src/slot.rs
git commit -m "fix(agents): update capability access for nested AgentCapabilities"
```

---

## Task 4: Add `cwd` and `mcp_servers` to `session/load` params

**Files:**
- Modify: `crates/anyclaw-agents/src/manager.rs`

OpenCode's `session/load` requires `cwd` and `mcpServers` alongside `sessionId`. Without them it returns `-32602 Invalid params`.

- [ ] **Step 1: Extract `fetch_mcp_servers` helper**

The MCP server URL fetching logic is duplicated between `start_session` and the new `heal_session` path. Extract it:

```rust
async fn fetch_mcp_servers(&self, slot_idx: usize) -> Vec<McpServerInfo> {
    let (reply_tx, reply_rx) = oneshot::channel();
    let tool_names = if self.slots[slot_idx].config.tools.is_empty() {
        None
    } else {
        Some(self.slots[slot_idx].config.tools.clone())
    };
    if self.tools_handle.send(ToolsCommand::GetMcpUrls { tool_names, reply: reply_tx }).await.is_err() {
        return Vec::new();
    }
    let urls: Vec<McpServerUrl> = reply_rx.await.unwrap_or_default();
    urls.iter()
        .map(|u| McpServerInfo {
            name: u.name.clone(),
            server_type: "http".into(),
            url: u.url.clone(),
            command: String::new(),
            args: vec![],
            env: vec![],
            headers: vec![],
        })
        .collect()
}
```

- [ ] **Step 2: Update `heal_session` to pass full params**

In the `session/load` path of `heal_session`, build params with `cwd` and `mcp_servers`:

```rust
let cwd = resolve_agent_cwd(&self.slots[slot_idx].config.workspace);
let mcp_servers = self.fetch_mcp_servers(slot_idx).await;

let params = serde_json::to_value(SessionLoadParams {
    session_id: acp_id.clone(),
    cwd: Some(cwd.to_string_lossy().into_owned()),
    mcp_servers: Some(mcp_servers),
})?;
```

- [ ] **Step 3: Update `try_restore_session` similarly**

Same change in `try_restore_session` (~line 1300). Avoid borrowing `self.slots[slot_idx]` mutably while calling `self.fetch_mcp_servers()` — fetch MCP servers before taking the mutable borrow.

- [ ] **Step 4: Run diagnostics**

Run: `cargo clippy -p anyclaw-agents --no-deps -- -D warnings`
Expected: clean

- [ ] **Step 5: Commit**

```bash
git add crates/anyclaw-agents/src/manager.rs
git commit -m "fix(agents): pass cwd and mcpServers in session/load params"
```

---

## Task 5: Route `CreateSession` through stale session recovery

**Files:**
- Modify: `crates/anyclaw-agents/src/manager.rs`

The channels manager's `routing_table` is in-memory — empty after restart. When a Telegram message arrives post-restart, channels sends `AgentsCommand::CreateSession` which calls `create_session()` directly, bypassing `stale_sessions` entirely. `heal_session` only runs from `prompt_session`, but `CreateSession` happens first.

Fix: the `CreateSession` command handler must check `stale_sessions` and attempt `heal_session` before falling back to `create_session`.

- [ ] **Step 1: Write failing test**

```rust
#[rstest]
#[tokio::test]
async fn when_create_session_with_stale_entry_then_heal_session_attempted() {
    // Setup: agent slot with a stale session for the same session_key
    // Send CreateSession command
    // Verify: heal_session path is taken (session/load attempted or fallback create)
    // Verify: session_map is populated
}
```

Model this after the existing `when_stale_sessions_populated_from_store_then_slot_stale_map_contains_them` test pattern.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anyclaw-agents -- when_create_session_with_stale`
Expected: FAIL

- [ ] **Step 3: Update CreateSession handler**

In the `AgentsCommand::CreateSession` match arm (~line 391), replace:

```rust
AgentsCommand::CreateSession {
    agent_name,
    session_key,
    reply,
} => {
    let result = self.create_session(&agent_name, session_key).await;
    let _ = reply.send(result.map_err(|e| e.to_string()));
}
```

With:

```rust
AgentsCommand::CreateSession {
    agent_name,
    session_key,
    reply,
} => {
    let slot_idx = find_slot_by_name(&self.slots, &agent_name);
    let has_stale = slot_idx
        .map(|idx| self.slots[idx].stale_sessions.contains_key(&session_key))
        .unwrap_or(false);

    let result = if has_stale {
        let idx = slot_idx.unwrap();
        match self.heal_session(idx, &agent_name, &session_key).await {
            Ok(()) => self.slots[idx]
                .session_map
                .get(&session_key)
                .cloned()
                .ok_or(AgentsError::ConnectionClosed),
            Err(e) => Err(e),
        }
    } else {
        self.create_session(&agent_name, session_key).await
    };
    let _ = reply.send(result.map_err(|e| e.to_string()));
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p anyclaw-agents -- when_create_session_with_stale`
Expected: PASS

- [ ] **Step 5: Run full test suite**

Run: `cargo test -p anyclaw-agents`
Expected: all pass

- [ ] **Step 6: Commit**

```bash
git add crates/anyclaw-agents/src/manager.rs
git commit -m "fix(agents): check stale sessions before creating new ones"
```

---

## Task 6: Suppress replay events during `session/load`

**Files:**
- Modify: `crates/anyclaw-agents/src/slot.rs`
- Modify: `crates/anyclaw-agents/src/manager.rs`

When `session/load` succeeds, OpenCode replays the full conversation history as `session/update` notifications. These use the same event types as live events — no replay marker exists in the ACP spec. Anyclaw forwards them all to Telegram, flooding the user with old messages.

Fix: track per-session "prompted" state. A session that hasn't received a `session/prompt` since being loaded can only have replay events. Suppress forwarding until the first prompt is sent.

- [ ] **Step 1: Add `awaiting_first_prompt` set to `AgentSlot`**

In `slot.rs`, add a field to track sessions loaded via `session/load` that haven't been prompted yet:

```rust
pub(crate) awaiting_first_prompt: std::collections::HashSet<String>,
```

Initialize as `HashSet::new()` in the constructor.

- [ ] **Step 2: Populate the set when `session/load` succeeds**

In `heal_session`, after a successful `session/load`, insert the ACP session ID:

```rust
slot.awaiting_first_prompt.insert(acp_id.clone());
```

Do the same in `try_restore_session` for the crash recovery path.

- [ ] **Step 3: Clear the flag when `session/prompt` is sent**

In `prompt_session`, after successfully sending the prompt, remove the session from the set:

```rust
self.slots[slot_idx].awaiting_first_prompt.remove(acp_session_id);
```

- [ ] **Step 4: Suppress forwarding in `forward_session_update`**

In `forward_session_update`, before forwarding to channels, check:

```rust
if self.slots[slot_idx].awaiting_first_prompt.contains(&event.session_id) {
    tracing::debug!(
        agent = %self.slots[slot_idx].name(),
        session_id = %event.session_id,
        update_type,
        seq,
        "suppressed replay event during session/load"
    );
    return;
}
```

This goes right after the `normalize_tool_event_fields` call, before the `if let Some(session_key)` block.

- [ ] **Step 5: Write test**

```rust
#[rstest]
#[tokio::test]
async fn when_session_loaded_then_updates_suppressed_until_first_prompt() {
    // Setup: agent slot with awaiting_first_prompt containing "ses-1"
    // Send session/update for "ses-1"
    // Verify: no DeliverMessage sent to channels
    // Then: send prompt for "ses-1"
    // Then: send session/update for "ses-1"
    // Verify: DeliverMessage IS sent to channels
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p anyclaw-agents`
Expected: all pass

- [ ] **Step 7: Commit**

```bash
git add crates/anyclaw-agents/src/slot.rs crates/anyclaw-agents/src/manager.rs
git commit -m "fix(agents): suppress replay events until first prompt after session/load"
```

---

## Task 7: Persist agent data directory across restarts

**Files:**
- Modify: `examples/02-real-agents-telegram-bot/Dockerfile`
- Modify: `examples/02-real-agents-telegram-bot/Dockerfile.dev-builder`
- Modify: `examples/02-real-agents-telegram-bot/anyclaw.yaml`
- Modify: `examples/02-real-agents-telegram-bot/docker-compose.yml`

For `session/load` to work end-to-end, the agent's data directory must survive container recreation. OpenCode stores session data under `XDG_DATA_HOME` (`/home/node/.local/share`). The agent container is spawned by bollard via the Docker socket proxy.

- [ ] **Step 1: Add volume to `anyclaw.yaml`**

Under the agent's `workspace` config, add:

```yaml
volumes:
  - "opencode-agent-data:/home/node/.local/share"
```

- [ ] **Step 2: Enable VOLUMES in socket-proxy**

In `docker-compose.yml`, change `VOLUMES: 0` to `VOLUMES: 1` in the socket-proxy environment. Named volumes are benign storage — not an execution risk.

- [ ] **Step 3: Pre-create data dirs in both Dockerfiles**

In both `Dockerfile` and `Dockerfile.dev-builder`, add before `USER node`:

```dockerfile
RUN mkdir -p /home/node/.local/share /home/node/.local/state && chown -R node:node /home/node/.local
```

Named volumes mount as root; the container runs as `node`. Without this, OpenCode gets `EACCES` on first write.

- [ ] **Step 4: Add anyclaw data volume for session store**

In `docker-compose.yml`, add a volume for anyclaw's SQLite session store:

```yaml
volumes:
  - ./anyclaw.yaml:/workspace/anyclaw.yaml:ro
  - anyclaw-data:/workspace/data
```

And declare the volume at the bottom:

```yaml
volumes:
  anyclaw-data:
```

- [ ] **Step 5: Verify build**

Run: `docker compose -f docker-compose.yml -f docker-compose.dev.yml up --build -d`
Expected: all containers start, no EACCES errors in logs

- [ ] **Step 6: Commit**

```bash
git add examples/02-real-agents-telegram-bot/
git commit -m "fix(example-02): persist agent and session data across restarts"
```

---

## Task 8: Remove `session/close` and `mark_closed` from shutdown

**Files:**
- Modify: `crates/anyclaw-agents/src/manager.rs`
- Modify: `crates/anyclaw-sdk-types/src/acp.rs` (if `session/close` types exist)
- Modify: `ext/agents/mock-agent/src/main.rs`

Sessions are long-lived; TTL handles expiry. `shutdown_all()` should not mark sessions closed or send `session/close` — doing so prevents `load_open_sessions()` from finding them on restart.

- [ ] **Step 1: Remove `mark_closed` loop from `shutdown_all`**

Remove the `for session_key in slot.session_map.keys()` loop that calls `store.mark_closed()`.

- [ ] **Step 2: Remove `session/close` notification from `shutdown_all`**

Remove the `session/close` send block (including the `has_session_capability` check for close).

- [ ] **Step 3: Remove `session/close` handler from mock-agent**

In `ext/agents/mock-agent/src/main.rs`, remove the `"session/close"` match arm and the `handle_session_close` function.

- [ ] **Step 4: Remove `close` field from `SessionCapabilities` if present**

Check if `SessionCapabilities` has a `close` field. If so, remove it.

- [ ] **Step 5: Run tests**

Run: `cargo test -p anyclaw-agents && cargo test -p mock-agent`
Expected: all pass

- [ ] **Step 6: Commit**

```bash
git add crates/anyclaw-agents/src/manager.rs crates/anyclaw-sdk-types/src/acp.rs ext/agents/mock-agent/src/main.rs
git commit -m "fix(agents): remove session/close from shutdown, let TTL handle expiry"
```

---

## Task 9: Update AGENTS.md documentation

**Files:**
- Modify: `AGENTS.md`
- Modify: `crates/anyclaw-agents/AGENTS.md`

- [ ] **Step 1: Update root AGENTS.md**

Update the lifecycle persistence bullet:
- `shutdown_all()` leaves sessions open — TTL handles cleanup
- Sessions survive graceful restarts
- Add agent data persistence note (volumes for agent data dir)

- [ ] **Step 2: Update agents crate AGENTS.md**

- Remove `session/close` from the ACP methods table
- Note that `session/load` requires `cwd` and `mcpServers` params
- Note replay suppression behavior (updates suppressed until first prompt)

- [ ] **Step 3: Commit**

```bash
git add AGENTS.md crates/anyclaw-agents/AGENTS.md
git commit -m "docs: update session lifecycle and ACP method docs"
```

---

## Verification

After all tasks are complete:

1. `cargo clippy --workspace -- -D warnings` — clean
2. `cargo test --workspace` — all pass
3. Manual test: `docker compose -f docker-compose.yml -f docker-compose.dev.yml up --build -d`
   - Send message to bot on Telegram
   - `docker compose restart anyclaw`
   - Send another message — bot should remember previous conversation
   - No replay messages should appear in Telegram
   - Logs should show `recovery_outcome=loaded`
