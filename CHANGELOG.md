# Changelog

All notable changes to pathlint are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

While the project is in `0.0.x`, minor and patch releases may both introduce
breaking changes to the TOML schema or CLI surface; once a `0.1.0` is cut,
regular semver rules apply.

## [Unreleased]

## [0.0.2] - 2026-04-29

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

[Unreleased]: https://github.com/ShortArrow/pathlint/compare/v0.0.2...HEAD
[0.0.2]: https://github.com/ShortArrow/pathlint/releases/tag/v0.0.2
