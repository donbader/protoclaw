# Changelog

All notable changes to the **anyclaw binary** are documented in this file.

For SDK crate changelogs, see:
- [anyclaw-sdk-types](crates/anyclaw-sdk-types/CHANGELOG.md)
- [anyclaw-sdk-agent](crates/anyclaw-sdk-agent/CHANGELOG.md)
- [anyclaw-sdk-channel](crates/anyclaw-sdk-channel/CHANGELOG.md)
- [anyclaw-sdk-tool](crates/anyclaw-sdk-tool/CHANGELOG.md)

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

## [0.9.3] — 2026-04-18

### Fixed

- **Tool call update throttling**: Telegram channel now applies the same edit cooldown to tool call updates as response edits, preventing Telegram API rate limiting during verbose commands (e.g. `apt-get install`) that produce many rapid heartbeats (#63)
- **Finalization delivery fallback**: If editing the response message fails (rate-limited, deleted), the channel now falls back to sending a new message instead of silently dropping the response (#63)

## [0.9.2] — 2026-04-18

### Fixed

- **Stale ToolCallUpdate flood**: Telegram channel now discards tool call heartbeats after the turn enters finalization or the tool reaches a terminal status, preventing the final agent response from being dropped (#61)
- **Agent errors surfaced to channels**: Error messages from agent crashes, timeouts, session failures, and malformed updates are now delivered to the user instead of being silently swallowed (#61)
- **Idle-based prompt timeout**: New `prompt_idle_timeout_secs` config (default: 120s) detects hung agents by tracking activity rather than wall-clock time, avoiding false kills during long-running operations (#61)
- **Telegram access_control schema**: Added access control config to JSON schema for IDE autocomplete (#58)

### Changed

- **Config dotfile paths**: Renamed config directory paths for container environments (#59)

## [0.9.1] — 2026-04-18

### Added

- **Telegram access control**: Group/user allowlists, mention gating, and reply context access control for Telegram channels (#57)

## [0.8.2] — 2026-04-18

### Added

- **Docker extra_hosts support**: `extra_hosts` can now be configured on Docker workspace agents, enabling custom host-to-IP mappings inside agent containers (#54)

## [0.8.1] — 2026-04-17

### Added

- **Reply photo forwarding**: Photos attached to replied-to messages are now downloaded and forwarded to the agent as base64-encoded image data, giving vision-capable models full visual context of reply targets (#52)

## [0.7.1] — 2026-04-17

### Added

- **Media message handling**: Photo, video, audio, voice, document, sticker, animation, video_note, location, and contact messages are now forwarded to the agent instead of being silently dropped. Photos are downloaded and sent as base64-encoded image data so vision-capable models can see them (#49)

## [0.7.0] — 2026-04-17

### Added

- **Rich reply context**: Replies now include sender attribution, partial quotes, and media placeholders for richer conversational context (#46)

## [0.6.1] — 2026-04-17

### Added

- **Rich media delivery**: Images, files, and audio can now flow between agents and channels in both directions (#33)
- **Reply/thread context**: Agents receive context about which message the user is replying to (#33)
- **Agent-initiated push**: Agents can push messages to channels without user input via `session/push` (#33)

### Changed

- **Docker-only distribution**: Dropped native binary releases in favor of Docker-only distribution (#41)
- **CI security scanning**: Replaced `cargo-audit` with `cargo-deny` and optimized CI pipeline (#39)

## [0.5.2] — 2026-04-17

### Fixed

- **Session recovery after idle**: When an agent reports "session not found" for a prompt (e.g., after idle timeout), the dead ACP session mapping is now preserved in `stale_sessions` so `heal_session` can attempt `session/resume` or `session/load` on the next prompt — previously the mapping was dropped, losing conversation history (#34)
- **MCP tool server idle disconnection**: Enable 30s SSE keepalive pings on the aggregated MCP tool server. Without keepalive, rmcp clients time out after their default 300s period, killing the MCP session. This cascaded into agent process death and an infinite crash-respawn loop every ~10 minutes (#36)
- **Kiro session persistence**: Add `kiro-session-data` volume mount for `~/.kiro/` in the kiro example variant. Kiro CLI stores ACP session files at `~/.kiro/sessions/cli/`, which was not covered by the existing `kiro-auth-data` volume — sessions were lost on container restart (#35)

### Changed

- Extract MCP server config construction into testable `build_server_config()` with dedicated tests for each `StreamableHttpServerConfig` requirement (keepalive, stateful mode, allowed hosts)

## [0.5.1] — 2026-04-16

Starting point for tracked binary releases. Prior versions were not formally documented.

[Unreleased]: https://github.com/donbader/anyclaw/compare/v0.9.3...HEAD
[0.9.3]: https://github.com/donbader/anyclaw/compare/v0.9.2...v0.9.3
[0.9.2]: https://github.com/donbader/anyclaw/compare/v0.9.1...v0.9.2
[0.9.1]: https://github.com/donbader/anyclaw/compare/v0.8.2...v0.9.1
[0.8.2]: https://github.com/donbader/anyclaw/compare/v0.8.1...v0.8.2
[0.8.1]: https://github.com/donbader/anyclaw/compare/v0.7.1...v0.8.1
[0.7.1]: https://github.com/donbader/anyclaw/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/donbader/anyclaw/compare/v0.6.1...v0.7.0
[0.6.1]: https://github.com/donbader/anyclaw/compare/v0.5.2...v0.6.1
[0.5.2]: https://github.com/donbader/anyclaw/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/donbader/anyclaw/releases/tag/v0.5.1
