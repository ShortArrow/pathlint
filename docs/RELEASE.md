# Releasing pathlint

Releases are cut from `main` by running the `release` workflow on
GitHub Actions. The workflow handles the version bump, tag, build,
and GitHub Release. crates.io publishing is opt-in.

## How to release

1. Open the repo on GitHub. Go to **Actions** → **release** →
   **Run workflow**.
2. Enter the new version number (e.g. `0.0.8`). Don't include the
   `v`; the workflow adds it for the tag.
3. Decide whether to publish to crates.io. The checkbox is off by
   default. Tick it once Trusted Publishing is set up (see below).
4. Click **Run workflow**.

The workflow will:

1. bump `Cargo.toml` and refresh `Cargo.lock`,
2. run fmt / clippy / test / package,
3. commit `chore: release X.Y.Z`, tag `vX.Y.Z`, push to `main`,
4. cross-build for Linux / macOS / Windows,
5. create a GitHub Release with auto-generated notes,
6. (if asked) publish to crates.io.

## Branch and merge policy

`main` is the only long-lived branch.

- Day-to-day work happens on feature branches (`feat/...`,
  `fix/...`, etc.) and lands on `main` via squash-merged PRs.
- PR titles must follow Conventional Commits (`feat:`, `fix:`,
  `refactor:`, `chore:`, `docs:`, `test:`, `ci:`, ...). The squash
  commit's subject is the PR title; that becomes the line
  GitHub's auto-generated release notes pick up.
- The only commits that bypass PR review are `chore: release
  X.Y.Z` from the release workflow's `prepare` job, run as
  `github-actions[bot]`.

Recommended GitHub repo settings:

- Pull requests: allow squash merging only; default to PR title
  for the squash commit subject.
- Branch protection on `main`: require PR + status checks (`ci`,
  `pr-title-check`), require linear history, allow
  `github-actions` to push for the release commit.

## Versioning

While the version starts with `0.`, both minor and patch bumps may
break the TOML schema or CLI. Once `0.1.0` ships, regular semver
applies.

## crates.io publishing

The first publish has to be done by hand:

```sh
cargo publish
```

After that, set up Trusted Publishing on the crate's settings
page on crates.io and the `release` workflow can do it. Tick
**Also publish to crates.io** when running the workflow.

## When something goes wrong

- **prepare fails.** Nothing was pushed. Fix on `main`, run the
  workflow again.
- **build fails after prepare succeeded.** The bump commit and
  tag are already on `main`. Either fix forward and bump again,
  or delete the tag (`git push origin :refs/tags/vX.Y.Z`) and
  re-run with the same version.
- **publish-github fails.** Re-run that job alone; the artifacts
  are still on the build job.
- **publish-crates fails.** crates.io won't accept a republish of
  the same version, so any retry has to use the next version
  number.

To abandon a release entirely:

```sh
git switch main
git pull --ff-only
git reset --hard HEAD~1
git push --force-with-lease origin main
git push origin :refs/tags/vX.Y.Z
```

## Manual fallback

If the workflow itself is broken:

```sh
./scripts/release-check.sh X.Y.Z   # local fmt/clippy/test/package
cargo set-version X.Y.Z
git commit -am "chore: release X.Y.Z"
git tag -a vX.Y.Z -m "pathlint X.Y.Z"
git push origin main vX.Y.Z
gh release create vX.Y.Z --generate-notes ...
cargo publish      # if you want it on crates.io
```
