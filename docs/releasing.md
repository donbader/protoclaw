# Releasing

## SDK Crates (automated)

SDK crate releases are fully automated via [release-plz](https://release-plz.ieni.dev/):

1. On every push to `main`, release-plz opens/updates a "release PR" with version bumps and changelog entries
2. When the release PR is merged, release-plz publishes to crates.io and creates git tags (`anyclaw-sdk-<crate>-v<version>`)
3. Uses crates.io Trusted Publishing — no stored API tokens

**Versioning:** SDK crates follow [semver](https://semver.org/). release-plz detects breaking changes via `cargo-semver-checks` and bumps accordingly.

**Publish order:** `sdk-types` → `sdk-agent`, `sdk-channel`, `sdk-tool` (dependency order).

**Workflow:** `.github/workflows/release-sdk.yml`

## Binary (one-click workflow_dispatch)

Anyclaw is distributed as Docker images — no native binary releases. The entire release is a single click:

1. Go to Actions → "Release" → Run workflow
2. Optionally provide a version (e.g. `0.10.0`). If left empty, version is auto-detected from conventional commits since the last tag (feat → minor bump, otherwise → patch bump).
3. The workflow handles everything:
   - Detects version from commits (if not provided)
   - Generates changelog entries via git-cliff
   - Bumps `crates/anyclaw/Cargo.toml`
   - Commits to `main` and pushes
   - Creates the `v<version>` git tag
   - Builds multi-arch Docker images (amd64 + arm64)
   - Pushes to GHCR with tags: `<version>`, `<major>.<minor>`, `<sha>`, `latest`
   - Runs Trivy vulnerability scan
   - Verifies multi-arch manifest

**Workflow:** `.github/workflows/release.yml`

**CLI shortcut:**

```bash
gh workflow run release.yml
gh workflow run release.yml -f version=0.10.0
```

**Versioning:** The binary follows [semver](https://semver.org/). Bump minor for new features, patch for bugfixes.

**Run-name:** Each run shows as "Release v0.X.Y" in the Actions tab for easy identification.

## When to Release

- **SDK crates**: Automatically on every push to `main` that changes SDK code (release-plz decides)
- **Binary**: When accumulated changes warrant it. No fixed schedule. The maintainer decides.

## Checklist Before Releasing

- [ ] All CI checks pass on `main`
- [ ] You're on `main` with a clean working tree
- [ ] No known regressions in the examples
