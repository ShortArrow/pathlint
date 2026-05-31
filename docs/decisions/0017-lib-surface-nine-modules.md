# ADR-0017: lib surface narrowed to 9 supported `pub mod` plus a `#[doc(hidden)] pub` middle tier

- **Status**: Accepted (later extended additively to 10 modules in 0.0.23 — see ADR-0001)
- **Date**: 2026-05-05 (decision); recorded retroactively in 0.0.32 (2026-05-31)
- **Release**: 0.0.15 (initial narrowing to 9 modules); 0.0.17 (`cli` / `run` move out + `shell_quote` privatised + final `#[doc(hidden)] pub` middle tier)
- **Category**: 1. Public API surface (also touches 2. Module boundary / dependency direction and 8. Process / governance for the boundary policy)

## Context

Pre-0.0.15 pathlint's `src/lib.rs` re-exported essentially
everything: `cli`, `run`, `format`, `report`, `init`,
`path_source`, `resolve`, `catalog_view`, `shell_quote` were
all `pub mod` alongside the actual domain modules (`config`,
`lint`, `trace`, `sort`, `doctor`, `catalog`, `source_match`,
`os_detect`, `expand`). Embedders had no signal which of the
~18 modules they could rely on across releases.

Two problems compounded:

1. **No supported-surface line.** A downstream user
   `use pathlint::format::strip_control_chars` had the same
   visibility as `use pathlint::config::Config` — both
   compiled, both worked. But pathlint's design intent
   treated `format` as a presentation-layer helper that
   could be reshaped any time, while `config` was a stable
   schema. Without a `pub` vs not-`pub` distinction at the
   surface, every reshape risked breaking embedders who
   reached into modules they were never meant to use.

2. **The binary needs to reach lib internals.** pathlint
   ships as a binary (`src/bin/pathlint/main.rs`) and a
   library. Cargo treats the binary as a separate crate;
   `src/bin/pathlint/run.rs` calls `pathlint::format::*`,
   `pathlint::report::*`, `pathlint::path_source::*` etc
   to do its work. Pure `pub(crate)` on those modules
   would block the binary from compiling.

The 0.0.15 cut narrowed the surface to nine `pub mod`
entries that constituted the supported API; the 0.0.17 cut
finished the job by moving `cli` and `run` out of the lib
entirely (into `src/bin/pathlint/`) and adding a
`#[doc(hidden)] pub` middle tier for the modules the binary
genuinely needs to call across the crate boundary.

(0.0.23 later added `path_entry` as a tenth supported
module — see ADR-0001; the 0.0.15-17 baseline was nine.)

## Decision

The lib surface has three tiers:

1. **Supported `pub mod`** (9 in 0.0.15-22, 10 in 0.0.23+):
   `config`, `lint`, `trace`, `sort`, `doctor`, `catalog`,
   `source_match`, `os_detect`, `expand` (+`path_entry`
   from 0.0.23). These are the docs.rs-visible surface.
   `tests/public_api.rs` pins each by import + callability
   so removing or renaming a listed symbol fails CI.

2. **`#[doc(hidden)] pub`** (the middle tier introduced in
   0.0.17): `catalog_view`, `format`, `init`,
   `path_source`, `report`, `resolve`. These are reachable
   from `src/bin/pathlint/` because Cargo treats the
   binary as a separate crate. They are **intentionally
   not** re-exported on docs.rs and are not part of the
   supported lib API surface. Embedders that reach into
   them get no docs.rs documentation and get a CHANGELOG
   `### Breaking` entry with no migration support if the
   reshape breaks them.

3. **`pub(crate)`** (everything else, e.g. `shell_quote`):
   strictly internal. Not reachable from the binary, not
   visible to embedders, not part of any contract.

Additional 0.0.17 motions:

- `pathlint::cli` and `pathlint::run` removed from the lib
  entirely. They lived as `#[doc(hidden)] pub mod` for the
  binary at `src/main.rs` (pre-0.0.17 the binary lived in
  `src/main.rs`); the 0.0.17 reshape moved the binary into
  `src/bin/pathlint/` with its own `cli.rs` and `run.rs`,
  so the lib-side `cli` / `run` had no caller.

- `format::quote_for` (POSIX / PowerShell single-quote
  helpers) moved into `shell_quote` and was demoted from
  `pub` to `pub(crate)`. Use in user-facing trace uninstall
  hints stays the same; embedders that wanted to quote
  shell strings should read the already-quoted output from
  `trace --json uninstall.command` rather than calling the
  helper directly.

