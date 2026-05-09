# Changelog

All notable changes to **pathlint** are recorded here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The 0.0.x line treats each `0.0.x → 0.0.(x+1)` bump as
MAJOR-equivalent (Cargo's pre-1.0 convention). Breaking changes are
allowed within 0.0.x and announced under `### Breaking`. Whether and
when 0.0.x graduates to 0.1.0 is undecided.

## [Unreleased]

## [0.0.23] — 2026-05-10

### Breaking

- **PATH entry handling moved to a `PathEntry { raw, expanded }`
  type.** `pathlint::doctor::analyze`,
  `pathlint::doctor::analyze_real`, `pathlint::sort::sort_path`,
  and the doc-hidden `path_source::PathRead` /
  `resolve::resolve` / `resolve::split_path` all now consume or
  return `&[PathEntry]` instead of `&[String]`. The boundary
  point at which `expand_env` runs is now exactly one place
  (`pathlint::path_source::read_path`), so detectors that reason
  about *what the user typed* (Shortenable, RelativePathEntry)
  see `entry.raw` and detectors that reason about *the directory
  on disk* (Missing, WriteablePathDir, the resolver) see
  `entry.expanded`. **Migration**: lift any raw `String` into
  `PathEntry::from_raw(s)` at the call site — `from_raw` runs
  `expand_env` once and gives you both forms.

### Added

- `pathlint::path_entry::PathEntry { raw, expanded }` is the new
  10th public module. Construct via `PathEntry::from_raw` so
  `expand_env` runs exactly once at the boundary.
- `pathlint::path_source::decode_reg_string` (Windows-only,
  crate-internal): UTF-16 LE decoder for `REG_SZ` /
  `REG_EXPAND_SZ` registry values. Lossy on invalid surrogate
  pairs, `Err` on unsupported registry types — never panics on a
  hostile payload.

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

[Unreleased]: https://github.com/ShortArrow/pathlint/compare/v0.0.22...HEAD
[0.0.22]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.22
[0.0.21]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.21
[0.0.20]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.20
[0.0.19]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.19
[0.0.18]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.18
[0.0.17]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.17
[0.0.16]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.16
[0.0.15]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.15
[0.0.14]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.14
