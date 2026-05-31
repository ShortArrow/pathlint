# ADR-0012: defer schemars 1.0 migration past 0.0.x graduation

- **Status**: Accepted
- **Date**: 2026-05-31
- **Release**: 0.0.31
- **Category**: 6. External dependency

## Context

pathlint pins `schemars = "0.8"` in `Cargo.toml`. Five binaries
under `src/bin/` (`gen_schema`, `gen_check_schema`,
`gen_doctor_schema`, `gen_trace_schema`, `gen_sort_schema`)
call `schemars::schema_for!` on the live types
(`pathlint::config::Config`, `pathlint::lint::CheckOutcomeView`,
`pathlint::doctor::Diagnostic`, `pathlint::trace::TraceJsonOutput`,
`pathlint::sort::SortPlan`) and mutate
`schema.schema.metadata` to inject the canonical `$id`. Five
drift-gate tests under `tests/` re-run those generators on every
CI run and `assert_eq!` against the checked-in
`schemas/*.schema.json` files. `release.yml` regenerates the
schemas from the tagged commit and ships them as GitHub Release
assets at stable URLs.

Five domain types carry `#[derive(JsonSchema)]` plus
`#[schemars(...)]` attributes (descriptions, discriminator
renames). The `Cargo.toml` dependency is on
`schemars = "0.8"` with `serde` for the derive macro.

PRD §3.1's **graduation criterion 3** says:

> Schemars 1.0 migration evaluated. Either migrated, or an ADR
> rejects the migration for 0.1.0 with a written reason.

`schemars 1.0.0` shipped (current latest at 0.0.31 cut is
`schemars 1.2.1`). The criterion forces an explicit decision
rather than letting 0.0.x graduate with an unevaluated dependency.

