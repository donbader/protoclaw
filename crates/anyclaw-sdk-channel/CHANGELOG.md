# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
