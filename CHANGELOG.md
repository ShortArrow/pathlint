# Changelog

All notable changes to pathlint are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

While the project is in `0.0.x`, minor and patch releases may both introduce
breaking changes to the TOML schema or CLI surface; once a `0.1.0` is cut,
regular semver rules apply.

## [Unreleased]

## [0.0.7] - 2026-05-02

### Added

- **`pathlint doctor --json`.** Machine-readable companion to the
  human view, completing the 3-way `check / where / doctor` JSON
  surface. Emits a JSON array; each element carries `index`,
  `entry`, `severity`, the discriminator `kind`, and any per-kind
  payload (`suggestion`, `canonical`, `first_index`, `reason`,
  `shim_indices` / `install_indices`). The include / exclude
  filters still apply; `--quiet` is ignored in JSON mode (the
  output is intended to be complete for tooling). Schema is stable
  through 0.0.x.
- **`[[expect]] severity = "warn"`.** Per-rule severity knob for
  CI scenarios where a `prefer` mismatch should be surfaced but
  not block the build. `severity = "error"` (default) keeps 0.0.x
  behaviour: NG escalates to exit 1. `severity = "warn"` reports
  the diagnostic with a `[warn]` tag and keeps exit 0. The shape
  of the failure (status, resolved path, matched sources) is
  unchanged — only the exit-code consequence differs. The
  `severity` field is also surfaced in `check --json` so CI gates
  can pattern-match on it.
- **`pathlint check --explain`.** Expands every NG outcome into a
  multi-line breakdown (resolved path / matched sources /
  prefer / avoid / diagnosis / hint) instead of the single-line
  detail. Each NG variant gets a tailored diagnosis: `NgWrongSource`
  names the offending `avoid` source if there is one, otherwise
  states which `prefer` names were missed; `NgUnknownSource` says
  the path lies outside every defined `[source.<name>]` and points
  at adding one; `NgNotFound` advises installing or marking the
  rule `optional = true`; `NgNotExecutable` carries the underlying
  reason (directory shadow / broken symlink / missing +x bit) and
  points at the most plausible cause. Off by default — the existing
  one-line detail is unchanged.
- **`pathlint check --json`.** Machine-readable companion to
  `--explain`: emits a single pretty-printed JSON array, one
  element per expectation, carrying `command`, `status`,
  `resolved`, `matched_sources`, `prefer`, `avoid`, and a tagged
  `diagnosis` object (`kind = "wrong_source"` /
  `"unknown_source"` / `"not_found"` / `"not_executable"` /
  `"config"`) on failures. The schema is stable through 0.0.x and
  parallels `where --json`. `--explain` and `--json` are mutually
  exclusive.

### Changed

- (Internal) Introduced `lint::Diagnosis`, a pure-data view of the
  *why* behind each NG status. Both the human (`--explain`,
  one-line detail) and JSON (`--json`) views now derive from this
  single value via `lint::diagnose`, eliminating the previous risk
  that the two presentations could drift out of sync.
- (Internal) Presentation logic factored into `src/format.rs`
  (doctor / where formatters) and `src/report.rs` gained
  `explain_lines` plus a new `Style.explain` flag. `run.rs` shrunk
  from 384 to 264 lines. Pure formatters, fully unit-tested. No
  observable CLI behaviour change other than `--explain` /
  `--json` themselves.

## [0.0.6] - 2026-05-02

### Added

- **`pathlint where --json`.** Switches the output to a single
  machine-readable JSON object so scripts can pipe pathlint into
  jq, awk, or another tool without parsing the human format. The
  schema (stable through `0.0.x`) uses a `kind` discriminator
  on `uninstall` and `provenance` so consumers can pattern-match
  rather than string-search. NotFound is `{"command":"...",
  "found":false}` and still exits 1.
- **`pathlint doctor --include` / `--exclude`.** Filters
  diagnostics by snake-case kind name (`duplicate`, `missing`,
  `shortenable`, `trailing_slash`, `case_variant`, `short_name`,
  `malformed`, `mise_activate_both`). The two flags are mutually
  exclusive. Filtering also affects exit code: `--exclude
  malformed` lets a run pass even when the underlying analysis
  would have escalated to exit 1. An unknown kind name is a
  config error (exit 2) with the valid list printed.

## [0.0.5] - 2026-05-02

### Added

