# Releasing pathlint

🌐 **English** | [日本語](RELEASE.jp.md)

From 0.0.36 onward, releases are cut from `main` by **pushing a
`vX.Y.Z` tag** from a human's machine. The workflow runs
automatically on the tag, builds binaries, generates schemas,
publishes a GitHub Release, and publishes to crates.io. There is
no Actions tab dispatch and no version input — the tag itself is
the authoritative trigger. See
[ADR-0029](decisions/0029-release-trigger-tag-push.md) for why
the shape changed (it supersedes
[ADR-0010](decisions/0010-release-workflow-bump-skip.md)).

## How to release

### 1. Pre-release checklist

- Update `README.md`'s schema-pin example so the `<TAG>` placeholder
  example reads as the **previous** released version (so users
  copy-pasting see a known-good URL, not an unreleased one).
  Search for `https://github.com/ShortArrow/pathlint/releases/download/`
  in `README.md`.
- (optional, recommended) Run `scripts/bench.sh` and paste the
  hyperfine table into the release notes draft. Include the host
  description (CPU model, OS) so the numbers stay interpretable
  later. PRD §12 claims `<50 ms startup` — the bench script is the
  receipt.
- **English / Japanese parity check.** For each of the three pairs
  below, diff the change set since the last release and confirm
  both files were updated together:
    - `README.md` ↔ `docs/README.jp.md`
    - `docs/RELEASE.md` ↔ `docs/RELEASE.jp.md`
    - `docs/PRD.md` ↔ `docs/PRD.jp.md`
  Drift like `os_baseline_linux_sbin` documented only in EN
  (the 0.0.14 case the 0.0.19 docs sweep finally caught) is the
  kind of bug this checklist is here to prevent.

  Note: `docs/ARCHITECTURE.md` and `CHANGELOG.md` are intentionally
  English-only — a JP translation may follow in a future release
  if user feedback asks for it, but neither is gated by this parity
  check today.

### 2. Bump PR

Open a PR that:

- Bumps `Cargo.toml` to the new version (e.g. `version = "0.0.36"`)
  and refreshes `Cargo.lock`.
- Adds the `[X.Y.Z]` entry to `CHANGELOG.md`.
- Updates `docs/PRD.md` / `docs/README.jp.md` / `docs/PRD.jp.md`
  if the release ships any user-facing change.

Squash-merge it. If this release should **skip** the crates.io
publish, put the literal token `[skip publish]` (exact spelling,
one space, square brackets) in the squash commit message. The
default is to publish.

```text
chore: release 0.0.36

Release notes here.

[skip publish]   ← include this line only if you want to skip crates.io
```

### 3. Tag and push from main

Right after the bump PR merges, on a clean `main`:

```sh
git switch main
git pull --ff-only origin main
git tag -a vX.Y.Z -m "pathlint X.Y.Z"
git push origin vX.Y.Z
```

That `git push origin vX.Y.Z` triggers the workflow. The workflow
will:

1. Check that the `Cargo.toml` version at the tagged commit
   matches `X.Y.Z`. If not, the build fails immediately.
2. Cross-build for Linux / macOS / Windows.
3. Re-generate the JSON schemas from the tagged commit.
4. Create a GitHub Release with auto-generated notes.
5. (unless the tagged commit's message contains `[skip publish]`)
   exchange an OIDC token via Trusted Publishing and run
   `cargo publish`.

### Tag-on-`main` rule

Tags are cut on `main` only. The workflow does not enforce this
(runex follows the same convention, and pathlint has one releaser
today); the rule is upheld by discipline. If you need to release
from a hotfix branch, merge the hotfix to `main` first.

### `[skip publish]` token

The exact spelling is `[skip publish]` — square brackets, lowercase,
one space between `skip` and `publish`. Variants like
`[skip-publish]`, `[skip_publish]`, or `[ skip publish ]` do **not**
match the `contains()` check in the workflow and will publish to
crates.io anyway. Review the bump PR's squash commit message
carefully if you intend to skip.

## Branch and merge policy

`main` is the only long-lived branch.

- Day-to-day work happens on feature branches (`feat/...`,
  `fix/...`, etc.) and lands on `main` via squash-merged PRs.
- PR titles must follow Conventional Commits (`feat:`, `fix:`,
  `refactor:`, `chore:`, `docs:`, `test:`, `ci:`, ...). The squash
  commit's subject is the PR title; that becomes the line
  GitHub's auto-generated release notes pick up.
- No commits bypass PR review. From 0.0.36 onward the workflow
  never pushes to `main` (the version bump rides in a normal PR).

Recommended GitHub repo settings:

- Pull requests: allow squash merging only; default to PR title
  for the squash commit subject.
- Branch protection on `main`: require PR + status checks (`ci`,
  `pr-title-check`), require linear history. The
  `github-actions[bot]` push exemption that earlier releases
  needed is no longer required.

## Versioning

While the version starts with `0.`, both minor and patch bumps may
break the TOML schema or CLI. Once `0.1.0` ships, regular semver
applies.

## crates.io publishing

The first publish has to be done by hand:

```sh
cargo publish
```

After that, Trusted Publishing is configured on the crate's
settings page on crates.io. From 0.0.36 onward every tag push
publishes by default; include `[skip publish]` in the bump commit
message to opt out for a specific release.

## When something goes wrong

The new shape removes the partial-release recovery branch that
the `workflow_dispatch` flow needed. Re-pushing the same tag is
rejected by `git push` (non-fast-forward), and even if the tag
were force-deleted and re-pushed crates.io would reject the
duplicate publish.

**Recovery is the next patch release.** This is the same
discipline that 0.0.34 → 0.0.35 had to discover the hard way:
when a release fails partway, do not retry the same version; bump
to the next patch and re-release.

Specific failure modes:

- **Version mismatch guard fails.** The tag points at a commit
  whose `Cargo.toml` is at a different version. Delete the tag
  (`git push origin :refs/tags/vX.Y.Z`), open a PR that bumps
  `Cargo.toml` correctly, then re-tag.
- **Build fails on one OS matrix entry.** Re-run that job from
  the Actions tab if the failure looks transient. If the failure
  is real, fix forward and cut the next patch release.
- **publish-github fails.** Re-run that job. The build artifacts
  are still on the build job and the tag is unchanged.
- **publish-crates fails.** crates.io will not accept a republish
  of the same version. Cut the next patch release with the fix.
  Do not re-tag.

If a release needs to be abandoned entirely:

```sh
git switch main
git pull --ff-only
git push origin :refs/tags/vX.Y.Z
gh release delete vX.Y.Z --yes
```

The bump PR's commit stays on `main`; deleting that history is a
force-push to `main`, which branch protection should refuse.
Instead, open a follow-up PR that reverts the bump.
