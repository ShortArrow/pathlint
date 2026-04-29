# Changelog

All notable changes to pathlint are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

While the project is in `0.0.x`, minor and patch releases may both introduce
breaking changes to the TOML schema or CLI surface; once a `0.1.0` is cut,
regular semver rules apply.

## [Unreleased]

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

### Designed (pre-implementation)

The schema is now centered on **`[[expect]]` plus a `[source.<name>]`
catalog**, replacing the earlier "PATH-entry-order rules" sketch:

- **`[[expect]]`** declares per-command expectations:
  ```toml
  [[expect]]
  command = "runex"
  prefer  = ["cargo"]
  avoid   = ["winget"]
  ```
- **`[source.<name>]`** declares how to recognize an installer on
  disk via per-OS path substrings.
- **Built-in source catalog** ships in the binary: `cargo`, `go`,
  `npm_global`, `pip_user`, `mise`, `volta`, `aqua`, `asdf`,
  `winget`, `choco`, `scoop`, `brew_arm`, `brew_intel`, `macports`,
  `apt`, `pacman`, `dnf`, `flatpak`, `snap`, `pkg` (Termux),
  `WindowsApps`, `strawberry`, `mingw`, `msys`, plus `system_*`
  baselines per OS. Users override field-by-field or add new sources
  in their own `pathlint.toml`.
- **Single file, all OSes.** Expectations carry `os = [...]`
  (`windows | macos | linux | termux | unix`); sources carry per-OS
  keys. Slashes and env vars are normalized so the same file works
  cross-platform.
- **`which` subcommand dropped.** It overlapped too much with `where`
  / `type -a` / `Get-Command -All`. The interesting question is "is
  the right one winning?", which `check` answers directly.

See [docs/PRD.md](docs/PRD.md) §3, §7, §8, §9 for the full design.

[Unreleased]: https://github.com/ShortArrow/pathlint/commits/main
