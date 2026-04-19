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

### 2. Trigger the prepare workflow

```bash
gh workflow run release-prepare.yml -f version=<version>
```

Or without version (auto-detect):

```bash
gh workflow run release-prepare.yml
```

This creates a `release/v<version>` branch with changelog + version bump, opens a PR, and enables auto-merge.

### 3. Verify the prepare workflow

```bash
gh run list --workflow=release-prepare.yml --limit 3
```

Confirm the PR was created. The user reviews and merges (or auto-merge completes after CI passes).

### 4. Publish happens automatically

When the release PR merges, the **Release — Publish** workflow triggers automatically. It:
- Extracts the version from the `release/v*` branch name
- Creates the `v<version>` git tag
- Builds multi-arch Docker images (amd64 + arm64)
- Pushes to GHCR with tags: `<version>`, `<major>.<minor>`, `<sha>`, `latest`
- Runs Trivy vulnerability scan
- Verifies multi-arch manifest

### 5. Verify the publish workflow

```bash
gh run list --workflow=release-publish.yml --limit 3
```

Confirm the release completed. Report the GHCR image URL:
`ghcr.io/donbader/anyclaw:<version>`
