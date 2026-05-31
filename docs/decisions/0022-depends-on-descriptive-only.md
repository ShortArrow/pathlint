# ADR-0022: `depends_on` relation is descriptive-only, no runtime effect on detectors

- **Status**: Accepted
- **Date**: 2026-05-05 (decision); recorded retroactively in 0.0.32 (2026-05-31)
- **Release**: 0.0.14
- **Category**: 5. Architectural style (relation scope policy)

## Context

The `[[relation]]` family in `plugins/*.toml` (and merged from
user `pathlint.toml`) carries several `kind` values, each with
different runtime semantics:

- `served_by_via` (a.k.a. provenance): drives `trace` provenance
  attribution and `Conflict` detector's wrapper-installer
  awareness.
- `conflicts_when_both_in_path`: drives the `Conflict` detector
  directly.
- `alias_of`: drives source-name resolution in
  `source_match::find`.
- `prefer_order`: drives `sort_path` ordering.
- `depends_on`: introduced 0.0.14 — but what does it *do*?

The 0.0.14 PR adding `depends_on` was framed as documentation:
catalog authors wanted to describe relationships like "the
`mise_shims` source depends on `mise` being on PATH for shims
to resolve". The downstream consumers had three options:

1. Make `depends_on` runtime-visible: surface a doctor warning
   when the dependent's PATH dir is present but the
   dependency's isn't.
2. Make `depends_on` runtime-visible: factor it into `sort`
   ordering (dependency comes first).
3. Keep `depends_on` descriptive: only show it in
   `pathlint catalog relations` listing; no detector reads
   it.

The 0.0.14 cut was already shipping major surface motion
(`sort` subcommand, `trace` rename, catalog source renames per
ADR-0014, JSON shape unification per ADR-0016). Adding another
runtime-visible detector would have stretched the release
further.

## Decision

`depends_on` is **descriptive-only**. It is:

- Parsed by `[[relation]] kind = "depends_on"` in catalog files.
- Validated for referential integrity by `build.rs` (the
  source and target must both resolve to defined sources —
  see ADR-0021).
- Included in `pathlint::catalog::check_acyclic`'s DAG check
  (a `depends_on` cycle is a catalog bug worth catching at
  build time).
- Listed by `pathlint catalog relations` (default human
  output + `--json` form).

It is **not** consulted by any detector, the resolver, or
`sort`. A binary that depends on another source being on
PATH but whose dependency isn't on PATH does not trigger any
diagnostic; the user discovers the issue through their actual
workflow, not through pathlint.

## Alternatives considered

- **A. Add a `MissingDependency` doctor detector that fires
  when a source's PATH dir is present but its `depends_on`
  target's dir isn't.** Rejected for 0.0.14 because:
  - The detector's correctness depends on substring matching
    against every PATH entry for both source and target, which
    is a non-trivial expansion of `source_match`'s call
    pattern.
  - The catalog at 0.0.14 had a small number of `depends_on`
    relations; the false-positive surface (a user who
    deliberately has only `mise_shims` on PATH without `mise`
    on PATH because they're running `mise activate` in a way
    that skips the installer dir) would have been larger than
    the true-positive surface.
  - The decision can be revisited later by superseding this
    ADR; pre-committing the detector in 0.0.14 would have
    been hard to undo.

- **B. Factor `depends_on` into `sort_path` ordering (dependency
  comes first).** Rejected because `prefer_order` already
  exists for this purpose; adding a parallel input would
  create two ways to express the same ordering constraint
  and force authors to choose between them. The intent of
  `depends_on` was *documentation* of a runtime relationship,
  not *prescription* of sort order.

- **C. Don't add `depends_on` at all (keep the catalog smaller).**
  Rejected because catalog authors had asked for a way to
  document non-prescriptive relationships; without
  `depends_on` the only places to record the relationship
  were `description = "..."` strings, which `pathlint catalog
  list` shows but `pathlint catalog relations --json` does
  not, and which downstream consumers cannot query
  structurally.

- **D. Make `depends_on` user-only (allow in `pathlint.toml`,
  reject in built-in plugins).** Rejected because the
  built-in catalog is exactly where the
  "well-known installer relationship" knowledge lives;
  forcing users to re-declare relationships their installer
  already has would defeat the purpose of a built-in catalog.

## Consequences

- **Positive.** The 0.0.14 cut shipped without adding another
  detector, keeping the release scope contained. New
  `depends_on` rows can be added to plugin files without
  triggering detector behavioural change (other than the
  acyclicity check), so the documentation value is realised
  immediately.

- **Positive.** `pathlint catalog relations` provides a
  queryable, machine-readable form of the relationship for
  any downstream tool that wants to act on it (an editor
  plugin showing "this source depends on X", a dotfiles
  installer that uses the dependency graph to order its
  setup steps).

- **Positive.** The DAG check is a real value: if a future
  catalog author writes `mise_shims depends_on mise` and
  `mise depends_on mise_shims`, `build.rs` fails with a
  cycle report. Catches catalog-author mistakes at the
  earliest possible point.

- **Negative.** Users reading `pathlint catalog relations`
  output might assume `depends_on` has runtime semantics
  (a reasonable inference from the name); the
  `pathlint catalog --json` shape does not say
  "descriptive-only" anywhere. Mitigated by the CHANGELOG
  0.0.14 entry stating the scope and by this ADR; the
  output itself could be argued to need an explicit note
  but no consumer has filed an issue.

- **Negative.** If a future detector wants to consume
  `depends_on` (e.g. a `Conflict`-style warning for missing
  dependencies), this ADR will need superseding. The
  partial commitment ("descriptive today, possibly
  runtime-visible later") leaves a small policy footprint
  to revisit, but the cost of the revisit is itself bounded.

- **Follow-up.** None scheduled. The 7 0.0.x-line releases
  since 0.0.14 have not surfaced detector requests against
  `depends_on`; if one arrives, a superseding ADR records
  the policy change.