## Alternatives considered

- **A. Keep everything `pub` (status quo).** Rejected
  because the supported-surface line is the point: every
  reshape of `format`, `report`, `path_source` etc would
  count as a BREAKING release for downstream users, and
  pathlint's CHANGELOG would balloon. The narrow surface
  lets the lib evolve internals freely while pinning the
  surface explicitly.

- **B. Make every internal module `pub(crate)`, build
  the binary as a separate crate that depends on
  `pathlint` and a `pathlint-internals` crate.** Rejected
  because Cargo workspaces add maintenance overhead
  (workspace `Cargo.toml`, dependency wiring, two
  publishing flows) for what is essentially a one-binary
  project. The `#[doc(hidden)] pub` compromise keeps
  pathlint single-crate.

- **C. Move the binary into a `src/bin/` sibling that
  re-exports internals via a feature flag (`unstable-internals`).**
  Rejected because feature flags multiply the build
  matrix; CI would need to test both `--features
  unstable-internals` and `--no-default-features`.
  Single-tier `#[doc(hidden)] pub` requires no feature
  flag and is reachable from one specific binary path
  rather than a free-for-all.

- **D. Move `cli` / `run` to `src/bin/pathlint/` but keep
  every other internal module `pub mod` (no
  `#[doc(hidden)]` tier).** Rejected because the cleanup
  was prompted by the same supported-surface line: a
  reader on docs.rs should see exactly the supported
  modules, not the internal helpers. The middle tier
  exists *because* the binary needs reachability that
  `pub(crate)` blocks, not because the internals are
  semi-supported.

- **E. Document the supported surface in PRD / README
  without enforcing it in code.** Rejected because
  documentation drifts; the `tests/public_api.rs` pin
  enforces the surface at CI time, catching accidental
  renames or removals that documentation would miss.

## Consequences

- **Positive.** Embedders have a clear contract: "the
  9 (now 10) supported modules in `src/lib.rs` are the
  surface; anything else is internal and may change
  without notice." docs.rs reinforces this by hiding the
  middle tier.

- **Positive.** Internal modules can be reshaped freely:
  `format::strip_control_chars` could move to a different
  module or change signature, and embedders who weren't
  using it directly wouldn't notice. The 0.0.27 layered
  `*Deps` carriers (ADR-0007) and 0.0.28 `Attribution`
  split (ADR-0008) were both internal-shape changes that
  didn't disturb the 10-module surface.

- **Positive.** The binary stays in the same crate, with
  one `Cargo.toml` and one publishing flow. The
  `#[doc(hidden)] pub` modules are an honest signal:
  "yes, these are technically reachable, but they are
  not the supported surface".

- **Negative.** The middle tier (`#[doc(hidden)] pub`) is
  a load-bearing detail that a future contributor might
  not appreciate. A PR removing `#[doc(hidden)]` from one
  of the six middle-tier modules would silently promote
  it to docs.rs-visible, eroding the surface narrowing.
  Mitigation: `tests/public_api.rs` pins the supported
  surface but does not (yet) pin the middle tier; a
  future test could.

- **Negative.** Embedders pre-0.0.15 who used `format` /
  `report` / `init` / `path_source` / `resolve` /
  `catalog_view` directly must either migrate to the
  supported surface or accept the
  "may change without notice" stance. The 0.0.15
  CHANGELOG explicitly names the lib surface and the
  removed modules; the 0.0.17 CHANGELOG repeats the
  story for `cli`, `run`, `shell_quote`.

- **Negative.** `pathlint trace --json` exposes the
  already-quoted `uninstall.command` strings; if an
  embedder wants different quoting semantics (Fish
  shell, for example), they cannot reach
  `shell_quote::quote_posix` and must reimplement.
  Acceptable because shell quoting is a deceptively
  hard problem and exposing the helper would be making
  a half-formed promise.

- **Follow-up.** ADR-0001 (0.0.23) adds `path_entry` as
  the 10th module, applying this policy additively. The
  10-module count holds through 0.0.31. ADR-0007 /
  ADR-0008 introduced `pathlint::CommonDeps` and
  `pathlint::Attribution` at the crate root (not in a
  module); they are part of the supported surface but
  live outside the 10-module list. A future ADR may
  formalise the crate-root surface separately if it
  grows further.
