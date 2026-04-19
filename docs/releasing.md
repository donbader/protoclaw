# Releasing

## SDK Crates (automated)

SDK crate releases are fully automated via [release-plz](https://release-plz.ieni.dev/):

1. On every push to `main`, release-plz opens/updates a "release PR" with version bumps and changelog entries
2. When the release PR is merged, release-plz publishes to crates.io and creates git tags (`anyclaw-sdk-<crate>-v<version>`)
3. Uses crates.io Trusted Publishing — no stored API tokens

**Versioning:** SDK crates follow [semver](https://semver.org/). release-plz detects breaking changes via `cargo-semver-checks` and bumps accordingly.

**Publish order:** `sdk-types` → `sdk-agent`, `sdk-channel`, `sdk-tool` (dependency order).

**Workflow:** `.github/workflows/release-sdk.yml`

## Binary (two-stage workflow)

Anyclaw is distributed as Docker images — no native binary releases. Two images are published:

- `ghcr.io/donbader/anyclaw` — core binary only (distroless)
- `ghcr.io/donbader/anyclaw-ext` — all ext/ binaries (distroless, for `COPY --from=` usage)

### Stage 1: Prepare (manual trigger)

1. Go to Actions → "Release — Prepare" → Run workflow
2. Optionally provide a version (e.g. `0.10.0`). If left empty, version is auto-detected from conventional commits since the last tag (feat → minor bump, otherwise → patch bump).
3. The workflow:
   - Detects version from commits (if not provided)
   - Generates changelog entries via git-cliff
   - Bumps `crates/anyclaw/Cargo.toml`
   - Creates a `release/v<version>` branch and opens a PR
   - Enables auto-merge (squash) on the PR

**Workflow:** `.github/workflows/release-prepare.yml`

**CLI shortcut:**

```bash
gh workflow run release-prepare.yml
gh workflow run release-prepare.yml -f version=0.10.0
```

### Stage 2: Publish (automatic on PR merge)

When the release PR merges to `main`, the publish workflow triggers automatically:
   - Extracts version from the `release/v*` branch name
   - Creates the `v<version>` git tag
   - Builds multi-arch Docker images (amd64 + arm64) for both `core` and `ext`
   - Pushes to GHCR with tags: `<version>`, `<major>.<minor>`, `<sha>`, `latest`
   - Runs Trivy vulnerability scan
   - Verifies multi-arch manifest

**Workflow:** `.github/workflows/release-publish.yml`

**Versioning:** The binary follows [semver](https://semver.org/). Bump minor for new features, patch for bugfixes.

## When to Release

- **SDK crates**: Automatically on every push to `main` that changes SDK code (release-plz decides)
- **Binary**: When accumulated changes warrant it. No fixed schedule. The maintainer decides.

## Checklist Before Releasing

- [ ] All CI checks pass on `main`
- [ ] You're on `main` with a clean working tree
- [ ] No known regressions in the examples
