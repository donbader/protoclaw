# Requirements: Anyclaw Config-Driven Architecture

**Defined:** 2026-04-15
**Core Value:** Every line of code should be there for a reason, with typed data flowing through typed interfaces.

## v1.0.0 Requirements

Requirements for the config-driven architecture milestone. Each maps to roadmap phases.

### Defaults Consolidation

- [ ] **DFLT-01**: `defaults.yaml` expanded to cover all fields with fixed YAML paths (backoff, crash_tracker, wasm_sandbox, admin_port, tools_server_host, ttl_days)
- [ ] **DFLT-02**: Drift-detection test asserts serde default fn values match `defaults.yaml` values for all shared fields
- [ ] **DFLT-03**: `anyclaw init` generated YAML omits supervisor section — only emit sections user must customize (agents, channels)
- [ ] **DFLT-04**: Defaults.yaml completeness test — deserializes defaults.yaml alone and asserts all non-HashMap-interior fields have values

### Config Schema Cleanup

- [ ] **SCHM-01**: All 4 legacy `#[serde(alias)]` attributes on `AnyclawConfig` removed (agents-manager, channels-manager, tools-manager, session-store)
- [ ] **SCHM-02**: Unknown top-level config keys produce a warning log (not rejection). Optional `--strict` flag on `anyclaw validate` treats them as errors.

### JSON Schema Generation

- [ ] **JSCH-01**: `#[derive(JsonSchema)]` on all config types in `anyclaw-config`
- [ ] **JSCH-02**: Manual `JsonSchema` impls for `StringOrArray`, `PullPolicy`, and `deserialize_string_map`
- [ ] **JSCH-03**: Committed `anyclaw.schema.json` file in repository root
- [ ] **JSCH-04**: `anyclaw schema` CLI subcommand that prints JSON Schema to stdout
- [ ] **JSCH-05**: `anyclaw validate` validates config against JSON Schema (not just binary path checks)

### CI Integration

- [ ] **CI-01**: Schema drift test — regenerates schema, compares against committed file
- [ ] **CI-02**: Example `anyclaw.yaml` files validated against schema in CI

### IDE Support

- [ ] **IDE-01**: `anyclaw init` output includes YAML modeline for yaml-language-server schema association

### Per-Extension Defaults

- [ ] **EXT-01**: Each extension can ship a `defaults.yaml` layered into Figment (base defaults → extension defaults → user config)

## Deferred Requirements

Tracked but not in current milestone.

### Performance

- **PERF-01**: Feature-gate wasmtime behind cargo feature flag
- **PERF-02**: Feature-gate bollard (Docker SDK) behind cargo feature flag
- **PERF-03**: Replace poll_channels() polling workaround with FuturesUnordered/StreamMap

### New Capabilities

- **FEAT-01**: Rate limiting on inbound channel messages
- **FEAT-02**: Multi-agent routing (different sessions to different agent types)

### Config Enhancements

- **CFGE-01**: Env var override layer (`ANYCLAW_` prefix with `__` separator) — documented but unimplemented

## Out of Scope

| Feature | Reason |
|---------|--------|
| New runtime features | This is config architecture work, not new capabilities |
| Performance optimization | Separate concern from config cleanup |
| Env var override layer | Documented but unimplemented — separate milestone |
| Feature-gating wasmtime/bollard | Feature change, not config change |
| `deny_unknown_fields` on AnyclawConfig | Conflicts with extension defaults (EXT-01) and forward compatibility |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| DFLT-01 | Phase 8 | Pending |
| DFLT-02 | Phase 8 | Pending |
| DFLT-03 | Phase 8 | Pending |
| DFLT-04 | Phase 8 | Pending |
| SCHM-01 | Phase 7 | Pending |
| SCHM-02 | Phase 10 | Pending |
| JSCH-01 | Phase 9 | Pending |
| JSCH-02 | Phase 9 | Pending |
| JSCH-03 | Phase 9 | Pending |
| JSCH-04 | Phase 9 | Pending |
| JSCH-05 | Phase 9 | Pending |
| CI-01 | Phase 10 | Pending |
| CI-02 | Phase 10 | Pending |
| IDE-01 | Phase 10 | Pending |
| EXT-01 | Phase 11 | Pending |

**Coverage:**
- v1.0.0 requirements: 15 total
- Mapped to phases: 15
- Unmapped: 0 ✓

---
*Requirements defined: 2026-04-15*
*Last updated: 2026-04-15 after Oracle design review*
