# Architecture Decision Records

This directory records the **why** behind pathlint's load-bearing
design choices, in the format popularised by Michael Nygard. Each
ADR captures one decision: the forces that made the obvious
answer wrong, what was rejected, and what shipping it actually
costs.

**Start here**: [ADR-0000](0000-adr-categories.md) defines the
eight decision categories pathlint recognises, the positive
criteria for writing an ADR (PA1-PA8), and the negative criteria
for *not* writing one (NA1-NA4). Every other ADR in this
directory belongs to one or more of those categories.

## Index by number (timeline view)

| ADR | Title | Category | Status | Shipped in |
|---|---|---|---|---|
| [0000](0000-adr-categories.md) | ADR categories and when to write one | 8 (self) | Accepted | 0.0.25 |
| [0001](0001-pathentry-as-tenth-public-module.md) | PathEntry as the 10th public module | 1 (+4) | Accepted (partially superseded by 0004) | 0.0.23 |
| [0002](0002-from-raw-closure-injection.md) | `PathEntry::from_raw` takes a closure | 3 (+1) | Accepted | 0.0.23 |
| [0003](0003-reg-expand-sz-raw-decode.md) | Decode `REG_EXPAND_SZ` ourselves | 4 | Accepted | 0.0.23 |
| [0004](0004-process-target-registry-provenance-overlay.md) | Process-target registry provenance overlay | 1 (+5) | Accepted (supersedes part of 0001) | 0.0.24 |
| [0005](0005-pre-1-0-breaking-policy.md) | 0.0.x line allows MAJOR-equivalent BREAKING | 8 | Accepted | 0.0.x |
| [0006](0006-source-match-env-closure-injection.md) | `_with` env-lookup closures on `expand_and_normalize` and `source_match` | 3 (+1) | Accepted | 0.0.26 |
| [0007](0007-deps-bag-layered.md) | layered `*Deps` carriers + per-function production wrappers | 2 (+1) | Accepted | 0.0.27 |
| [0008](0008-attribution-type-split.md) | split `Attribution` out of `PathEntry` | 1 (+2) | Accepted (closes ADR-0001 & 0004 Follow-up) | 0.0.28 |
| [0009](0009-read-only-stance.md) | Read-only stance (no PATH / registry / dotfiles mutation) | 5 (+4) | Accepted | 0.0.x (recorded retroactively in 0.0.30) |
| [0010](0010-release-workflow-bump-skip.md) | Release workflow tolerates an already-bumped `Cargo.toml` | 8 | Superseded by ADR-0029 | 0.0.24 (recorded retroactively in 0.0.30; superseded in 0.0.36) |
| [0011](0011-normalize-substring-match-policy.md) | `expand::normalize` policy (case-insensitive + slash unify, substring match without canonicalisation) | 3 | Accepted | 0.0.x (recorded retroactively in 0.0.30) |
| [0012](0012-schemars-1-0-deferred.md) | Defer schemars 1.0 migration past 0.0.x graduation | 6 | Accepted | 0.0.31 |
| [0013](0013-graduation-criteria-record.md) | Graduation criteria satisfaction record (0.0.31 snapshot) | 8 | Accepted (Criterion 5 section superseded by ADR-0025) | 0.0.31 |
| [0014](0014-source-naming-convention.md) | Source naming convention — `<provenance>_<scope>` + `os_baseline_*` split | 7 | Accepted | 0.0.14 (recorded retroactively in 0.0.32) |
| [0015](0015-provenance-wrapper-installer-rename.md) | `Provenance::WrapperInstaller` generalises from mise-only naming | 1 | Accepted | 0.0.14 (recorded retroactively in 0.0.32) |
| [0016](0016-json-wire-shape-kind-discriminator.md) | JSON wire shape — every union uses top-level `kind` + schema `required` honesty | 7 | Accepted | 0.0.14 / 0.0.15 / 0.0.17 (recorded retroactively in 0.0.32) |
| [0017](0017-lib-surface-nine-modules.md) | Lib surface narrowed to 9 supported `pub mod` + `#[doc(hidden)] pub` middle tier | 1 (+2, +8) | Accepted | 0.0.15 / 0.0.17 (recorded retroactively in 0.0.32) |
| [0018](0018-resolver-outcome-type-simplification.md) | Resolver `Option<PathBuf>` + unit-variant `Status` with `Outcome::reason` | 1 | Accepted | 0.0.16 / 0.0.17 (recorded retroactively in 0.0.32) |
| [0019](0019-cli-alias-deprecation-runway.md) | 6-release deprecation runway for CLI renames (`where`/`--rules`) | 5 (+8) | Accepted | 0.0.14 → 0.0.22 (recorded retroactively in 0.0.32) |
| [0020](0020-doctor-analyze-closure-tuple.md) | `doctor::analyze` open-ended closure tuple as new detectors land | 1 (+3) | Accepted (superseded by ADR-0007 as of 0.0.27) | 0.0.19 / 0.0.21 (recorded retroactively in 0.0.32) |
| [0021](0021-build-rs-aggregate-violations.md) | `build.rs` aggregates plugin referential-integrity violations | 8 | Accepted | 0.0.14 (recorded retroactively in 0.0.32) |
| [0022](0022-depends-on-descriptive-only.md) | `depends_on` relation is descriptive-only, no runtime effect | 5 | Accepted | 0.0.14 (recorded retroactively in 0.0.32) |
| [0023](0023-catalog-version-reserved-for-embedded.md) | `catalog_version` is reserved for the embedded catalog | 7 | Accepted | 0.0.14 / 0.0.15 (recorded retroactively in 0.0.32) |
| [0024](0024-color-flag-activation.md) | `--color` flag activation (parsed-but-ignored → effective) | 8 | Accepted | 0.0.17 (recorded retroactively in 0.0.32) |
| [0025](0025-criterion-5-closure.md) | Graduation criterion 5 fully satisfied (11/11 Breaking releases ADR-linked) | 8 | Accepted (supersedes ADR-0013 §Criterion 5) | 0.0.32 |
| [0026](0026-trybuild-for-negative-invariants.md) | Adopt `trybuild` as dev-dependency for compile-fail negative-invariant tests | 6 (+8) | Accepted | 0.0.33 |
| [0027](0027-lib-env-read-boundaries.md) | Lib has two intentional env-read boundaries; `_with` is the injection seam, wrapper is CLI-convenience | 3 (+4) | Accepted | 0.0.33 |
| [0028](0028-doctor-lint-responsibility-split.md) | `doctor` is pathlint's selfcheck; PATH analysis moves to a new `lint` subcommand | 1 (+8) | Accepted | 0.0.34 |
| [0029](0029-release-trigger-tag-push.md) | Release workflow trigger moves from `workflow_dispatch` to `on: push: tags` (supersedes ADR-0010) | 8 | Accepted | 0.0.36 |