- **R3 — `MiseActivateBoth` doctor diagnostic.** `pathlint doctor`
  now warns when PATH simultaneously exposes `mise/shims/` and one
  or more `mise/installs/...` entries. Common causes: `mise
  activate` configured in both shim and PATH-rewrite modes, or
  stale entries from a past configuration. Output enumerates
  every shim entry alongside every install entry so the user can
  pick which to remove. PRD §16 mise-activate-vs-shims question
  marked resolved on the doctor side.
- **R4 — mise plugin provenance.** `pathlint where <command>` now
  inspects mise plugin segments. When the resolved binary lives
  under `mise/installs/<segment>/...` and `<segment>` starts with
  `cargo-` / `npm-` / `pipx-` / `go-` / `aqua-`, the output adds
  a `provenance:` line naming the upstream installer (e.g.
  "cargo (via mise plugin `cargo-jesseduffield-lazygit`)") and
  the `hint:` line becomes
  `mise uninstall <installer>:<rest>  (best-guess; verify with
  `mise plugins ls`)`. The "best-guess" caveat is real — the
  segment-to-id mapping is lossy.
- The provenance is a R4-only heuristic, not an R1 source match.
  `[[expect]] prefer = ["cargo"]` does **not** match a binary
  served from `mise/installs/cargo-foo/...`; users who want such
  matching still need a custom `[source.X]`.
- PRD §16 mise-plugin-attribution open question marked resolved.

## [0.0.4] - 2026-05-01

### Added

- **R4 — `pathlint where <command>`.** New subcommand surfaces
  what `check` already computed internally: the resolved full
  path, the matched sources (most specific first; the catch-all
  `mise` alias falls to the back when a more specific
  `mise_shims` / `mise_installs` is also present), and a best-
  guess uninstall hint derived from the catalog. When pathlint
  cannot pick a command (the matched source has no template, or
  no source matched at all) the output says so explicitly rather
  than guessing.
- **`[source.<name>]` gains an optional `uninstall_command`
  field.** Rendered by `pathlint where`. The `{bin}` token is
  substituted with the file stem of the resolved binary
  (extension stripped on Windows). The embedded catalog now
  declares templates for `cargo`, `npm_global`, `pip_user`,
  `volta`, `winget`, `choco`, `scoop`, `brew_arm`, `brew_intel`,
  `macports`, `apt`, `pacman`, `dnf`, `flatpak`, `snap`, `pkg`.
  Sources without a clear single uninstall command (`mise_shims`,
  `mise_installs`, `aqua`, `asdf`, `system_*`, etc.) leave it
  unset on purpose.
- **R2 — `[[expect]] kind = "executable"`.** Verifies the
  resolved path actually points at an executable file, in addition
  to the source check. Catches cases where a directory of the
  same name shadows the binary, the resolved path is a broken
  symlink, or (on Unix) the file lacks a `+x` mode bit. Mismatches
  surface as a new `NG (not_executable: <reason>)` status. The
  shape check only escalates an OK to NG; an existing source
  mismatch (NG already) is left alone so users don't get two
  diagnostics for the same line.
- PRD redefined around four roles (R1 resolve order, R2 existence
  & shape, R3 PATH hygiene, R4 provenance) with a new
  subcommand-to-role map at the top of §7. R5 (predicting future
  installs) explicitly listed as a non-goal in §4.

### Changed

- (Schema) `[[expect]]` gains an optional `kind` field
  (`"executable"` only). Existing rules without `kind` keep working.
- (Catalog) `catalog_version` bumped from `1` to `2` to mark the
  arrival of `uninstall_command` on the built-in sources. Users
  who pinned `require_catalog = 1` still match correctly because
  the new field is purely additive; users who want the uninstall
  hints to be present can pin `require_catalog = 2`.

## [0.0.3] - 2026-04-30

### Added

- **mise overhaul.** The single `mise` source now has two
  finer-grained siblings, so rules can be specific about which
  layer of mise served the binary:
  - `mise_shims` — `mise/shims/<bin>` (the layer `mise activate`
    front-loads onto PATH; recommended for most rules).
  - `mise_installs` — `mise/installs/<tool>/<ver>/bin/<bin>` (used
    when mise activates a runtime via PATH-rewriting, or when a
    plugin like `cargo-*` / `npm-*` installs through mise).
- The catch-all `mise` source is kept as an alias matching either
  layer, so rules written for 0.0.2 keep working without edits.
- `pathlint init` starter files now reference `mise_shims` in their
  example expectations (Windows / macOS / Linux), nudging new
  users toward the more specific source.
- README and PRD gain a "Working with mise" section explaining
  shims vs installs, plus how to override the catalog when mise
  lives in a non-standard location (`MISE_DATA_DIR` /
  `XDG_DATA_HOME`).
