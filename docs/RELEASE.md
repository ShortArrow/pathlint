# Release process

How to cut a new pathlint release. Optimised for the `0.0.x` /
`0.1.x` cadence — small, frequent, occasionally schema-breaking —
and structured so each step has a concrete check you can paste into
a terminal. The goal is **reproducible**: anyone with push rights
should be able to follow this and produce a clean release without
remembering what the last person did.

## Prerequisites

- The work for the new version is on `develop`, CI green.
- Working tree clean on both `develop` and `main`.
- You have push rights to `origin`.
- For a publishing release: `cargo login` set up locally if you
  plan to `cargo publish` (the GitHub Release pipeline handles the
  prebuilt binaries on its own).

## Versioning policy

- `0.0.x` and `0.1.x` may both introduce breaking changes to the
  TOML schema and CLI surface; this is documented in `CHANGELOG.md`.
- A patch bump (`0.0.A` → `0.0.A+1`) is the default during
  pre-1.0 — bump anytime there is shippable behaviour change.
- A minor bump (`0.0.x` → `0.1.0`) is reserved for the moment we
  declare schema/CLI stable enough to call the regular semver
  contract into effect.

## Two release shapes

There are two version-bump timings, and both leave a clean
`first-parent main` history. Pick the one that matches how the work
landed:

- **Bump-on-main (default).** `Cargo.toml` and `CHANGELOG.md` stay
  on the previous version through the whole development cycle on
  `develop`. The bump happens on `main` *after* the no-ff merge,
  as the `chore: release` commit. This is what 0.0.2 – 0.0.6
  used. Best when the version number isn't decided until cut time.
- **Bump-on-develop.** `Cargo.toml` was bumped to `X.Y.Z` early in
  the cycle (e.g. when the first feature commit defined the
  release theme), and `CHANGELOG.md` already has a `## [X.Y.Z] -
  YYYY-MM-DD` section above `[Unreleased]` (date may still be a
  placeholder). The `chore: release` commit on `main` then only
  fixes the date and refreshes `Cargo.lock`. This is what 0.0.7
  used. Best when bumping early lets `cargo run --version` track
  the in-progress release while you work.

The steps below cover both — look for the **A: Bump-on-main** /
**B: Bump-on-develop** call-outs at the version-bump step.

## Steps

### 1. Sanity-check `develop`

Run the bundled checker; it gates everything below:

```sh
git switch develop
git pull --ff-only
./scripts/release-check.sh X.Y.Z
```

`release-check.sh` runs:

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `cargo package --allow-dirty` (catches manifest / `exclude` slips)
- For **bump-on-develop**: verifies `Cargo.toml` already says
  `X.Y.Z` and `CHANGELOG.md` has a `## [X.Y.Z]` section.
- For **bump-on-main**: verifies the *current* version is
  `<X.Y.Z` and `CHANGELOG.md` has a `## [Unreleased]` section
  ready to be renamed.

If any of those fail, fix on `develop` first. Don't carry red into
the merge.

### 2. Merge `develop` into `main` with `--no-ff`

```sh
git switch main
git pull --ff-only           # keep main aligned with origin first
git merge --no-ff develop -m "Merge branch 'develop' for vX.Y.Z

<one paragraph: what's notable in this version, why it's worth
bumping, anything users should look out for>"
```

`--no-ff` matters: it leaves a single merge commit per release in
`main`'s history, so `git log --first-parent main` reads as a
release-by-release timeline. Squash or fast-forward would lose
that shape.

Sanity check after the merge:

```sh
git log --first-parent main -3 --oneline
```

The top entry must be your new `Merge branch 'develop' for vX.Y.Z`.

### 3. Land the release commit on `main`

#### A. Bump-on-main (the version is still the old one)

Edit:

- `Cargo.toml` — `version = "X.Y.Z"`
- `CHANGELOG.md`:
  - Replace the leading `## [Unreleased]` section with
    `## [X.Y.Z] - YYYY-MM-DD` (today, ISO-8601).
  - Add a fresh empty `## [Unreleased]` above it.
  - Update the comparison links at the bottom:
    - `[Unreleased]: .../compare/vX.Y.Z...HEAD`
    - `[X.Y.Z]: .../releases/tag/vX.Y.Z`

