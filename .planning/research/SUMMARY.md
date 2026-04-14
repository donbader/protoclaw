# Research Summary: Anyclaw Code Quality Milestone

**Domain:** Rust workspace code quality improvement
**Researched:** 2026-04-14
**Overall confidence:** HIGH

## Executive Summary

The anyclaw workspace is a well-structured Rust project (12 core crates, edition 2024, rust-version 1.94) with solid architectural foundations but inconsistent quality enforcement. The codebase has zero clippy configuration, no workspace-level lint policy, no rustfmt config, and a deny.toml that only checks licenses. The biggest technical debt is 259 `serde_json::Value` usages where typed structs should exist — this is the highest-impact quality improvement available.

The good news: the Rust code quality ecosystem is mature and stable. Every tool needed is well-established (clippy, cargo-deny, cargo-llvm-cov, thiserror, rstest). No new dependencies are required for the core quality work — it's about configuring existing tools and enforcing existing conventions consistently. The only new install is cargo-llvm-cov for coverage measurement.

The project's existing conventions (thiserror in libraries, anyhow at entry points, rstest with BDD naming, flat lib.rs modules) are correct and well-documented. The quality milestone is about mechanical enforcement of these conventions across all 12+ crates, not about changing direction.

The riskiest work is the typed JSON migration (259 Value usages across agents, channels, SDK types, and connection layers). This touches protocol boundaries and requires careful attention to backward compatibility of the ACP wire format. The safest approach is crate-by-crate, starting with leaf crates (sdk-types, jsonrpc) and working inward.

## Key Findings

**Stack:** No new dependencies needed. Configure clippy (workspace lints + clippy.toml), expand deny.toml, add rustfmt.toml, install cargo-llvm-cov for coverage.
**Architecture:** Parse serde_json::Value into typed structs at system boundaries (connection layer). Internal code should never touch Value.
**Critical pitfall:** The typed JSON migration in sdk-types and agents manager is the highest-risk work — it touches the ACP wire protocol and has 259 touch points.

## Implications for Roadmap

Based on research, suggested phase structure:

1. **Tooling & Lint Infrastructure** — Set up workspace lints, clippy.toml, rustfmt.toml, expand deny.toml
   - Addresses: Zero-config baseline, consistent enforcement
   - Avoids: Doing quality work without automated checks to prevent regression
   - Low risk, high leverage — every subsequent phase benefits

2. **Leaf Crate Cleanup** — sdk-types, jsonrpc, core types
   - Addresses: Typed JSON foundations, error enum consistency, dead code removal
   - Avoids: Changing protocol boundaries before internal types are solid
   - These crates are depended on by everything — clean them first

3. **Manager Crate Quality** — agents, channels, tools managers
   - Addresses: The 3,708-line agents manager, 259 Value usages, clone() reduction
   - Avoids: Breaking changes to SDK crates mid-refactor
   - Highest risk phase — needs the typed foundations from phase 2

4. **SDK & External Crate Polish** — sdk-agent, sdk-channel, sdk-tool, ext/ binaries
   - Addresses: Missing docs, test coverage gaps, consistent patterns
   - Avoids: SDK changes before internal types stabilize

5. **Coverage & Verification** — cargo-llvm-cov baseline, fill test gaps
   - Addresses: Testing gaps identified in CONCERNS.md
   - Avoids: Writing tests before the code they test is refactored

**Phase ordering rationale:**
- Tooling first because every subsequent phase needs lint enforcement to prevent regression
- Leaf crates before managers because managers depend on leaf crate types
- Managers before SDK because SDK types flow from internal protocol decisions
- Coverage last because measuring coverage of code that's about to be refactored is wasteful

**Research flags for phases:**
- Phase 3 (Manager Crate Quality): Likely needs deeper research on ACP protocol message shapes before typing them
- Phase 1 (Tooling): Standard patterns, unlikely to need additional research

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | No new deps. Tooling is mature, well-documented. Workspace lints stable since Rust 1.74. |
| Features | HIGH | Categories derived directly from PROJECT.md requirements and CONCERNS.md findings. |
| Architecture | HIGH | Patterns are standard Rust (module extraction, serde derive, thiserror). No novel approaches needed. |
| Pitfalls | HIGH | Pitfalls derived from known Rust refactoring risks + specific codebase concerns (wire compat, async clones, test visibility). |

## Gaps to Address

- Exact count of `serde_json::Value` usages per crate (need per-crate breakdown to estimate typed JSON effort)
- Which Value usages are at wire boundaries vs internal (determines how many typed structs are needed)
- Test coverage baseline numbers (need cargo-llvm-cov report before setting coverage targets)
- Whether any existing tests rely on private access patterns that would break during file decomposition

---

*Research summary: 2026-04-14*
