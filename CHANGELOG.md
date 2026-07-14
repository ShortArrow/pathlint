# Changelog

All notable changes to **pathlint** are recorded here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The 0.0.x line treats each `0.0.x → 0.0.(x+1)` bump as
MAJOR-equivalent (Cargo's pre-1.0 convention). Breaking changes are
allowed within 0.0.x and announced under `### Breaking`. The
0.0.x → 0.1.0 graduation gate (when this licence retires) is
defined in [PRD §3.1](docs/PRD.md#31-graduation-to-010); see also
[ADR-0005](docs/decisions/0005-pre-1-0-breaking-policy.md).
Design decisions behind BREAKING entries accumulate in
[`docs/decisions/`](docs/decisions/).

## [Unreleased]

## [0.0.42] — 2026-07-14

**Feature release** — `pathlint lint --sarif` emits SARIF 2.1.0
for GitHub Code Scanning and other static-analysis aggregators,
implementing the output mode
[ADR-0031](docs/decisions/0031-ecosystem-integration-via-sarif-and-schemastore.md)
committed to in 0.0.38. Additive: no CLI, schema, or library
surface changes beyond the new flag and one new lib formatter; no
`### Breaking`; zero new dependencies. The schemastore.org
registration promised by the same ADR remains open.

### Added

- **`pathlint lint --sarif`.** Emits the (already-filtered)
  diagnostics as a SARIF 2.1.0 log. Rule ids are the snake_case
  kind names the `--json` output uses — now a doubly published
  contract (JSON `kind` + SARIF `ruleId`); renaming one is a
  Breaking change. Severity maps to level (`error` / `warning` /
  `note`); each result satisfies GitHub Code Scanning's ingestion
  minimum (physical location anchored at the discovered
  `pathlint.toml`, `startLine` 1) and carries the PATH entry in
  its message and `logicalLocations`. Mutually exclusive with
  `--json`; exit codes unchanged. Design and rejected
  alternatives (serde-sarif 0.8, zizmor-sarif, an external
  converter binary, a sixth published schema, `check --sarif`) in
  [ADR-0034](docs/decisions/0034-sarif-output-hand-rolled-emit.md).
- `format::doctor_sarif` joins the lib's formatter surface
  (additive). The SARIF message wording reuses the same per-kind
  detail sentences as the human renderer, extracted into a shared
  helper so the two outputs never drift apart.
- The container e2e smoke gains step 9: `lint --sarif` exits 0/1
  and carries the SARIF envelope.

### Documented

- README (EN + JP) replace the "SARIF is planned" paragraph with
  a "Uploading findings to GitHub Code Scanning" section carrying
  a minimal `upload-sarif` workflow recipe.
- `docs/README.jp.md` also gains the "streaming findings to a log
  shipper" section that had been EN-only since 0.0.38 — a parity
  gap the SARIF work surfaced.
- PRD §7.5 (EN + JP) document the SARIF mode's contract: rule-id
  stability, level mapping, and the config-anchored location
  strategy.

## [0.0.41] — 2026-07-12

**Feature release** — config discovery becomes monorepo-aware and
gains an explicit layer selector. Both are additive: the default
`--scope=auto` reproduces the 0.0.40 precedence exactly, and the
parent-directory walk only fires where discovery previously found
nothing. No `### Breaking`; zero library-surface change (the walk,
the flag, and the init wiring all live in the binary crate). The
release also ships the two documentation batches that accumulated
under `[Unreleased]` since 0.0.40 (see `### Documented` below).

### Added

- **Monorepo config discovery.** When the cwd has no
  `pathlint.toml`, parent directories are searched up to and
  including the directory that contains `.git` (a directory in a
  normal checkout, a marker file in a linked worktree). Without a
  `.git` boundary anywhere above the cwd no parent is searched at
  all, so a stray config in e.g. the home directory can never win
  by accident. Design and rejected alternatives in
  [ADR-0033](docs/decisions/0033-config-discovery-walk-and-scope.md).
- **`--scope <auto|local|global>` global option.** `auto`
  (default) searches cwd → walk → user-global XDG, matching the
  pre-0.0.41 behaviour; `local` stops after the repo-local layers
  (running with the empty config when they yield nothing);
  `global` reads only `$XDG_CONFIG_HOME/pathlint/pathlint.toml`.
  An explicit `--config <path>` always wins over the flag.
  `--scope=system` is deliberately reserved — pathlint has no
  system-wide config location today, and inventing one is a new
  trust boundary that waits for field demand (ADR-0033
  §Alternatives).
- **`pathlint --scope=global init`** writes the starter file into
  the user-global location, creating `$XDG_CONFIG_HOME/pathlint/`
  if needed, instead of the cwd.
- New e2e suite `tests/config_discovery.rs` pins the walk
  semantics (boundary stop, worktree marker files, no-`.git`
  no-walk), the three scope values, `--config`-beats-`--scope`,
  and the init target switch.

### Documented

- Swept the last three leftovers of the 0.0.34 `doctor` / `lint`
  split (ADR-0028) out of the docs, found by a full-project docs
  audit:
  - PRD §11 CLI surface table (EN + JP) now lists `lint` and
    describes `doctor` as the selfcheck; previously the table
    omitted `lint` entirely and still described `doctor` with its
    pre-0.0.34 PATH-hygiene wording.
  - `docs/README.jp.md` gains the `lint` bullet (mirroring the EN
    README) and its subcommand count goes 6 → 7; previously the JP
    README never mentioned `lint` and described `doctor` with the
    old behaviour.
  - PRD.jp.md §3's R3 bullet now describes the two sibling
    commands (`lint` + `doctor` selfcheck) matching the EN PRD;
    it was the last JP paragraph still describing the merged
    pre-0.0.34 `doctor`.
- **Documentation dependency-direction sweep.** Living documents
  (README EN/JP, PRD EN/JP, PRINCIPLES EN/JP, RELEASE EN/JP,
  SECURITY, ARCHITECTURE, the e2e README, `release.yml`'s header
  comment, source docstrings, and test comments) no longer cite
  decision records by number. Each former "see ADR-NNNN" either
  became redundant (the surrounding text already carried the
  fact) or was replaced by the rationale stated inline. The
  decision journal (`docs/decisions/`, this CHANGELOG) remains
  the only layer that cites ADRs — references flow journal →
  living document, never the reverse. ~70 citations removed
  across 20 files; `schemas/doctor.schema.json` regenerated
  because one `Kind` variant docstring feeds its `description`.
- `docs/ARCHITECTURE.md`'s release-pipeline section described the
  pre-0.0.36 `workflow_dispatch` flow (CI-side version bump, the
  `prepare` job); rewritten to the current tag-push shape
  (guard → build matrix → publish-github → publish-gate/
  publish-crates, opt-out via a standalone `[skip publish]`
  line).
- Both READMEs' operational-details paragraphs claimed the
  `where` / `--rules` aliases were still shipped as visible
  aliases; they were removed in 0.0.22. The same stale claim in
  PRD §16's Resolved list is fixed too.
- README (EN/JP) schema-pin examples updated `v0.0.21` →
  `v0.0.40`; PRD (EN/JP) examples updated `v0.0.13` → `v0.0.40`.
- Three PRD §16 open questions whose text already said
  "(Resolved in 0.0.x)" moved into the `### Resolved` subsection
  (symlinked system dirs, mise plugin attribution, the
  warn-when-both half of mise activate vs shims); the genuinely
  open remainder (mise activate mode auto-detection) stays as an
  open question. EN and JP restructured identically.
- PRD §11 (EN + JP) now states that `sort` requires `--dry-run`
  (running without it exits 2).
- The ADR index's Status column for ADR-0001 / ADR-0004 now
  records their partial supersession by ADR-0008, matching what
  the ADR files themselves already said.

## [0.0.40] — 2026-06-27

**Documentation release** — the seven cross-cutting product
principles previously embedded in PRD §3 are extracted to a
standalone document so they can be cited independently of PRD
section numbering. The text is duplicated, not moved: PRD §3
still carries the same content and now links to the standalone
file. No CLI, schema, or library surface change vs 0.0.39.

### Documented

- New `docs/PRINCIPLES.md` (EN) and `docs/PRINCIPLES.jp.md` (JP)
  reproduce the seven principles — Declarative; Source labels,
  not paths; Built-in catalog with override; One file, all OSes;
  Substring + case-insensitive match; Honest exit codes;
  Read-only — verbatim from PRD §3, with a short header
  explaining the cross-cutting role and a "How the principles
  cross-cut the rest of the docs" section linking back to PRD §3,
  PRD §4 (Non-goals), ADR-0009 (read-only), ADR-0014 / 0015 /
  0023 / 0031 (catalog as extension surface), and ADR-0032 (scope
  anchor).
- PRD.md §3 / PRD.jp.md §3 each gain one leading paragraph
  pointing to the standalone file. PRD content is otherwise
  unchanged; the principles remain canonically stated in PRD §3
  with the standalone file as a duplicate for citation
  convenience.

### Next

- 0.0.41 candidate: extend `locate_rules()` to walk cwd → enclosing
  `.git` boundary for monorepo config discovery, and add a
  `--scope=auto|local|global|system` global option (default `auto`,
  preserving today's precedence — additive, no BREAKING). Design
  and trade-offs will land in a separate ADR (tentative ADR-0033).
  Originally scoped for 0.0.40 in 0.0.39's release notes; 0.0.40
  carved out the principles extraction as a docs-only intermediate
  release so the implementation lands in 0.0.41 with a focused
  PR.

## [0.0.39] — 2026-06-23

**Documentation release** — single new ADR anchors pathlint's scope
policy (OS knowledge first-class, tool meta declarative-only, no
modelling of tool runtime behavior). No CLI, schema, or library
surface change vs 0.0.38. The policy itself has been in force since
0.0.3 and is reflected in every catalog addition since; 0.0.39 records
the *why* in one citable place so future "should pathlint know mise /
asdf / volta state?" requests have a canonical answer.

### Documented

- [ADR-0032](docs/decisions/0032-scope-os-knowledge-tool-meta-declaration.md)
  consolidates the scope boundary that has been implicit across
  ADR-0009 (read-only), ADR-0014 (`os_baseline_*` naming split),
  ADR-0015 (wrapper-installer generalisation), ADR-0022
  (descriptive-only relations), ADR-0023 (catalog identity), and
  ADR-0031 (SARIF as the integration layer). Rejects five
  alternatives: absorbing tool behavior into pathlint, dynamic
  plugin loaders, sidecar binary protocols, Cargo feature flag
  plugins, and "PRD prose only without an ADR". Defines "plugin"
  as "a catalog `[source.<name>]` entry" — the same mechanism the
  built-in catalog uses.
- PRD §4 (EN) and §4 (JP) each gain one trailing paragraph linking
  to ADR-0032 as the canonical rejection target for future
  tool-state-query requests. No other PRD content changes.

### Next

- 0.0.40 candidate: extend `locate_rules()` to walk from cwd up to
  the enclosing `.git` boundary for monorepo config discovery, and
  add a `--scope=auto|local|global|system` global option (default
  `auto`, preserving today's precedence — additive, no BREAKING).
  Design and trade-offs will land in a separate ADR (tentative
  ADR-0033) at the moment the implementation does.

## [0.0.38] — 2026-06-20

**Documentation release** — two new ADRs codify
release-engineering and ecosystem-integration policy. No CLI,
schema, or library surface change vs 0.0.37. The release exists
to ship the ADRs as part of the release notes and to exercise the
(new in 0.0.36) tag-push workflow a third time under normal
conditions (the first two runs shipped a release-engineering
BREAKING change and a follow-up fix, respectively; 0.0.38 is the
first ordinary additive release under the new flow).

### Documented

- [ADR-0030](docs/decisions/0030-container-e2e-for-linux-portability.md)
  records the existing `scripts/e2e/` harness as the Linux
  portability gate: container smoke against Ubuntu / Arch /
  Fedora, run locally before any release that touches the
  `doctor` selfcheck, the `lint` detector set, the built-in
  catalog, `/etc/os-release`, or `expand_env`. Explicitly rejects
  four adjacent options (Vagrant multi-VM, macOS / Windows
  post-publish smoke, CI integration, fork-repo release
  rehearsal) with one paragraph each. The ADR is retroactive —
  the harness has been in place since 0.0.14 / 0.0.21; ADR-0029
  prompted the sweep that surfaced this gap.
- [ADR-0031](docs/decisions/0031-ecosystem-integration-via-sarif-and-schemastore.md)
  commits pathlint to SARIF 2.1.0 output and schemastore.org
  registration as its ecosystem integration points. Implementation
  deferred to 0.0.40 or later; the ADR records the policy plus
  six rejected alternatives (LSP server, bespoke RPC, ESLint
  plugin, OpenTelemetry / OTLP exporter, "wait for someone else",
  "JSON only"). The OTLP rejection is the load-bearing one — it
  explains why Cloudflare Workers Logs / Datadog interop goes
  through SARIF or JSONL streaming, not OTLP.

### Changed

- `scripts/e2e/smoke.sh` and `scripts/e2e/README.md` synced with
  the 0.0.34 `doctor`-vs-`lint` split (ADR-0028). The smoke
  script now exercises both `doctor --json` (selfcheck — typically
  `[]`) and `lint --json` (PATH detectors). The README's "What is
  checked" list grew from 7 entries to 8.
- `docs/RELEASE.md` and `docs/RELEASE.jp.md` pre-release checklist
  gains one line pointing the releaser at `scripts/e2e/run.sh`
  when the release touches doctor / lint / catalog /
  `/etc/os-release` / `expand_env`. Honour-based, not enforced,
  per ADR-0030.
- `README.md` gains a "Streaming findings to a log shipper"
  subsection under Operational details with the
  `pathlint lint --json | jq -c '.[]'` recipe for Cloudflare
  Logpush / Loki / Datadog / Splunk / ELK ingestion. Links
  forward to ADR-0031 so the SARIF-coming-later context is one
  click away.

## [0.0.37] — 2026-06-13

**Additive follow-up to 0.0.36** — robustness fix for the
`[skip publish]` opt-out gate, and the smoke release that
exercises crates.io publishing through the new tag-push
workflow for the first time. No CLI or lib surface change.

### Note on 0.0.36 crates.io publishing

0.0.36 shipped as a GitHub Release on 2026-06-13 but **was not
published to crates.io**. The `publish-crates` job's `if:
!contains(github.event.head_commit.message, '[skip publish]')`
check fired because the release commit's body *discussed* the
`[skip publish]` token in prose (the CHANGELOG entry and the PR
description both quoted the literal string). The `contains()`
expression is substring match and does not distinguish a token
mention from a token directive.

This was acknowledged as a residual risk in ADR-0029
(Negative: a typo of `[skip publish]` silently publishes; the
dual risk is an in-prose mention silently skipping), but the
fix was deferred until the mode actually fired. It fired on the
very first release under the new flow. 0.0.37 ships the fix,
and the 0.0.36 functional change (workflow rewrite) is available
via `cargo install --git
https://github.com/ShortArrow/pathlint --tag v0.0.36` or by
downloading the GitHub Release binary directly.

### Changed

- `.github/workflows/release.yml` adds a `publish-gate` job
  between `guard` and `publish-crates`. The new job parses the
  head commit's message line by line and outputs
  `publish=true|false`. Only a **standalone** `[skip publish]`
  line (whitespace-trimmed) flags a skip; in-prose mentions of
  the token do not. `publish-crates`' `if` condition becomes
  `needs.publish-gate.outputs.publish == 'true'`. The
  publish-gate job runs in parallel with `build`, so it does
  not lengthen the critical path.
- `docs/RELEASE.md` and `docs/RELEASE.jp.md` document the
  standalone-line semantics with copy-pasteable examples for
  both "skip" and "publish-but-mention-the-token" commit
  bodies.
- ADR-0029 Consequences gets an Update (0.0.37) note under the
  existing "Negative: typo of `[skip publish]`" entry,
  recording that the dual risk fired in 0.0.36 and was closed
  by the standalone-line gate. Body of ADR-0029 unchanged
  otherwise.

## [0.0.36] — 2026-06-13

**BREAKING release** — release workflow migrates from
`workflow_dispatch` (CI-managed tag) to `on: push: tags`
(human-managed tag). Default crates.io publishing flips from
opt-in to opt-out. See
[ADR-0029](docs/decisions/0029-release-trigger-tag-push.md)
(supersedes
[ADR-0010](docs/decisions/0010-release-workflow-bump-skip.md)) for
the decision record, and
[docs/RELEASE.md](docs/RELEASE.md) for the human-facing procedure.

### Breaking

- `.github/workflows/release.yml` trigger changes from
  `workflow_dispatch` (with `version` / `publish_crates` inputs)
  to `on: push: tags: ["v*"]`. The `prepare` job (which ran
  `cargo set-version`, then committed and tagged from CI) is
  removed entirely. **Version bump and tag creation are now
  human responsibilities**: open a PR that bumps `Cargo.toml`
  and `CHANGELOG.md`, squash-merge, then on `main` run
  `git tag -a vX.Y.Z -m "..." && git push origin vX.Y.Z`. The
  tag push triggers the workflow. The 0.0.34 partial-release
  failure mode (tagged-but-not-published) becomes structurally
  impossible because the tag *is* the trigger.
- The `publish-crates` job's default flips from opt-in (`if:
  inputs.publish_crates`) to **opt-out** (`if:
  !contains(github.event.head_commit.message, '[skip publish]')`).
  Tagged releases publish to crates.io by default. To skip
  publishing for a specific release, include the literal token
  `[skip publish]` (exact spelling, one space, square brackets)
  in the bump commit's message. Variants like `[skip-publish]`
  do not match the `contains()` check and will publish anyway —
  review the squash commit message carefully before merge.
- The PR #34 "tolerate an existing tag at HEAD" recovery branch
  introduced in 0.0.35 is removed. Re-tagging the same version
  is no longer supported (and `git push` would reject it as
  non-fast-forward anyway). Recovery from a partial release is
  exclusively "cut the next patch release" — the same
  discipline pathlint had to discover the hard way during the
  0.0.34 → 0.0.35 incident.
- [ADR-0010](docs/decisions/0010-release-workflow-bump-skip.md)
  is marked **Superseded by ADR-0029**. Its "tolerate an
  already-bumped Cargo.toml" branch becomes inapplicable because
  CI no longer runs `cargo set-version` — `Cargo.toml` is
  authoritative at the tagged commit, and a `version` /
  `${GITHUB_REF_NAME#v}` mismatch fails the new `guard` job
  before any expensive build runs.

### Added

- `.github/workflows/release.yml` gains a `guard` job that
  reads `Cargo.toml` at the tagged commit and fails the build
  if its `version =` line does not equal `${GITHUB_REF_NAME#v}`.
  Catches the most likely new footgun under the human-tag flow
  ("tagged the wrong commit") at workflow start instead of after
  ~10 minutes of build matrix.
- The `publish-crates` job gains an idempotency guard that
  queries `https://crates.io/api/v1/crates/pathlint/$version`
  before publishing. If the version is already on crates.io the
  step skips with a message, instead of failing with "crate
  already exists on crates.io index". Lets the `publish-crates`
  job be re-run safely after a transient failure of a later
  step.

### Changed

- `docs/RELEASE.md` and `docs/RELEASE.jp.md` rewritten end-to-end
  for the human-tag flow. New section ordering: pre-release
  checklist → bump PR → tag-and-push. The §"Manual fallback"
  section is removed (the new normal flow IS what the manual
  fallback used to be).
- `docs/decisions/README.md` index reflects ADR-0029 (Proposed
  → Accepted in this release), ADR-0010 supersession, and a
  status fix for ADR-0028 (Proposed → Accepted; the file itself
  had been Accepted since 0.0.34, only the index was stale).

## [0.0.35] — 2026-06-09

**Additive release** — workflow recovery + 0.0.34 crates.io
publishing follow-up. No CLI or lib surface change vs 0.0.34;
the `lint`/`doctor` split shipped in 0.0.34 stands. First
release of the new BREAKING streak after the 0.0.18→0.0.33
additive streak ended at 0.0.34.

### Added

- `.github/workflows/release.yml` tolerates an existing tag at
  HEAD across re-runs (PR #34). The prepare job now detects an
  already-pushed `v<version>` tag, verifies it points at the
  current HEAD (hard-error otherwise), and skips the `git tag`
  step in that case. Lets the workflow be re-run with a different
  `publish_crates` value after a partial first run.

### Note on 0.0.34 crates.io publishing

0.0.34 shipped as a GitHub Release with binary archives and
schema assets on 2026-06-07, but **was not published to
crates.io**. The first release-workflow run had
`publish_crates=false` (default) so the crates.io publish step
was skipped; the re-run with `publish_crates=true` failed at the
`git tag` step because `v0.0.34` already existed from the first
run. PR #34 patched the workflow to tolerate an existing tag at
HEAD, but by the time the patch merged, `main` had moved past
`f9845b1` (the 0.0.34 release commit) — the patched workflow's
safety check refused to publish a tree that no longer matched the
existing tag.

The recovery path is to fold the crates.io publish into the next
release (0.0.35). The 0.0.34 functional surface (lint / doctor
split) is available via `cargo install --git
https://github.com/ShortArrow/pathlint --tag v0.0.34` or by
downloading the GitHub Release binary directly. This mirrors the
0.0.29–0.0.32 publishing-gap pattern documented above; the same
mechanical mismatch caused both, and graduation criterion 1 (count
of consecutive additive CHANGELOG entries) is unaffected since it
counts CHANGELOG entries, not crates.io publishes.

## [0.0.34] — 2026-06-07

**BREAKING release — `pathlint doctor` responsibility split.**
First BREAKING since 0.0.18 (16-release additive streak resets;
graduation criterion 1's counter restarts at 0.0.35+). Drives the
split from Round 1 dotfiles dogfooding (PR ShortArrow/dotfiles #3)
where the 0.0.33 `doctor --json` surfaced 202 diagnostics on a
real Windows host and revealed that doctor was doing two
unrelated jobs (PATH hygiene + pathlint selfcheck) under one
name.

### Breaking

- **`pathlint doctor` is now selfcheck only.** Three checks
  (binary self-locate, `pathlint.toml` discovery + parse,
  `env_lookup` operational); does **not** inspect PATH for
  duplicates / shortenable / shadowed / etc. The 0.0.33 doctor
  output is gone — that JSON wire shape no longer comes out of
  `pathlint doctor`. Migration: scripts calling
  `pathlint doctor --json` for PATH anomalies should switch to
  `pathlint lint --json` (same `Diagnostic` JSON shape, same
  kind names, same `--include` / `--exclude` filter UX). No
  `--legacy` flag, no alias runway — see
  [ADR-0028](docs/decisions/0028-doctor-lint-responsibility-split.md)
  for why a runway was not adopted (no observed JSON consumers,
  0.0.x BREAKING licence, code simplicity).
- **`Severity` enum gains `info`** as a discriminant alongside
  `warn` / `error`. Selfcheck's `config_not_found` emits at info
  severity (running pathlint without a config is legitimate).
  JSON consumers parsing the `severity` enum strictly must
  accept `info` or filter it out client-side.

### Added

- **`pathlint lint` subcommand** — inherits the 12 detector
  kinds previously emitted by `pathlint doctor` (0.0.13–0.0.33):
  `duplicate`, `missing`, `shortenable`, `trailing_slash`,
  `case_variant`, `short_name`, `malformed`, `conflict`
  (incl. user-declared `[[relation]]` diagnostics),
  `per_source_missing_required`, `duplicate_but_shadowed`,
  `relative_path_entry`, `writeable_path_dir`. The
  `--include` / `--exclude` filter UX, `--json` output array,
  and `Diagnostic` schema are preserved verbatim.
- **Selfcheck `Kind` variants** in
  `schemas/doctor.schema.json` (shared by both subcommands):
  `binary_not_in_path`, `config_parse_error`,
  `config_not_found`, `env_lookup_failed`.
- [ADR-0028](docs/decisions/0028-doctor-lint-responsibility-split.md)
  (Cat 1 + 8): doctor is pathlint's selfcheck; PATH analysis
  moves to the new `lint` subcommand. Records 6 rejected
  alternatives (keep doctor as-is + add lint; rename doctor to
  lint; alias runway; fold into check; text-only output;
  detector kinds in check).

### Changed

- **`tests/doctor.rs` → `tests/lint.rs`** (17 PATH-hygiene
  integration tests moved verbatim; the only edits are
  subcommand-name substitutions and a header comment).
- **PRD §3 R3 description** updated to reflect the
  doctor/lint split (graduation criterion "PRD matches
  implementation" maintained).
- **PRD §7.5** rewritten to document `pathlint lint` and
  `pathlint doctor` as sibling commands; the 0.0.33 doctor
  description survives as historical context inside the new
  `lint` section.
- **Source-validation gate** (catalog-needle-too-short / root
  path rejection) and **relation-acyclicity gate** moved from
  `doctor` to `lint`. `pathlint check`, `pathlint trace`,
  `pathlint sort` continue to enforce both gates as before.
- **Test renames in `tests/security.rs`:** two doctor-flavoured
  scenarios become lint-flavoured (`lint_rejects_user_source_pointing_at_root`,
  `lint_rejects_user_relation_cycle`).

### Note on intermediate release publishing (0.0.29–0.0.32)

CHANGELOG entries for 0.0.29 / 0.0.30 / 0.0.31 / 0.0.32 are
present above, but **no `v0.0.29` / `v0.0.30` / `v0.0.31` /
`v0.0.32` tag exists on GitHub and no corresponding version was
published to crates.io**. The 0.0.33 release on 2026-06-01
bundles all five steps' commits (Step 5a through Step 7) into
one rolling release.

Why: the `release.yml` workflow's `prepare` job runs
`cargo set-version`, which enforces monotonic version bumps —
once main had been bumped to 0.0.33 (the last commit in the
sequence), attempting to trigger a release for 0.0.29 / 0.0.30 /
0.0.31 / 0.0.32 failed with
`Cannot downgrade from 0.0.33 to <older>`. Re-publishing the
intermediate versions would require either modifying the
workflow to bypass the safety guard or hand-tagging past
commits and triggering per-tag releases — both have unrelated
risks (workflow safety regression; tag-history rewrites).

Implications:

- **Graduation criteria are unaffected.** PRD §3.1 #1 reads
  "≥ 2 consecutive releases without `### Breaking` in
  `CHANGELOG.md`"; the count is over CHANGELOG entries, not
  over crates.io publishes. The 5-consecutive-additive
  streak (0.0.29 → 0.0.33) remains satisfied.
- **Embedders pinning to intermediate versions cannot fetch
  them from crates.io.** They can pin to 0.0.33 (which
  contains every Step 5a–Step 7 commit) or use a
  `git`-dependency against the corresponding tagged commit
  in this repository — the commits exist on `main` at
  `18422c5` (0.0.29) / `762a220` (0.0.30) / `c014508` (0.0.31)
  / `0561e78` (0.0.32) / `4d7ab5b` (0.0.33).
- **Pre-graduation policy unchanged.** [ADR-0005](docs/decisions/0005-pre-1-0-breaking-policy.md)'s
  pin convention is "pin exact `0.0.x`"; users following that
  convention pin to 0.0.33 directly.

Future releases should be triggered at the time `Cargo.toml` is
bumped (one bump per release), not in batch after multiple
bumps land on `main`. If a batch situation recurs, the recovery
is the same as here: ship the latest version, document the
gap.

## [0.0.33] — 2026-06-01

**Additive-only docs + test-infra release** (no `### Breaking`
section). Fifth consecutive additive release after 0.0.29 /
0.0.30 / 0.0.31 / 0.0.32; graduation criterion 1's counter now
reads 5.

Closes the **3 M findings carried forward** from the 2026-05-31
codex 6-axis audit (see CHANGELOG 0.0.30 Notes). All 3 close
additively; no source code changes. The carry-forward list is
now empty; the next audit cycle starts from a clean baseline.

### Added

- **trybuild dev-dependency + `tests/ui/` harness for
  negative-invariant tests.** First snippet is
  `tests/ui/path_entry_has_no_provenance_raw.rs`, pinning
  ADR-0008's invariant (PathEntry has no `provenance_raw`
  field, no `with_provenance` / `effective_raw_for_user_intent`
  methods — those moved to `Attribution` in the 0.0.28 split).
  If a future refactor re-introduces any of these on
  `PathEntry`, `cargo test --test ui_compile_fail` fails.
  Regenerate `.stderr` snapshots with `TRYBUILD=overwrite cargo
  test --test ui_compile_fail` after a rustc upgrade. See
  [ADR-0026](docs/decisions/0026-trybuild-for-negative-invariants.md).
- [ADR-0026](docs/decisions/0026-trybuild-for-negative-invariants.md)
  (Cat 6 +8): adopt `trybuild` as the dev-dependency for
  compile-fail negative tests. Records the 5 rejected
  alternatives (comment-only pin / hand-rolled macro trick /
  `compiletest_rs` / runtime panic check / defer adoption).
- [ADR-0027](docs/decisions/0027-lib-env-read-boundaries.md)
  (Cat 3 +4): lib has two intentional env-read boundaries
  (source catalog resolution + PATH entry construction); the
  `_with` family is the injection seam, the wrapper family is
  the CLI-convenience surface. Records why the residual
  `std::env::var` calls flagged by the codex audit are
  intentional architecture, with 5 rejected alternatives
  (delete wrappers / unified `Deps` carrier / move env reads
  to caller / accept finding without ADR / automated
  enforcement test).

### Changed

- `docs/SECURITY.md` trust-boundary table gains a row for
  environment-variable values returned by
  `CommonDeps::env_lookup` (`PATHEXT`, `HOME`, `USERPROFILE`,
  source-path expansion targets). Sanitisation pointers
  section's `*Deps` bullet cross-references ADR-0027 for the
  two-boundary architecture.
- `docs/decisions/README.md` index gains ADR-0026 / 0027 rows
  in the timeline view and category-view pointers (Cat 3 / 4 /
  6 / 8).

### Notes

- **M findings closed (codex 2026-05-31 carry-forward)**:
  - **TDD**: ADR-0026 + `tests/ui/path_entry_has_no_provenance_raw.rs`
    pin closes M. Future negative invariants follow the same
    `tests/ui/` template.
  - **FP**: ADR-0027 formalises "intentional boundary, `_with`
    variant already shipped" as the close. Internal callers
    use `_with` exclusively (verified callgraph); wrappers
    stay as the CLI-convenience surface.
  - **Security**: SECURITY.md row addition + sanitisation
    pointer cross-reference close M.
- **Carry-forward list now empty.** The next codex audit
  cycle (whenever it happens) starts from a clean baseline.
- **Graduation criteria status unchanged**: all 7 ✅ as of
  0.0.32 / ADR-0025. The M caveats that ADR-0013 noted on
  criteria 4 and 7 are now closed by this release; the
  criteria themselves were already satisfied.

## [0.0.32] — 2026-05-31

**Additive-only docs release** (no `### Breaking` section).
Fourth consecutive additive release after 0.0.29 / 0.0.30 /
0.0.31; graduation criterion 1's counter now reads 4.

This release closes **graduation criterion 5** (ADR completeness)
by backfilling 11 new ADRs for the 7 pre-ADR-system releases
(0.0.14 / 0.0.15 / 0.0.16 / 0.0.17 / 0.0.19 / 0.0.21 / 0.0.22)
whose CHANGELOG `### Breaking` sections previously lacked ADR
links. ADR-0013's Criterion 5 section (which recorded
"Partially satisfied" at the 0.0.31 snapshot) is now
superseded by ADR-0025; ADR-0013's frontmatter gains a
one-line additive Status pointer per README §Supersession.

**Release cut beyond 0.0.32 (number, timing, whether to cut
0.1.0) remains user judgement** — this release continues
ADR-0013's separation of "criteria recording" from
"graduation cut".

### Added

- 11 new ADRs covering pre-ADR-system Breaking releases:
  - [ADR-0014](docs/decisions/0014-source-naming-convention.md) (Cat 7): Source naming convention — `<provenance>_<scope>` snake_case + `os_baseline_*` family split (0.0.14)
  - [ADR-0015](docs/decisions/0015-provenance-wrapper-installer-rename.md) (Cat 1): `Provenance::WrapperInstaller` generalises from mise-only naming (0.0.14)
  - [ADR-0016](docs/decisions/0016-json-wire-shape-kind-discriminator.md) (Cat 7): JSON wire shape — every union uses top-level `kind` discriminator + `*.schema.json` `required` honesty (0.0.14 / 0.0.15 / 0.0.17 bundled)
  - [ADR-0017](docs/decisions/0017-lib-surface-nine-modules.md) (Cat 1, +2/+8): Lib surface narrowed to 9 supported `pub mod` + `#[doc(hidden)] pub` middle tier (0.0.15 / 0.0.17 bundled)
  - [ADR-0018](docs/decisions/0018-resolver-outcome-type-simplification.md) (Cat 1): Resolver `Option<PathBuf>` + unit-variant `Status` with `Outcome::reason` (0.0.16 / 0.0.17 bundled)
  - [ADR-0019](docs/decisions/0019-cli-alias-deprecation-runway.md) (Cat 5, +8): 6-release deprecation runway for CLI renames (`where`/`--rules`, 0.0.14 → 0.0.20 warning → 0.0.22 removal)
  - [ADR-0020](docs/decisions/0020-doctor-analyze-closure-tuple.md) (Cat 1, +3): `doctor::analyze` open-ended closure tuple as new detectors land (0.0.19 / 0.0.21; superseded by ADR-0007 as of 0.0.27)
  - [ADR-0021](docs/decisions/0021-build-rs-aggregate-violations.md) (Cat 8): `build.rs` aggregates plugin referential-integrity violations into one failure (0.0.14)
  - [ADR-0022](docs/decisions/0022-depends-on-descriptive-only.md) (Cat 5): `depends_on` relation is descriptive-only, no runtime effect on detectors (0.0.14)
  - [ADR-0023](docs/decisions/0023-catalog-version-reserved-for-embedded.md) (Cat 7): `catalog_version` is reserved for the embedded catalog; user TOML rejection (0.0.14 post-parse → 0.0.15 structural)
  - [ADR-0024](docs/decisions/0024-color-flag-activation.md) (Cat 8): `--color` flag activation — parsed-but-ignored → effective (0.0.17)
- [ADR-0025](docs/decisions/0025-criterion-5-closure.md) (Cat 8): graduation criterion 5 fully satisfied; supersedes ADR-0013 §Criterion 5. Records the 11-of-11 (release × ≥1 ADR) audit matrix.

### Changed

- [ADR-0013](docs/decisions/0013-graduation-criteria-record.md) frontmatter gains additive Status pointer: "Criterion 5 section superseded by ADR-0025 as of 0.0.32". Body unchanged per README §Supersession.
- `docs/decisions/README.md` index gains 12 new ADR rows (timeline view) plus category-view pointers for Cat 1 (4 new), Cat 5 (2 new), Cat 7 (3 new), Cat 8 (3 new).
- CHANGELOG entries for 0.0.14 / 0.0.15 / 0.0.16 / 0.0.17 / 0.0.19 / 0.0.21 / 0.0.22 each gain `*(See ADR-NNNN.)*` parentheticals on their Breaking bullets, mirroring the existing `*(Alias removed in 0.0.22.)*` style.

### Notes

- **Graduation criterion status as of 0.0.32**: criterion 5 transitions from ⚠️ Partially satisfied (ADR-0013) to ✅ Fully satisfied (ADR-0025). Criteria 1 / 2 / 3 / 4 / 6 / 7 remain as recorded in ADR-0013 (criteria 4 and 7 carry M caveats from the 2026-05-31 codex audit, non-blocking).
- ADR-0000's Known ADR backlog table is **unchanged** — that table records the backlog state at the time of the ADR system's introduction (0.0.25). New ADRs in this release are recorded retroactively in their per-ADR `Release:` metadata.
- No source code change (`src/` untouched). All work is in `docs/`, `CHANGELOG.md`, and `Cargo.toml`.

## [0.0.31] — 2026-05-31

Step 5c of the 0.0.25-0.1.0 roadmap. **Additive-only docs release**
(no `### Breaking` section). Third consecutive additive release
after 0.0.29 / 0.0.30; graduation criterion 1's counter now
reads 3 (the criterion requires ≥ 2).

This release closes the Step 5 plan: graduation criteria are now
**recorded** in [ADR-0013](docs/decisions/0013-graduation-criteria-record.md).
Whether and when to cut graduation (next release as 0.0.32 vs
0.1.0, what number, what cadence) is **user judgement** —
ADR-0013 records the audit, it does not schedule the cut.

### Added

- [ADR-0012](docs/decisions/0012-schemars-1-0-deferred.md) —
  defer schemars 1.0 migration past 0.0.x graduation. Category 6
  (external dependency); records the 0.8 → 1.0 cost (5 binaries +
  5 derive sites + 5 schema files + 5 drift-gate tests + downstream
  re-pin), the four rejected alternatives (migrate in this release;
  migrate in 0.0.32; migrate as part of 0.1.0 itself; never migrate;
  switch to a different crate), and the trigger conditions that
  would re-open the decision (security advisory, draft-2020-12-only
  consumer ask, pathlint feature needing 1.x machinery, 0.2.x
  dependency-refresh window). Graduation criterion 3 ("Schemars 1.0
  evaluated") satisfied by this ADR's existence per PRD §3.1.
- [ADR-0013](docs/decisions/0013-graduation-criteria-record.md) —
  graduation criteria satisfaction record at the 0.0.31 cut.
  Category 8 (process / governance); records the 7 criteria with
  pointers to where each is satisfied (CHANGELOG entries, prior
  ADRs, codex audit notes). Criterion 5 is recorded as **partially
  satisfied** (7 pre-ADR-system releases lack ADR links); two
  options for the user — accept partial state, or backfill 5–7
  short ADRs for 0.0.14–22 — are spelled out in the ADR. The four
  rejected alternatives (cut 0.1.0 in this ADR; reinterpret
  criterion 5; backfill within 0.0.31; defer the record) are
  pinned so the choice rationale is durable.

### Changed

- `docs/decisions/README.md` index gains ADR-0012 / 0013 rows and
  category-view pointers (Cat 6 first occupant; Cat 8 third
  occupant after ADR-0000 / 0005 / 0010).

## [0.0.30] — 2026-05-31

Step 5b of the 0.0.25-0.1.0 roadmap. **Additive-only docs release**
(no `### Breaking` section). Second consecutive additive release
after 0.0.29 — graduation criterion 1's "≥ 2 consecutive releases
without `### Breaking`" counter now reads 2.

ADR backlog drainage: three of the ten Known ADR backlog rows in
[ADR-0000](docs/decisions/0000-adr-categories.md) now have
dedicated ADRs, dropping the backlog to seven. The drainage
targets historical decisions that already shipped (read-only
stance since 0.0.1, release workflow bump-skip since PR #22 /
0.0.24, `expand::normalize` policy since 0.0.x baseline) — no
runtime behaviour change.

### Added

- [ADR-0009](docs/decisions/0009-read-only-stance.md) — pathlint
  is read-only on `PATH`, registry, and dotfiles. Category 5
  (architectural style); records the stance that has been in
  force since 0.0.1 and the four rejected alternatives (ship
  `sort --apply` from day one; opt-in `--write`; separate
  `pathlint-apply` binary; let `init` overwrite without `--force`).
- [ADR-0010](docs/decisions/0010-release-workflow-bump-skip.md) —
  release workflow tolerates an already-bumped `Cargo.toml`.
  Category 8 (process / governance); records PR #22's 0.0.24
  fix that lets bump-in-feature-PR releases land without a
  separate `chore: release` commit.
- [ADR-0011](docs/decisions/0011-normalize-substring-match-policy.md) —
  `expand::normalize` is case-insensitive + slash-unifying;
  substring match without canonicalisation. Category 3
  (cross-cutting concern); records the policy every path
  comparison goes through and the five rejected alternatives
  (canonicalize both sides; structural component compare;
  locale-aware case folding; slash-only without case folding;
  tokenise + trie).

### Changed

- [ADR-0000](docs/decisions/0000-adr-categories.md) Known ADR
  backlog table reduced from 10 to 7 rows. Drained entries now
  carry pointers to the new ADRs they shipped as.

### Fixed (docs drift)

- `docs/PRD.md` §3.1 / `docs/PRD.jp.md` §3.1 no longer claim
  "ADR-0009 (planned) will be the graduation verification
  record" — ADR-0009 in this release is the Read-only stance,
  so the graduation verification record is left without a
  reserved number and will land at whatever number is next when
  the criteria audit passes.
- `docs/RELEASE.md`, `docs/RELEASE.jp.md`, and
  `docs/ARCHITECTURE.md` release-pipeline descriptions now
  point at ADR-0010 for the idempotent `cargo set-version` /
  conditional `chore: release` commit behaviour (previously
  described as always bumping + always committing).

### Notes (codex 6-axis audit re-run, 2026-05-31)

- **0 H findings** — graduation criterion 7 ("no open H
  severity codex audit findings") remains satisfied after the
  0.0.28 / 0.0.29 / 0.0.30 work.
- **3 M findings** carried forward to be evaluated in the next
  audit cycle:
  - TDD: the "PathEntry has no `provenance_raw` field" invariant
    is asserted in a comment in `tests/public_api.rs` but not
    pinned by a compile-fail test.
  - FP: `std::env::var` is still called directly inside the lib
    from `source_match::find` / `validate_sources` / `names_only`
    wrappers, `expand::expand_and_normalize`, `resolve::split_path`
    / `resolve::pathext_list`, and `path_source::read_path`
    helpers. The main `doctor` / `lint` / `trace` / `sort`
    call graphs flow through `CommonDeps::env_lookup`; only
    the wrappers and infra boundaries still read the live env.
  - Security: `SECURITY.md` describes the `CommonDeps::env_lookup`
    closure as trusted in-process code but does not catalogue
    the bytes that production lookups return (`PATHEXT`, `HOME`,
    `USERPROFILE`, source-path expansion targets) as untrusted
    inputs in their own right.
- **2 L Docs findings** addressed in this release (see
  "Fixed (docs drift)" above).

## [0.0.29] — 2026-05-31

Step 5a of the 0.0.25-0.1.0 roadmap. **Additive-only docs release**
(no `### Breaking` section). First entry in the
two-consecutive-additive-release window that graduation criterion 1
needs after 0.0.27 / 0.0.28 both shipped a BREAKING change.

### Changed

- `docs/SECURITY.md` refreshed for the 0.0.27 `*Deps` env-injection
  surface and the 0.0.28 `Attribution` split. Trust-boundary table
  now lists `Attribution.provenance_raw` overlay (Windows
  `--target process`) and the `CommonDeps::env_lookup` closure
  (caller-supplied at the lib boundary). Sanitisation pointers
  table gains entries for `Attribution::effective_raw_for_user_intent`
  and `CommonDeps::production` so the single-place reuse rule is
  visible for both 0.0.28 additions.

### Fixed (docs drift)

- N/A this release — PRD §11 CLI surface table and
  `docs/ARCHITECTURE.md` Repo map / 10-module table already matched
  the implementation as of 0.0.28; both were re-verified during
  0.0.29 preparation and required no edits.

## [0.0.28] — 2026-05-24

Step 4 of the 0.0.25-0.1.0 roadmap, and the last intentionally
BREAKING release in the line. Closes the ADR-0001 and ADR-0004
Follow-up notes by splitting the 0.0.24 `PathEntry` into a pure
observation type plus a new cross-source carrier. See
[ADR-0008](docs/decisions/0008-attribution-type-split.md) for the
rationale, the four alternatives that were rejected, and the
consequence of resetting the criterion 1 counter one more time
before Step 5's additive-only window.

### Breaking

- **`pathlint::path_entry::PathEntry` is back to `{ raw, expanded }`**.
  `provenance_raw` field removed; `with_provenance` and
  `effective_raw_for_user_intent` methods removed. PathEntry is
  again a pure single-source observation.
- **`pathlint::Attribution` is the new cross-source carrier** at
  the crate root (next to `CommonDeps`). It wraps a `PathEntry`
  plus an `Option<String>` provenance and owns
  `with_provenance` / `effective_raw_for_user_intent`.
- **Entry-list parameters switch from `&[PathEntry]` to `&[Attribution]`**
  across every public entry point: `doctor::analyze` /
  `analyze_real`, `sort::sort_path` / `sort_path_real`,
  `lint::EvaluateDeps::production` / `evaluate_real`,
  `trace::LocateDeps::production` / `locate_real`,
  `resolve::resolve` / `split_path`, `format::doctor_line`. The
  `path_source::PathRead.entries` field and
  `reconcile_process_with_registry` move to `Vec<Attribution>`.

Migration:
```rust
// before
PathEntry { raw, expanded, provenance_raw: None }
pe.effective_raw_for_user_intent()
pe.with_provenance(reg_raw)

// after
Attribution::new(PathEntry::from_raw(raw, env_lookup))
attrib.effective_raw_for_user_intent()
attrib.with_provenance(reg_raw)
```

### Added

- `pathlint::Attribution { observed, provenance_raw }` at the
  crate root.
- `Attribution::new(PathEntry)`, `Attribution::with_provenance(String) -> Self`,
  `Attribution::effective_raw_for_user_intent(&self) -> &str`.

### Fixed

- ADR-0001 Follow-up closed: PathEntry's concept purity restored.
- ADR-0004 Follow-up closed: cross-source overlay moved to its
  own type.

## [0.0.27] — 2026-05-24

Step 3 of the 0.0.25-0.1.0 roadmap. Closes the codex 2026-05-17 audit's
CA H finding on `doctor::analyze` and the ADR-0006 Follow-up on internal
closure threading by bundling per-function injected closures into typed
`*Deps` carriers across all four public entry points at once. See
[ADR-0007](docs/decisions/0007-deps-bag-layered.md) for the rationale,
the six alternatives that were rejected (status quo, flat per-function
carriers, single Option-laden bag, trait+associated-types, generic
closure fields, builder pattern), and the closure-HRTB limit that
drove the `Box<dyn>` over generic.

### Breaking

- **`pathlint::doctor::analyze` signature change.** The four
  positional closures (`fs_exists`, `env_lookup`, `fs_list_dir`,
  `is_writable_dir`) collapse into a single `AnalyzeDeps<'_>`
  carrier. `analyze_real` is unchanged.
  Migration:
  ```rust
  analyze(e, s, r, os, fs, env, ls, wr)
  // becomes
  analyze(e, s, r, os, AnalyzeDeps {
      common: CommonDeps { env_lookup: Box::new(env) },
      fs_exists: Box::new(fs),
      fs_list_dir: Box::new(ls),
      is_writable_dir: Box::new(wr),
  })
  ```
- **`pathlint::lint::evaluate` signature change.** The `resolver` /
  `shape_check` positional closures collapse into `EvaluateDeps<'_>`.
- **`pathlint::trace::locate` signature change.** The `resolver`
  positional closure collapses into `LocateDeps<'_>`.
- **`pathlint::sort::sort_path` signature change.** Now takes a
  `SortDeps<'_>` carrier (which today only carries the shared env
  oracle; the carrier exists so future cross-cutting deps are
  additive).

### Added

- `pathlint::CommonDeps` — shared dependency carrier holding the
  env oracle. Embedded by every per-function `*Deps`.
- `pathlint::doctor::AnalyzeDeps` / `lint::EvaluateDeps` /
  `trace::LocateDeps` / `sort::SortDeps` — per-function carriers
  with `production()` constructors.
- `pathlint::lint::evaluate_real` / `trace::locate_real` /
  `sort::sort_path_real` — production wrappers mirroring the
  pre-existing `doctor::analyze_real` so all four entry points
  have the same zero-extra-closures shape for CLI / production
  use.
- Type aliases in the carriers (`EnvLookupFn`, `FsBoolFn`,
  `FsListDirFn`, `ResolverFn`, `ShapeCheckFn`) — public surface so
  `tests/public_api.rs` can pin them and embedders can name
  callable shapes without re-deriving `Box<dyn Fn ... + 'a>`.

### Fixed

- ADR-0006 Follow-up: internal callers inside the lib
  (`doctor::matched_entries_for_source`,
  `doctor::add_relation_conflict_diagnostics`,
  `lint::evaluate_one`, `trace::locate`'s provenance walk,
  `sort::sort_path`'s indexer) now thread `env_lookup` through
  `source_match::*_with` instead of falling back to the wrappers.
- `#[allow(clippy::too_many_arguments)]` on `doctor::analyze`
  removed — the carrier replaces the positional closures that
  earned the lint.

## [0.0.26] — 2026-05-23

Additive-only release. No BREAKING. Closes the env-injection
scope of ADR-0002 by giving the `expand` and `source_match` layers
`_with` variants that take a caller-supplied env lookup. Embedders
that exclusively call the `_with` family can now resolve catalog
source paths without ever touching `std::env::var`. See
[ADR-0006](docs/decisions/0006-source-match-env-closure-injection.md)
for the rationale, the four alternatives that were rejected, and
the follow-up tied to the `AnalyzeDeps` work (Step 3 of the
0.0.25-0.1.0 roadmap).

### Added

- `pathlint::expand::expand_and_normalize_with(input, env_lookup)`
  is the new injection-aware form. The existing
  `expand::expand_and_normalize(input)` becomes a thin wrapper
  that passes `|v| std::env::var(v).ok()` — same shape as the
  0.0.23 `expand_env` / `expand_env_with` pair.
- `pathlint::source_match::find_with(haystack, sources, os, env_lookup)`,
  `pathlint::source_match::validate_sources_with(sources, os, env_lookup)`,
  and `pathlint::source_match::names_only_with(haystack, sources, os, env_lookup)`
  let the catalog source's path be resolved through the closure
  instead of the live process environment. The existing
  `find` / `validate_sources` / `names_only` become wrappers.

### Fixed

- ADR-0002 Follow-up (codex 2026-05-17 audit, FP H severity):
  the lib's public boundary now closes the env-injection scope.
  Internal callers (`lint::evaluate_one`, `trace::locate`,
  `sort::sort_path`, the `doctor` matchers, the binary's
  `enforce_source_validation`) still read the process env via
  the wrappers in production, but every public entry point on
  the matching surface accepts an explicit env closure.

## [0.0.25] — 2026-05-17

Docs-only release. No public API change. The 0.0.24 → 0.0.25 bump
introduces the architecture-decision system that anchors the 0.0.x
→ 0.1.0 design-concept overhaul; subsequent releases will reference
ADRs from their `### Breaking` entries.

### Added

- `docs/decisions/` directory with an
  [README](docs/decisions/README.md) and a meta-ADR
  ([ADR-0000](docs/decisions/0000-adr-categories.md)) that
  defines the eight categories pathlint recognises, the
  positive criteria for writing an ADR (PA1-PA8), and the
  negative criteria for *not* writing one (NA1-NA4). Subsequent
  ADRs must declare a `Category: N. <name>` metadata line so
  the index can sort by topic.
- ADRs covering five load-bearing past decisions:
  [ADR-0001](docs/decisions/0001-pathentry-as-tenth-public-module.md)
  (PathEntry as the 10th public module, 0.0.23 — category 1+4),
  [ADR-0002](docs/decisions/0002-from-raw-closure-injection.md)
  (`from_raw` closure injection, 0.0.23 — category 3+1),
  [ADR-0003](docs/decisions/0003-reg-expand-sz-raw-decode.md)
  (registry `REG_EXPAND_SZ` raw decode, 0.0.23 — category 4),
  [ADR-0004](docs/decisions/0004-process-target-registry-provenance-overlay.md)
  (process-target provenance overlay, 0.0.24 — category 1+5),
  [ADR-0005](docs/decisions/0005-pre-1-0-breaking-policy.md)
  (pre-1.0 BREAKING policy — category 8).
- [`docs/SECURITY.md`](docs/SECURITY.md) — trust boundaries,
  sanitisation pointers, security non-goals, threat model, and
  vulnerability reporting channel.
- [PRD §3.1](docs/PRD.md#31-graduation-to-010) — the 7-criteria
  graduation gate that 0.0.x must satisfy before 0.1.0 ships.
  Mirrored in JP PRD §3.1.

### Fixed (docs drift)

- `docs/ARCHITECTURE.md` updated from "9 public modules" to "10
  public modules"; `path_entry` row added to the public-module
  table; the `tests/cli_strings.rs` description was corrected to
  reflect the 0.0.22 alias removal.
- JP PRD §17 reduced from ~303 lines of inline cumulative
  changelog to a 25-line pointer at `CHANGELOG.md` and
  `docs/decisions/`, matching the EN PRD §17 structure that
  was already pointer-only.
- CLI `--target` help text and README `--target` section now
  call out the Windows-only registry overlay (the 0.0.24
  semantics that previously lived only in PRD §10.1).

## [0.0.24] — 2026-05-10

### Breaking

- **`pathlint::path_entry::PathEntry` gains a third public field
  `provenance_raw: Option<String>`.** Embedders that construct a
  `PathEntry` by struct literal (`PathEntry { raw, expanded }`)
  must add `provenance_raw: None` to keep compiling.
  `PathEntry::from_raw(raw, env_lookup)` stays source-compatible
  and remains the recommended construction path; it leaves
  `provenance_raw = None` on every newly-built entry.

### Added

- `pathlint::path_entry::PathEntry::effective_raw_for_user_intent(&self) -> &str`
  returns `provenance_raw` when set, otherwise `raw`. Detectors
  that reason about *what the user typed* (`Shortenable`,
  `Malformed`, `TrailingSlash`, `ShortName`) and human-facing
  renderers (`Diagnostic.entry`, the `Duplicate` first-path
  reference, the per-group entry in `Conflict` output) all go
  through this accessor so a Windows process-target entry whose
  registry form is `%LocalAppData%\...` is treated as the user's
  authored form, not the OS-expanded literal.
- `pathlint::path_entry::PathEntry::with_provenance(self, registry_raw: String) -> Self`
  is a chainable setter used by the `path_source` reconciler. Idempotent.
- `pathlint::path_source::reconcile_process_with_registry(process, user_reg, machine_reg)`
  is a pure function (no I/O, no env access) that overlays the
  registry raw form onto a process entry whose `expanded` matches
  a registry entry's `expanded`. Match rule: `expand::normalize`
  equality (case-insensitive + slash-unify). Tie-break: HKCU
  before HKLM, then first occurrence within a source. Skipped
  silently when a process entry has no expanded match (false-
  negative is preferred over false-suppression for race / runtime
  PATH injection).

### Fixed

- **Windows: `pathlint doctor` with the default `--target process`
  no longer mis-suggests shortening registry-authored PATH
  entries.** Before 0.0.24, `getenv("PATH")` returned the OS-expanded
  literal (`C:\Users\me\AppData\Local\Microsoft\WindowsApps`),
  bypassing the 0.0.23 raw-preservation fix that protects
  `--target user` / `--target machine`. The new path_source
  reconciler reads HKCU and HKLM raw at process-target start-up
  and overlays the registry's `%VAR%` form onto matching entries
  via `provenance_raw`, so `Shortenable` (and the other user-intent
  detectors) see what the user wrote in `regedit`. Entries with
  no registry counterpart — typically PATH injected at runtime via
  `set PATH=...` or by a child shell — keep their literal form
  and continue to trip `Shortenable` when applicable.

## [0.0.23] — 2026-05-10

### Breaking

- **PATH entry handling moved to a `PathEntry { raw, expanded }`
  type.** `pathlint::doctor::analyze`,
  `pathlint::doctor::analyze_real`, `pathlint::sort::sort_path`,
  and the doc-hidden `path_source::PathRead` /
  `resolve::resolve` / `resolve::split_path` all now consume or
  return `&[PathEntry]` instead of `&[String]`. The boundary
  point at which env expansion runs is now exactly one place
  (`PathEntry::from_raw`, called from
  `pathlint::path_source::read_path` and `resolve::split_path`),
  so detectors that reason about *what the user typed*
  (Shortenable, RelativePathEntry) see `entry.raw` and
  detectors that reason about *the directory on disk* (Missing,
  WriteablePathDir, the resolver) see `entry.expanded`.
- **`PathEntry::from_raw` takes a `(raw, env_lookup)` pair.** The
  constructor is closure-receiving so pathlint never touches
  `std::env::var` from inside it. Production callers inject
  `|v| std::env::var(v).ok()` at the two infrastructure
  boundary points; lib embedders and tests inject deterministic
  closures.
  **Migration**: replace `PathEntry::from_raw(s)` (never
  released) with `PathEntry::from_raw(s, |v|
  std::env::var(v).ok())` for production behaviour, or pass a
  custom closure for deterministic env handling.

### Added

- `pathlint::path_entry::PathEntry { raw, expanded }` is the new
  10th public module. `PathEntry::from_raw(raw, env_lookup)`
  takes a `Fn(&str) -> Option<String>` so callers control the
  env oracle — pathlint never touches the process env from
  inside the constructor. Production callers (the
  `path_source::read_path` and `resolve::split_path` boundary
  points) inject `|v| std::env::var(v).ok()`.
- `pathlint::expand::expand_env_with(input, env_lookup)` is the
  injection-aware form of the existing `expand_env`, which is
  now a thin wrapper over `expand_env_with` that reads the live
  process env. Public surface; embedders and tests can drive
  `%VAR%` / `$VAR` / `${VAR}` / `~` expansion deterministically.
- `pathlint::path_source::decode_reg_string` (Windows-only,
  crate-internal): UTF-16 LE decoder for `REG_SZ` /
  `REG_EXPAND_SZ` registry values. Lossy on invalid surrogate
  pairs (offending code unit replaced with `U+FFFD`), `Err` on
  unsupported registry types (`REG_MULTI_SZ`, `REG_BINARY`,
  `REG_DWORD`, …). In both error cases `read_path` returns a
  warning and an empty `entries` vector — pathlint never panics
  on a hostile payload, never silently emits diagnostics built
  from garbled bytes.

### Fixed

- **Windows: `doctor` no longer falsely suggests "shorten with
  `%LocalAppData%`" for entries the user already wrote in that
  form.** Before 0.0.23, `winreg`'s
  `RegKey::get_value::<String, _>` silently expanded
  `REG_EXPAND_SZ` registry payloads via
  `ExpandEnvironmentStringsW`, so pathlint received a fully
  expanded `C:\Users\...\AppData\Local\...` string for an entry
  the user had stored as `%LocalAppData%\...`. The Shortenable
  detector's `entry.contains('%')` skip therefore never fired,
  and the user got a confusing "shorten this entry that is
  already shortened" warning. 0.0.23 reads the raw bytes via
  `RegKey::get_raw_value`, decodes them as UTF-16 LE in
  `decode_reg_string`, and lets `expand_env` run exactly once —
  so the raw form is preserved through the whole lint pipeline.
  Doctor output for a Windows registry-driven PATH now also
  displays `%LocalAppData%`-style entries verbatim, matching
  what the user has in their environment.

## [0.0.22] — 2026-05-09

### Breaking

- **`pathlint where` and `--rules` aliases removed.** The
  6-release deprecation runway (0.0.14 introduction, 0.0.20
  warning phase, 0.0.21 second runway release) is over. clap no
  longer accepts `where` as a subcommand alias of `trace` or
  `--rules` as a long-flag alias of `--config`; both produce the
  standard "unknown argument" error and exit 2.
  **Migration**: rename to `pathlint trace` and `--config`.
  Scripts that grepped for the old spelling on the warning line
  in stderr can drop the grep entirely — the warning is gone with
  the alias. *(See [ADR-0019](docs/decisions/0019-cli-alias-deprecation-runway.md).)*

### Changed

- **`WriteablePathDir` on Windows now probes Authenticated Users
  and BUILTIN\\Users in addition to Everyone.** 0.0.21 shipped
  the detector with a single SID check (`S-1-1-0`/Everyone),
  which captured the dictionary case but missed the common one —
  Windows hosts almost always grant write through `BUILTIN\\Users`
  (`S-1-5-32-545`) or `Authenticated Users` (`S-1-5-11`), not
  Everyone. 0.0.22 probes all three SIDs in turn and
  short-circuits on the first effective `FILE_GENERIC_WRITE` /
  `FILE_APPEND_DATA`, so the typical "writes inherited through a
  group" case is now flagged. Unix behaviour and the closure
  contract are unchanged; the detector is still approximation
  (DENY ACEs and arbitrary per-user grants outside these three
  groups are not modelled).

## [0.0.21] — 2026-05-09

### Breaking

- **`doctor::analyze` gains `is_writable_dir` closure parameter.**
  The function now takes an 8th `Fn(&str) -> bool` argument used
  by the new `WriteablePathDir` detector. Embedders that built
  their own resolver loop must add the closure (production wiring
  in `pathlint::doctor::is_writable_dir_real` is the reference;
  Unix checks the others-write bit, Windows reads the DACL via
  `GetEffectiveRightsFromAclW`). `analyze_real` is unchanged for
  CLI-only callers. *(See [ADR-0020](docs/decisions/0020-doctor-analyze-closure-tuple.md); superseded by ADR-0007 as of 0.0.27.)*

### Added

- **`pathlint doctor` learned the `writeable_path_dir` detector.**
  PATH entry resolves to a directory writable by users other than
  the owner. On Unix, the others-write bit (`mode & 0o002`) is
  the trigger. On Windows, the DACL is queried and the detector
  fires when the well-known "Everyone" SID has effective
  `FILE_GENERIC_WRITE` or `FILE_APPEND_DATA`. Approximation, not
  a full ACL audit: group-inherited writes are not yet checked.
  Suppress with `--exclude writeable_path_dir`.
- **`pathlint::doctor::is_writable_dir_real`** added as the
  production wrapper for the new closure parameter. Returns
  `false` on permission errors, missing dirs, non-directories, or
  any winapi failure.
- **Plugin description phrasing unified across 7 built-in
  sources** (`mise`, `mise_installs`, `os_baseline_linux_sbin`,
  `npm_global`, `pip_user`, `asdf`, with `mise_shims` already
  short and unchanged). `pathlint catalog list` is now scannable
  at a glance; distro / implementation context moved into TOML
  comment lines next to each source.
- **windows-sys 0.59** added to
  `[target.'cfg(windows)'.dependencies]` for the DACL and SID
  API surface used by `is_writable_dir_real` on Windows. Linux,
  macOS, and Termux builds are unaffected.

## [0.0.20] — 2026-05-08

### Added

- **`pathlint doctor` learned the `relative_path_entry` detector.**
  Fires when a PATH entry expands to a relative path (`.`,
  `./bin`, bare `bin`, …). The shell would resolve these against
  the cwd at command-invocation time — almost always a security
  or portability footgun. Env vars are expanded first; an
  unresolved `$VAR/bin` stays verbatim and fires (config bug
  worth surfacing). "Absolute" is judged by the target OS, not
  the host. Suppress with `--exclude relative_path_entry`.
- **`pathlint where` and `--rules` now print a one-line
  deprecation warning to stderr on use.** Canonical names
  `trace` and `--config` remain unchanged. Removal is planned
  for a future breaking release; the warning is the migration
  runway. *(Removal landed in 0.0.22.)*
- **5 schema top-level descriptions tidied** for editor hover
  use. Implementation jargon (`deny_unknown_fields`,
  "discriminated union") removed; checked-in schemas regenerated.
  Drift gates green.
- **`source_match` rustdoc example** replaced with a concrete
  `find()` call against `/usr/bin/ls`; the doctest now actually
  validates the API instead of asserting a tautology.
- **RELEASE checklist** clarifies that `docs/ARCHITECTURE.md` is
  intentionally English-only and not gated by EN/JP parity.

## [0.0.19] — 2026-05-06

### Breaking

- **`doctor::analyze` gains `fs_list_dir` closure parameter.**
  The function now takes a 7th `Fn(&str) -> Vec<String>` argument
  used by the new `DuplicateButShadowed` detector to enumerate
  executables in each PATH dir. Embedders that built their own
  resolver loop must add the closure (production wiring in
  `pathlint::doctor::fs_list_dir_real` is the reference).
  `analyze_real` is unchanged for CLI-only callers. *(See [ADR-0020](docs/decisions/0020-doctor-analyze-closure-tuple.md); superseded by ADR-0007 as of 0.0.27.)*

### Added

- **`pathlint doctor` learned the `duplicate_but_shadowed`
  detector.** Fires when the same command basename exists as a
  real executable in two or more PATH dirs. Reports the winning
  PATH index, the shadowed indices, and the command name. Windows
  compares case-insensitively after stripping PATHEXT extensions
  (so `python.exe` and `python.bat` count as the same command).
  Suppress with `--exclude duplicate_but_shadowed`.

  Design choice — no alias filter. mise activate's typical
  shims+installs layout is not "expected noise" the detector
  should ignore: in mise's standard usage, only one of the two
  dirs is on PATH at a time (`mise activate` exposes shims;
  `mise hook-env` exposes installs). Both being on PATH at once
  is itself a misconfiguration, already warned about from the
  relation angle by the existing `mise_activate_both` Conflict
  detector. Filtering out the same situation in a second detector
  would hide the same mistake from a different angle. When the
  host's noise is genuinely unwanted, suppress per host with
  `--exclude`.

- **`pathlint::doctor::fs_list_dir_real`** added as the production
  wrapper for the new closure parameter.

## [0.0.18] — 2026-05-06

### Added

- **`pathlint doctor` learned the `per_source_missing_required`
  detector.** Fires when a `[source.<name>]` entry from the
  user's `pathlint.toml` points at a per-OS path that does not
  exist on the host. Built-in catalog sources are deliberately
  skipped (most hosts are missing 80% of the catalog by design).
- **`--no-glyphs` now applies to `doctor` / `trace` / `sort`
  output.** Pre-0.0.18 the flag only routed through `report.rs`
  (check OK/NG tags). Em-dash and rightwards-arrow now fall back
  to `-` and `->` across every human renderer.
- **`pathlint::catalog::RelationIndex` typed accessor view.**
  Internal-only refactor; no change to the `[[relation]] kind=...`
  TOML shape. Consumers (sort / doctor / trace / cycle check)
  read through `iter_aliases()` / `iter_conflicts()` /
  `iter_provenances()` / `iter_depends_on()` /
  `iter_prefer_orders()` instead of open `match Relation { ... }`.
- **`scripts/bench.sh` startup-time baseline.** hyperfine wrapper;
  paste the table into release notes to verify the PRD §12
  `<50 ms` claim on the host.

## [0.0.17] — 2026-05-05

### Breaking

- **`Status` enum is unit-only; `Outcome` gains `reason`.**
  `Status::NgNotExecutable(String)` and `Status::ConfigError(String)`
  used to carry their human-readable detail in the variant
  payload. As of 0.0.17 the payload is gone and the detail rides
  on a separate `Outcome::reason: Option<String>`. Downstream
  effect: `pathlint check --json` now emits
  `{"kind": "ng_not_executable", "reason": "..."}` instead of
  `{"kind": {"ng_not_executable": "..."}}`. Consumers branching
  on `kind` as a string can finally do so without a fallback for
  the two payload-carrying variants. *(See [ADR-0018](docs/decisions/0018-resolver-outcome-type-simplification.md).)*
- **`pathlint::cli` and `pathlint::run` removed from the lib.**
  Both modules used to be `#[doc(hidden)] pub mod` so the binary
  in `src/main.rs` could reach across the crate boundary. They
  now live in `src/bin/pathlint/` and are binary-only. Anything
  embedding pathlint as a library had no business calling them;
  they are gone from the surface. *(See [ADR-0017](docs/decisions/0017-lib-surface-nine-modules.md).)*
- **Lib internal modules behind `#[doc(hidden)] pub`.**
  `catalog_view`, `format`, `init`, `path_source`, `report`,
  `resolve` shifted from `pub(crate)` to `#[doc(hidden)] pub` so
  the binary at `src/bin/pathlint/` can call them across the
  lib/bin boundary. Same compromise cli/run had pre-0.0.17. *(See [ADR-0017](docs/decisions/0017-lib-surface-nine-modules.md).)*
- **`check.schema.json` `required` no longer lists
  `prefer` / `avoid` / `reason` / `diagnosis` / `resolved`.** The
  runtime applied `skip_serializing_if` on these fields, but the
  schema flagged them as required. The schema is now honest about
  what the wire form actually emits. JSON validators that assumed
  those fields were always present must accept their absence. *(See [ADR-0016](docs/decisions/0016-json-wire-shape-kind-discriminator.md).)*
- **Shell quoting moved to internal `shell_quote` module.**
  Pre-0.0.17 `pathlint::format::quote_for` etc. were public. They
  were never advertised as supported and are now `pub(crate)` in
  `pathlint::shell_quote`. Embedders should read the
  already-quoted string from `trace --json uninstall.command`. *(See [ADR-0017](docs/decisions/0017-lib-surface-nine-modules.md).)*
- **`--color` flag is now effective.** Pre-0.0.17 the global
  `--color {auto,always,never}` flag was parsed by clap and
  silently ignored. As of 0.0.17 it actually colourises status
  tags in the human output (and respects `--color never`). Output
  of pipelines that captured `pathlint check` stdout may now
  contain ANSI escapes when the captured stream is also pathlint's
  stdout and `--color always` is set. *(See [ADR-0024](docs/decisions/0024-color-flag-activation.md).)*

## [0.0.16] — 2026-05-05

### Breaking

- **Lib resolver signature simplified.** `pathlint::lint::evaluate`
  and `pathlint::trace::locate` now take a resolver closure
  returning `Option<std::path::PathBuf>`, not the internal
  `Resolution { full_path: PathBuf }` wrapper. Embedders that
  built their own resolver closures must drop the wrapper:
  `Some(Resolution { full_path: pb })` → `Some(pb)`. *(See [ADR-0018](docs/decisions/0018-resolver-outcome-type-simplification.md).)*
- **`Resolution` type removed.** `pathlint::resolve::resolve()`
  now returns `Option<PathBuf>` directly. Internal-only impact —
  the type was never on the public surface, but downstream
  embedders accessing pathlint via `git` dependencies might
  notice. *(See [ADR-0018](docs/decisions/0018-resolver-outcome-type-simplification.md).)*

## [0.0.15] — 2026-05-05

### Breaking

- **`pathlint check --json` discriminator renamed.** Each outcome
  array element now uses `kind` (matches doctor / trace / sort /
  catalog relations) instead of the pre-0.0.15 `status`. The
  values themselves are unchanged. **Migration**: any consumer
  that branched on `.status` must read `.kind` instead. *(See [ADR-0016](docs/decisions/0016-json-wire-shape-kind-discriminator.md).)*
- **Lib public surface narrowed to nine supported modules.**
  `config`, `lint`, `trace`, `sort`, `doctor`, `catalog`,
  `source_match`, `os_detect`, `expand`. Internals are
  `pub(crate)` or `#[doc(hidden)] pub` (the latter only for
  `cli` / `run` reachable from `src/main.rs`). Embedders relying
  on previously-public modules (e.g. `format`, `report`) must
  migrate. *(See [ADR-0017](docs/decisions/0017-lib-surface-nine-modules.md).)*
- **UserConfig and the embedded catalog file are distinct types.**
  A user `pathlint.toml` declaring `catalog_version` is now a
  structural parse error (deny_unknown_fields) instead of the
  post-parse error 0.0.14 introduced. *(See [ADR-0023](docs/decisions/0023-catalog-version-reserved-for-embedded.md).)*

## [0.0.14] — 2026-05-05

### Breaking

- **`pathlint where` → `pathlint trace`.** `where` remains as a
  clap visible alias for the rest of 0.0.x. *(Alias removed in
  0.0.22.)* *(See [ADR-0019](docs/decisions/0019-cli-alias-deprecation-runway.md).)*
- **`--rules` → `--config`.** `--rules` remains as a visible
  alias for the rest of 0.0.x. *(Alias removed in 0.0.22.)* *(See [ADR-0019](docs/decisions/0019-cli-alias-deprecation-runway.md).)*
- **Source rename, no aliases.** `WindowsApps` → `windows_apps`.
  `system_windows` / `system_macos` / `system_linux` →
  `os_baseline_windows` / `os_baseline_macos` /
  `os_baseline_linux`. New `os_baseline_linux_sbin` for
  `/usr/sbin`. *(See [ADR-0014](docs/decisions/0014-source-naming-convention.md).)* **Migration**:
  ```sh
  sed -i \
    -e 's/WindowsApps/windows_apps/g' \
    -e 's/system_windows/os_baseline_windows/g' \
    -e 's/system_macos/os_baseline_macos/g' \
    -e 's/system_linux/os_baseline_linux/g' \
    pathlint.toml
  ```
- **`trace --json` shape change.** Top-level `kind` discriminator
  (`"found"` / `"not_found"`) replaces the old `found: bool`
  field. JSON consumers that branched on `found` must switch to
  `kind`. *(See [ADR-0016](docs/decisions/0016-json-wire-shape-kind-discriminator.md).)*
- **`Provenance::MiseInstallerPlugin` → `Provenance::WrapperInstaller`.**
  Visible in `trace --json` as
  `provenance.kind = "wrapper_installer"`. `installer` and
  `plugin_segment` payload fields are unchanged. *(See [ADR-0015](docs/decisions/0015-provenance-wrapper-installer-rename.md).)*
- **`sort --dry-run` is opt-in.** `pathlint sort` without
  `--dry-run` exits 2 with a message naming the flag. A future
  `--apply` (post-1.0) would override this; today the only mode
  shipped is `--dry-run`. *(See [ADR-0009](docs/decisions/0009-read-only-stance.md).)*
- **`catalog_version = N` in user `pathlint.toml` is rejected.**
  The field was always reserved for the embedded catalog;
  `Config::from_path` now exits 2 if a user TOML sets it. (0.0.15
  promoted this from a post-parse to a structural error.) *(See [ADR-0023](docs/decisions/0023-catalog-version-reserved-for-embedded.md).)*
- **`depends_on` is descriptive only.** It surfaces in
  `pathlint catalog relations` but does not affect doctor /
  trace / sort behaviour. *(See [ADR-0022](docs/decisions/0022-depends-on-descriptive-only.md).)*
- **`build.rs` aggregates referential integrity violations.** CI
  surfaces every offending plugin in one failure instead of
  bailing on the first. *(See [ADR-0021](docs/decisions/0021-build-rs-aggregate-violations.md).)*

## Releases prior to 0.0.14

Earlier releases predate this changelog format and are not
re-tabulated here. The git history (`git log --oneline`) and tags
`v0.0.x` are the canonical record.

[Unreleased]: https://github.com/ShortArrow/pathlint/compare/v0.0.33...HEAD
[0.0.33]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.33
[0.0.32]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.32
[0.0.31]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.31
[0.0.30]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.30
[0.0.29]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.29
[0.0.28]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.28
[0.0.27]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.27
[0.0.26]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.26
[0.0.25]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.25
[0.0.24]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.24
[0.0.23]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.23
[0.0.22]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.22
[0.0.21]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.21
[0.0.20]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.20
[0.0.19]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.19
[0.0.18]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.18
[0.0.17]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.17
[0.0.16]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.16
[0.0.15]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.15
[0.0.14]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.14