Sync `Cargo.lock` and verify:

```sh
cargo build
cargo test
./target/debug/pathlint --version   # must print X.Y.Z
```

Commit:

```sh
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "chore: release X.Y.Z

<short summary>"
```

#### B. Bump-on-develop (Cargo.toml already says X.Y.Z)

You only need to:

- Confirm `CHANGELOG.md` `## [X.Y.Z]` line carries today's date
  (fix it if a placeholder snuck in).
- Confirm the comparison links at the bottom of `CHANGELOG.md`
  point at `vX.Y.Z`.

Then refresh `Cargo.lock` (no version diff, but `cargo build` may
still touch it) and verify:

```sh
cargo build
cargo test
./target/debug/pathlint --version   # must print X.Y.Z
```

Commit (only the files you actually touched):

```sh
git add CHANGELOG.md  # plus Cargo.lock if it changed
git commit -m "chore: release X.Y.Z

<short summary>"
```

In both cases, the resulting `git log --first-parent main` should
show `chore: release X.Y.Z` directly above
`Merge branch 'develop' for vX.Y.Z`. That's the canonical shape.

### 4. Forward-merge `main` back to `develop`

So `develop` always contains everything `main` has plus the
in-progress work for the next version.

```sh
git switch develop
git merge --ff-only main
```

If this isn't a fast-forward (i.e. `develop` got commits while you
were releasing), use a regular `git merge main` — but try to
serialise releases to keep this simple.

### 5. Push branches and the tag

The tag triggers `release.yml`, which builds binaries for
`x86_64-{linux,windows,darwin}` + `aarch64-darwin`, packages them
into archives + checksums, and creates the GitHub Release.

```sh
git push origin main develop
git tag -a vX.Y.Z -m "pathlint X.Y.Z"
git push origin vX.Y.Z
```

Watch the Actions tab — `release.yml` should turn green within a
few minutes. The release is marked as `prerelease: true` while the
version starts with `v0.`; it flips to a normal release at
`v1.0.0`.

### 6. Publish to crates.io (optional, when ready)

`0.0.x` is **not** auto-published. When you want to publish:

```sh
cargo publish --dry-run     # check the package layout first
cargo publish
```

Don't `cargo publish` until `release.yml` has finished green —
crates.io can't be unpublished, so binaries should land first as a
sanity check.

## Verification

After step 5, fetch the published artifact on a clean machine:

```sh
# From GitHub Releases:
curl -L -o pathlint.tar.gz \
  "https://github.com/ShortArrow/pathlint/releases/download/vX.Y.Z/pathlint-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz"
tar -xzf pathlint.tar.gz
./pathlint-vX.Y.Z-x86_64-unknown-linux-gnu/pathlint --version
```

The version printed must match the tag.

## Rollback

If something is wrong **before** the tag push: just delete the
`chore: release` commit on `main`, force-push if already pushed
(coordinate with anyone else who has fetched), and try again.

If the tag is already pushed but `release.yml` failed mid-flight
or produced a broken artifact:

```sh
# Delete the GitHub Release and the tag, both locally and remotely.
gh release delete vX.Y.Z --yes
git push origin :refs/tags/vX.Y.Z
git tag -d vX.Y.Z
```

Then fix the issue on `develop`, bump to **X.Y.Z+1** (do NOT
reuse the same number — even if no one downloaded the broken
release, crates.io and people's local toolchain caches won't
notice the overwrite), and run the process again.

## Cheatsheet

```sh
# From develop, ready to cut X.Y.Z:
./scripts/release-check.sh X.Y.Z

git switch main && git pull --ff-only
git merge --no-ff develop -m "Merge branch 'develop' for vX.Y.Z"

# Bump-on-main: edit Cargo.toml + CHANGELOG.md.
# Bump-on-develop: just refresh CHANGELOG date + Cargo.lock.
cargo build && cargo test
./target/debug/pathlint --version    # must print X.Y.Z

git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "chore: release X.Y.Z"

# Forward-merge to develop:
git switch develop && git merge --ff-only main

# Tag and push:
git push origin main develop
git tag -a vX.Y.Z -m "pathlint X.Y.Z" && git push origin vX.Y.Z

# Optional, once the GitHub Release lands clean:
cargo publish --dry-run && cargo publish
```
