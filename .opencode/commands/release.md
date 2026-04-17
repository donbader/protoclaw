---
description: Release a new version — bump, changelog, trigger Docker build
---

# Release

Argument: `<version>` (optional). If omitted, auto-detect from commits.

## Step 0. Determine version

If no version argument provided, auto-detect:

1. Get the latest binary tag: `git tag -l 'v*' --sort=-v:refname | head -1`
2. List commits since that tag: `git log <tag>..HEAD --format='%s' --no-merges`
3. Parse conventional commit prefixes:
   - Any `feat:` or `feat(` → bump minor
   - Only `fix:`, `perf:`, `refactor:`, `chore:`, `docs:`, `ci:`, `test:` → bump patch
4. Suggest the version and ask the user to confirm before proceeding

## Prerequisites

Verify before starting:
1. You are on `main` and it's clean (`git status`)
2. Pull latest: `git pull origin main`
3. All CI checks pass on `main`
4. No existing `v<version>` tag (`git tag -l v<version>`)
5. The version follows semver: patch for bugfixes, minor for features

## Steps

### 1. Generate changelog

List commits since the last binary tag:

```bash
git log $(git tag -l 'v*' --sort=-v:refname | head -1)..HEAD --format='%h %s' --no-merges
```

Categorize into `### Fixed`, `### Added`, `### Changed` sections following [Keep a Changelog](https://keepachangelog.com/). Include PR numbers. Skip `chore:`, `ci:`, `docs:` commits unless they're user-facing.

### 2. Bump version

- Update `crates/anyclaw/Cargo.toml`: set `version = "<version>"`
- Update `CHANGELOG.md`:
  - Add new version section under `[Unreleased]` with today's date
  - Add changelog entries from step 1
  - Update bottom links: add `[<version>]` compare link, update `[Unreleased]` link
- Run `cargo check -p anyclaw` to regenerate `Cargo.lock`

### 3. Commit and push to main

```bash
git add CHANGELOG.md Cargo.lock crates/anyclaw/Cargo.toml
git commit -m 'chore: release v<version>'
git push origin main
```

No PR needed — the code was already tested on main before the release.

### 4. Trigger Docker build

```bash
gh workflow run docker.yml -f version=<version>
```

The workflow validates that `Cargo.toml` matches the input version, creates the `v<version>` git tag, then builds and pushes multi-arch Docker images to GHCR.

### 5. Verify

```bash
gh run list --workflow=docker.yml --limit 3
```

Confirm the Docker workflow is running. Report the GHCR image URL when done:
`ghcr.io/donbader/anyclaw:<version>`
