# ADR-0029: release workflow trigger moves from `workflow_dispatch` to `on: push: tags`

- **Status**: Accepted
- **Date**: 2026-06-13
- **Release**: 0.0.36
- **Category**: 8. Process / governance (release engineering)

## Context

Through 0.0.34 the release workflow was `workflow_dispatch`-triggered:
a human opened the Actions tab, typed the version number, ticked
"Also publish to crates.io" (off by default), and clicked **Run
workflow**. CI did everything else — `cargo set-version`,
`git commit`, `git tag`, `git push`. ADR-0010 records the
"tolerate an already-bumped Cargo.toml" refinement of that flow
shipped in 0.0.24.

0.0.34 exposed a failure mode of that design. The first dispatch
left `publish_crates` at its default `false`, so the crate
shipped a GitHub Release but **not** crates.io. The retry hit
the workflow's unconditional `git tag` and failed. PR #34
patched the workflow to tolerate an existing tag at HEAD. The
third dispatch then refused because `main` had moved past the
tagged commit (the PR #34 merge itself shifted HEAD), and the
safety check correctly flagged "tag points elsewhere". Recovery
required cutting an entirely new patch release (0.0.35) just to
get the crates.io publish through. A version number was consumed
for a release that contained no functional change.

The root cause is not the `publish_crates=false` default alone;
it is the **CI-managed tag** design. When CI creates the tag,
recovery from any partial failure must either
- re-tag the same version (which CI refuses, correctly, to
  prevent silent overwrites of a published release), or
- bump and re-release (which costs a version number per
  recovery).

V:\runex has been running a different shape long enough to
prove out: trigger on `push: tags: ["v*"]`, version-bump
performed by a human in a normal feature PR, tag pushed by the
human from `main` after merge. crates.io publishing is opt-out
via `[skip publish]` in the bump commit message. There is no
CI-managed tag and no partial-release recovery branch in the
workflow — recovery is "cut the next patch release", the same
discipline pathlint had to discover the hard way in
0.0.34 → 0.0.35.

User feedback verbatim (2026-06-09, after the 0.0.34 failure
streak): "tagはmainブランチでしか切らないもしくは
release.yamlの中でtag付けするflowにしたほうがよさそう。
contribute.mdに書くかrelease.yamlで自動化するか"
("tags should only be cut on main, or the workflow itself should
do the tagging — either document it in CONTRIBUTING.md or
automate in release.yaml").

This ADR records the decision to align pathlint's release
workflow with runex's shape, and to mark ADR-0010 as superseded
because its "tolerate an already-bumped Cargo.toml" refinement
is meaningful only when CI runs `cargo set-version`. Under the
new shape CI never bumps the version, so the special case ADR-0010
solves cannot occur.

## Decision

The release workflow trigger changes from `workflow_dispatch`
to `on: push: tags: ["v*"]`. The `prepare` job (set-version,
commit, push, tag) is deleted. Version bump and tag creation
become **human responsibilities performed on `main`**, in this
order:

1. Open a PR that bumps `Cargo.toml` (and `Cargo.lock`) to the
   target version and updates `CHANGELOG.md`. Squash-merge as
   usual. If the release should skip crates.io publishing, include
   `[skip publish]` (exact spelling, single space) in the squash
   commit message.
2. On the `main` branch at the merge commit:
   `git tag -a vX.Y.Z -m "pathlint X.Y.Z" && git push origin vX.Y.Z`.
3. The push triggers the workflow. It builds binaries, generates
   schemas, publishes a GitHub Release, and (unless
   `[skip publish]` was in the bump commit) publishes to
   crates.io.

The workflow keeps one defensive step that runex does not need
because runex's release branch is short-lived: a **version
mismatch guard** that reads the `version =` line from
`Cargo.toml` at the tagged commit and fails the build if it does
not equal `${GITHUB_REF_NAME#v}`. This catches the new failure
mode introduced by the design — a human tagging a commit where
`Cargo.toml` was not bumped — at workflow start instead of after
the build matrix.

crates.io publishing flips from opt-in (`inputs.publish_crates`
default `false`) to opt-out (`!contains(github.event.head_commit.message, '[skip publish]')`).
The default for a tagged release is "publish everywhere". This
makes the 0.0.34 failure mode — forgetting to tick the box and
shipping a half release — structurally impossible.

Tag-on-`main` is enforced by **documentation only**, not by a
workflow step. runex follows the same convention; pathlint has
one releaser today; an enforcement step would be the right
follow-up if mis-tagging ever happens.

## Alternatives considered

- **A. Keep `workflow_dispatch`, harden the existing recovery
  branch.** PR #34 already added "tolerate an existing tag at
  HEAD". Further hardening could allow re-tagging the same
  version if no GitHub Release exists yet, or could detect the
  partial-release state and offer to "complete" it on a second
  dispatch. Rejected: every extra branch in the recovery path
  is another code path that can be wrong. The runex shape
  eliminates the recovery branch entirely (re-tag is impossible
  by push semantics; recovery is the next patch release). The
  design is smaller, not bigger, and pathlint already learned
  this lesson once in 0.0.34 → 0.0.35.

- **B. Keep `workflow_dispatch` but flip the publish default to
  `true`.** Solves the specific "forgot to tick the box"
  failure of 0.0.34 without changing trigger shape. Rejected:
  this leaves the CI-managed-tag race intact for every future
  partial failure (build matrix flake, OIDC auth blip, etc.).
  The 0.0.34 incident was the *visible* symptom; the *cause*
  is "CI owns the tag, so recovery from any failure stages
  through the tag step".

- **C. Move the version bump into the workflow itself — enforce
  fresh `Cargo.toml` on every run.** This would be ADR-0010's
  alternative A revisited (which ADR-0010 rejected because it
  forbids the bump-in-feature-PR pattern). The new world is
  different: with tag-push trigger, the tag *is* the
  authoritative version, so `Cargo.toml` becomes a redundant
  source of truth that the guard step polices. Forcing a
  workflow-side bump on top of that would re-introduce CI
  commits to `main` — exactly the property we are trying to
  remove.

- **D. Add a workflow step that enforces tag-on-`main`
  (`git branch -r --contains $GITHUB_SHA | grep -q origin/main`).**
  Rejected for 0.0.36: runex's precedent is doc-only enforcement,
  pathlint has one releaser, and the enforcement step adds a
  failure mode of its own (e.g. on a force-pushed `main` the
  check could pass for a tag that *was* on `main` and now is
  not). Reasonable follow-up if mis-tagging ever happens.

- **E. Run the workflow on both `workflow_dispatch` and
  `push: tags`.** Lets manual dispatch coexist with tag-push.
  Rejected: the two triggers would have to share the same job
  graph, which means either the `prepare` job stays (the
  CI-managed-tag race comes back) or the dispatch trigger
  becomes a footgun that promises behavior it cannot deliver
  (no version input, no bump). The simpler design is "one
  trigger, one shape, document the human steps". If a future
  release needs an emergency-only escape hatch, it can be added
  then.

- **F. Keep the publish-crates gate as opt-in but require a
  separate "yes, really publish" PR-template-style confirmation.**
  More ceremony for the same goal as the default-flip. Rejected:
  ceremony does not solve "forgot to tick the box"; it just adds
  steps that can also be forgotten. The token `[skip publish]`
  in a commit message is reviewed in the bump PR itself, where
  the author is already paying attention to the version number.

## Consequences

- **Positive (primary).** Partial-release recovery cannot
  stage through the tag step, because the tag is what
  triggered the workflow. If the build matrix or publish job
  fails, the recovery is the next patch release, never a re-tag.
  This is what pathlint had to discover the hard way in
  0.0.34 → 0.0.35; codifying it in the workflow shape prevents
  future contributors from rediscovering it.

- **Positive.** crates.io is published by default. The 0.0.34
  "shipped GitHub Release but forgot crates.io" mode is now
  impossible without an explicit `[skip publish]` token in the
  bump commit.

- **Positive.** Zero CI-authored commits on `main`. Every
  commit on `main` is now the squash of a reviewed PR. Branch
  protection no longer needs to allow `github-actions[bot]` to
  push, and the `chore: release X.Y.Z` lines disappear from
  `git log` (the version bump is folded into whatever PR also
  ships the release).

- **Positive.** ADR-0010's complexity ("tolerate an
  already-bumped `Cargo.toml`") becomes inapplicable: CI never
  runs `cargo set-version`, so the no-op-then-no-commit branch
  has nothing to handle. ADR-0010 is marked Superseded by this
  ADR.

- **Negative.** A typo of the `[skip publish]` token
  (`[skip-publish]`, `[skip_publish]`, etc.) silently publishes
  to crates.io. runex accepts the same trade-off. Mitigation is
  documentation only: `docs/RELEASE.md` carries the exact token
  with a copy-pasteable example, and the bump PR's commit
  message is reviewed before merge.

- **Negative.** A human tagging the wrong commit (a commit
  whose `Cargo.toml` does not match the tag) is now possible
  because CI no longer rewrites the version. The version
  mismatch guard step in the workflow catches this at workflow
  start. The remaining residual risk — a commit whose
  `Cargo.toml` is correct but is *not* on `main` — is not
  guarded by the workflow at 0.0.36; it is a doc-only convention.

- **Negative.** The "fix forward and re-tag" recovery option
  documented under ADR-0010 (and explicit in pre-0.0.36
  `docs/RELEASE.md` §"When something goes wrong") goes away.
  Recovery becomes "cut the next patch release" exclusively. In
  the 0.0.x line patch numbers are cheap, but a future release
  in a more restricted scheme (1.x with semver) may want to
  revisit this.

- **Follow-up.** If mis-tagging happens (tag on non-`main`,
  or tag on a commit whose `Cargo.toml` is wrong), follow up
  with a workflow-side enforcement step rather than expanding
  the documentation further. Document the actual incident in
  the new ADR's Context so the trigger is on the record.
