# ADR-0013: graduation criteria satisfaction record (0.0.31 snapshot)

- **Status**: Accepted (Criterion 5 section superseded by [ADR-0025](0025-criterion-5-closure.md) as of 0.0.32; other sections unchanged)
- **Date**: 2026-05-31
- **Release**: 0.0.31
- **Category**: 8. Process / governance

## Context

[PRD §3.1](../PRD.md#31-graduation-to-010) lists seven criteria
that the 0.0.x line must satisfy before 0.1.0 can be cut. The
criteria are designed to be mechanical: a future user reading
this ADR should be able to check each box without subjective
judgement.

Step 5 of the 0.0.25–0.1.0 roadmap was designed around closing
the remaining criteria one at a time across three additive-only
releases (0.0.29 / 0.0.30 / 0.0.31). This ADR is the record at
the close of Step 5c.

**This ADR records satisfaction, it does not cut a release.**
Whether the next release is 0.0.32 / 0.1.0 / something else, and
when that release ships, is **user judgement**. The plan
deliberately separates "criteria are satisfied" from "we are
cutting graduation".

## Decision

At the 0.0.31 cut, the criteria are in the following state.

### Criterion 1: Public API freeze (lib) — ✅ Satisfied

> The 10 modules listed in `tests/public_api.rs` keep their
> surfaces for ≥ 2 consecutive releases without a `### Breaking`
> entry in `CHANGELOG.md`.

The last `### Breaking` release was 0.0.28
([ADR-0008](0008-attribution-type-split.md), `Attribution`
split). Since then:

- 0.0.29 — no `### Breaking` section ([CHANGELOG](../../CHANGELOG.md#0029--2026-05-31))
- 0.0.30 — no `### Breaking` section ([CHANGELOG](../../CHANGELOG.md#0030--2026-05-31))
- 0.0.31 — no `### Breaking` section (this release)

Three consecutive additive-only releases — the criterion requires
two, this ships one of margin.

### Criterion 2: CLI surface freeze — ✅ Satisfied

> `pathlint <subcommand>` and the global flag set match the
> table in §11 for ≥ 2 consecutive releases.

Verified in 0.0.29 (CHANGELOG 0.0.29 `Fixed (docs drift)` entry
records the re-verification) and re-confirmed in 0.0.31. PRD §11
matches `src/bin/pathlint/cli.rs`: subcommands `check`, `init`,
`catalog`, `doctor`, `trace`, `sort`, `help`; global flags
`--target`, `--config`, `--verbose`, `--quiet`, `--color`,
`--no-glyphs`, `--help`, `--version`.

### Criterion 3: Schemars 1.0 migration evaluated — ✅ Satisfied

> Either migrated, or an ADR rejects the migration for 0.1.0
> with a written reason.

[ADR-0012](0012-schemars-1-0-deferred.md) records the deferral
decision, its trigger conditions for revisiting, and the four
alternatives that were rejected.

### Criterion 4: Trust model documented — ✅ Satisfied

> `docs/SECURITY.md` describes every boundary, with sanitisation
> pointers into code, and is kept in sync with the implementation.

`docs/SECURITY.md` was introduced in 0.0.25 and last refreshed
in 0.0.29 (Attribution overlay and `*Deps::env_lookup` rows
added to the trust-boundary table; new entries in the
sanitisation-pointers section for
`Attribution::effective_raw_for_user_intent` and
`CommonDeps::production`).

Caveat carried forward from the 0.0.30 codex audit re-run:
SECURITY.md describes the `CommonDeps::env_lookup` closure as
trusted in-process code, but does not catalogue every byte
source that production lookups return (`PATHEXT`, `HOME`,
`USERPROFILE`, source-path expansion targets) as untrusted
inputs in their own right. This is recorded as an M severity
finding in [CHANGELOG 0.0.30 Notes](../../CHANGELOG.md#0030--2026-05-31)
and is non-blocking for the criterion under the current
"every *boundary* is described, sanitisation pointers exist" reading.

### Criterion 5: ADR completeness — ⚠️ Partially satisfied

> Every release in the 0.0.x line whose `### Breaking` section
> in `CHANGELOG.md` names a publicly visible type or function
> has at least one ADR linked from the corresponding
> `docs/decisions/NNNN-*.md` file.

Releases with `### Breaking` sections, in order, and their ADR
status:

| Release | ADR(s) | Status |
|---|---|---|
| 0.0.14 | none | pre-ADR system |
| 0.0.15 | none | pre-ADR system |
| 0.0.16 | none | pre-ADR system |
| 0.0.17 | none | pre-ADR system |
| 0.0.19 | none | pre-ADR system |
| 0.0.21 | none | pre-ADR system |
| 0.0.22 | none | pre-ADR system |
| 0.0.23 | ADR-0001 / 0002 / 0003 | ✅ |
| 0.0.24 | ADR-0004 | ✅ |
| 0.0.27 | ADR-0007 | ✅ |
| 0.0.28 | ADR-0008 | ✅ |

The strict reading is "7 of 11 releases lack ADR links". The
contextual reading is that the ADR system itself shipped in
0.0.25 ([ADR-0000](0000-adr-categories.md)); the seven
pre-system releases predate the policy. ADR-0000's Decision
explicitly backfilled five "load-bearing past decisions"
(ADR-0001 through ADR-0005) but stopped there because the
remaining pre-system Breaking entries were judged either
covered by their CHANGELOG entry alone (rename-only changes
like 0.0.14's `where` → `trace`) or by the alias-removal
runway story (0.0.22).

Two options for the user:

1. **Accept partial satisfaction with the contextual reading.**
   ADR-0000 ratified the boundary at 0.0.23; releases before
   that point are documented by the CHANGELOG entry alone.
   This was the implicit stance throughout Step 5 — neither
   plan nor any prior ADR proposed backfilling 0.0.14–22.

2. **Backfill the pre-ADR Breaking releases.** Writing 5–7
   short ADRs for the rename / runway-removal / schema-shape
   changes shipped in 0.0.14–22. Each one would be Category 1
   or 7 (Public API / Persistence) and could fit in roughly
   30–80 lines.

This ADR does not pick between the two; that is the user's
call when deciding whether and when to cut graduation.

### Criterion 6: Documentation parity — ✅ Satisfied (with note)

> EN ↔ JP PRD diff is < 50 lines of semantic content
> (table-of-contents-only and link-only diffs excluded).

Verified during 0.0.31 prep. Both PRDs share a 1:1 section
structure (37 headings in identical order, identical numbering
through §18). Line counts: `docs/PRD.md` 1312, `docs/PRD.jp.md`
1224 — an 88-line gap accounted for entirely by formatting
density differences and JP-side prose conciseness, not by
content omissions. The 0.0.25 sweep that retired JP §17's
inline cumulative changelog brought §17 to parity with EN; no
section-level coverage gap exists.

This ADR adopts the "section-structure-and-coverage" reading
of "semantic diff" rather than a per-line diff (which would be
nonsensical between EN and JP source forms).

### Criterion 7: No open H severity codex audit findings — ✅ Satisfied

> Either resolved or downgraded with an ADR that explains why
> the H rating no longer applies.

codex 6-axis audit re-run on 2026-05-31 (recorded in
[CHANGELOG 0.0.30](../../CHANGELOG.md#0030--2026-05-31)):
**0 H findings**.

3 M findings carried forward to the next audit cycle (TDD
negative-invariant pin, FP wrapper `env_lookup` completion,
SECURITY `env_lookup`-returned-bytes catalogue). M findings do
not block this criterion under the current wording.

## Summary

| # | Criterion | State |
|---|---|---|
| 1 | Public API freeze (lib) | ✅ |
| 2 | CLI surface freeze | ✅ |
| 3 | Schemars 1.0 evaluated | ✅ (ADR-0012 defers) |
| 4 | Trust model documented | ✅ (M-finding caveat noted) |
| 5 | ADR completeness | ⚠️ partial (pre-0.0.23 releases lack ADRs) |
| 6 | EN/JP PRD parity | ✅ (1:1 section structure) |
| 7 | No open H severity findings | ✅ (codex re-run 2026-05-31) |

## Alternatives considered

- **A. Cut 0.1.0 in this ADR.** Rejected per Step 5 plan
  constraint and per user instruction
  ("0.1.0 を勝手に決めないで"): release cut is user decision,
  this ADR records satisfaction only.

- **B. Declare criterion 5 fully satisfied by reinterpreting
  "every release" as "every post-ADR-system release".**
  Rejected. The criterion's text says "Every release in the
  0.0.x line"; reinterpreting it after the fact erodes the
  mechanical-check property the criterion was designed for.
  Leaving the partial state visible is more honest.

- **C. Backfill 5–7 ADRs for 0.0.14–22 within 0.0.31.**
  Rejected as out of scope for Step 5c (this release is for
  graduation criteria recording, not for further ADR
  drainage). The backfill remains a user-decided option in the
  Criterion 5 section above.

- **D. Defer this ADR until criterion 5 is fully clean.**
  Rejected because the other six criteria are satisfied today
  and recording the state now is more useful than waiting for
  a hypothetical "perfect" snapshot. Future audit cycles can
  supersede this ADR if state changes.

## Consequences

- **Positive.** Six of seven criteria are recorded as
  satisfied with verifiable pointers (CHANGELOG entries, ADR
  files, codex re-run notes). A user deciding to cut
  graduation can quote this ADR rather than re-deriving the
  audit.

- **Positive.** The one partial criterion (5) is explicitly
  bounded and offers two unambiguous options the user can
  choose between. No hidden state, no implicit reinterpretation.

- **Positive.** The plan-vs-cut separation is preserved:
  Step 5 is now mechanically complete (criteria are recorded);
  whatever the user does next (0.0.32 backfill; 0.1.0 cut;
  longer additive runway) starts from this clean record.

- **Negative.** Criterion 5's partial state means a strict
  reading of PRD §3.1 cannot pass without further work. A
  future supersession ADR (after the user picks option 1 or 2)
  will close this out.

- **Negative.** The two M caveats (criterion 4's
  `env_lookup`-bytes catalogue gap, criterion 7's 3 carried-
  forward M findings) are non-blocking today but are visible
  future work. They are not graduation blockers under the
  current criterion wording but a user opting to harden the
  trust model further could address them in a 0.1.x release.

- **Follow-up.** Whatever the user decides for criterion 5
  (option 1 acceptance or option 2 backfill) results in either
  a successor ADR-NNNN that closes this one out, or no action
  at all if the partial state is taken as-is. This ADR's body
  stays intact regardless, per
  [README §Supersession](README.md#supersession).
