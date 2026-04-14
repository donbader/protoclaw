# Phase 1: Tooling & Lint Infrastructure - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-14
**Phase:** 1-tooling-lint-infrastructure
**Areas discussed:** Clippy strictness, Rustfmt style, deny.toml scope, Coverage strategy

---

## Clippy Strictness

| Option | Description | Selected |
|--------|-------------|----------|
| Moderate | Default clippy warnings + workspace-level warn on key groups (clippy::unwrap_used, clippy::pedantic subset) | ✓ |
| Strict pedantic | clippy::pedantic fully enabled, deny on most warnings — strict but noisy initially | |
| Defaults only | Just the defaults, fix what's already flagged, no new lint groups | |

**User's choice:** Moderate
**Notes:** None

| Option | Description | Selected |
|--------|-------------|----------|
| Warn locally, deny in CI | Warnings in dev, CI denies — lets you iterate locally without blocking | ✓ |
| Deny everywhere | Deny everywhere — forces immediate compliance, no wiggle room | |
| Warn everywhere | Warn everywhere — advisory only, relies on discipline | |

**User's choice:** Warn locally, deny in CI
**Notes:** None

| Option | Description | Selected |
|--------|-------------|----------|
| Ban Value | Ban serde_json::Value in clippy.toml — new code can't introduce untyped JSON | ✓ |
| Ban Value + unwrap + println | Ban Value + bare unwrap() + println! in library code | |
| No bans | No disallowed-types — rely on code review | |

**User's choice:** Ban Value
**Notes:** None

---

## Rustfmt Style

| Option | Description | Selected |
|--------|-------------|----------|
| Edition defaults | Edition 2024 defaults, just create the file to make it explicit and consistent | ✓ |
| Custom rules | Custom rules: max_width, imports_granularity, group_imports, etc. | |
| You decide | Pick sensible defaults for a Rust 2024 workspace | |

**User's choice:** Edition defaults
**Notes:** None

---

## deny.toml Scope

| Option | Description | Selected |
|--------|-------------|----------|
| Deny vulns, warn unmaintained | Deny known vulnerabilities, warn on unmaintained crates | ✓ |
| Deny all advisories | Deny both vulnerabilities and unmaintained crates | |
| Warn only | Warn only — don't block builds on advisories | |

**User's choice:** Deny vulns, warn unmaintained
**Notes:** None

| Option | Description | Selected |
|--------|-------------|----------|
| Ban key duplicates | Ban duplicate versions of key deps (serde, tokio, etc.) to keep the dep tree clean | ✓ |
| Ban all duplicates | Ban all duplicate crate versions — strictest, may require dep resolution work | |
| No bans | No bans section — just advisories and sources | |

**User's choice:** Ban key duplicates
**Notes:** None

---

## Coverage Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Baseline only | Run cargo-llvm-cov, record the baseline number, no floor enforced yet | |
| Enforce a floor | Set a minimum coverage floor and fail CI if it drops | ✓ |
| Per-crate targets | Per-crate targets — SDK crates higher, internal crates lower | |

**User's choice:** Enforce a floor
**Notes:** Exact floor percentage to be determined after baseline measurement

| Option | Description | Selected |
|--------|-------------|----------|
| cargo-llvm-cov | Standard Rust coverage tool, source-based instrumentation | ✓ |
| You decide | Pick whatever works best for this workspace | |

**User's choice:** cargo-llvm-cov
**Notes:** None

---

## Agent's Discretion

- Exact clippy pedantic lint subset
- Exact coverage floor percentage (based on baseline)
- Which key dependencies to include in deny.toml duplicate ban list
- `[sources]` section specifics in deny.toml

## Deferred Ideas

None — discussion stayed within phase scope.
