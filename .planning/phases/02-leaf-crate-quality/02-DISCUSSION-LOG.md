# Phase 2: Leaf Crate Quality - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-14
**Phase:** 2-leaf-crate-quality
**Areas discussed:** Typed JSON strategy, Unwrap cleanup scope, Serde conventions, Error enum approach

---

## Typed JSON Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| New typed structs | Define new typed structs in each crate — full control, no external deps | |
| Reuse ACP schema + fill gaps | Reuse types from agent-client-protocol-schema (v0.11) where they exist, new structs for the rest | ✓ |
| Newtype wrappers | Use Value but wrap in newtype structs for type safety at boundaries | |

**User's choice:** Reuse ACP + new structs
**Notes:** User initially asked what the ACP schema crate was — clarified it's an open-source crate already in the workspace dependency tree, re-exporting AgentCapabilities, McpCapabilities, etc.

| Option | Description | Selected |
|--------|-------------|----------|
| Parse at boundary | Parse JSON into typed structs at codec/connection layer — internal code never sees Value | ✓ |
| Lazy parsing | Keep Value at boundaries, convert to typed when accessed | |
| You decide | Per-file decision | |

**User's choice:** Parse at boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Typed + flatten for extras | #[serde(flatten)] with HashMap<String, Value> for unknown fields | ✓ |
| Strict, no fallback | Reject unknown fields, no Value anywhere | |
| You decide | Per-type decision | |

**User's choice:** Typed + flatten for extras

---

## Unwrap Cleanup Scope

| Option | Description | Selected |
|--------|-------------|----------|
| All unwraps | Replace every bare .unwrap() in leaf crate production code | ✓ |
| High-risk only | Focus on codec, session store, ACP parsing | |
| You decide | Context per-call | |

**User's choice:** All unwraps

| Option | Description | Selected |
|--------|-------------|----------|
| Prefer ? operator | Use ? with proper error propagation, .expect() only for true invariants | ✓ |
| Prefer .expect() | Replace with .expect("reason") everywhere | |
| Convention match | Mix based on AGENTS.md convention | |

**User's choice:** Prefer ? operator

---

## Serde Conventions

| Option | Description | Selected |
|--------|-------------|----------|
| Tests in this phase | Add round-trip serde tests for every public wire type in leaf crates now | ✓ |
| Defer tests to Phase 5 | Just enforce attributes now, test later | |
| You decide | Risk-based | |

**User's choice:** Tests in this phase

| Option | Description | Selected |
|--------|-------------|----------|
| Full audit + fix | Audit and fix ALL serde attributes in leaf crates | ✓ |
| Only changed types | Only fix types being modified for typed JSON | |
| You decide | Based on findings | |

**User's choice:** Full audit + fix

---

## Error Enum Approach

| Option | Description | Selected |
|--------|-------------|----------|
| Audit + fill gaps | Audit existing, fill gaps, ensure every fallible path has a typed variant | |
| Restructure from scratch | Clean hierarchy, consistent patterns from scratch | ✓ |
| Verify boundary only | Only verify no anyhow in libraries | |

**User's choice:** Restructure from scratch

| Option | Description | Selected |
|--------|-------------|----------|
| Mix of #[from] + manual | Automatic conversion where sensible, manual From for context-adding | ✓ |
| Always #[from] | Simpler, less boilerplate | |
| You decide | Per-error decision | |

**User's choice:** Mix of #[from] + manual

---

## Agent's Discretion

- Exact struct field names for new typed structs
- Which specific ACP schema types to reuse vs write fresh
- Internal organization of error enum variants
- Whether to use #[serde(default)] on optional fields or Option<T>

## Deferred Ideas

None — discussion stayed within phase scope.
