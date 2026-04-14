# Requirements: Anyclaw Code Quality

**Defined:** 2026-04-14
**Core Value:** Every line of code should be there for a reason, with typed data flowing through typed interfaces.

## Current Milestone Requirements

Requirements for the code quality milestone. Each maps to roadmap phases.

### Tooling & Infrastructure

- [x] **TOOL-01**: Workspace-level lint configuration via `[workspace.lints]` in root Cargo.toml
- [x] **TOOL-02**: `clippy.toml` with `disallowed-types` banning raw `serde_json::Value` in new code
- [x] **TOOL-03**: `rustfmt.toml` for consistent formatting across all crates
- [x] **TOOL-04**: Expand `deny.toml` with advisories, bans, and sources sections (currently licenses only)
- [x] **TOOL-05**: Coverage measurement setup with cargo-llvm-cov and baseline report

### Dead Code & Hygiene

- [x] **HYGN-01**: Remove all unused imports across workspace
- [x] **HYGN-02**: Remove stale modules and unreachable branches
- [x] **HYGN-03**: Zero clippy warnings across entire workspace (`cargo clippy --workspace`)

### Error Handling

- [x] **ERRH-01**: Verify thiserror used in all library crates — no anyhow leaking into library code
- [x] **ERRH-02**: Verify each manager crate has a proper typed error enum
- [x] **ERRH-03**: Eliminate bare `.unwrap()` in all production code (replace with `.expect("reason")` or `?`)

### Typed JSON

- [x] **JSON-01**: Replace `serde_json::Value` with typed structs in `anyclaw-sdk-types`
- [x] **JSON-02**: Replace `serde_json::Value` with typed structs in `anyclaw-jsonrpc`
- [x] **JSON-03**: Replace `serde_json::Value` with typed structs in `anyclaw-core`
- [x] **JSON-04**: Replace `serde_json::Value` with typed structs in `anyclaw-agents`
- [x] **JSON-05**: Replace `serde_json::Value` with typed structs in `anyclaw-channels`
- [x] **JSON-06**: Replace `serde_json::Value` with typed structs in `anyclaw-tools`
- [ ] **JSON-07**: Replace `serde_json::Value` with typed structs in SDK crates (sdk-agent, sdk-channel, sdk-tool)
- [ ] **JSON-08**: Replace `serde_json::Value` with typed structs in ext/ binaries and examples

### Serde Patterns

- [x] **SERD-01**: All SDK wire types use `#[serde(rename_all = "camelCase")]` consistently
- [x] **SERD-02**: All config types use `snake_case` consistently
- [ ] **SERD-03**: Round-trip serialization tests exist for all wire types

### Clone Reduction

- [ ] **CLON-01**: Eliminate unnecessary `.clone()` calls in `anyclaw-agents` manager (103 clones baseline)
- [x] **CLON-02**: Audit and reduce `.clone()` calls across all other crates
- [x] **CLON-03**: Use borrowing or `&str` where ownership transfer isn't needed

### Documentation

- [ ] **DOCS-01**: `#![warn(missing_docs)]` enabled on all crates (not just SDK crates)
- [ ] **DOCS-02**: Meaningful doc comments on all public types and functions
- [ ] **DOCS-03**: Inline limitation comments added at relevant code locations (from AGENTS.md known issues)

### Test Coverage

- [ ] **TEST-01**: Tests for `anyclaw-core/src/health.rs` (HealthSnapshot, HealthStatus)
- [ ] **TEST-02**: Tests for `anyclaw-sdk-agent/src/error.rs`
- [ ] **TEST-03**: Tests for `anyclaw-sdk-tool/src/error.rs` and `anyclaw-sdk-tool/src/lib.rs`
- [ ] **TEST-04**: Tests for `anyclaw-agents/src/acp_types.rs` and `anyclaw-agents/src/backend.rs`
- [ ] **TEST-05**: All new tests use rstest 0.23 with BDD naming convention

### File Decomposition

- [ ] **DECO-01**: Break `anyclaw-agents/src/manager.rs` (3,708 lines) into focused modules (fs_sandbox, session_recovery, tool_events, run loop)
- [ ] **DECO-02**: Break `anyclaw-supervisor/src/lib.rs` (927 lines) into sub-modules (signal handling, shutdown orchestration, health monitoring)
- [ ] **DECO-03**: All extracted modules use `pub(crate)` boundaries, preserving public API surface

