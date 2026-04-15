# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.2](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-types-v0.5.1...anyclaw-sdk-types-v0.5.2) - 2026-04-15

### Other

- *(04-02)* add round-trip serde tests for all sdk-types wire types

## [0.5.1](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-types-v0.5.0...anyclaw-sdk-types-v0.5.1) - 2026-04-14

### Added

- *(02-01)* type channel_event.rs, update lib.rs allows, fix downstream
- *(02-01)* type acp.rs — replace Value fields with typed structs

### Fixed

- *(01-02)* resolve all clippy warnings across workspace

### Other

- *(01-01)* propagate lint inheritance to all 19 workspace crates

## [0.5.0](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-types-v0.4.0...anyclaw-sdk-types-v0.5.0) - 2026-04-14

### Fixed

- *(sdk-types)* read availableCommands instead of commands from wire format
- use official ACP schema types for agent capabilities

### Other

- add agent-client-protocol-schema v0.11

## [0.4.0](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-types-v0.3.0...anyclaw-sdk-types-v0.4.0) - 2026-04-13

### Added

- *(config)* [**breaking**] replace args field with StringOrArray, update Example 02 for direct ACP spawn

### Fixed

- *(agents)* handle OpenCode permission request schema mismatch

### Other

- *(sdk-types)* add missing field-level doc comments to ACP wire types

## [0.3.0](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-types-v0.2.3...anyclaw-sdk-types-v0.3.0) - 2026-04-12

### Added

- *(sdk)* mark public enums non_exhaustive and document API stability
- *(sdk-types,telegram)* add ContentKind::AvailableCommandsUpdate and Telegram setMyCommands

### Other

- *(acp)* document extension types and clean agent-specific references
- update AGENTS.md with v0.3.0 Phase 80 changes

## [0.2.3](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-types-v0.2.2...anyclaw-sdk-types-v0.2.3) - 2026-04-12

### Other

- fix trailing whitespace in test modules

## [0.2.2](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-types-v0.2.1...anyclaw-sdk-types-v0.2.2) - 2026-04-12

### Fixed

- remove unused rstest imports across workspace (43 files)

## [0.2.1](https://github.com/donbader/anyclaw/compare/anyclaw-sdk-types-v0.2.0...anyclaw-sdk-types-v0.2.1) - 2026-04-12

### Other

- add missing_docs lint and doc comments to all SDK crates
