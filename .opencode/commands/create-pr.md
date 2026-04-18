---
description: Create GitHub PR with worktree isolation, conventional commits, and CI verification
---

# Create PR

Argument: `<type>/<description>` (optional). If omitted, infer from current branch or ask.

## Step 0. Determine branch

If on `main`:
1. If argument provided, use it as branch name (e.g., `feat/wasm-permissions`)
2. If no argument, ask: "What type of change? (feat/fix/docs/chore/refactor/ci) and short description?"

If already on a feature branch, skip to Step 3.

Valid prefixes: `feat/`, `fix/`, `docs/`, `chore/`, `refactor/`, `ci/`

## Step 1. Create worktree (only from `main`)

```bash
MAIN="$(git rev-parse --show-toplevel)"
WT="$MAIN/.worktrees/<branch-name>"

git pull origin main --ff-only

git worktree add "$WT" -b <branch-name>

# Mirror gitignored items from main into worktree as symlinks.
# Directories are symlinked whole (no recursion into them).
# Non-gitignored directories are walked to find nested gitignored items.
# Uses find instead of globs to avoid zsh nomatch errors on empty directories.
mirror_ignored() {
  local src="$1" dst="$2"
  find "$src" -maxdepth 1 -mindepth 1 -not -name ".git" -not -name ".worktrees" | while IFS= read -r f; do
    name="$(basename "$f")"
    if git check-ignore -q "$f" 2>/dev/null; then
      ln -sf "$f" "$dst/$name"
    elif [ -d "$f" ]; then
      mkdir -p "$dst/$name"
      mirror_ignored "$f" "$dst/$name"
    fi
  done
}
mirror_ignored "$MAIN" "$WT"
```

Verify clean baseline in the worktree (e.g., build or type-check). All subsequent commands run in the worktree.

## Step 2. Do the work

Hand control back to the caller. This command does NOT implement changes.

**TDD requirement**: All code changes MUST follow test-driven development. For each behavior change:
1. Write a failing test that captures the expected behavior
2. Implement the minimal code to make it pass
3. Refactor if needed

No code lands without a test that exercises it. For bugfixes, the test must reproduce the bug before the fix is applied.

## Step 3. Pre-flight checks

First, check what actually changed: `git diff main...HEAD --name-only`. Only run checks relevant to the changed file types.

| Detected | Format | Lint | Skip when |
|----------|--------|------|-----------|
| `Cargo.toml` | `cargo fmt --all -- --check` | `cargo clippy --workspace -- -D warnings` | No `.rs` files changed |
| `package.json` | `npm run format --check` or `prettier --check .` | `npm run lint` or `eslint .` | No `.js`/`.ts`/`.jsx`/`.tsx` files changed |
| `pyproject.toml` / `setup.py` | `ruff format --check .` or `black --check .` | `ruff check .` or `flake8` | No `.py` files changed |
| `go.mod` | `gofmt -l .` | `go vet ./...` | No `.go` files changed |

Fix formatting automatically. Stop and report if lint issues require design decisions.

Run the relevant test suite for changed crates/packages. All tests must pass.

## Step 4. Pre-commit checklist

Before committing, create a todo list and verify each item. Do NOT skip items — this is the gate.

- [ ] **Format + lint clean**: Step 3 checks pass
- [ ] **Tests pass**: Run relevant test suite for changed crates/packages
- [ ] **Test coverage adequate**: For each new behavior/branch/guard added, verify a corresponding test exists. List untested paths and add tests or justify why they're untestable (e.g., requires real network). Aim for: every `if`/`match` arm you added has a test that exercises it.
- [ ] **No absolute symlinks staged**: Run staging guard (below)
- [ ] **Knowledge base updated**: If module structure, public APIs, or conventions changed, update relevant docs (e.g., `AGENTS.md`, `README.md` roadmap)
- [ ] **Defaults updated**: If new config options were added, update defaults files
- [ ] **No debug artifacts**: No `println!`, `console.log`, `dbg!`, or temp files in diff

**Staging guard**: Before `git add`, verify no symlinks to absolute paths will be staged:

```bash
git status --porcelain | grep '^?' | while read -r _ f; do
  [ -L "$f" ] && readlink "$f" | grep -q '^/' && echo "WARNING: absolute symlink: $f"
done
```

If any are found, add them to `.gitignore` or exclude from staging. Never commit symlinks to absolute paths.

## Step 5. Commit

Conventional commits. Subject: `<type>: <description>` — imperative, lowercase, no period, ≤72 chars. Body: explain *why*. Types: `feat`, `fix`, `docs`, `chore`, `refactor`, `test`, `ci`.

## Step 6. Push and create PR

```bash
git push -u origin HEAD
```

Generate title from `git log main..HEAD` and `git diff main...HEAD --stat`. Title MUST be conventional commits format (CI enforces).

PR body MUST include:

```markdown
## Motivation
[Why this change exists]

## Solution
[Technical approach — key decisions, not a changelog]

## Testing
[How verified — commands, manual steps]
```

```bash
gh pr create --draft --title "<type>: <description>" --body "..." --assignee @me
```

## Step 7. Verify CI

Delegate CI monitoring to a background subagent:

```
task(category="quick", load_skills=[], run_in_background=true,
  description="Monitor CI for PR #<number>",
  prompt="Monitor CI for PR #<number> in <repo>. Working directory: <worktree-path>.
    Poll `gh pr checks <number>` every 30s. Max wait: 10 minutes.
    When all checks complete, report ONE of:
    - CI_PASSED: All checks green. List check names and durations.
    - CI_FAILED: List failed checks. For each, run `gh run view <run-id> --log-failed` and include the failure reason.
    Your report MUST start with exactly CI_PASSED or CI_FAILED on the first line.")
```

After firing the subagent, **end your response immediately**. Do NOT poll `background_output`. The system will notify you when the subagent completes.

When the notification arrives and you collect the result:

- **CI_FAILED**: Read failure report, fix root cause, push, re-run Step 7. Max 3 attempts, then stop.
- **CI_PASSED**: Proceed to Step 8.

## Step 8. Self-review

Before handing off, review your own diff critically. Run `git diff main...HEAD` and check:

1. **Every new code path has a test**: For each `if`/`match`/guard you added, find the test that exercises it. If missing, write it now.
2. **No accidental changes**: Revert any unrelated formatting, import reordering, or whitespace-only changes.
3. **Commit hygiene**: Each commit has a clear purpose. Squash fixup commits if needed.
4. **Edge cases**: Think about what happens when inputs are empty, None, zero, or arrive out of order. Add tests for non-obvious cases.

If you find gaps, fix them, push, and let CI re-run before proceeding.

## Step 9. Update PR if needed

If further commits were pushed after PR creation, update title and description:

```bash
gh pr edit <pr-number> --title "<type>: <updated description>" --body "..."
```

## Step 10. Hand off to user

Report the PR URL and ask a blocking question:

```
PR is ready for review: <url>

Let me know when it's merged and I'll clean up the worktree.
```

Do NOT poll, check status, or take further action. Wait for the user's response.

## Step 11. Worktree cleanup (after user confirms merge)

```bash
chmod -R u+w .worktrees/<branch-name> 2>/dev/null || true
git worktree remove --force .worktrees/<branch-name>
git worktree prune
git branch -d <branch-name>
```
