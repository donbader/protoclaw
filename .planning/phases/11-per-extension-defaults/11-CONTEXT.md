# Phase 11: Per-Extension Defaults - Context

**Gathered:** 2026-04-16
**Status:** Ready for planning

<domain>
## Phase Boundary

Enable extensions to ship a `defaults.yaml` sidecar file that layers into Figment between global defaults and user config. The merge targets the `options` map of the corresponding entity config (agent/channel/tool).

</domain>

<decisions>
## Implementation Decisions

### Discovery mechanism
- **D-01:** Sidecar file approach — after binary path resolution, look for `<resolved_binary_path>.defaults.yaml` (e.g. `/usr/local/bin/channels/telegram.defaults.yaml`)
- **D-02:** If the sidecar file doesn't exist, silently skip — no error, no warning. Extensions are not required to ship defaults.
- **D-03:** Sidecar files are plain YAML — no `${VAR}` substitution (that's the user config's job)

### Merge semantics
- **D-04:** Extension defaults merge into the entity's `options` map only — they don't override structural fields like `binary`, `enabled`, `args`
- **D-05:** Layering order: global defaults.yaml → extension defaults → user anyclaw.yaml. User config wins over extension defaults.
- **D-06:** Use `serde_yaml::from_str` to parse the sidecar, then merge the resulting map into the entity's `options` HashMap. User-provided options keys override extension defaults.

### Integration point
- **D-07:** Add `pub fn load_extension_defaults(config: &mut AnyclawConfig)` to `anyclaw-config` — walks all agents/channels/tools, resolves sidecar paths, merges options
- **D-08:** Called in `Supervisor::new()` after `resolve_all_binary_paths()` and before managers are constructed
- **D-09:** Only applies to entities with resolved binary paths (absolute paths after resolution) — skip entities with unresolved or Docker workspace types

### Sidecar file format
- **D-10:** The sidecar YAML is a flat key-value map that merges into `options`. Example for telegram channel:
```yaml
# /usr/local/bin/channels/telegram.defaults.yaml
debounce_ms: 500
max_message_length: 4096
```
These become `options["debounce_ms"]` and `options["max_message_length"]` on the ChannelConfig.

### Agent's Discretion
- Whether to log at debug/trace level when a sidecar is found and loaded
- Error handling for malformed sidecar YAML (recommendation: warn and skip)
- Test approach (create temp sidecar files, verify merge)

</decisions>

<canonical_refs>
## Canonical References

### Config loading
- `crates/anyclaw-config/src/lib.rs` — Figment chain, `generate_schema()`, `DEFAULTS_YAML`
- `crates/anyclaw-config/src/resolve.rs` — `resolve_binary_path()`, `resolve_all_binary_paths()`
- `crates/anyclaw-config/src/types.rs` — `AgentConfig.options`, `ChannelConfig.options`, `ToolConfig.options`

### Supervisor integration
- `crates/anyclaw-supervisor/src/lib.rs` — `Supervisor::new()` line 77, after `resolve_all_binary_paths()`

### Research
- `.planning/research/PITFALLS.md` — Pitfall on Figment merge vs join semantics
- `.planning/research/ARCHITECTURE.md` — Extension defaults design

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `resolve_all_binary_paths()` already walks all entities and resolves paths — similar iteration pattern needed
- `options: HashMap<String, serde_json::Value>` already exists on all entity configs — natural merge target
- `serde_yaml::from_str` already used throughout for YAML parsing

### Established Patterns
- `resolve.rs` walks agents/channels/tools with nested match on workspace type — same pattern for sidecar discovery
- Binary paths are mutated in-place on `&mut AnyclawConfig` — same approach for options merge

### Integration Points
- `crates/anyclaw-config/src/lib.rs` or new file — `load_extension_defaults()` function
- `crates/anyclaw-supervisor/src/lib.rs` — call site after `resolve_all_binary_paths()`

</code_context>

<specifics>
## Specific Ideas

No specific requirements beyond the standard implementation.

</specifics>

<deferred>
## Deferred Ideas

- Extension defaults for Docker workspace types (would need to read from image or mount)
- Nested options merging (deep merge vs shallow) — shallow for v1.0.0
- Extension defaults validation against a per-extension schema

</deferred>

---

*Phase: 11-per-extension-defaults*
*Context gathered: 2026-04-16*
