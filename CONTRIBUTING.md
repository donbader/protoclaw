# Contributing to anyclaw

Thank you for your interest in contributing! This document covers the development workflow, test commands, and PR process.

## Code of Conduct

This project follows the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct). By participating, you are expected to uphold its standards. See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for details.

## Getting Started

Looking for somewhere to start? Check out issues labeled [`E-help-wanted`](https://github.com/donbader/anyclaw/labels/E-help-wanted) or [`E-easy`](https://github.com/donbader/anyclaw/labels/E-easy).

## Development Workflow

### Prerequisites

- Rust toolchain (stable) — install via [rustup](https://rustup.rs/)
- `cargo fmt`, `cargo clippy` available via `rustup component add rustfmt clippy`

### Building

```sh
cargo build
```

### Running Tests

**Unit tests (all workspace crates):**

```sh
cargo test
```

**Lint:**

```sh
cargo clippy --workspace
```

**Format check:**

```sh
cargo fmt --all -- --check
```

**Integration tests** (requires binaries built first):

```sh
cargo build --bin mock-agent --bin debug-http --bin sdk-test-tool --bin sdk-test-channel
cargo test -p anyclaw-integration-tests
```

### Test Conventions

Tests use [rstest](https://github.com/la10736/rstest) with BDD-style naming:

- Test functions: `when_action_then_result` or `given_precondition_when_action_then_result`
- No `test_` or `it_` prefix
- Fixtures are free functions named `given_*`
- Parameterised cases use `#[case::label_name]`
- Async unit tests: `#[rstest] #[tokio::test]`
- Async integration tests: `#[rstest] #[test_log::test(tokio::test)]`

## Commit Message Format

This project uses [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>: <short description>

[optional body]
```

**Types:**

| Type | When to use |
|------|-------------|
| `feat` | New functionality |
| `fix` | Bug fixes |
| `docs` | Documentation changes |
| `chore` | Dependency updates, tooling, housekeeping |
| `refactor` | Restructuring without behavior change |
| `test` | Adding or updating tests |
| `ci` | CI/CD pipeline changes |

- Subject line: imperative mood, lowercase after type, no trailing period, ≤72 characters
- Body: wrap at 72 characters, explain *why* not *what*

## Pull Request Process

1. Fork the repository and create a branch from `main`
2. Make your changes, ensuring tests pass and `cargo clippy` is clean
3. Open a PR with the following sections in the description:

```markdown
## Motivation

[Why is this change needed? What problem does it solve?]

## Solution

[What did you change and why? Key design decisions.]

## Testing

[How was this tested? Relevant test commands or output.]
```

4. A maintainer will review and provide feedback
5. Once approved and CI passes, the PR will be merged

## License

By contributing, you agree that your contributions will be licensed under the same **MIT OR Apache-2.0** license as the rest of the project.