The 0.8 → 1.0 release notes ([upstream](https://github.com/GREsau/schemars))
list several BREAKING changes that affect every pathlint
consumer:

- The output `Schema` type is reshaped — `schema.schema.metadata`
  on 0.8 no longer exists as a typed path on 1.x; metadata
  injection has to go through 1.x's new builder API. Every
  `src/bin/gen_*_schema.rs` binary needs the metadata-injection
  block rewritten.
- The `#[derive(JsonSchema)]` macro's attribute syntax has
  several incompatibilities (`#[schemars(rename_all = "snake_case")]`
  still works, but several rarer attributes — `default`, `with`,
  `bound` — moved or were renamed). Every derive site needs
  an audit.
- The generated JSON Schema output changes subtly: 1.x emits
  draft 2020-12 by default where 0.8 emitted draft-07, the
  `definitions` key becomes `$defs`, and some shapes (most
  notably tagged enums) get reorganised. All five checked-in
  `schemas/*.schema.json` files would have to be regenerated
  byte-for-byte; downstream consumers (anyone validating
  pathlint's JSON output against a pinned schema) would have to
  re-pin to the 1.x form.
- The `serde_json::to_string_pretty(&schema)` pretty-print path
  changes ordering in ways that are not deterministic across
  schemars 0.8 ↔ 1.x without extra effort.

The work scope is real: every schema binary (5 files), every
derive site (5 types across `config` / `lint` / `doctor` /
`trace` / `sort`), every drift-gate test (5 tests, all will
fail until the schemas are regenerated), and every published
schema asset (5 URLs that downstream tooling pins). Net result
is a BREAKING release for schema-pinning consumers even though
the Rust API itself does not change.

## Decision

**Defer the schemars 1.0 migration past 0.0.x graduation**.
Continue pinning `schemars = "0.8"` for the current Step 5c
release and any 0.1.0 / 0.1.x window that follows.

This ADR is the "ADR rejects the migration for 0.1.0 with a
written reason" form that PRD §3.1 criterion 3 allows. The
record stays in force until a future ADR supersedes it.

Trigger for revisiting:

- A 0.8 security advisory or unfixed soundness bug — pathlint
  would have to upgrade regardless of the scope cost.
- A consumer-driven request for draft 2020-12 features that
  0.8 cannot express (the current schemas use `$ref`,
  `oneOf`, and `discriminator` which all work in draft-07).
- A pathlint feature that needs schemars 1.x machinery
  (the `#[schemars(transform = ...)]` macro 1.x added is
  the most plausible trigger — none of the current 5 schemas
  need it).
- A 0.2.x window with deliberate scope for dependency
  refreshes, where the schema-pinning consumers can be given
  a migration runway through CHANGELOG.

When any of those triggers fire, the migration ADR will:
- Audit every `schemars::` / `#[schemars(...)]` site and list
  the per-file diff.
- Regenerate all five `schemas/*.schema.json` files and
  CHANGELOG them under `### Breaking` for schema-pinning
  consumers.
- Bump `Cargo.toml` to a `schemars = "1"` (or the latest
  available) constraint and refresh the lockfile.

## Alternatives considered

- **A. Migrate to schemars 1.x in 0.0.31 (this release).**
  Rejected because 0.0.31 is the closing additive-only release
  of Step 5b/c, intended to record graduation-criteria
  satisfaction without changing source-level behaviour.
  Migrating in this release would invalidate the additive-only
  premise that criterion 1 is counting (0.0.29 / 0.0.30 / 0.0.31
  as three consecutive additive releases) and would force a
  4th additive cycle to re-establish the streak.

- **B. Migrate in a 0.0.32 BREAKING release before 0.1.0.**
  Rejected because it adds one more 0.0.x release with no
  user-visible feature work, restarts the criterion 1 counter
  yet again, and bundles a dependency BREAKING into the same
  pre-graduation window that this Step 5 plan was built to
  close. The cost is real (5 binaries + 5 derives + 5 schemas
  + 5 tests) without offsetting upside.

- **C. Migrate as part of 0.1.0 itself.**
  Rejected because 0.1.0 (when and if user decides to cut it)
  should retire the pre-1.0 BREAKING licence (ADR-0005) and
  start standard SemVer. Bundling a dependency BREAKING into
  that cut means the 0.1.0 release notes carry both "graduation
  to stable" *and* "schema consumers must re-pin" in the same
  entry, conflating two unrelated stories for downstream
  users.

- **D. Pin `schemars = "0.8"` permanently and never migrate.**
  Rejected because schemars 0.8 will eventually stop receiving
  upstream attention; the long-term maintenance cost of an
  abandoned dependency exceeds the one-time cost of a migration
  ADR. The deferral here is "until a 0.2.x dependency-refresh
  window", not "forever".

- **E. Use a different JSON Schema crate (e.g. `apistos-schemars`,
  `jsonschema-tree`, hand-written schemas).**
  Rejected. Hand-written schemas drift from the live types and
  would invalidate the drift-gate design. The available
  alternatives (`apistos-schemars`, `vld-schemars`) are
  themselves built on schemars and would inherit the same 0.8
  / 1.x decision; none offer a meaningfully different API
  surface. The drift-gate-against-live-types architecture is
  load-bearing and pathlint is staying with schemars; only the
  version pin is in scope here.

## Consequences

- **Positive.** Graduation criterion 3 is satisfied by this
  ADR's existence. The criterion check ("Either migrated, or
  an ADR rejects the migration for 0.1.0 with a written reason")
  is mechanical: this file is the written reason.

- **Positive.** 0.0.31 stays additive-only, preserving the
  criterion 1 streak (0.0.29 / 0.0.30 / 0.0.31 = three
  consecutive `### Breaking`-free releases).

- **Positive.** The deferral cost is recorded explicitly (5 files
  / 5 derives / 5 schemas / 5 tests + downstream re-pin) so the
  successor ADR can quote it without re-deriving the scope.

- **Negative.** schemars 0.8 will accumulate dependency-staleness
  pressure over time; the longer the deferral, the larger the
  diff at migration time. The 0.2.x trigger is the planned
  off-ramp, but if no 0.2.x release happens for >12 months, the
  staleness becomes its own technical debt.

- **Negative.** Schema consumers pinning to pathlint's published
  schemas keep getting draft-07 output. Anyone needing
  2020-12-only features (most current tooling supports both) has
  to wait for the migration or run their own schemars 1.x
  generator from pathlint's source.

- **Follow-up.** None scheduled. The trigger conditions above
  are the watch-list. When one fires, the migration ADR
  supersedes this one per [README §Supersession](README.md#supersession).
