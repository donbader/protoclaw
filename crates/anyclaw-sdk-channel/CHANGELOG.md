# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.10](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-channel-v0.3.9...anyclaw-sdk-channel-v0.3.10) - 2026-04-19

### Other

- updated the following local packages: anyclaw-sdk-types

## [0.3.9](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-channel-v0.3.8...anyclaw-sdk-channel-v0.3.9) - 2026-04-19

### Added

- add SenderInfo and was_mentioned to ChannelSendMessage ([#83](https://github.com/donbader/anyclaw/pull/83))

## [0.3.8](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-channel-v0.3.7...anyclaw-sdk-channel-v0.3.8) - 2026-04-18

### Other

- updated the following local packages: anyclaw-sdk-types

## [0.3.7](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-channel-v0.3.6...anyclaw-sdk-channel-v0.3.7) - 2026-04-17

### Other

- updated the following local packages: anyclaw-sdk-types

## [0.3.6](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-channel-v0.3.5...anyclaw-sdk-channel-v0.3.6) - 2026-04-17

### Other

- updated the following local packages: anyclaw-sdk-types

## [0.3.5](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-channel-v0.3.4...anyclaw-sdk-channel-v0.3.5) - 2026-04-17

### Added

- rich media delivery, reply/thread context, and agent-initiated push ([#33](https://github.com/donbader/anyclaw/pull/33))

## [0.3.4](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-channel-v0.3.3...anyclaw-sdk-channel-v0.3.4) - 2026-04-16

### Other

- finalize roadmap, add channel features, fix sdk-channel doc link

## [0.3.3](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-channel-v0.3.2...anyclaw-sdk-channel-v0.3.3) - 2026-04-16

### Other

- expand SDK crate READMEs with examples and quick start

## [0.3.2](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-channel-v0.3.1...anyclaw-sdk-channel-v0.3.2) - 2026-04-16

### Added

- *(sdk)* extensions report defaults via initialize response

## [0.3.1](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-channel-v0.3.0...anyclaw-sdk-channel-v0.3.1) - 2026-04-15

### Other

- *(04-03)* inline all known limitations from AGENTS.md and CONCERNS.md

## [0.3.0](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-channel-v0.2.8...anyclaw-sdk-channel-v0.3.0) - 2026-04-14

### Added

- *(04-01)* type Channel trait + ChannelHarness with JsonRpc structs

### Fixed

- *(sdk-channel)* two-phase shutdown to prevent permission response loss
- *(01-02)* resolve all clippy warnings across workspace

### Other

- update AGENTS.md for non-blocking permission API change
- *(sdk-channel)* make permission handling non-blocking in harness
- *(01-01)* propagate lint inheritance to all 19 workspace crates

## [0.2.8](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-channel-v0.2.7...anyclaw-sdk-channel-v0.2.8) - 2026-04-14

### Other

- clean up AGENTS.md files — remove changelogs, fix stale refs, deduplicate

## [0.2.7](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-channel-v0.2.6...anyclaw-sdk-channel-v0.2.7) - 2026-04-13

### Other

- lint

## [0.2.6](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-channel-v0.2.5...anyclaw-sdk-channel-v0.2.6) - 2026-04-13

### Added

- *(dx)* log noise filtering, permission tracing, dev.sh rebuild helper

## [0.2.5](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-channel-v0.2.4...anyclaw-sdk-channel-v0.2.5) - 2026-04-13

### Other

- updated the following local packages: anyclaw-sdk-types

## [0.2.4](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-channel-v0.2.3...anyclaw-sdk-channel-v0.2.4) - 2026-04-12

### Added

- *(sdk)* mark public enums non_exhaustive and document API stability
- *(agents)* negotiate ACP protocol version (v1/v2 both accepted)

## [0.2.3](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-channel-v0.2.2...anyclaw-sdk-channel-v0.2.3) - 2026-04-12

### Other

- fix trailing whitespace in test modules

## [0.2.2](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-channel-v0.2.1...anyclaw-sdk-channel-v0.2.2) - 2026-04-12

### Fixed

- remove unused rstest imports across workspace (43 files)

## [0.2.1](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-channel-v0.2.0...anyclaw-sdk-channel-v0.2.1) - 2026-04-12

### Other

- release ([#13](https://github.com/donbader/anyclaw/pull/13))

## [0.2.0](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-channel-v0.1.0...anyclaw-sdk-channel-v0.2.0) - 2026-04-12

### Fixed

- add tokio io-std feature for sdk-channel (fixes release publish)

### Other

- add missing_docs lint and doc comments to all SDK crates
- remove async-trait, trim tokio features, bump SDK crates to 0.2.0
