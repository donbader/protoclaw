# Changelog

All notable changes to the **anyclaw binary** are documented in this file.

For SDK crate changelogs, see:
- [anyclaw-sdk-types](crates/anyclaw-sdk-types/CHANGELOG.md)
- [anyclaw-sdk-agent](crates/anyclaw-sdk-agent/CHANGELOG.md)
- [anyclaw-sdk-channel](crates/anyclaw-sdk-channel/CHANGELOG.md)
- [anyclaw-sdk-tool](crates/anyclaw-sdk-tool/CHANGELOG.md)

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

## [0.5.2] — 2026-04-17

### Fixed

- **Session recovery after idle**: When an agent reports "session not found" for a prompt (e.g., after idle timeout), the dead ACP session mapping is now preserved in `stale_sessions` so `heal_session` can attempt `session/resume` or `session/load` on the next prompt — previously the mapping was dropped, losing conversation history (#34)
- **MCP tool server idle disconnection**: Enable 30s SSE keepalive pings on the aggregated MCP tool server. Without keepalive, rmcp clients time out after their default 300s period, killing the MCP session. This cascaded into agent process death and an infinite crash-respawn loop every ~10 minutes (#36)
- **Kiro session persistence**: Add `kiro-session-data` volume mount for `~/.kiro/` in the kiro example variant. Kiro CLI stores ACP session files at `~/.kiro/sessions/cli/`, which was not covered by the existing `kiro-auth-data` volume — sessions were lost on container restart (#35)

### Changed

- Extract MCP server config construction into testable `build_server_config()` with dedicated tests for each `StreamableHttpServerConfig` requirement (keepalive, stateful mode, allowed hosts)

## [0.5.1] — 2026-04-16

Starting point for tracked binary releases. Prior versions were not formally documented.

[Unreleased]: https://github.com/donbader/anyclaw/compare/v0.5.2...HEAD
[0.5.2]: https://github.com/donbader/anyclaw/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/donbader/anyclaw/releases/tag/v0.5.1
