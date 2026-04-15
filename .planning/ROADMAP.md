# Roadmap: Anyclaw

## Milestones

- ✅ **v0.x Code Quality** - Phases 1-6 (shipped 2026-04-15)
- 🚧 **v1.0.0 Config-Driven Architecture** - Phases 7-11 (in progress)

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

<details>
<summary>✅ v0.x Code Quality (Phases 1-6) — SHIPPED 2026-04-15</summary>

- [x] **Phase 1: Tooling & Lint Infrastructure** - Workspace lints, clippy.toml, rustfmt.toml, deny.toml, coverage setup, dead code removal
- [x] **Phase 2: Leaf Crate Quality** - Typed JSON in sdk-types/jsonrpc/core, error enum audit, serde consistency
- [x] **Phase 3: Manager Crate Quality** - Typed JSON in agents/channels/tools, clone reduction, DashMap migration
- [x] **Phase 4: SDK & External Polish** - Typed JSON in SDK + ext binaries, docs enforcement, inline limitation comments
- [x] **Phase 5: Test Coverage & Verification** - Fill test gaps, coverage baseline, property-based testing for wire types
- [x] **Phase 6: File Decomposition** - Break up agents manager and supervisor into focused modules

</details>

### 🚧 v1.0.0 Config-Driven Architecture (In Progress)

- [x] **Phase 7: Config Schema Cleanup** - Remove legacy serde aliases (breaking change, clean slate for downstream work) (completed 2026-04-15)
- [x] **Phase 8: Defaults Consolidation** - Migrate all default_* fns into defaults.yaml, single source of truth (completed 2026-04-15)
- [x] **Phase 9: JSON Schema Generation** - schemars derives, manual impls, committed schema, CLI subcommand, validate subcommand (completed 2026-04-15)
- [x] **Phase 10: CI, IDE & Validation** - Schema drift test, example validation, YAML modeline, unknown-key warnings (completed 2026-04-15)
- [ ] **Phase 11: Per-Extension Defaults** - Extension defaults.yaml layered into Figment chain

## Phase Details

<!-- v0.x phases collapsed above — detail preserved in git history -->

### Phase 7: Config Schema Cleanup
**Goal**: Config field names are clean and consistent — legacy aliases removed so the schema surface is correct before generation
**Depends on**: Phase 6 (previous milestone)
**Requirements**: SCHM-01
**Success Criteria** (what must be TRUE):
  1. All 4 legacy `#[serde(alias)]` attributes on `AnyclawConfig` are removed
  2. Existing `anyclaw.yaml` files in examples/ use the canonical snake_case field names (no hyphenated keys)
  3. Loading a config with old hyphenated keys silently ignores them (Figment behavior — fields get defaults instead of the intended values)
**Plans:** 1/1 plans complete
Plans:
- [x] 07-01-PLAN.md — Remove 4 legacy serde aliases from AnyclawConfig and update backward-compat test

### Phase 8: Defaults Consolidation
**Goal**: Every config default lives in defaults.yaml — no dual-default mechanism, no DEFAULT_* constants duplicating values
**Depends on**: Phase 7
**Requirements**: DFLT-01, DFLT-02, DFLT-03, DFLT-04
**Success Criteria** (what must be TRUE):
  1. `defaults.yaml` covers all config fields (backoff, crash_tracker, wasm_sandbox, admin_port, tools_server_host, ttl_days)
  2. A drift-detection test asserts that serde default fn values match defaults.yaml values for all shared fields
  3. `anyclaw init` generated YAML omits the supervisor section — only emits sections the user must customize (agents, channels)
  4. A completeness test deserializes defaults.yaml alone and asserts all non-HashMap-interior fields have values
  5. All existing tests pass — no regressions from defaults migration
**Plans:** 2/2 plans complete
Plans:
- [x] 08-01-PLAN.md — Expand defaults.yaml to full coverage + drift-detection and completeness tests
- [x] 08-02-PLAN.md — Remove supervisor section from anyclaw init template

