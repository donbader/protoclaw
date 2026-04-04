# protoclaw-sdk-agent — Agent SDK

SDK for building ACP agent adapters. Provides the `AgentAdapter` trait with per-method hooks and `GenericAcpAdapter` as a passthrough default. Used by protoclaw internally to intercept/transform ACP messages between the supervisor and agent subprocess.

## Files

| File | Purpose |
|------|---------|
| `adapter.rs` | `AgentAdapter` trait — per-method hooks for ACP lifecycle |
| `generic.rs` | `GenericAcpAdapter` — default passthrough implementation |
| `error.rs` | `AgentSdkError` enum (thiserror) |
| `lib.rs` | Re-exports `AgentAdapter`, `GenericAcpAdapter`, `AgentSdkError` |

## Key Types

```rust
#[async_trait]
pub trait AgentAdapter: Send + Sync + 'static {
    async fn on_initialize_params(&self, params: Value) -> Result<Value, AgentSdkError>;
    async fn on_initialize_result(&self, result: Value) -> Result<Value, AgentSdkError>;
    async fn on_session_new_params(&self, params: Value) -> Result<Value, AgentSdkError>;
    async fn on_session_new_result(&self, result: Value) -> Result<Value, AgentSdkError>;
    async fn on_session_prompt_params(&self, params: Value) -> Result<Value, AgentSdkError>;
    async fn on_session_update(&self, event: Value) -> Result<Value, AgentSdkError>;
    async fn on_permission_request(&self, request: Value) -> Result<Value, AgentSdkError>;
}

#[derive(Debug, Default, Clone)]
pub struct GenericAcpAdapter;  // All methods pass through unchanged
```

## How to Implement

1. Create a struct implementing `AgentAdapter`
2. Override only the hooks you need — all methods have default passthrough implementations
3. Each hook receives the raw `serde_json::Value` and returns a (possibly transformed) `Value`
4. Use `GenericAcpAdapter` when no transformation is needed (zero-cost passthrough)

**Hook pairs:** `on_initialize_params`/`on_initialize_result`, `on_session_new_params`/`on_session_new_result` — intercept both request and response sides of ACP methods.

**Example:** An adapter that injects custom system prompts into `session/prompt`:
```rust
#[async_trait]
impl AgentAdapter for MyAdapter {
    async fn on_session_prompt_params(&self, mut params: Value) -> Result<Value, AgentSdkError> {
        // Transform params before they reach the agent
        Ok(params)
    }
}
```

## Anti-Patterns (this crate)

- **Don't depend on internal crates** — this is external-facing SDK
- **Don't bypass the adapter** — always go through `AgentAdapter` hooks for ACP message interception
- **Don't add state to `GenericAcpAdapter`** — it's a stateless passthrough; custom adapters own their state
