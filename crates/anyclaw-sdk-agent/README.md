# anyclaw-sdk-agent

[![crates.io](https://img.shields.io/crates/v/anyclaw-sdk-agent.svg)](https://crates.io/crates/anyclaw-sdk-agent)
[![docs.rs](https://img.shields.io/docsrs/anyclaw-sdk-agent)](https://docs.rs/anyclaw-sdk-agent)

Agent adapter for intercepting and transforming ACP messages inside the anyclaw supervisor.

Part of [anyclaw](https://github.com/donbader/anyclaw) — an infrastructure sidecar connecting AI agents to channels and tools.

> ⚠️ **Unstable** — APIs may change between releases.

## What this crate is (and isn't)

**This crate is not for building agent binaries.** Agent binaries speak the ACP wire protocol directly over stdio and don't depend on any anyclaw crate. See [Building agent extensions](https://github.com/donbader/anyclaw/blob/main/ext/agents/AGENTS.md) for that.

This crate provides hooks for intercepting ACP messages *inside* the supervisor, between the manager and the agent subprocess. Use it when you need to transform requests before they reach the agent, or transform responses before they're routed onward — for example, injecting a system prompt into every `session/prompt` call, or logging all `session/update` events.

## Key Types

| Type | Description |
|------|-------------|
| `AgentAdapter` | Trait with per-method hooks for ACP lifecycle events. Override only the methods you need; all others pass through unchanged. |
| `GenericAcpAdapter` | Zero-cost passthrough implementation. Use this when no transformation is needed. |
| `DynAgentAdapter` | Dyn-compatible wrapper for `AgentAdapter`, used internally by the supervisor. |
| `AgentSdkError` | Error type returned by adapter hooks. |

### `AgentAdapter` hooks

| Method | Intercepts |
|--------|-----------|
| `on_initialize_params` / `on_initialize_result` | ACP `initialize` request/response |
| `on_session_new_params` / `on_session_new_result` | ACP `session/new` request/response |
| `on_session_prompt_params` | ACP `session/prompt` request |
| `on_session_update` | Streaming `session/update` events |
| `on_permission_request` | Permission prompt from the agent |

Each hook receives the typed value and returns a (possibly transformed) value of the same type.

## Documentation

- [API reference on docs.rs](https://docs.rs/anyclaw-sdk-agent)
- [Building agent binaries (ACP wire protocol)](https://github.com/donbader/anyclaw/blob/main/ext/agents/AGENTS.md)
- [Building extensions guide](https://github.com/donbader/anyclaw/blob/main/docs/building-extensions.md)

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
