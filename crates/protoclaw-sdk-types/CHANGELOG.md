# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0](https://github.com/donbader/protoclaw/compare/protoclaw-sdk-types-v0.3.0...protoclaw-sdk-types-v0.4.0) - 2026-04-13

### Added

- *(config)* [**breaking**] replace args field with StringOrArray, update Example 02 for direct ACP spawn

### Fixed

- *(agents)* handle OpenCode permission request schema mismatch

### Other

- *(sdk-types)* add missing field-level doc comments to ACP wire types

## [0.3.0](https://github.com/donbader/protoclaw/compare/protoclaw-sdk-types-v0.2.3...protoclaw-sdk-types-v0.3.0) - 2026-04-12

### Added

- *(sdk)* mark public enums non_exhaustive and document API stability
- *(sdk-types,telegram)* add ContentKind::AvailableCommandsUpdate and Telegram setMyCommands

### Other

- *(acp)* document extension types and clean agent-specific references
- update AGENTS.md with v0.3.0 Phase 80 changes

## [0.2.3](https://github.com/donbader/protoclaw/compare/protoclaw-sdk-types-v0.2.2...protoclaw-sdk-types-v0.2.3) - 2026-04-12

### Other

- fix trailing whitespace in test modules

## [0.2.2](https://github.com/donbader/protoclaw/compare/protoclaw-sdk-types-v0.2.1...protoclaw-sdk-types-v0.2.2) - 2026-04-12

### Fixed

- remove unused rstest imports across workspace (43 files)

## [0.2.1](https://github.com/donbader/protoclaw/compare/protoclaw-sdk-types-v0.2.0...protoclaw-sdk-types-v0.2.1) - 2026-04-12

### Other

- add missing_docs lint and doc comments to all SDK crates
