# Release process

How to cut a new pathlint release. Cutting a release is one
button press: **Actions → release → Run workflow → enter the new
version → Run**. Everything else (bump, tag, build, GitHub
Release, crates.io publish) runs from `.github/workflows/release.yml`.

No `develop` branch. No `chore: release` commit you have to hand-
write. No `CHANGELOG.md` to keep in sync — release notes are
auto-generated from PR titles via Conventional Commits.

## TL;DR

1. Open Actions → **release** → **Run workflow**.
2. Enter the new version (e.g. `0.0.8`). Drop the `v`; the workflow
   adds it for the tag.
3. Click **Run workflow**.

That's the whole user-facing flow. The rest of this document
describes what the workflow does, what to do when it fails, and
the one-time setup needed for a fresh repo.

## Versioning policy

- `0.0.x` and `0.1.x` may both introduce breaking changes to the
  TOML schema and CLI surface; this is announced in the GitHub
  Release notes.
- A patch bump (`0.0.A` → `0.0.A+1`) is the default during pre-1.0
  — bump anytime there is shippable behaviour change.
- A minor bump (`0.0.x` → `0.1.0`) is reserved for the moment we
  declare schema/CLI stable enough to call the regular semver
  contract into effect.

## Branch and merge policy

`main` is the only long-lived branch. Day-to-day work happens on
feature branches and lands on `main` via squash-merged PRs. The
result: `git log --oneline main` reads as a list of PRs, one
commit per PR, plus the occasional `chore: release X.Y.Z` from
the release workflow.

- **Feature branches.** Use `feat/<name>` / `fix/<name>` /
  `refactor/<name>` / `chore/<name>` etc. Push them to your fork
  or to a topic branch on `origin`; open a PR against `main`.
- **Squash merge only.** PRs land on `main` with **Squash and
  merge**. Merge commits and rebase merging are off. The squash
  commit's subject is the **PR title**, and the body is whatever
  GitHub's "Default to PR title for squash merge commits" puts
  there. PR titles must follow Conventional Commits — enforced
  by `.github/workflows/pr-title-check.yml`.
- **No direct push to `main`.** The single exception is the
  `release.yml` workflow's `prepare` job, which pushes
  `chore: release X.Y.Z` (and the matching tag) on behalf of
  `github-actions[bot]` via `GITHUB_TOKEN`. Branch protection
  should allow that bot account to bypass push restrictions.
- **Linear history.** "Require linear history" is on, so the only
  shapes that can appear on `main` are squash commits and the
  release bot's commits — no merge commits, no fast-forward of
  arbitrary local branches.

The recommended GitHub repo settings (one-time):

- Settings → General → Pull Requests:
  - Allow merge commits: **off**
  - Allow squash merging: **on**
  - Allow rebase merging: **off**
  - Default to PR title for squash merge commits: **on**
- Settings → Branches → main → Branch protection:
  - Require a pull request before merging: **on**
  - Require status checks to pass: `ci`, `pr-title-check`
  - Require linear history: **on**
  - Restrict who can push to matching branches: allow
    `github-actions` (so the release bot can push the bump commit
    + tag).

## What the workflow does

`release.yml` runs four jobs in sequence:

1. **prepare**: bumps `Cargo.toml` (and refreshes `Cargo.lock`) to
   the input version using `cargo set-version`, runs the standard
   gate (`fmt --check`, `clippy -D warnings`, `cargo test`,
   `cargo package --allow-dirty`), commits as `chore: release
   X.Y.Z`, tags `vX.Y.Z`, and pushes both back to `main` using the
   auto-provided `GITHUB_TOKEN`.
2. **build**: cross-builds release binaries on ubuntu-latest,
   windows-latest, and macos-latest for `x86_64-unknown-linux-gnu`
   / `x86_64-pc-windows-msvc` / `x86_64-apple-darwin` /
   `aarch64-apple-darwin`. Termux users build from source.
3. **publish-github**: assembles `SHA256SUMS`, creates the GitHub
   Release, attaches every archive + checksums, and writes
   release notes from the PR titles between the previous tag and
   this one (`generate_release_notes: true`). Releases tagged
   `v0.*` are marked prerelease.
