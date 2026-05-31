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
  the alias.

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
  CLI-only callers.

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
  `analyze_real` is unchanged for CLI-only callers.

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
  the two payload-carrying variants.
- **`pathlint::cli` and `pathlint::run` removed from the lib.**
  Both modules used to be `#[doc(hidden)] pub mod` so the binary
  in `src/main.rs` could reach across the crate boundary. They
  now live in `src/bin/pathlint/` and are binary-only. Anything
  embedding pathlint as a library had no business calling them;
  they are gone from the surface.
- **Lib internal modules behind `#[doc(hidden)] pub`.**
  `catalog_view`, `format`, `init`, `path_source`, `report`,
  `resolve` shifted from `pub(crate)` to `#[doc(hidden)] pub` so
  the binary at `src/bin/pathlint/` can call them across the
  lib/bin boundary. Same compromise cli/run had pre-0.0.17.
- **`check.schema.json` `required` no longer lists
  `prefer` / `avoid` / `reason` / `diagnosis` / `resolved`.** The
  runtime applied `skip_serializing_if` on these fields, but the
  schema flagged them as required. The schema is now honest about
  what the wire form actually emits. JSON validators that assumed
  those fields were always present must accept their absence.
- **Shell quoting moved to internal `shell_quote` module.**
  Pre-0.0.17 `pathlint::format::quote_for` etc. were public. They
  were never advertised as supported and are now `pub(crate)` in
  `pathlint::shell_quote`. Embedders should read the
  already-quoted string from `trace --json uninstall.command`.
- **`--color` flag is now effective.** Pre-0.0.17 the global
  `--color {auto,always,never}` flag was parsed by clap and
  silently ignored. As of 0.0.17 it actually colourises status
  tags in the human output (and respects `--color never`). Output
  of pipelines that captured `pathlint check` stdout may now
  contain ANSI escapes when the captured stream is also pathlint's
  stdout and `--color always` is set.

## [0.0.16] — 2026-05-05

### Breaking

- **Lib resolver signature simplified.** `pathlint::lint::evaluate`
  and `pathlint::trace::locate` now take a resolver closure
  returning `Option<std::path::PathBuf>`, not the internal
  `Resolution { full_path: PathBuf }` wrapper. Embedders that
  built their own resolver closures must drop the wrapper:
  `Some(Resolution { full_path: pb })` → `Some(pb)`.
- **`Resolution` type removed.** `pathlint::resolve::resolve()`
  now returns `Option<PathBuf>` directly. Internal-only impact —
  the type was never on the public surface, but downstream
  embedders accessing pathlint via `git` dependencies might
  notice.

## [0.0.15] — 2026-05-05

### Breaking

- **`pathlint check --json` discriminator renamed.** Each outcome
  array element now uses `kind` (matches doctor / trace / sort /
  catalog relations) instead of the pre-0.0.15 `status`. The
  values themselves are unchanged. **Migration**: any consumer
  that branched on `.status` must read `.kind` instead.
- **Lib public surface narrowed to nine supported modules.**
  `config`, `lint`, `trace`, `sort`, `doctor`, `catalog`,
  `source_match`, `os_detect`, `expand`. Internals are
  `pub(crate)` or `#[doc(hidden)] pub` (the latter only for
  `cli` / `run` reachable from `src/main.rs`). Embedders relying
  on previously-public modules (e.g. `format`, `report`) must
  migrate.
- **UserConfig and the embedded catalog file are distinct types.**
  A user `pathlint.toml` declaring `catalog_version` is now a
  structural parse error (deny_unknown_fields) instead of the
  post-parse error 0.0.14 introduced.

## [0.0.14] — 2026-05-05

### Breaking

- **`pathlint where` → `pathlint trace`.** `where` remains as a
  clap visible alias for the rest of 0.0.x. *(Alias removed in
  0.0.22.)*
- **`--rules` → `--config`.** `--rules` remains as a visible
  alias for the rest of 0.0.x. *(Alias removed in 0.0.22.)*
- **Source rename, no aliases.** `WindowsApps` → `windows_apps`.
  `system_windows` / `system_macos` / `system_linux` →
  `os_baseline_windows` / `os_baseline_macos` /
  `os_baseline_linux`. New `os_baseline_linux_sbin` for
  `/usr/sbin`. **Migration**:
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
  `kind`.
- **`Provenance::MiseInstallerPlugin` → `Provenance::WrapperInstaller`.**
  Visible in `trace --json` as
  `provenance.kind = "wrapper_installer"`. `installer` and
  `plugin_segment` payload fields are unchanged.
- **`sort --dry-run` is opt-in.** `pathlint sort` without
  `--dry-run` exits 2 with a message naming the flag. A future
  `--apply` (post-1.0) would override this; today the only mode
  shipped is `--dry-run`.
- **`catalog_version = N` in user `pathlint.toml` is rejected.**
  The field was always reserved for the embedded catalog;
  `Config::from_path` now exits 2 if a user TOML sets it. (0.0.15
  promoted this from a post-parse to a structural error.)
- **`depends_on` is descriptive only.** It surfaces in
  `pathlint catalog relations` but does not affect doctor /
  trace / sort behaviour.
- **`build.rs` aggregates referential integrity violations.** CI
  surfaces every offending plugin in one failure instead of
  bailing on the first.

## Releases prior to 0.0.14

Earlier releases predate this changelog format and are not
re-tabulated here. The git history (`git log --oneline`) and tags
`v0.0.x` are the canonical record.

[Unreleased]: https://github.com/ShortArrow/pathlint/compare/v0.0.30...HEAD
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
