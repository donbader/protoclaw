---
description: Create GitHub PR with worktree isolation, conventional commits, and CI verification
---

# Create PR

Argument: `<type>/<description>` (optional). If omitted, infer from current branch or ask.

## Step 0. Determine branch

If on `main`:
1. If argument provided, use it as branch name (e.g., `feat/wasm-permissions`)
2. If no argument, ask: "What type of change? (feat/fix/docs/chore/refactor/ci) and short description?"

If already on a feature branch, skip worktree creation — use current branch.

Valid prefixes: `feat/`, `fix/`, `docs/`, `chore/`, `refactor/`, `ci/`

## Step 1. Create worktree (only if on `main`)

`.worktrees/` exists and is gitignored. Create the worktree there:

```bash
git worktree add .worktrees/<branch-name> -b <branch-name>
```

Then bootstrap the worktree:
- All subsequent commands run in `.worktrees/<branch-name>/`
- Symlink gitignored files from the main checkout into the worktree:

```bash
MAIN="$(git rev-parse --show-toplevel)"
WT=".worktrees/<branch-name>"

# Symlink all root-level gitignored files/dirs (skip .worktrees to avoid recursion)
for f in "$MAIN"/.* "$MAIN"/*; do
  name="$(basename "$f")"
  [ "$name" = ".worktrees" ] && continue
  git check-ignore -q "$f" 2>/dev/null || continue
  ln -sf "$f" "$WT/$name"
done

# Also symlink nested .env files (gitignored, buried in examples/)
find "$MAIN" -name '.env' -not -path '*/.worktrees/*' -not -path '*/target/*' | while read f; do
  rel="${f#$MAIN/}"
  mkdir -p "$WT/$(dirname "$rel")"
  ln -sf "$f" "$WT/$rel"
done
```
- Run `cargo check` to verify clean baseline (faster than full build since target/ is shared)

If already on a feature branch, skip this step entirely.

## Step 2. Do the work

Hand control back to the user (or the calling agent) to make changes in the worktree. This command does NOT implement changes — it sets up the workspace and creates the PR after work is done.

If invoked after work is already done on a feature branch, skip to Step 3.

## Step 3. Pre-flight checks

Run fast local checks only. Full test suite runs on CI.

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
```

If fixing a bug, also run scoped tests for affected crates:

```bash
cargo test -p <affected-crate>
```

Do NOT run `cargo test` (full suite) or `cargo doc` locally — CI handles those.

If any check fails:
- Fix if straightforward (formatting → `cargo fmt --all`)
- Report and stop if fix requires design decisions

## Step 4. Commit (if uncommitted changes exist)

Follow conventional commits:

- Subject: `<type>: <short description>` — imperative mood, lowercase, no period, max 72 chars
- Body: explain *why*, not *what*
- Types: `feat`, `fix`, `docs`, `chore`, `refactor`, `test`, `ci`

Each commit must leave the codebase buildable. Separate concerns into multiple commits if needed.

## Step 5. Push and create PR

```bash
git push -u origin HEAD
```

Create the PR with `gh pr create`. The PR title MUST follow conventional commits format (CI enforces via `amannn/action-semantic-pull-request`).

Generate title by analyzing all commits and the diff against `main`:

```bash
git log main..HEAD --format='%s' --no-merges
git diff main...HEAD --stat
```

PR body template (MUST include all three sections):

```markdown
## Motivation

[Why this change exists — the problem or need it addresses]

## Solution

[Technical approach — key design decisions, not a line-by-line changelog]

## Testing

[How changes were verified — test commands, manual steps]

- cargo test: [pass/fail]
- cargo clippy: [pass/fail]
- cargo fmt: [pass/fail]
```

If changes affect module structure, public APIs, or conventions, verify relevant `AGENTS.md` files were updated.

Create as draft PR with self-assignment:

```bash
gh pr create --draft --title "<type>: <description>" --body "..." --assignee @me
```

## Step 6. Verify CI

After push, check CI status:

```bash
gh pr checks <pr-number> --watch
```

If CI fails: read failure logs, fix root cause, push again. Up to 3 attempts. After 3 failures, stop and report.

## Step 7. Report

Return the PR URL. Do NOT merge or approve — PRs exist for the user to review.

## Worktree cleanup

After PR is merged (separate invocation), clean up:

```bash
chmod -R u+w .worktrees/<branch-name> 2>/dev/null || true
git worktree remove --force .worktrees/<branch-name>
git worktree prune
git branch -d <branch-name>
```
