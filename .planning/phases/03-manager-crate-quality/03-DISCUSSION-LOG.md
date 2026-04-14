# Phase 3: Manager Crate Quality - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-14
**Phase:** 3-manager-crate-quality
**Areas discussed:** Value replacement approach, Clone reduction strategy, DashMap migration, Error handling in managers

---

## Value Replacement Approach

| Option | Description | Selected |
|--------|-------------|----------|
| Crate by crate | Each crate fully typed before moving to the next | ✓ |
| All at once | Replace Value in all 3 crates simultaneously | |
| You decide | Based on dependency order | |

**User's choice:** Crate by crate

| Option | Description | Selected |
|--------|-------------|----------|
| Full typed pipeline | Replace shims, managers consume JsonRpcMessage directly | ✓ |
| Keep shims as adapters | Managers use own internal types, shims translate | |
| You decide | Per-crate decision | |

**User's choice:** Full typed pipeline

---

## Clone Reduction Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Full audit | Systematic audit of all 107 clones, categorize and eliminate unnecessary | ✓ |
| Hot paths only | Focus on message routing, session dispatch | |
| You decide | Based on ownership analysis | |

**User's choice:** Full audit

| Option | Description | Selected |
|--------|-------------|----------|
| 50%+ reduction | Eliminate at least half of unnecessary clones | |
| Maximum cleanup | Eliminate every clone that isn't strictly necessary | ✓ |
| Obvious only | Just remove the obvious ones | |

**User's choice:** Maximum cleanup

---

## DashMap Migration

| Option | Description | Selected |
|--------|-------------|----------|
| DashMap | Direct replacement — cleaner API, standard concurrent map, one new dep | ✓ |
| RwLock | tokio RwLock<HashMap> — no new dep, async-aware | |
| Keep current | Leave as-is — works, locks are short-lived | |
| You decide | Based on usage pattern | |

**User's choice:** DashMap
**Notes:** User asked for explanation of what Arc<Mutex<HashMap>> does and the tradeoffs. Explained the pending request callback pattern, how Mutex/DashMap/RwLock differ, and that this is a "nicer pattern" change not a bug fix.

---

## Error Handling in Managers

| Option | Description | Selected |
|--------|-------------|----------|
| Verify + fill gaps | Verify consistency with Phase 2 pattern, fill gaps | |
| Restructure from scratch | Clean hierarchy, every fallible path typed | ✓ |
| You decide | Per-crate decision | |

**User's choice:** Restructure from scratch

---

## Agent's Discretion

- Order of the three manager crates
- Which specific clones are necessary vs unnecessary
- Internal error enum variant organization

## Deferred Ideas

None — discussion stayed within phase scope.
