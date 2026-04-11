---
gsd_state_version: 1.0
milestone: v7.0
milestone_name: Tech Debt & Optimization
status: executing
stopped_at: Phase 72 context gathered
last_updated: "2026-04-11T16:10:21.282Z"
last_activity: 2026-04-11
progress:
  total_phases: 16
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-11)

**Core value:** The agent must stay alive, connected to channels, and able to call tools — regardless of crashes, restarts, or network issues.
**Current focus:** Phase 72 — core-image-optimization

## Current Position

Phase: 72
Plan: Not started
Status: Executing Phase 72
Last activity: 2026-04-11

Next action: Plan Phase 72 (Core Image Optimization)

## Performance Metrics

**Velocity:**

  - v6.0: 8 phases (56-63), 25 requirements — OSS launch complete
  - v5.1: 11 phases (45-55), 25 plans
  - v5.0: 9 phases (36-44), 12 plans, ~40 commits
  - v4.0: 15 phases (23-35), ~31 plans, 193 commits
  - v3.1: 4 phases, 8 plans
  - v3.0: 6 phases, 12 plans
  - v2.1: 4 phases, 16 plans
  - v1.0: 4 phases, 15 plans

## Accumulated Context

### Roadmap Evolution

- v1.0 shipped with 4 phases (1-4), 15 plans
- v2.0 shipped with 4 phases (5-8), 14 plans
- v2.1 shipped with 4 phases (9-12), 16 plans
- v3.0 shipped with 6 phases (13-18), 12 plans
- v3.1 shipped with 4 phases (19-22), 8 plans
- v4.0 shipped with 15 phases (23-35), ~31 plans
- v5.0 shipped with 9 phases (36-44), 12 plans — 29/30 requirements satisfied, 1 accepted-as-is
- v5.1 shipped with 11 phases (45-55), 25 plans — 25/25 requirements satisfied
- v6.0 shipped with 8 phases (56-63), 25 requirements — OSS launch complete, 4 SDK crates on crates.io, Docker images on ghcr.io
- v7.0 shipped with 8 phases (64-71), 22 requirements — tech debt, async-trait removal, architecture dedup, doc lints
  - v7.1 defined with 5 phases (72-76), 16 requirements — Docker image optimization, extension architecture, ghcr.io lifecycle

### Decisions

- No backward compatibility for v5.0 — clean architecture over migration paths
- serde_yaml 0.9.34 deprecated (Figment dependency) — accepted as tech debt, needs future attention
- DOCK-11 accepted-as-is — unified Cargo workspace kept, Docker layer architecture satisfies intent
- Snake_case config keys use serde alias for backward compat with existing kebab-case configs
- Figment env override layer removed — config via YAML file only (SubstYaml interpolation for env vars)
- WASM tools load into native_host but don't register server_urls — WASM-only config produces [mcp:0]
- v6.0: .planning/ gitignored — GSD stays local-only, useful content extracted to docs/
- v6.0: Dual license MIT + Apache-2.0
- v6.0: Fully open contribution model, maintainer reviews all PRs
- v6.0: Automated releases via release-plz (not release-please — cargo workspace bugs #2111, #1896)
- v6.0: Container images on ghcr.io
- v6.0: SDK crates only on crates.io (sdk-types, sdk-channel, sdk-tool, sdk-agent)
- v7.0: MSRV set to 1.85 for native async fn in traits (enables async-trait removal)
- v7.0: SDK crates bump to 0.2.0 after async-trait removal (breaking API change)
- v7.0: Architecture dedup (Phase 68) sequenced after deps (Phase 66) — async-trait must go first

### Blockers/Concerns

- serde_yaml 0.9.34 is deprecated (archived by dtolnay) but Figment 0.10.19 depends on it — v7.0 DEPS-02 patch updates may resolve this

## Session Continuity

Last session: 2026-04-11T14:44:38.193Z
Stopped at: Phase 72 context gathered
  Next action: Plan Phase 64 (CI Hardening — cargo audit, MSRV enforcement, --locked flag)
