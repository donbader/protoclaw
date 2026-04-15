# Phase 7: Config Schema Cleanup - Context

**Gathered:** 2026-04-15
**Status:** Ready for planning

<domain>
## Phase Boundary

Remove 4 legacy `#[serde(alias)]` attributes from `AnyclawConfig` in `crates/anyclaw-config/src/types.rs`. This is a breaking config change that cleans field names before JSON Schema generation in Phase 9.

</domain>

<decisions>
## Implementation Decisions

### Alias removal
- **D-01:** Remove all 4 aliases: `agents-manager`, `channels-manager`, `tools-manager`, `session-store` from `AnyclawConfig` in `types.rs` (lines 167, 170, 173, 179)
- **D-02:** The `permission.rs` alias (`alias = "name"`) is out of scope — it's in SDK types, not config
- **D-03:** No migration path needed — zero example YAML files use hyphenated keys. Breaking change is acceptable per PROJECT.md.
- **D-04:** No custom error message for old keys — Figment silently ignores unknown keys, so old hyphenated keys just won't be read (fields get defaults instead). This is the expected behavior.

### Agent's Discretion
- Whether to add a comment noting the aliases were removed and when
- Test approach (existing tests should pass as-is since no YAML uses hyphenated keys)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Config types
- `crates/anyclaw-config/src/types.rs` — Lines 167, 170, 173, 179 contain the 4 alias attributes to remove
- `crates/anyclaw-config/src/lib.rs` — Figment loading chain (verify no test uses hyphenated keys)

### Research
- `.planning/research/PITFALLS.md` — Pitfall 2 covers alias removal and Figment's silent ignore behavior
- `.planning/research/STACK.md` — Confirms schemars doesn't support serde(alias)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- None needed — this is a 4-line deletion

### Established Patterns
- All example YAML files already use snake_case (`agents_manager`, `channels_manager`, etc.)
- Figment silently ignores unknown keys — old hyphenated keys won't error, they'll just be ignored

### Integration Points
- `types.rs` is the only file with these aliases
- All downstream consumers already use snake_case field names

</code_context>

<specifics>
## Specific Ideas

No specific requirements — straightforward deletion of 4 serde attributes.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 07-config-schema-cleanup*
*Context gathered: 2026-04-15*