## Index by category (topical view)

The category numbers refer to the list in
[ADR-0000](0000-adr-categories.md). Importance ordering reflects
how often the category bites in pathlint specifically; the order
is **not** a universal ADR hierarchy.

### 1. Public API surface

- [ADR-0001](0001-pathentry-as-tenth-public-module.md) — PathEntry as the 10th public module
- [ADR-0004](0004-process-target-registry-provenance-overlay.md) — provenance overlay on `--target process`
- [ADR-0006](0006-source-match-env-closure-injection.md) — secondary (Added `_with` variants on `expand` / `source_match`)
- [ADR-0007](0007-deps-bag-layered.md) — secondary (4 BREAKING signature changes on `analyze` / `evaluate` / `locate` / `sort_path`)
- [ADR-0008](0008-attribution-type-split.md) — split `Attribution` out of `PathEntry`; entry-list parameters now take `&[Attribution]`
- [ADR-0015](0015-provenance-wrapper-installer-rename.md) — `Provenance::WrapperInstaller` generalises from mise-only naming
- [ADR-0017](0017-lib-surface-nine-modules.md) — Lib surface narrowed to 9 supported `pub mod` + `#[doc(hidden)] pub` middle tier
- [ADR-0018](0018-resolver-outcome-type-simplification.md) — Resolver `Option<PathBuf>` + unit-variant `Status` with `Outcome::reason`
- [ADR-0020](0020-doctor-analyze-closure-tuple.md) — `doctor::analyze` open-ended closure tuple (superseded by ADR-0007)
- [ADR-0028](0028-doctor-lint-responsibility-split.md) — `doctor` is selfcheck; PATH analysis moves to a new `lint` subcommand

### 2. Module boundary / dependency direction

- [ADR-0007](0007-deps-bag-layered.md) — layered `*Deps` carriers across `doctor::analyze`, `lint::evaluate`, `trace::locate`, `sort::sort_path`
- [ADR-0008](0008-attribution-type-split.md) — secondary (Attribution carrier hosted at the crate root next to `CommonDeps`)

### 3. Cross-cutting concern

- [ADR-0002](0002-from-raw-closure-injection.md) — env injection via closure on `PathEntry::from_raw`
- [ADR-0006](0006-source-match-env-closure-injection.md) — env injection extended to `expand::expand_and_normalize_with` and `source_match::*_with`
- [ADR-0011](0011-normalize-substring-match-policy.md) — `expand::normalize` case-insensitive + slash unify; substring match without canonicalisation
- [ADR-0027](0027-lib-env-read-boundaries.md) — two intentional env-read boundaries (source catalog resolution + PATH entry construction); wrapper / `_with` split is the injection seam

### 4. Trust / security boundary

- [ADR-0001](0001-pathentry-as-tenth-public-module.md) — secondary (registry decode boundary)
- [ADR-0003](0003-reg-expand-sz-raw-decode.md) — `decode_reg_string` lossy UTF-16 + type reject
- [ADR-0009](0009-read-only-stance.md) — secondary (no host mutation removes one whole attack surface from the trust boundary)
- [ADR-0027](0027-lib-env-read-boundaries.md) — secondary (env_lookup-returned bytes documented in SECURITY.md as untrusted)

