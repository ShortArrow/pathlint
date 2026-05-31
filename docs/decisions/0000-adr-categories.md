# ADR-0000: ADR categories and when to write one

- **Status**: Accepted
- **Date**: 2026-05-23
- **Release**: 0.0.25
- **Category**: 8. Process / governance (self-referential — this ADR itself records a process decision)

## Context

ADRs 0001-0005 landed before this ADR. They were written
retroactively from commit messages, CHANGELOG entries, plan
files, and codex review output (see ADR-0005 Consequences for
the policy that drove that work). At the time of writing them
there was no shared definition of *what kind of decision earns
an ADR*, so the first batch was selected by gut feel.

This is fine for a one-off retrospective but breaks down for
ongoing work: future contributors looking at the same code change
will not agree on whether to write an ADR. Worse, the
graduation criterion in [PRD §3.1](../PRD.md#31-graduation-to-010)
("every Breaking entry naming a public symbol has at least one
ADR linked from it") is mechanical only if "ADR-worthy decision"
is itself well-defined.

The ADR literature (Nygard 2011; Olaf Zimmermann's *Sustainable
Architectural Decisions* paper, 2014; ThoughtWorks Tech Radar
notes on ADR adoption) converges on a small handful of decision
categories. Naming them, ordering them by how often they bite,
and pinning the application criteria gives every future PR
author a one-line answer to "should this be an ADR?".

## Decision

pathlint recognises **eight ADR categories**, ordered by how
load-bearing each is for this specific project. Each numbered
ADR carries a `Category: N. <name>` metadata line so the
[index](README.md) can sort by category or by time.

### Category list (in importance order for pathlint)

1. **Public API surface** — decisions that add, remove, rename,
   or change the signature of a publicly visible type / function
   / module on the surface pinned by `tests/public_api.rs`.
   Includes additive (BREAKING-free) additions to `pub` symbols
   when those symbols are intended to outlive the next release.
   *pathlint examples*: `PathEntry` introduced (ADR-0001),
   `from_raw` closure injection (ADR-0002), `provenance_raw` /
   `effective_raw_for_user_intent` (ADR-0004).

2. **Module boundary / dependency direction** — decisions that
   move responsibility across layers (infrastructure / domain /
   presentation), introduce or remove a module, or shift what
   `pub(crate)` vs `#[doc(hidden)] pub` vs `pub` means for an
   item. Includes adopting a new architectural pattern (DI bag,
   service trait, builder) that propagates.
   *pathlint examples*: planned `AnalyzeDeps` introduction
   (Step 3 of the 0.0.25-0.1.0 roadmap), planned `Attribution`
   split (Step 4).

3. **Cross-cutting concern** — decisions about how a single
   policy is implemented everywhere it applies: env injection,
   logging, error handling, normalisation rules. The decision
   reaches multiple modules but does not change any single
   module's external shape.
   *pathlint examples*: env injection via closure
   (ADR-0002 — also category 1 because the constructor signature
   changed); planned uniform `_with` family across `resolve` /
   `source_match` / `expand_and_normalize` (Step 2).

4. **Trust / security boundary** — decisions about what input
   is treated as untrusted, where it is sanitised, and what the
   non-goals of the security model are. Always co-edits
   `docs/SECURITY.md` and usually links to a specific sanitiser
   function in code.
   *pathlint examples*: registry `REG_EXPAND_SZ` lossy decode
   policy (ADR-0003); `strip_control_chars` reach on every
   human renderer (no dedicated ADR yet — implicit in 0.0.11).

5. **Architectural style** — decisions that commit the project
   to a coarse-grained stance: read-only vs read-write, single
   binary vs daemon, batch vs streaming, what subcommands the
   CLI exposes. These rarely change but when they do every
   downstream design is affected.
   *pathlint examples*: read-only stance documented in PRD §4
   (no dedicated ADR yet — implicit since 0.0.1).

6. **External dependency** — adopting, dropping, or pinning a
   non-trivial crate, especially when the dependency's
   versioning policy interacts with pathlint's own (schemars,
   winreg, clap, serde, …). Includes choosing between two
   crates that solve the same problem.
   *pathlint examples*: planned schemars 1.0 evaluation (Step 5
   T.B.D.); `winreg` adopted in 0.0.x (no dedicated ADR — was
   the only realistic option for HKLM/HKCU access in Rust at
   the time).

7. **Persistence / data format** — decisions about TOML schema
   shape, JSON wire format on `*.schema.json`, on-disk catalog
   layout, or any other format pathlint produces or consumes
   that has external readers.
   *pathlint examples*: JSON schema discriminator field renamed
   from `status` to `kind` in 0.0.15 (no dedicated ADR — covered
   by the CHANGELOG Breaking entry; would deserve one going
   forward).

8. **Process / governance** — versioning policy (pre-1.0
   BREAKING licence, graduation criteria), release cadence,
   deprecation runway, ADR system itself.
   *pathlint examples*: pre-1.0 BREAKING-allowed policy
   (ADR-0005); ADR categories and application criteria (this
   ADR, ADR-0000); release workflow's bump-skip behaviour
   (PR #22 — no dedicated ADR yet, would deserve one).

### When to write an ADR (positive criteria)

A code change earns an ADR when **any** of the following holds:

- **PA1**: it introduces, removes, or changes the signature of a
  `pub` symbol on a module exported from `src/lib.rs`. (Category 1.)
- **PA2**: it moves a responsibility across the
  `path_source` (infrastructure) / `doctor` `lint` `sort` `trace`
  (domain) / `format` (presentation) split, or introduces a new
  cross-cutting type (DI bag, view model, attribution carrier).
  (Category 2.)
- **PA3**: it changes how a single cross-cutting policy is
  applied — e.g. "now every caller passes an env closure", "now
  every renderer goes through `strip_control_chars`". (Category 3.)
- **PA4**: it changes what pathlint treats as untrusted, what
  it sanitises, or where the sanitisation happens; or it adds /
  removes a security non-goal. Always paired with a
  `docs/SECURITY.md` edit. (Category 4.)
- **PA5**: it changes the project's stance on what pathlint
  *does* — adds a subcommand, retires one, flips the read-only
  invariant, broadens to a new input source class (launchd /
  systemd / brew shellenv). (Category 5.)
- **PA6**: it adopts a new crate dependency that has its own
  semver story, or pins / un-pins an existing one in a way that
  affects pathlint's compatibility window. (Category 6.)
- **PA7**: it changes the shape of a `pathlint.toml` field, a
  `*.schema.json` field, or any other format pathlint reads or
  emits to an external consumer. (Category 7.)
- **PA8**: it changes the versioning policy, the release process,
  the deprecation runway length, or the ADR system itself.
  (Category 8.)

### When NOT to write an ADR (negative criteria)

A code change does **not** need an ADR when **all** of the
following hold:

- **NA1**: the change is internal to a single module (no `pub`
  surface motion).
- **NA2**: the change has no impact on a CHANGELOG `### Breaking`
  entry.
- **NA3**: the change is self-explanatory from its commit message
  (rename of a local variable, typo fix, comment update).
- **NA4**: the change does not commit pathlint to a stance that
  a future contributor would want to revisit. Most bug fixes
  qualify — the fix is the answer, the context lives in the
  commit message, and there is no rejected alternative worth
  preserving.

Concrete examples of "no ADR needed":

- **Bug fixes** whose entire story fits in the commit message
  (a fix in `format::quote_for` for an edge case that was
  always intended to be quoted; a `Result<...>` propagation that
  was previously `.unwrap()`).
- **Refactors** that keep public surface identical (extracting a
  private helper, splitting a long function).
- **Docs typo fixes / drift fixes** that do not change a policy
  (e.g. ADR-0001 mentioning "9 modules" instead of "10" — that
  was a drift fix in 0.0.25, no ADR was needed for the fix
  itself; the categories were defined here, not because the
  drift fix demanded a new ADR).
- **Test additions** that pin existing behaviour without
  changing what is being pinned.
- **CI changes** with no downstream visibility (a workflow
  step's ordering, a cache key tweak, a lint update). Note: a
  CI change that *modifies the release contract* (PR #22's
  bump-skip behaviour) crosses into Category 8 and would now
  deserve an ADR going forward.
- **Dependency patch / minor bumps** that the dependency
  itself promises are non-breaking. A *major* bump or a
  pin-change goes through Category 6.

When in doubt, lean toward writing the ADR. The cost is half an
hour; the benefit accrues every time someone asks "why did we
decide this?".

## Alternatives considered

- **A. No category system; rely on individual judgement.** The
  default before this ADR. Rejected because it produced no
  signal during PR review — reviewers cannot point to "this
  category was missed". Without categories the graduation
  criterion ("every public-symbol BREAKING has an ADR") is also
  ambiguous: which BREAKINGs count?
- **B. Category by code area (one category per top-level module).**
  Rejected because real decisions cross module boundaries (env
  injection touches `expand`, `path_entry`, `resolve`, `source_match`
  in one go). Category-by-code would either duplicate one ADR
  across 4 categories or pick an arbitrary "primary" module.
- **C. Borrow Y-statement format (Olaf Zimmermann's structured
  "in the context of X, facing Y, we decided Z to achieve W,
  accepting Q") instead of Nygard.** Rejected for pathlint because
  the Nygard format already maps onto what the existing 5 ADRs
  do (Context / Decision / Alternatives / Consequences) and a
  format change would force rewriting them. The Y-statement
  format is excellent for compressing one decision into one line
  in a roadmap; pathlint can adopt it later as a *summary* layer
  on top of Nygard ADRs if the index gets too long.
- **D. Treat the codex review output as the implicit ADR set.**
  Rejected because codex review is run on demand, not on every
  PR; its output lives in temp files; and its formatting is
  optimised for monologue critique, not for `git log`-style
  retrieval. ADRs are the durable record; codex review is the
  source that *triggers* writing them.

## Consequences

- **Positive.** Every future PR has a one-line decision: which
  of PA1-PA8 applies, or none? If none, no ADR is needed. If
  one or more, the ADR slot must be filled before the PR can be
  merged.
- **Positive.** The README index ([docs/decisions/README.md](README.md))
  can sort by category to give a topical view ("show me every
  cross-cutting decision") and by number to give a timeline
  view ("show me what changed in 0.0.23").
- **Positive.** The graduation criterion in PRD §3.1 becomes
  precisely countable: a CHANGELOG Breaking entry that names a
  public symbol is automatically PA1 (Category 1) and requires
  an ADR with that link. The "naming a public symbol" filter is
  resolved here.
- **Negative.** Backfilling categories onto ADR-0001 through
  ADR-0005 is one-time work; ADR-0001 in particular spans
  Category 1 and 4 (it both introduced a public type and changed
  what counts as a sanitisation point because Windows registry
  decoding now has a documented boundary). Multi-category ADRs
  are allowed but the *primary* category drives the index sort.
- **Negative.** Category 5 (architectural style) is loosely
  defined — `pathlint sort --apply` would clearly qualify, but
  smaller stance changes might fall through the cracks. The
  ambiguity is deliberate: forcing every stance-flavoured
  decision into a tight definition would over-fit on today's
  list. Re-tighten in a future ADR if it becomes a problem.
- **Follow-up.** ADRs 0001-0005 must be amended (additive
  edit) to add the `Category:` metadata line. The README must
  be reorganised to support category-sorted view. Both are part
  of the same 0.0.25 PR that ships this ADR.

## Known ADR backlog

While defining ADR-0000 and backfilling ADRs 0001-0005, the
following past decisions were identified as ADR-worthy under the
PA1-PA8 criteria but **have no ADR yet**. They are tracked here
so the graduation criterion in [PRD §3.1 #5][grad5] can be
audited mechanically (CHANGELOG `### Breaking` entries naming
public symbols must each link an ADR by graduation time).

[grad5]: ../PRD.md#31-graduation-to-010

| Decision | Category | First shipped | Notes |
|---|---|---|---|
| Public / internal module split (10 `pub mod` + `#[doc(hidden)] pub` + `pub(crate)`) | 2 (module boundary) | 0.0.17 | The boundary was re-shuffled in 0.0.17 (`cli` / `run` moved binary-side); an ADR would record why the `#[doc(hidden)] pub` middle tier exists. |
| Compile-time catalog embed (`build.rs` + `include_str!` + `embedded_catalog.toml`) | 7 (persistence / data format) | 0.0.x baseline | Rejected runtime catalog discovery; an ADR would record the trade-off (vendor lock vs distribution-time freshness). |
| `winreg` crate adoption | 6 (external dependency) | 0.0.x baseline | Listed in ADR-0003 Context but no dedicated dependency-policy ADR. |
| `Config::from_path` DoS guards (16 MiB cap + symlink hop check) | 4 (trust / security) | 0.0.11 | Listed in SECURITY.md but no dedicated ADR; the alternatives (no cap, multi-hop allowed) deserve recording. |
| `strip_control_chars` reach on every human renderer | 4 (trust / security) | 0.0.11 | Listed in SECURITY.md; the policy ("ASCII control bytes → `?`, preserve `\t`/`\n`") would deserve an ADR that justifies that specific byte range. |
| `pathlint trace` provenance + mise plugin attribution heuristic | 5 (architectural style) | 0.0.5 | Recorded in PRD §16 Resolved; an ADR would capture the rejected design ("treat plugin segment as a real source label"). |
| JSON schema discriminator rename (`status` → `kind`) | 7 (persistence / data format) | 0.0.15 | Covered by 0.0.15 CHANGELOG Breaking; an ADR would record the cross-schema consistency motivation. |

**Drained in 0.0.30** (now have dedicated ADRs, removed from
backlog):

- Read-only stance → [ADR-0009](0009-read-only-stance.md)
- Release workflow bump-skip → [ADR-0010](0010-release-workflow-bump-skip.md)
- `expand::normalize` substring-match policy → [ADR-0011](0011-normalize-substring-match-policy.md)

The backlog is the explicit list — adding to it means writing a
note here; removing from it means writing the actual ADR. The
0.0.26+ releases of pathlint will tick these off in batches of
2-3 per release rather than all at once, so each ADR can be
reviewed properly. The expected drainage cadence is captured in
the 0.0.25-0.1.0 roadmap.

This list is not exhaustive — it is the set caught while writing
ADRs 0000-0005. If a future contributor spots another ADR-worthy
historical decision that isn't here, the right move is to add a
row first (cheap, documents the gap) and write the ADR later.