### Phase 9: JSON Schema Generation
**Goal**: A JSON Schema exists for anyclaw.yaml — generated from Rust types, committed to repo, accessible via CLI, and usable for config validation
**Depends on**: Phase 8
**Requirements**: JSCH-01, JSCH-02, JSCH-03, JSCH-04, JSCH-05
**Success Criteria** (what must be TRUE):
  1. All config types in anyclaw-config derive `JsonSchema` (schemars 1.2)
  2. `StringOrArray` and `PullPolicy` have manual `JsonSchema` impls that accept all valid config forms (e.g., `binary: "opencode"` as string, not just array)
  3. `anyclaw.schema.json` is committed at repository root and validates against known-good example configs
  4. `anyclaw schema` CLI subcommand prints the JSON Schema to stdout
  5. `anyclaw validate` validates config files against the JSON Schema (not just binary path checks)
**Plans:** 2/2 plans complete
Plans:
- [x] 09-01-PLAN.md — Add schemars JsonSchema derives + manual impls for all config types
- [x] 09-02-PLAN.md — Generate committed schema, CLI subcommand, validate enhancement

### Phase 10: CI, IDE & Validation
**Goal**: The schema is enforced in CI and surfaced in IDEs — config errors are caught before runtime
**Depends on**: Phase 9
**Requirements**: CI-01, CI-02, IDE-01, SCHM-02
**Success Criteria** (what must be TRUE):
  1. A schema drift test regenerates the schema and compares against the committed file — fails CI if they differ
  2. All example `anyclaw.yaml` files in the repo validate against the committed schema in CI
  3. `anyclaw init` output includes a YAML modeline (`# yaml-language-server: $schema=...`) for IDE schema association
  4. Unknown top-level config keys produce a warning log at startup (not rejection), and `anyclaw validate --strict` treats them as errors
**Plans:** 2/2 plans complete
Plans:
- [x] 10-01-PLAN.md — Add YAML modeline to anyclaw init output (IDE-01) + confirm CI-01/CI-02 already satisfied
- [x] 10-02-PLAN.md — Unknown top-level key warnings + --strict flag on validate (SCHM-02)

### Phase 11: Per-Extension Defaults
**Goal**: Extensions can ship their own defaults.yaml that layers into the Figment chain — base defaults → extension defaults → user config
**Depends on**: Phase 8
**Requirements**: EXT-01
**Success Criteria** (what must be TRUE):
  1. An extension binary can include a `defaults.yaml` that provides default values for its config section
  2. Extension defaults layer correctly: base defaults → extension defaults → user config (extension fills gaps, user overrides all)
  3. Conflicting values between extension defaults and user config resolve in favor of user config
**Plans**: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 7 → 8 → 9 → 10 → 11
(Phase 11 depends on Phase 8, not Phase 10 — can run in parallel with 9/10 if needed)

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Tooling & Lint Infrastructure | v0.x | 3/3 | Complete | 2026-04-14 |
| 2. Leaf Crate Quality | v0.x | 3/3 | Complete | 2026-04-14 |
| 3. Manager Crate Quality | v0.x | 4/4 | Complete | 2026-04-14 |
| 4. SDK & External Polish | v0.x | 4/4 | Complete | 2026-04-15 |
| 5. Test Coverage & Verification | v0.x | 3/3 | Complete | 2026-04-15 |
| 6. File Decomposition | v0.x | 2/2 | Complete | 2026-04-15 |
| 7. Config Schema Cleanup | v1.0.0 | 1/1 | Complete    | 2026-04-15 |
| 8. Defaults Consolidation | v1.0.0 | 2/2 | Complete    | 2026-04-15 |
| 9. JSON Schema Generation | v1.0.0 | 2/2 | Complete    | 2026-04-15 |
| 10. CI, IDE & Validation | v1.0.0 | 2/2 | Complete    | 2026-04-15 |
| 11. Per-Extension Defaults | v1.0.0 | 0/0 | Not started | - |
