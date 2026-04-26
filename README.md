# pathlint

[![crates.io](https://img.shields.io/crates/v/pathlint.svg)](https://crates.io/crates/pathlint)
[![CI](https://github.com/ShortArrow/pathlint/actions/workflows/ci.yml/badge.svg)](https://github.com/ShortArrow/pathlint/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/crates/l/pathlint.svg)](#license)

> Lint the `PATH` environment variable against declarative ordering rules.

> **⚠ Pre-alpha (0.0.x).** Schema and CLI surface are still moving; not
> ready for production wiring. Skeleton only — no working binary yet.

---

## Why

Most "PATH problems" come from one place: **the wrong copy of an
executable resolves first.** Examples:

- `python.exe` from a Microsoft Store stub shadows your `mise` install.
- `gcc` from Strawberry Perl shadows the toolchain you actually want.
- `pwsh` from a stale `WindowsPowerShell\v1.0` entry runs instead of
  PowerShell 7.

`which python` will tell you what wins, but won't tell you whether
that's what *should* win. `pathlint` makes that intent explicit:
you write down "**A must come before B**" rules in a TOML file, and
the tool checks them against the actual `PATH`.

## Status

This crate currently ships only a project skeleton (Cargo manifest,
license, docs). The implementation is being ported from a PowerShell
prototype that lives at
<https://github.com/ShortArrow/dotfiles/blob/develop/windows/Test-PathOrder.ps1>.
See [docs/PRD.md](docs/PRD.md) for the planned scope.

## Planned usage

```sh
# Check the current process PATH against ./pathlint.toml
pathlint check

# Same, but on the User-only or Machine-only PATH (Windows registry)
pathlint check --target user
pathlint check --target machine

# Explain where a command resolves and which other shadowed copies exist
pathlint which python

# (planned) propose a reordered PATH that satisfies every rule
pathlint sort --target user --dry-run
```

## Planned `pathlint.toml` schema

```toml
[[rule]]
name   = "mise shims override system tools"
before = "mise\\shims"
after  = ["chocolatey\\bin", "Strawberry\\c\\bin"]

[[rule]]
name   = "PowerShell 7 precedes legacy WindowsPowerShell"
before = "PowerShell\\7"
after  = ["WindowsPowerShell\\v1.0"]
```

Match is substring + case-insensitive, evaluated after env-var
expansion.

## Installation

```sh
# From source (once published)
cargo install pathlint

# From source (latest main)
cargo install --git https://github.com/ShortArrow/pathlint
```

## Documentation

- [日本語 README](docs/README.jp.md)
- [PRD (English)](docs/PRD.md)
- [PRD (日本語)](docs/PRD.jp.md)
- [Changelog](CHANGELOG.md)

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
