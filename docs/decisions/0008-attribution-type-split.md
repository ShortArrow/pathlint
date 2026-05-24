# ADR-0008: split `Attribution` out of `PathEntry` (PathEntry restored to raw/expanded purity)

- **Status**: Accepted
- **Date**: 2026-05-24
- **Release**: 0.0.28
- **Category**: 1. Public API surface — also touches 2. Module boundary / dependency direction

## Context

ADR-0001 and ADR-0004 both end with the same Follow-up note: the
0.0.24 decision to put `provenance_raw: Option<String>` on
`PathEntry` was a deliberate shape-simplification compromise, and
the price was that one type had to answer two questions:

- "what does this single source say about the entry?"
  (`raw` / `expanded`)
- "what does a different source say about the entry?"
  (`provenance_raw`)

Quoting ADR-0001 §Follow-up:

> A future ADR will likely split provenance into its own type
> before 0.1.0 graduates.

Quoting ADR-0004 §Follow-up:

> Step 4 of the 0.0.25-0.1.0 roadmap revisits this by splitting
> `PathEntry` into `PathEntry { raw, expanded }` and a new
> `Attribution { observed: PathEntry, provenance_raw }` carrier,
> restoring `PathEntry`'s concept purity at the cost of one more
> BREAKING.

This ADR is that Step 4. It also happens to be the last
intentionally BREAKING release of the 0.0.x line — Step 5 of the
roadmap is additive-only, and the criterion 1 counter
("≥ 2 consecutive releases without `### Breaking`") starts over
from here.

The cost of waiting for "the right moment" is real: ADR-0001 and
ADR-0004 used PathEntry's `effective_raw_for_user_intent()` as a
free-floating accessor that depends on `provenance_raw` being on
the same struct. Every detector that calls it knows about
`provenance_raw` by accident. Step 4 untangles that.

## Decision

Split the type. PathEntry is back to a two-field observation:

```rust
// src/path_entry.rs
pub struct PathEntry {
    pub raw: String,
    pub expanded: String,
}

impl PathEntry {
    pub fn from_raw<V>(raw: impl Into<String>, env_lookup: V) -> Self
    where V: Fn(&str) -> Option<String> { /* unchanged */ }
}
```

`Attribution` lives at the crate root (next to `CommonDeps`, for
the same "lib-cross-cutting carrier" reason):

```rust
// src/lib.rs
pub struct Attribution {
    pub observed: PathEntry,
    pub provenance_raw: Option<String>,
}

impl Attribution {
    pub fn new(observed: PathEntry) -> Self { /* provenance_raw: None */ }
    pub fn with_provenance(self, registry_raw: String) -> Self { /* … */ }
    pub fn effective_raw_for_user_intent(&self) -> &str { /* … */ }
}
```

Every public entry-list parameter switches from `&[PathEntry]` to
`&[Attribution]`. `path_source::read_path` returns
`Vec<Attribution>`; `reconcile_process_with_registry` operates on
`&[Attribution]`. Detectors, the resolver, and the formatter all
read `attrib.observed.raw`, `attrib.observed.expanded`, or
`attrib.effective_raw_for_user_intent()` depending on which side
of the user-intent / filesystem-shape boundary they sit on.

## Alternatives considered

- **A. Status quo — leave `provenance_raw` on `PathEntry`.**
  Rejected because ADR-0001 and ADR-0004 already committed to
  fixing this before 0.1.0 graduates, and 0.0.28 is the last
  intentionally BREAKING release in the roadmap. Skipping it
  would freeze the concept conflation into the 0.1.0 public API.

- **B. Add `Attribution` but keep `provenance_raw` on `PathEntry`
  too.** Rejected as ad-hoc: two locations for the same concept,
  with no rule about which one a caller is supposed to consult.
  Detector code that uses `entry.observed.provenance_raw` and
  detector code that uses `entry.provenance_raw` would coexist,
  and future readers would have to grep both call sites to be
  sure what's authoritative.

- **C. Keep `Attribution` as an internal type that
  `path_source::reconcile_process_with_registry` returns, but
  expose only `Vec<PathEntry>` to detectors.** Rejected because
  the moment you flatten back to `PathEntry`, you have to either
  drop `provenance_raw` (regressing 0.0.24's Windows process-
  target fix) or put it back on `PathEntry` (regressing this
  ADR's whole point). The detector boundary is exactly where
  `effective_raw_for_user_intent()` needs to live, so the
  Attribution layer has to reach it.

- **D. Split `Attribution` and propagate it through every public
  entry-list parameter (chosen).** Detector signatures change,
  but each detector states its intent through the field it reads
  (`observed.raw`, `observed.expanded`,
  `effective_raw_for_user_intent`), and PathEntry stays pure.

## Consequences

- **Positive.** PathEntry has one concept per type again: a
  single-source observation. The 0.0.23 design intent
  ("`PathEntry` lives at the path_source boundary, every consumer
  picks its side from the type") is restored without losing the
  0.0.24 Windows provenance overlay.

- **Positive.** ADR-0001 and ADR-0004 Follow-up notes are
  closed. The graduation criterion 5 audit ("every release in
  0.0.x with a Breaking entry naming a public symbol has at
  least one ADR linked") gets an explicit closing entry for both
  ADRs instead of a dangling forward-pointer.

- **Positive.** The crate root now hosts two cross-cutting
  carriers (`CommonDeps` and `Attribution`). Both follow the
  same "lib-wide concept, not owned by any single module"
  rule from ADR-0007. The pattern is reusable: a future
  cross-cutting carrier (e.g. a logging hook context) belongs
  at the crate root next to these two.

- **Negative.** Criterion 1's counter resets again. Step 5 of
  the roadmap was already planned as additive-only to rebuild
  the 2-consecutive-release streak, but this ADR adds urgency:
  the next two releases need to land with no `### Breaking`
  section to put 0.1.0 within reach.

- **Negative.** Caller migration is mechanical but touches
  every detector test fixture, the binary's `read_path_entries`
  helper, and external embedders. The migration is one
  `PathEntry { raw, expanded, provenance_raw }` literal → one
  `Attribution::new(PathEntry::from_raw(…))` call. The Rust
  compiler catches every missing site.

- **Negative.** The `Attribution` indirection adds one field
  access per detector read of `observed.raw` /
  `observed.expanded`. In a code path dominated by filesystem
  I/O the cost is unmeasurable, but the source-level noise is
  real and reviewers should expect it.

- **Follow-up.** Step 5 (the additive-only release window) is
  the next item on the 0.0.25-0.1.0 roadmap. Its contents are
  the criterion-3 schemars 1.0 evaluation, criterion-4
  SECURITY.md refresh, ADR backlog drainage, and EN/JP PRD
  parity pass. Nothing in this ADR forecloses any of those.

- **Follow-up.** ADR-0001's Status moves to
  "Accepted (partially superseded by ADR-0004 for provenance
  overlay, further superseded by ADR-0008 for type split)" —
  recorded as an additive Status update, not a body edit.
  ADR-0004's Status gains
  "Accepted (superseded by ADR-0008 for PathEntry concept
  purity)". Both follow the supersession rule in
  [docs/decisions/README.md](README.md#supersession): never
  edit the body of a superseded ADR.
