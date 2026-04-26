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

### Designed (still pre-implementation)

- Cross-platform plan: a single `pathlint.toml` with optional
  `os = ["windows" | "macos" | "linux" | "termux" | "unix"]` filters
  on each rule. Termux is split out from `linux` because its
  filesystem layout is fundamentally different.
- Path normalization: `\` and `/` are normalized for matching, and
  `%VAR%` / `$VAR` / `${VAR}` / `~` are all expanded uniformly so the
  same rules file works across OSes.
- Subcommands: `check` (default, lint), `which` (resolve + show
  shadowed copies). Post-MVP: `init`, `sort`.
- See [docs/PRD.md](docs/PRD.md) §3, §8, §10 for the full surface.

[Unreleased]: https://github.com/ShortArrow/pathlint/commits/main
