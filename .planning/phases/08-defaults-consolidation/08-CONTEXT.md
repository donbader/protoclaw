# Phase 8: Defaults Consolidation - Context

**Gathered:** 2026-04-15
**Status:** Ready for planning

<domain>
## Phase Boundary

Expand `defaults.yaml` to cover all config fields with fixed YAML paths. Add drift-detection and completeness tests. Update `anyclaw init` to omit the supervisor section from generated YAML. Serde default fns remain as fallback (not removed) to avoid breaking 40+ existing tests.

</domain>

<decisions>
## Implementation Decisions

### Defaults YAML expansion
- **D-01:** Add all missing fields to `defaults.yaml`: `admin_port`, `ttl_days`, `tools_server_host`, `backoff` (base_delay_ms, max_delay_secs), `crash_tracker` (max_crashes, window_secs), `wasm_sandbox` (fuel_limit, epoch_timeout, memory_limit)
- **D-02:** Values in defaults.yaml MUST match the return values of the corresponding `default_*` fns in types.rs — single source of truth means identical values in both places until fns can be removed

### Serde default fn handling
- **D-03:** Keep all `#[serde(default = "fn")]` attributes as fallback — removing them would break 40+ tests that deserialize without Figment
- **D-04:** The 6 per-entity defaults MUST stay permanently: `default_true` (×3 on `enabled` fields), `default_readonly_true`, `default_agent`, `default_reaction_emoji` — these are inside HashMap values with no fixed YAML path

### Init.rs template
- **D-05:** `anyclaw init` generated YAML omits the supervisor section entirely — defaults apply automatically via Figment. Only emit sections the user must customize (agents, channels).

### Drift detection test
- **D-06:** Add a test that deserializes `defaults.yaml` into `AnyclawConfig` and compares key field values against the `default_*` fn return values. If they diverge, the test fails.

### Completeness test
- **D-07:** Add a test that deserializes `defaults.yaml` alone (without any user YAML) and asserts all non-HashMap-interior fields have non-default-type values (i.e., defaults.yaml provides real values, not zero/empty).

### Agent's Discretion
- Exact YAML structure for nested defaults (where to place backoff/crash_tracker — at manager level or as standalone sections)
- Whether to add doc comments in defaults.yaml explaining each section
- Test implementation details (rstest fixtures, assertion style)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Config types and defaults
- `crates/anyclaw-config/src/types.rs` — 24 `default_*` fns (lines 134, 644-710), all config structs
- `crates/anyclaw-config/src/defaults.yaml` — Current defaults (23 lines, ~40% coverage)
- `crates/anyclaw-config/src/lib.rs` — Figment loading chain (defaults.yaml → SubstYaml → Env)

### Init template
- `crates/anyclaw/src/init.rs` — `generate_config_yaml()` with hardcoded supervisor values

### Research
- `.planning/research/ARCHITECTURE.md` — Defaults migration details, which fns can/cannot move
- `.planning/research/PITFALLS.md` — Pitfall 3 (dual-default desync), Pitfall 4 (serde default semantics)

### Prior phase
- `.planning/phases/07-config-schema-cleanup/07-CONTEXT.md` — Phase 7 decisions (aliases removed)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `defaults.yaml` already exists and is embedded via `include_str!` in `lib.rs` — just needs expansion
- Figment loading chain already handles YAML → struct extraction — no new infrastructure needed
- Existing test `when_parsing_defaults_yaml_then_all_expected_values_present` can be extended for drift detection

### Established Patterns
- All defaults use `#[serde(default = "fn")]` with standalone functions returning literals
- `defaults.yaml` uses flat YAML structure with nested sections matching struct hierarchy
- Tests use `serde_yaml::from_str` directly (not Figment) — this is why serde defaults must stay

### Integration Points
- `crates/anyclaw/src/init.rs` — generates YAML template, currently hardcodes supervisor values
- `crates/anyclaw-config/src/lib.rs` — `DEFAULTS_YAML` constant embeds the file

</code_context>

<specifics>
## Specific Ideas

No specific requirements — standard defaults consolidation following research recommendations.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 08-defaults-consolidation*
*Context gathered: 2026-04-15*
