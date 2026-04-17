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

git worktree add "$WT" -b <branch-name>

# Symlink all root-level gitignored files (skip .worktrees to avoid recursion)
for f in "$MAIN"/.* "$MAIN"/*; do
  name="$(basename "$f")"
  [ "$name" = ".worktrees" ] && continue
  git check-ignore -q "$f" 2>/dev/null || continue
  ln -sf "$f" "$WT/$name"
done

# Symlink nested .env files
find "$MAIN" -name '.env' -not -path '*/.worktrees/*' -not -path '*/target/*' | while read f; do
  rel="${f#$MAIN/}"
  mkdir -p "$WT/$(dirname "$rel")"
  ln -sf "$f" "$WT/$rel"
done
```

Run `cargo check` in the worktree to verify clean baseline. All subsequent commands run in the worktree.

## Step 2. Do the work

Hand control back to the caller. This command does NOT implement changes.

## Step 3. Pre-flight checks

Fast local checks only — full suite runs on CI:

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
```

For bugfixes, also run `cargo test -p <affected-crate>`.

Fix formatting automatically (`cargo fmt --all`). Stop and report if clippy issues require design decisions.

## Step 4. Commit

Conventional commits. Subject: `<type>: <description>` — imperative, lowercase, no period, ≤72 chars. Body: explain *why*. Types: `feat`, `fix`, `docs`, `chore`, `refactor`, `test`, `ci`.

## Step 5. Push and create PR

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

If changes affect module structure, public APIs, or conventions, verify `AGENTS.md` files were updated.

```bash
gh pr create --draft --title "<type>: <description>" --body "..." --assignee @me
```

## Step 6. Verify CI

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

- **CI_FAILED**: Read failure report, fix root cause, push, re-run Step 6. Max 3 attempts, then stop.
- **CI_PASSED**: Proceed to Step 7.

## Step 7. Update PR if needed

If further commits were pushed after PR creation, update title and description:

```bash
gh pr edit <pr-number> --title "<type>: <updated description>" --body "..."
```

## Step 8. Hand off to user

Report the PR URL and ask a blocking question:

```
PR is ready for review: <url>

Let me know when it's merged and I'll clean up the worktree.
```

Do NOT poll, check status, or take further action. Wait for the user's response.

## Step 9. Worktree cleanup (after user confirms merge)

```bash
chmod -R u+w .worktrees/<branch-name> 2>/dev/null || true
git worktree remove --force .worktrees/<branch-name>
git worktree prune
git branch -d <branch-name>
```
