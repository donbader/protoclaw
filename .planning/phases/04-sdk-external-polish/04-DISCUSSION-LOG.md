# Phase 4: SDK & External Polish - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-15
**Phase:** 4-sdk-external-polish
**Areas discussed:** Value replacement in SDK/ext, missing_docs rollout, Inline limitation comments, Serde test coverage

---

## Value Replacement in SDK/ext

| Option | Description | Selected |
|--------|-------------|----------|
| Same patterns | Same Phase 2-3 patterns — typed structs, parse at boundary, flatten for extras | ✓ |
| Lighter touch for ext/ | Allow Value where it simplifies things in ext/ binaries | |
| You decide | Per-file decision | |

**User's choice:** Same patterns

---

## missing_docs Rollout

| Option | Description | Selected |
|--------|-------------|----------|
| All crates at once | Enable on all crates, fix all warnings in one pass | ✓ |
| Internal crates incrementally | Add to internal crates one by one | |
| You decide | Based on warning count | |

**User's choice:** All crates at once

| Option | Description | Selected |
|--------|-------------|----------|
| Meaningful docs | Explain WHY, not just WHAT. No stub comments. | ✓ |
| Brief stubs first | Get lint passing, improve later | |
| You decide | Per-item decision | |

**User's choice:** Meaningful docs

---

## Inline Limitation Comments

| Option | Description | Selected |
|--------|-------------|----------|
| All of them | All known limitations from AGENTS.md + CONCERNS.md | ✓ |
| High-risk only | Only poll_channels, single-agent, no rate limiting | |
| You decide | Based on proximity to code | |

**User's choice:** All of them

| Option | Description | Selected |
|--------|-------------|----------|
| LIMITATION tag + reference | Brief description + reference to AGENTS.md | |
| Full explanation inline | Self-contained, no need to look up AGENTS.md | ✓ |
| You decide | Per-comment decision | |

**User's choice:** Full explanation inline

---

## Serde Test Coverage

| Option | Description | Selected |
|--------|-------------|----------|
| All wire types | Round-trip tests for every public wire type across all SDK + ext | ✓ |
| Changed types only | Only types refactored during Phases 2-4 | |
| You decide | Risk-based | |

**User's choice:** All wire types

---

## Agent's Discretion

- Order of SDK crates vs ext binaries
- Exact wording of inline limitation comments
- Which doc comments need detailed vs brief explanations

## Deferred Ideas

None — discussion stayed within phase scope.
