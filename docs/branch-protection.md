# Branch Protection Rules

## `main` branch ruleset

Apply via GitHub → Settings → Rules → Rulesets → New ruleset.

### Target

- Branch name pattern: `main`

### Rules

| Rule | Setting |
|------|---------|
| Restrict deletions | Enabled |
| Require a pull request before merging | Enabled |
| Required approvals | 1 |
| Dismiss stale reviews on new pushes | Enabled |
| Require status checks to pass | Enabled |
| Required checks | `lint`, `test`, `MSRV (1.94)`, `Security Audit`, `doc` |
| Require branches to be up to date | Enabled |
| Block force pushes | Enabled |

### Bypass list

- @donbader (maintainer) — bypasses approval requirement only, CI still required

### Notes

- The bypass allows the solo maintainer to merge their own PRs after CI passes without self-approving
- External contributors still require 1 approval from a maintainer
- CI checks are required for everyone, including bypass users
- When a second maintainer joins, consider removing the bypass and requiring mutual review