- PRD §16 refreshed: open question on shim/install split is now
  marked resolved; new questions added on mise plugin attribution
  (0.0.4 candidate) and `mise activate` mode handling.
- **Catalog versioning.** The embedded source catalog now declares
  `catalog_version`. A user `pathlint.toml` may pin a minimum via
  `require_catalog = N` at the top level; if the running binary
  embeds an older catalog, pathlint exits 2 with a message naming
  the gap rather than silently matching against stale rules.
  `pathlint catalog list` prints the embedded version on its first
  line so users can pick a value.
- PRD §16 catalog-versioning open question marked resolved.
- The 0.0.3 catalog is `catalog_version = 1`. Bumping is reserved
  for changes to existing source paths or semantics; new sources
  alone don't bump the version.

## [0.0.2] - 2026-04-30

The first release that does anything. `0.0.1` was a skeleton and
was never published — `0.0.2` is what `cargo install pathlint` will
actually fetch.

### Added

- Initial repository skeleton: Cargo manifest with crates.io metadata,
  dual MIT / Apache-2.0 licenses, README and PRD in English and Japanese.
- Reference PowerShell prototype lives at
  `ShortArrow/dotfiles:windows/Test-PathOrder.ps1` and will be ported
  to Rust in a future release.
- First working binary covering PRD §7-§11:
  - `pathlint` (= `pathlint check`) reads `pathlint.toml` from
    `--rules`, then `./pathlint.toml`, then
    `$XDG_CONFIG_HOME/pathlint/pathlint.toml`.
  - Resolves each `[[expect]]` against the chosen PATH
    (`--target process|user|machine`; `user` / `machine` read the
    Windows registry, warn + fall back on Unix).
  - Reports per-expectation `OK` / `NG` / `skip` / `n/a` / `ERR` and
    exits `0` / `1` / `2` per spec.
  - Embeds the built-in source catalog and merges user
    `[source.<name>]` overrides field by field.
  - Expands `%VAR%`, `$VAR`, `${VAR}`, leading `~`, normalizes `\`
    to `/`, and matches case-insensitive substrings.
  - Honors `PATHEXT` on Windows and the executable bit on Unix.
  - 27 unit tests + 7 end-to-end CLI tests, clippy clean under
    `-D warnings`.
- `pathlint init` writes a starter `pathlint.toml` in the current
  directory with examples appropriate to the current OS. Refuses to
  overwrite an existing file unless `--force` is passed; the
  `--emit-defaults` flag also embeds the entire built-in source
  catalog so users can edit per-OS paths field by field.
- `pathlint catalog list` prints every known source — built-in plus
  any defined or overridden in the user's `pathlint.toml`. Default
  output shows the path for the running OS; `--all` shows every
  per-OS field; `--names-only` strips paths and descriptions for
  scripting.
- README and PRD now document the path-based matching boundary:
  AUR / `make install` / custom-prefix installs are not visible to
  pathlint until the user adds a `[source.<name>]` for that prefix,
  and `/usr/sbin → /usr/bin` symlink distros (Arch, openSUSE TW,
  Solus) need an explicit `[source.usr_sbin]` to recognize the
  distro install. Package-manager queries (`pacman -Qo` / `dpkg -S`
  / `rpm -qf` / `brew which-formula`) are deferred to 0.2.
- Linux starter emitted by `pathlint init` now declares
  `[source.usr_sbin] linux = "/usr/sbin"` and references both `apt`
  and `pacman` in `prefer`, so it works out of the box on
  Debian/Ubuntu and Arch alike.
- `pathlint doctor` lints the PATH itself, independent of any
  `[[expect]]` rules. Diagnostics:
  - **Error** (exits 1): malformed entries — embedded NUL, NTFS-
    illegal characters on Windows. The OS cannot use these as
    directories.
  - **Warn** (exits 0): duplicate entries (after env-var expansion
    and slash normalization), missing directories, trailing
    slashes, Windows 8.3 short names (`PROGRA~1`), case- /
    slash-variant duplicates, and entries that could be written
    using a known env var (`%LocalAppData%` / `%UserProfile%` /
    `$HOME` and friends — case and slash style preserved).
  - `--quiet` hides warns; errors always print.

[Unreleased]: https://github.com/ShortArrow/pathlint/compare/v0.0.7...HEAD
[0.0.7]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.7
[0.0.6]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.6
[0.0.5]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.5
[0.0.4]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.4
[0.0.3]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.3
[0.0.2]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.2