### 5. Architectural style

- [ADR-0004](0004-process-target-registry-provenance-overlay.md) — secondary (Windows `--target process` semantics)
- [ADR-0009](0009-read-only-stance.md) — pathlint is read-only on `PATH`, registry, and dotfiles
- [ADR-0019](0019-cli-alias-deprecation-runway.md) — 6-release deprecation runway for CLI renames
- [ADR-0022](0022-depends-on-descriptive-only.md) — `depends_on` relation is descriptive-only

### 6. External dependency

- [ADR-0012](0012-schemars-1-0-deferred.md) — defer schemars 1.0 migration past 0.0.x graduation; trigger conditions for revisiting recorded.
- [ADR-0026](0026-trybuild-for-negative-invariants.md) — adopt `trybuild` as dev-dependency for compile-fail negative tests

### 7. Persistence / data format

- [ADR-0014](0014-source-naming-convention.md) — Source naming convention + `os_baseline_*` split
- [ADR-0016](0016-json-wire-shape-kind-discriminator.md) — JSON wire shape uses top-level `kind` discriminator + schema `required` honesty
- [ADR-0023](0023-catalog-version-reserved-for-embedded.md) — `catalog_version` reserved for embedded catalog (user TOML rejection)

### 8. Process / governance

- [ADR-0000](0000-adr-categories.md) — this index, ADR categories and application criteria
- [ADR-0005](0005-pre-1-0-breaking-policy.md) — pre-1.0 BREAKING licence
- [ADR-0010](0010-release-workflow-bump-skip.md) — release workflow tolerates an already-bumped `Cargo.toml` (PR #22 formalised; **superseded by ADR-0029**)
- [ADR-0013](0013-graduation-criteria-record.md) — graduation criteria satisfaction record (0.0.31 snapshot; Criterion 5 superseded)
- [ADR-0021](0021-build-rs-aggregate-violations.md) — `build.rs` aggregates plugin referential-integrity violations
- [ADR-0024](0024-color-flag-activation.md) — `--color` flag activation
- [ADR-0025](0025-criterion-5-closure.md) — graduation criterion 5 fully satisfied (supersedes ADR-0013 §Criterion 5)
- [ADR-0026](0026-trybuild-for-negative-invariants.md) — secondary (test-infra policy)
- [ADR-0028](0028-doctor-lint-responsibility-split.md) — secondary (CLI subcommand topology)
- [ADR-0029](0029-release-trigger-tag-push.md) — release trigger moves to `on: push: tags`; supersedes ADR-0010

## When to write an ADR

Authoritative answer: see [ADR-0000 §Decision](0000-adr-categories.md#decision).

Short version: if your PR matches any of PA1-PA8 in ADR-0000, write an
ADR before merging. If your PR matches all of NA1-NA4, do not.
When in doubt, lean toward writing one.

## How to write an ADR

Use the template at the bottom of this file. Keep each section
short: a reader should be able to skim Context + Decision in
under two minutes and decide whether the rest is worth reading.

- **Status**: `Proposed` → `Accepted` → optionally `Superseded
  by ADR-NNNN` or `Deprecated`. Never delete an old ADR;
  rewriting history hides the chain of decisions.
- **Date**: `YYYY-MM-DD` of the decision (not the file edit). Add
  a second line for the supersession date if relevant.
- **Release**: which crate version shipped the decision, or
  `planned` if the ADR predates implementation.
- **Category**: one or more of the eight categories listed in
  [ADR-0000](0000-adr-categories.md). Pick the primary category
  for the index sort; note secondary categories in parentheses.

## Supersession

When a later ADR overrides an earlier one:

1. Mark the old ADR's status as `Superseded by ADR-NNNN` and add
   a one-line pointer at the top.
2. Do **not** edit the body of the superseded ADR — the body
   captures what was true at the time. (Exception: additive
   `Category:` metadata when this ADR system was introduced;
   that backfill is one-time work for ADRs 0001-0005, recorded
   here for transparency.)
3. The new ADR references the old one in its Context and
   explains what changed.

This way `git blame` on the docs reflects edits to the ADR
system itself, not retroactive rewrites of history.

## Template

```markdown
# ADR-NNNN: <short title>

- **Status**: Proposed | Accepted | Superseded by ADR-MMMM | Deprecated
- **Date**: YYYY-MM-DD
- **Release**: 0.0.X (or "planned")
- **Category**: N. <name> (and M. <name> if applicable)

## Context

Why this decision needed to be made. The forces in play, the
constraint that made the obvious answer wrong, the prior art.

## Decision

What we decided. One sentence at the top, then details.

## Alternatives considered

Each rejected alternative with one-paragraph reason for
rejection. Future readers asking "why didn't they just do X"
find their answer here, not in commit archaeology.

## Consequences

- **Positive**: what got better.
- **Negative**: what got worse / what tech debt we accepted.
- **Follow-up**: what this decision implies for future work.
```
