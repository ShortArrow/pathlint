# ADR-0010: release workflow tolerates an already-bumped `Cargo.toml`

- **Status**: Accepted
- **Date**: 2026-05-31
- **Release**: 0.0.24 (workflow change shipped via PR #22; ADR backfilled in 0.0.30)
- **Category**: 8. Process / governance (release engineering)

## Context

`release.yml` is `workflow_dispatch`-triggered with a `version`
input. The original 0.0.x workflow (pre-PR #22) ran
`cargo set-version <input>`, then unconditionally
`git commit -m "chore: release X.Y.Z"`, then tagged. That worked
when `Cargo.toml` on `main` was *not* already at the input
version.

PR #22 (shipped in 0.0.24) fixed a real-world failure: a
prep PR had already bumped `Cargo.toml` to the target version as
part of the feature it shipped (so the same PR could pin
`tests/help_contract.rs` against the new `--version` output).
When the release workflow then ran for that version,
`cargo set-version` was a no-op, the `git commit` had nothing
to commit, and the whole `prepare` job failed at the `git
commit -m "chore: release X.Y.Z"` step.

The blunt fixes ("require fresh Cargo.toml", "skip set-version
entirely") both have problems:

- Requiring fresh `Cargo.toml` forces the bump into the release
  workflow only, which conflicts with the desire to pin
  version-sensitive tests in the same PR that introduces the
  feature.
- Skipping `set-version` entirely means the workflow trusts
  whoever opened the PR to have set the version correctly. A
  typo in `Cargo.toml` would ship the wrong version.

PR #22 chose a third path: run `set-version`, then check whether
anything was actually staged, and only commit when there is a
diff to commit. Either way, tag `HEAD` with `vX.Y.Z`.

This ADR records that choice and pins the alternatives so a
future contributor wondering "why not just require fresh
Cargo.toml" finds the answer here, not in PR #22's archaeology.

## Decision

The `prepare` job in `release.yml` performs both bump and
commit, but treats `cargo set-version` being a no-op as a
**non-fatal** condition. The relevant block (lines 80-99 of
`.github/workflows/release.yml`) is:

```bash
git add Cargo.toml Cargo.lock
if git diff --cached --quiet; then
  echo "Cargo.toml already at <input>; tagging HEAD without a release commit."
else
  git commit -m "chore: release <input>"
  git push origin HEAD:main
fi
git tag -a "v<input>" -m "pathlint <input>"
git push origin "v<input>"
```

`cargo set-version` is still called every run — it is the
authority on what the version *should* be. If `Cargo.toml`
already matches, `cargo set-version` is a no-op and the
conditional commit is skipped. If `Cargo.toml` *almost* matches
(e.g. the PR bumped to 0.0.24-rc1 but the release input is
0.0.24), `cargo set-version` rewrites the version and the
commit runs as before.

This way both PR styles work:
- **Bump-then-feature PRs** (the version-sensitive-tests case):
  PR pre-bumps `Cargo.toml`, ships, then user triggers release
  with the same version. Workflow tags `HEAD` without an extra
  commit.
- **Feature-only PRs**: PR ships without touching `Cargo.toml`,
  user triggers release with the next version, workflow bumps
  and commits as before.

## Alternatives considered

- **A. Require fresh `Cargo.toml` on main (reject if already
  bumped).** Rejected because it forbids the
  bump-in-feature-PR pattern. Some 0.0.x PRs (0.0.14, 0.0.21,
  0.0.24) needed to pin `--version` output or BREAKING test
  fixtures in the same PR that introduced the change; forcing
  the bump out of the PR would split each release into two
  PRs (the feature PR + a separate bump PR) with no real
  benefit.

- **B. Skip `cargo set-version` entirely and trust the PR's
  `Cargo.toml`.** Rejected: a typo in `Cargo.toml` (e.g.
  `0.0.244` instead of `0.0.24`) would ship the wrong version
  with no workflow-side guard. `cargo set-version` doubles as a
  validator — it parses the input, normalises whitespace, and
  fails if the input is malformed.

- **C. Always commit (even an empty commit when no diff).**
  Rejected because empty commits clutter `git log --oneline`
  and confuse `git bisect`. The release engineering value of
  "every release tag points at a commit" is preserved by
  tagging `HEAD` regardless; whether that `HEAD` is the
  feature PR's merge commit or a separate `chore: release` is
  cosmetic.

- **D. Detect the situation in CI on the PR (refuse PRs that
  ship `Cargo.toml` bumps unless paired with a follow-up
  workflow run).** Rejected as over-engineering. The
  workflow-side conditional commit fits in 5 lines and has no
  failure mode of its own; a CI gate that polices PR shape adds
  process without removing the underlying problem.

## Consequences

- **Positive.** Both PR styles work: 0.0.14 / 0.0.21 / 0.0.24's
  bump-in-feature-PR pattern lands cleanly, and 0.0.18 /
  0.0.20 / 0.0.25's feature-then-bump pattern also lands
  cleanly. The workflow no longer constrains PR shape.

- **Positive.** The `cargo set-version` step still runs every
  release, so a manual typo in `Cargo.toml` is caught (the
  step rewrites the version to whatever the workflow input
  says, with the conditional commit picking up the
  correction).

- **Negative.** The release commit message
  (`chore: release X.Y.Z`) is not produced for every release;
  for "pre-bumped" releases there is no such commit and the
  tag points at the PR's squash commit instead. Anyone
  greping `git log --oneline | grep '^chore: release'` to
  enumerate releases misses those. Mitigation: use
  `git tag --list 'v*'` instead, which is the authoritative
  list anyway.

- **Negative.** The workflow contract is now slightly more
  complex (two paths through `prepare`). The complexity is
  contained in one `if` and one echo, so it does not pose a
  realistic maintenance hazard.

- **Follow-up.** None. If a future change introduces a new
  commit type (e.g. an annotated release commit with extra
  metadata), the conditional commit block stays as the model
  for "either commit or skip cleanly".
