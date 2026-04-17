---
description: Release a new binary version — bump, changelog, PR, tag, cleanup
---

# Release Binary

Argument: `<version>` (e.g., `0.5.3`). Required.

## Prerequisites

Verify before starting:
1. You are on `main` and it's clean (`git status`)
2. All CI checks pass on `main`
3. No existing `v<version>` tag (`git tag -l v<version>`)
4. The version follows semver: patch for bugfixes, minor for features

## Steps

### 1. Create worktree

```bash
git fetch origin main
git worktree add .worktrees/release-<version> -b chore/release-<version> origin/main
```

Work exclusively in `.worktrees/release-<version>` for all subsequent steps.

### 2. Generate changelog

List commits since the last tag:

```bash
git log $(git describe --tags --abbrev=0)..HEAD --format='%h %s' --no-merges
```

Categorize into `### Fixed`, `### Added`, `### Changed` sections following [Keep a Changelog](https://keepachangelog.com/). Include PR numbers. Skip `chore:`, `ci:`, `docs:` commits unless they're user-facing.

### 3. Bump version

- Update `crates/anyclaw/Cargo.toml`: set `version = "<version>"`
- Update `CHANGELOG.md`:
  - Add new version section under `[Unreleased]` with today's date
  - Add changelog entries from step 2
  - Update bottom links: add `[<version>]` compare link, update `[Unreleased]` link
- Run `cargo check -p anyclaw` to regenerate `Cargo.lock`

### 4. Commit and push

```bash
git add CHANGELOG.md Cargo.lock crates/anyclaw/Cargo.toml
git commit -m 'chore: bump version to <version> and update changelog'
git push -u origin chore/release-<version>
```

### 5. Create PR

```bash
gh pr create --title "chore: bump version to <version> and update changelog" \
  --body "Release prep for v<version>. Version bump and changelog only."
```

### 6. Wait for CI

```bash
gh pr checks <pr-number> --watch
```

If CI fails due to `Cargo.lock` mismatch, that means you forgot step 3's `cargo check`. Fix and push.

### 7. Merge PR

```bash
gh pr merge <pr-number> --squash --delete-branch
```

### 8. Tag release

```bash
git checkout main
git pull origin main
git tag v<version>
git push origin v<version>
```

This triggers binary release + Docker workflows.

### 9. Cleanup worktree

```bash
git worktree remove .worktrees/release-<version>
```

### 10. Verify

```bash
gh run list --branch v<version> --limit 5
```

Confirm binary-release and docker workflows are running.

Report the GitHub release URL when done.
