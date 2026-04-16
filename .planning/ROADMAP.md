# Roadmap: Anyclaw

## Milestones

- ✅ **v0.x Code Quality** — Phases 1-6 (shipped 2026-04-15)
- ✅ **v1.0.0 Config-Driven Architecture** — Phases 7-11 (shipped 2026-04-16)

## Phases

<details>
<summary>✅ v0.x Code Quality (Phases 1-6) — SHIPPED 2026-04-15</summary>

- [x] **Phase 1: Tooling & Lint Infrastructure** - Workspace lints, clippy.toml, rustfmt.toml, deny.toml, coverage setup, dead code removal
- [x] **Phase 2: Leaf Crate Quality** - Typed JSON in sdk-types/jsonrpc/core, error enum audit, serde consistency
- [x] **Phase 3: Manager Crate Quality** - Typed JSON in agents/channels/tools, clone reduction, DashMap migration
- [x] **Phase 4: SDK & External Polish** - Typed JSON in SDK + ext binaries, docs enforcement, inline limitation comments
- [x] **Phase 5: Test Coverage & Verification** - Fill test gaps, coverage baseline, property-based testing for wire types
- [x] **Phase 6: File Decomposition** - Break up agents manager and supervisor into focused modules

</details>

<details>
<summary>✅ v1.0.0 Config-Driven Architecture (Phases 7-11) — SHIPPED 2026-04-16</summary>

- [x] **Phase 7: Config Schema Cleanup** — Remove legacy serde aliases (1 plan)
- [x] **Phase 8: Defaults Consolidation** — Expand defaults.yaml, drift/completeness tests, init cleanup (2 plans)
- [x] **Phase 9: JSON Schema Generation** — schemars derives, manual impls, committed schema, CLI + validate (2 plans)
- [x] **Phase 10: CI, IDE & Validation** — YAML modeline, unknown key warnings, --strict flag (2 plans)
- [x] **Phase 11: Per-Extension Defaults** — Sidecar defaults.yaml loading with options merge (1 plan)

</details>

## Progress

| Phase | Milestone | Plans | Status | Completed |
|-------|-----------|-------|--------|-----------|
| 1-6 | v0.x | 19/19 | Complete | 2026-04-15 |
| 7-11 | v1.0.0 | 8/8 | Complete | 2026-04-16 |
