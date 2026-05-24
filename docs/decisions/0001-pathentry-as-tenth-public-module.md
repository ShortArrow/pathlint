# ADR-0001: PathEntry as the 10th public module

- **Status**: Accepted (partially superseded by ADR-0004 [provenance overlay] and further superseded by ADR-0008 [type split])
- **Date**: 2026-05-10
- **Release**: 0.0.23
- **Category**: 1. Public API surface (and 4. Trust / security — registry decoding boundary)

## Context

Before 0.0.23 every detector and resolver in pathlint took
`&[String]` or `&[&str]` for the PATH. The string was whatever
`path_source::read_path` happened to return — `getenv("PATH")` on
Unix, the result of `winreg::RegKey::get_value::<String, _>` on
Windows.

Two problems compounded:

1. `winreg::get_value::<String, _>` silently calls
   `ExpandEnvironmentStringsW` on `REG_EXPAND_SZ` values. A
   registry entry stored as `%LocalAppData%\Microsoft\WindowsApps`
   arrived at the detector as a fully-expanded literal
   (`C:\Users\me\AppData\Local\Microsoft\WindowsApps`).
2. The `Shortenable` detector skipped any entry containing `%` or
   `$` to avoid suggesting `%LocalAppData%` for an entry the user
   already wrote in that form. Combined with (1), Windows users got
   "shorten this to `%LocalAppData%\...`" on entries they had
   already shortened in `regedit`.

Each detector also had its own ad hoc decision about whether to
read the raw form or the expanded form. `Missing` needed the
expanded form (the filesystem doesn't know what `%LocalAppData%`
means). `Shortenable` needed the raw form. Without a shared type
this stayed implicit and brittle.

## Decision

Introduce a 10th public module `pathlint::path_entry` with a
single carrier type:

```rust
pub struct PathEntry {
    pub raw: String,
    pub expanded: String,
}
```

`path_source::read_path` builds a `Vec<PathEntry>` at the
boundary. Every consumer downstream picks `raw` or `expanded`
from the type and never has to ask "is this already expanded?" at
runtime. `analyze`, `sort_path`, `resolve`, `doctor_line`, and
`doctor_conflict` all change signature to `&[PathEntry]`.

The change ships as a 0.0.23 BREAKING release. The 0.0.x pre-1.0
convention (see ADR-0005) allows MAJOR-equivalent breaks within
the 0.0.x line.

## Alternatives considered

- **Detector-by-detector raw / expanded selection (the status
  quo).** Each detector keeps reading `&[String]` and does its
  own expansion. Rejected because the Windows registry expansion
  happens upstream of every detector — there is nothing a single
  detector can do to recover the raw form after `winreg::get_value`
  has already strip it. Fixing each detector in turn would also
  duplicate the boundary logic everywhere.
- **Lazy expansion at point-of-use.** Pass `&[String]` (raw) and
  call `expand_env` inside the detectors that need expanded. Rejected
  because the filesystem-side detectors (`Missing`,
  `WriteablePathDir`) all need the expanded form, so we would call
  `expand_env` for the same entry many times per `analyze` run.
  Computing it once at the boundary is the same cost as computing
  it on first use and avoids the cache-or-recompute question.
- **A wrapper type that hides `raw` / `expanded` behind methods.**
  `entry.raw()` / `entry.expanded()` instead of `entry.raw` /
  `entry.expanded`. Rejected for 0.0.23 because the fields are
  small Strings and accessor methods don't add safety — both forms
  are public anyway. The 9 existing public modules expose data via
  fields (e.g. `Diagnostic { entry, severity, kind }`); a new module
  using accessors would be the odd one out.

## Consequences

- **Positive.** Windows `--target user` / `--target machine`
  doctor output stops mis-suggesting shortening. Detectors now
  declare their intent (raw vs expanded) in the type system rather
  than in scattered `expand::expand_env` calls.
- **Positive.** The 10-module surface is small enough that
  `tests/public_api.rs` can pin all of it. `PathEntry` joins
  `Diagnostic`, `Config`, etc. as a typed boundary value.
- **Negative.** Every consumer of the PATH had to change
  signature in one release. The shift was large but the migration
  is mechanical (`&[String]` → `&[PathEntry]`, `path` →
  `entry.raw` or `entry.expanded`).
- **Negative.** `path_source::read_path` becomes the single
  authoritative place that decides what `expanded` means — if its
  env oracle is wrong, every detector reads the same wrong
  expanded value. ADR-0002 partially addresses this by making the
  env oracle injectable through `PathEntry::from_raw`.
- **Follow-up.** ADR-0004 added `provenance_raw: Option<String>`
  as a third field to handle the `--target process` Windows case.
  That decision keeps the carrier shape simple at the cost of
  conflating "single-source observation" with "cross-source
  reconstruction hint". A future ADR will likely split provenance
  into its own type before 0.1.0 graduates.
