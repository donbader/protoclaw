# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-04-11

### Added

- Three-manager architecture (Tools, Agents, Channels) with Supervisor orchestration
- ACP protocol (JSON-RPC 2.0 over stdio) for agent subprocess communication
- MCP host for external tool server connections
- WASM sandbox runner for isolated tool execution
- Telegram channel with message batching and debounce
- Debug HTTP channel for development and testing
- SDK crates for building custom agents, channels, and tools:
  - `protoclaw-sdk-types` — shared wire types, `SessionKey`, `ChannelEvent`
  - `protoclaw-sdk-agent` — `AgentAdapter` trait and `GenericAcpAdapter`
  - `protoclaw-sdk-channel` — `Channel` trait and `ChannelHarness`
  - `protoclaw-sdk-tool` — `Tool` trait and `ToolServer`
- Config-driven operation via `protoclaw.yaml` (Figment: defaults → YAML → env vars)
- Crash recovery with exponential backoff (100ms base, 30s cap) and crash loop detection
- Subprocess isolation — agent, channel, and tool crashes don't take down the supervisor
- `@built-in/` binary prefix resolved against configurable `extensions_dir`
- `ChannelInitializeParams.options` handshake for config-driven channel subprocesses
- Docker support with multi-stage builds and shared dependency cache
- Docker Compose example with mock agent and debug-http channel
- `protoclaw init` and `protoclaw status` CLI subcommands
- rstest-based test suite with BDD naming conventions throughout
- Integration tests that spawn a real supervisor with mock-agent binary