4. **publish-crates**: exchanges the workflow's OIDC identity for
   a short-lived crates.io token via
   `rust-lang/crates-io-auth-action@v1`, then runs `cargo
   publish`. No long-lived `CARGO_REGISTRY_TOKEN` is stored.

## How release notes get good content

The auto-generated notes are only as informative as the PR titles
that feed them. To keep the output readable, every PR title must
follow Conventional Commits, enforced by
`.github/workflows/pr-title-check.yml`. Allowed types:

```
feat fix refactor perf test docs build ci chore revert
```

Examples:

```
feat: pathlint sort --dry-run (R5 read-only PATH repair)
fix(catalog): correct unix fallback for termux
refactor!: drop bump-on-main flow
chore(deps): bump clap to 4.6
```

GitHub then groups the release body by section (`### Features` /
`### Bug Fixes` / `### Other Changes`) automatically.

## One-time setup

Two things have to be configured outside this repo, both done
once and forgotten:

1. **crates.io Trusted Publishing.** Open the crate's settings on
   crates.io → "Trusted Publishers" → "Add publisher". Fill in:
   - Repository owner / name: `ShortArrow/pathlint`
   - Workflow filename: `release.yml`
   - Environment: leave blank (we don't gate on a GitHub
     environment).
   The first publish has to be done with a manual token *before*
   Trusted Publishing can be set up; from 0.0.8 onward it's
   workflow-driven.
2. **Branch protection on `main`.** Require status checks `ci`
   and `pr-title-check` to pass before merging, and disallow
   force-pushes. The `prepare` job in `release.yml` pushes to
   `main` directly on behalf of `github-actions[bot]`, which the
   default protection rules already allow via `GITHUB_TOKEN`.

## When something goes wrong

The four jobs are sequenced so failure is easy to diagnose:

- **prepare fails**: nothing was pushed. Fix on `main`, re-run
  the workflow with the same version.
- **build fails after prepare succeeded**: the tag is already on
  `main` and pushed. Either fix forward and bump to `X.Y.Z+1`, or
  delete the tag (`git push origin :refs/tags/vX.Y.Z`,
  `git tag -d` locally) and re-run with the same version. The
  `chore: release` commit on main can stay — it's harmless.
- **publish-github fails**: the GitHub Release isn't visible. The
  artifacts are still on the build job as workflow artifacts; you
  can re-run just the publish-github job.
- **publish-crates fails**: the GitHub Release exists but
  crates.io is missing this version. Re-run only the
  publish-crates job. crates.io rejects republishing the same
  version, so you may need to bump to `X.Y.Z+1` if the failure
  was a network hiccup that crates.io recorded.

If you need to abandon a release entirely:

```sh
# Locally, undo the prepare commit + tag and force the remote.
git switch main && git pull --ff-only
git reset --hard HEAD~1                   # drop chore: release X.Y.Z
git push --force-with-lease origin main
git push origin :refs/tags/vX.Y.Z
```

Use `--force-with-lease` rather than `--force` so you don't
clobber anything else that landed in the meantime.

## Verifying a release locally

Pull the published artifact on a clean machine:

```sh
curl -L -o pathlint.tar.gz \
  "https://github.com/ShortArrow/pathlint/releases/download/vX.Y.Z/pathlint-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz"
tar -xzf pathlint.tar.gz
./pathlint-vX.Y.Z-x86_64-unknown-linux-gnu/pathlint --version
```

The version printed must match the tag. Verify the checksum
against `SHA256SUMS` from the same release.

## Manual fallback (if release.yml is broken)

If the workflow itself is misbehaving and you need to ship anyway,
`scripts/release-check.sh X.Y.Z` runs the same gate locally
(`fmt --check`, `clippy -D warnings`, `cargo test`, `cargo
package`). After it passes:

```sh
cargo set-version X.Y.Z
git commit -am "chore: release X.Y.Z"
git tag -a vX.Y.Z -m "pathlint X.Y.Z"
git push origin main vX.Y.Z
gh release create vX.Y.Z --generate-notes ...
cargo publish
```

This is intentionally tedious. The whole point of `release.yml`
is that you don't run these by hand on the regular path. Use the
fallback only when the workflow is broken; fix the workflow first
if you can.
