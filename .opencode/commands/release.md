---
description: Release a new version — trigger the one-click release workflow
---

# Release

Argument: `<version>` (optional). If omitted, the workflow auto-detects from commits.

## Prerequisites

Verify before starting:
1. You are on `main` and it's clean (`git status`)
2. Pull latest: `git pull origin main`
3. All CI checks pass on `main`
4. No existing `v<version>` tag (if version provided): `git tag -l v<version>`

## Steps

### 1. Determine version (if not provided)

Preview what the workflow will auto-detect:

1. Get the latest binary tag: `git tag -l 'v*' --sort=-v:refname | head -1`
2. List commits since that tag: `git log <tag>..HEAD --format='%s' --no-merges`
3. Parse conventional commit prefixes:
   - Any `feat:` or `feat(` → bump minor
   - Only `fix:`, `perf:`, `refactor:` → bump patch
4. Suggest the version and ask the user to confirm before proceeding

### 2. Trigger release workflow

```bash
gh workflow run release.yml -f version=<version>
```

Or without version (auto-detect):

```bash
gh workflow run release.yml
```

The workflow handles everything: changelog generation (git-cliff), Cargo.toml bump, commit to main, git tag, multi-arch Docker build, GHCR push, and Trivy scan.

### 3. Verify

```bash
gh run list --workflow=release.yml --limit 3
```

Confirm the release workflow is running. Report the GHCR image URL when done:
`ghcr.io/donbader/anyclaw:<version>`