### Advanced Quality

- [x] **ADVN-01**: Replace `Arc<Mutex<HashMap<u64, oneshot::Sender>>>` with DashMap in connection crates
- [ ] **ADVN-02**: Property-based testing (proptest) for all ACP/MCP wire types
- [ ] **ADVN-03**: Inline TODO/LIMITATION comments from AGENTS.md into source code at relevant locations

### Bug Fixes

- [x] **BUGF-01**: Fix any code bugs discovered during the quality pass (logic errors, incorrect behavior, edge cases)
- [x] **BUGF-02**: Fix any code smells that indicate latent bugs (unreachable match arms, silent error swallowing, incorrect type coercions)

## Deferred Requirements

Tracked but not in current milestone.

### Performance

- **PERF-01**: Feature-gate wasmtime behind cargo feature flag
- **PERF-02**: Feature-gate bollard (Docker SDK) behind cargo feature flag
- **PERF-03**: Replace poll_channels() polling workaround with FuturesUnordered/StreamMap

### New Capabilities

- **FEAT-01**: Rate limiting on inbound channel messages
- **FEAT-02**: Multi-agent routing (different sessions to different agent types)

## Out of Scope

| Feature | Reason |
|---------|--------|
| Dependency version upgrades | Doubles risk surface during refactoring — separate milestone |
| Performance optimization | Quality pass is about correctness and clarity, not speed |
| Feature-gating wasmtime/bollard | Feature change, not quality change — deferred |
| Rewriting poll_channels() workaround | Behavioral change, not quality fix — architecture milestone |
| Adding rate limiting | New capability, not quality improvement |
| Cross-manager communication redesign | Architectural work — enforce existing pattern, don't redesign |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| TOOL-01 | Phase 1 | Complete |
| TOOL-02 | Phase 1 | Complete |
| TOOL-03 | Phase 1 | Complete |
| TOOL-04 | Phase 1 | Complete |
| TOOL-05 | Phase 1 | Complete |
| HYGN-01 | Phase 1 | Complete |
| HYGN-02 | Phase 1 | Complete |
| HYGN-03 | Phase 1 | Complete |
| ERRH-01 | Phase 2 | Complete |
| ERRH-02 | Phase 2 | Complete |
| ERRH-03 | Phase 2 | Complete |
| JSON-01 | Phase 2 | Complete |
| JSON-02 | Phase 2 | Complete |
| JSON-03 | Phase 2 | Complete |
| JSON-04 | Phase 3 | Complete |
| JSON-05 | Phase 3 | Complete |
| JSON-06 | Phase 3 | Complete |
| JSON-07 | Phase 4 | Pending |
| JSON-08 | Phase 4 | Pending |
| SERD-01 | Phase 2 | Complete |
| SERD-02 | Phase 2 | Complete |
| SERD-03 | Phase 4 | Pending |
| CLON-01 | Phase 3 | Pending |
| CLON-02 | Phase 3 | Complete |
| CLON-03 | Phase 3 | Complete |
| DOCS-01 | Phase 4 | Pending |
| DOCS-02 | Phase 4 | Pending |
| DOCS-03 | Phase 4 | Pending |
| TEST-01 | Phase 5 | Pending |
| TEST-02 | Phase 5 | Pending |
| TEST-03 | Phase 5 | Pending |
| TEST-04 | Phase 5 | Pending |
| TEST-05 | Phase 5 | Pending |
| DECO-01 | Phase 6 | Pending |
| DECO-02 | Phase 6 | Pending |
| DECO-03 | Phase 6 | Pending |
| ADVN-01 | Phase 3 | Complete |
| ADVN-02 | Phase 5 | Pending |
| ADVN-03 | Phase 4 | Pending |
| BUGF-01 | All phases | Complete |
| BUGF-02 | All phases | Complete |

**Coverage:**
- Current milestone requirements: 41 total
- Mapped to phases: 41 ✓
- Unmapped: 0

---
*Requirements defined: 2026-04-14*
*Last updated: 2026-04-14 after roadmap creation*
