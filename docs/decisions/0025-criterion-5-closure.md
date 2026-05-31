# ADR-0025: graduation criterion 5 fully satisfied — 11 of 11 Breaking releases now linked to ADRs

- **Status**: Accepted (supersedes ADR-0013 §Criterion 5 only)
- **Date**: 2026-05-31
- **Release**: 0.0.32
- **Category**: 8. Process / governance (graduation criteria audit closure)

## Context

ADR-0013 (0.0.31 snapshot) recorded the graduation criteria
audit at the 0.0.31 cut. Six of seven criteria were marked
satisfied; **criterion 5** ("Every release in the 0.0.x line
whose `### Breaking` section in `CHANGELOG.md` names a publicly
visible type or function has at least one ADR linked")
was marked **⚠️ Partially satisfied**:

> Releases with `### Breaking` sections, in order, and their
> ADR status:
> | Release | ADR(s) | Status |
> |---|---|---|
> | 0.0.14 | none | pre-ADR system |
> | 0.0.15 | none | pre-ADR system |
> | 0.0.16 | none | pre-ADR system |
> | 0.0.17 | none | pre-ADR system |
> | 0.0.19 | none | pre-ADR system |
> | 0.0.21 | none | pre-ADR system |
> | 0.0.22 | none | pre-ADR system |
> | 0.0.23 | ADR-0001 / 0002 / 0003 | ✅ |
> | 0.0.24 | ADR-0004 | ✅ |
> | 0.0.27 | ADR-0007 | ✅ |
> | 0.0.28 | ADR-0008 | ✅ |
>
> The strict reading is "7 of 11 releases lack ADR links".

ADR-0013 offered the user two options:
1. Accept partial satisfaction with the contextual reading
   (ADR-0000 ratified the boundary at 0.0.23; pre-system
   releases are documented by the CHANGELOG entry alone).
2. Backfill the pre-ADR Breaking releases by writing
   5-7 short ADRs.

The user chose **option 2 (backfill)**. The 0.0.32 release
ships **12 new ADRs** (this one plus ADR-0014 through
ADR-0024) drafting from the audit of 25 Breaking entries
across the 7 pre-ADR-system releases. With those ADRs
landed, every CHANGELOG `### Breaking` release in the 0.0.x
line now has at least one ADR linked.

## Decision

**Criterion 5 is fully satisfied** as of 0.0.32.

The 11 × N (release × at-least-one-ADR) matrix:

| Release | ADR(s) | Status |
|---|---|---|
| 0.0.14 | [ADR-0014](0014-source-naming-convention.md) (source rename + os_baseline split), [ADR-0015](0015-provenance-wrapper-installer-rename.md) (Provenance::WrapperInstaller), [ADR-0016](0016-json-wire-shape-kind-discriminator.md) (trace --json discriminator), [ADR-0019](0019-cli-alias-deprecation-runway.md) (where/--rules runway introduction), [ADR-0021](0021-build-rs-aggregate-violations.md) (build.rs aggregation), [ADR-0022](0022-depends-on-descriptive-only.md) (depends_on scope), [ADR-0023](0023-catalog-version-reserved-for-embedded.md) (catalog_version reject — post-parse), [ADR-0009](0009-read-only-stance.md) (sort --dry-run opt-in reuse) | ✅ |
| 0.0.15 | [ADR-0016](0016-json-wire-shape-kind-discriminator.md) (check --json discriminator), [ADR-0017](0017-lib-surface-nine-modules.md) (lib narrowed to 9 modules), [ADR-0023](0023-catalog-version-reserved-for-embedded.md) (catalog_version reject — structural) | ✅ |
| 0.0.16 | [ADR-0018](0018-resolver-outcome-type-simplification.md) (Resolution removed) | ✅ |
| 0.0.17 | [ADR-0016](0016-json-wire-shape-kind-discriminator.md) (check.schema.json required honesty), [ADR-0017](0017-lib-surface-nine-modules.md) (cli/run removed + shell_quote privatised + doc(hidden) tier), [ADR-0018](0018-resolver-outcome-type-simplification.md) (Status unit-only + Outcome::reason), [ADR-0024](0024-color-flag-activation.md) (--color flag activation) | ✅ |
| 0.0.19 | [ADR-0020](0020-doctor-analyze-closure-tuple.md) (doctor::analyze 7th closure parameter) | ✅ |
| 0.0.21 | [ADR-0020](0020-doctor-analyze-closure-tuple.md) (doctor::analyze 8th closure parameter) | ✅ |
| 0.0.22 | [ADR-0019](0019-cli-alias-deprecation-runway.md) (where/--rules alias removal — closes runway) | ✅ |
| 0.0.23 | [ADR-0001](0001-pathentry-as-tenth-public-module.md), [ADR-0002](0002-from-raw-closure-injection.md), [ADR-0003](0003-reg-expand-sz-raw-decode.md) | ✅ (unchanged from ADR-0013) |
| 0.0.24 | [ADR-0004](0004-process-target-registry-provenance-overlay.md) | ✅ (unchanged from ADR-0013) |
| 0.0.27 | [ADR-0007](0007-deps-bag-layered.md) | ✅ (unchanged from ADR-0013) |
| 0.0.28 | [ADR-0008](0008-attribution-type-split.md) | ✅ (unchanged from ADR-0013) |

11 releases, 11 with ≥ 1 ADR link. **Criterion 5 fully
satisfied** per PRD §3.1.

This decision **supersedes ADR-0013 §Criterion 5** only.
ADR-0013's other six criteria sections remain in force as
the 0.0.31 snapshot. ADR-0013's body is **not edited** (per
[README §Supersession](README.md#supersession)); ADR-0013's
frontmatter gains a one-line additive Status note:
`Accepted (Criterion 5 section superseded by ADR-0025 as of
0.0.32)`.

ADR-0000's Known ADR backlog table is **not edited** —
that table preserves the historical state of the backlog at
the time of the ADR system's introduction; rows describing
decisions still without ADRs *as of 0.0.25* remain accurate
to that point in time. Drainage in 0.0.30 (ADR-0009 /
ADR-0010 / ADR-0011) was reflected via a separate "Drained
in 0.0.30" subsection at the table's bottom; this release
does the equivalent informally in this ADR's Context above.

## Decision: this ADR does not declare graduation

Per ADR-0013 Alternative A ("Cut 0.1.0 in this ADR" —
rejected) and the user instruction quoted there
("0.1.0 を勝手に決めないで"), the graduation release cut
is **user judgement**, not a plan-side or ADR-side
decision. This ADR records *criterion 5 closure*, which is
one of seven criteria. Whether the user chooses to cut
graduation as 0.0.33 / 0.1.0 / another number / not at all,
and when, is outside this ADR's scope.

## Alternatives considered

- **A. Reinterpret criterion 5's wording so the 0.0.31
  partial state was already fully satisfied.** Rejected
  in ADR-0013 Alternative B and not re-litigated here. The
  criterion's text says "Every release in the 0.0.x line";
  the strict reading is the durable one.

- **B. Backfill ADRs as they were originally written
  (one ADR per Breaking entry, so 25 new ADRs instead of
  11).** Rejected because some Breaking entries share the
  same underlying decision: `trace --json` discriminator
  (0.0.14) and `check --json` discriminator (0.0.15) and
  schema `required` honesty (0.0.17) are three facets of
  one wire-shape policy (now ADR-0016). Bundling kept
  ADR count manageable.

- **C. Backfill only the "moderate archaeology" entries
  (source rename rationale, lib narrowing, Status
  refactor, Provenance rename) and leave the trivial
  entries (`build.rs` aggregation, `depends_on`,
  `--color` activation, `catalog_version` reject) as
  CHANGELOG-only.** Rejected because the user explicitly
  promoted all four trivial entries to standalone ADRs
  during plan review; the "decision rationale captured
  in writing" value applies regardless of how trivial
  the rationale is.

- **D. Defer this ADR to a 0.0.33 release that also did
  the source-side M findings cleanup from the
  2026-05-31 codex audit.** Rejected because criterion 5
  closure is a docs-only concern; bundling with source
  changes would invalidate the additive-only premise
  ADR-0013 documented. The M findings stay on the
  CHANGELOG carry-forward list (0.1.x candidates) and
  are not gated by this ADR.

- **E. Edit ADR-0013's Criterion 5 section directly to
  flip "Partially satisfied" → "Fully satisfied".**
  Rejected per [README §Supersession](README.md#supersession)
  bullet 2: ADR bodies are immutable. The supersession
  mechanism (additive Status pointer + new ADR
  superseding) is the durable way to evolve state.

## Consequences

- **Positive.** Graduation criterion 5 is now mechanically
  satisfied. The audit count is 11/11 releases with ≥ 1
  ADR; a future reader checking the criterion can do so
  by running `grep '^## \[' CHANGELOG.md`,
  `grep -l 'Release.*0.0.NN' docs/decisions/*.md`, and
  verifying every release with `### Breaking` matches at
  least one ADR's `Release:` metadata line.

- **Positive.** The 11-of-11 matrix table above is the
  audit evidence; a future supersession (e.g. a hypothetical
  ADR-NNNN that further reorganises the ADR set) can
  start from this snapshot rather than re-deriving it.

- **Positive.** The supersession mechanism's correct use
  (additive Status pointer on ADR-0013 + this new ADR
  carrying the closure) demonstrates the pattern for
  future audits. A 0.1.x graduation-readiness audit (if
  one happens) follows the same shape.

- **Negative.** ADR-0013 now reads as a snapshot
  superseded in part — a future reader of ADR-0013 has
  to follow the Status pointer to ADR-0025 to get the
  current state of criterion 5. The supersession pattern
  trades reading-time complexity for write-time
  immutability; that's the README §Supersession contract.

- **Negative.** The audit at 0.0.32 does not address the
  two M caveats ADR-0013 noted on criterion 4 (`env_lookup`
  bytes catalogue) and criterion 7 (3 codex M findings
  carried forward). Those remain on the audit's
  carry-forward list as before; this ADR only closes
  criterion 5.

- **Follow-up.** None scheduled. If a future ADR
  supersedes additional sections of ADR-0013 (e.g. a
  refreshed criterion-4 trust-boundary audit), the same
  supersession pattern applies: additive Status pointer
  on ADR-0013, new ADR carrying the closure.
