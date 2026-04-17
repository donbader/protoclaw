# Releasing

## SDK Crates (automated)

SDK crate releases are fully automated via [release-plz](https://release-plz.ieni.dev/):

1. On every push to `main`, release-plz opens/updates a "release PR" with version bumps and changelog entries
2. When the release PR is merged, release-plz publishes to crates.io and creates git tags (`anyclaw-sdk-<crate>-v<version>`)
3. Uses crates.io Trusted Publishing — no stored API tokens

**Versioning:** SDK crates follow [semver](https://semver.org/). release-plz detects breaking changes via `cargo-semver-checks` and bumps accordingly.

**Publish order:** `sdk-types` → `sdk-agent`, `sdk-channel`, `sdk-tool` (dependency order).

## Binary (manual trigger)

Anyclaw is distributed as Docker images — no native binary releases. The release process:

1. Update `crates/anyclaw/Cargo.toml` version
2. Update `CHANGELOG.md` — move items from `[Unreleased]` to the new version section
3. Create a PR with these changes, merge it
4. Tag the merge commit and push:

```bash
git tag v<version>
git push origin v<version>
```

5. The tag triggers the **Docker** workflow (`.github/workflows/docker.yml`): builds multi-arch Docker images (amd64 + arm64), pushes to GHCR

**Versioning:** The binary follows [semver](https://semver.org/). Bump minor for new features, patch for bugfixes.

## When to Release

- **SDK crates**: Automatically on every push to `main` that changes SDK code (release-plz decides)
- **Binary**: When accumulated changes warrant it. No fixed schedule. The maintainer decides.

## Checklist Before Tagging a Binary Release

- [ ] All CI checks pass on `main`
- [ ] `CHANGELOG.md` is updated with the new version
- [ ] `crates/anyclaw/Cargo.toml` version matches the tag you're about to push
- [ ] No known regressions in the examples
