# pathlint

[![crates.io](https://img.shields.io/crates/v/pathlint.svg)](https://crates.io/crates/pathlint)
[![CI](https://github.com/ShortArrow/pathlint/actions/workflows/ci.yml/badge.svg)](https://github.com/ShortArrow/pathlint/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/crates/l/pathlint.svg)](#license)

> Verify that each command on `PATH` resolves from the installer you expect.

> **⚠ Pre-alpha (0.0.x).** Schema and CLI surface are still moving;
> not ready for production wiring. Skeleton only — no working binary
> yet.

---

## Why

Most "PATH problems" come from one place: **the wrong copy of an
executable resolves first.** Examples:

- I `cargo install runex` on this machine, but the binary that runs
  is the older one from `winget` — same name, different file.
- `python` should come from `mise`, not from the Microsoft Store
  `WindowsApps` stub.
- `node` should come from `volta`, not from the system `apt` install.
- macOS `gcc` should come from Homebrew, not from `/usr/bin/gcc`.

`which python` will tell you what wins, but won't tell you whether
that's what *should* win in a form you can commit to a dotfiles repo
and check on every machine.

`pathlint` makes that intent explicit: write down "**`runex` should
come from `cargo`, not from `winget`**" once, and the tool checks it
on every machine you own.

## How it works

Two TOML concepts:

1. **`[[expect]]`** — per-command expectations. "command X should be
   resolved from source S." This is what users actually write.
2. **`[source.<name>]`** — how to recognize an installer on disk
   ("`cargo` lives at `~/.cargo/bin`"). pathlint ships built-in
   defaults for `cargo`, `mise`, `volta`, `aqua`, `winget`, `choco`,
   `scoop`, `brew_arm`, `brew_intel`, `apt`, `pacman`, `dnf`, `pkg`,
   `flatpak`, `snap`, `WindowsApps`, and more — users only override
   when their layout is non-standard.

For each `[[expect]]`, pathlint resolves the command against the real
PATH, looks at where the winning binary lives, and matches that
location to the source labels.

## Status

This crate currently ships only a project skeleton (Cargo manifest,
license, docs). The implementation is being ported from a PowerShell
prototype that lives at
<https://github.com/ShortArrow/dotfiles/blob/develop/windows/Test-PathOrder.ps1>.
See [docs/PRD.md](docs/PRD.md) for the full design.

## Planned usage

```sh
# Check the current process PATH against ./pathlint.toml
pathlint                          # = pathlint check

# Check the User-only or Machine-only PATH (Windows registry)
pathlint --target user
pathlint --target machine

# Verbose: also show n/a expectations and the resolved PATH
pathlint --verbose
```

## Planned `pathlint.toml` (minimal example)

```toml
[[expect]]
command = "runex"
prefer  = ["cargo"]
avoid   = ["winget"]

[[expect]]
command = "python"
prefer  = ["mise"]
avoid   = ["WindowsApps", "choco"]

[[expect]]
command = "node"
prefer  = ["mise", "volta"]

[[expect]]
command = "gcc"
prefer  = ["mingw", "msys"]
avoid   = ["strawberry"]
os      = ["windows"]
```

No `[source.*]` section is needed for any of the names above —
they're all in the built-in catalog. The whole file is the user's
intent.

To override a built-in (mise installed in a non-standard location):

```toml
[source.mise]
windows = "D:/tools/mise"
```

To add a new source:

```toml
[source.my_dotfiles_bin]
unix = "$HOME/dotfiles/bin"
```

`os = [...]` accepts `windows | macos | linux | termux | unix`.
Match is substring + case-insensitive, after env-var expansion (both
`%VAR%` and `$VAR` work everywhere) and slash normalization.

## Installation

```sh
# From crates.io (once published)
cargo install pathlint

# From source (latest main)
cargo install --git https://github.com/ShortArrow/pathlint
```

## Documentation

- [日本語 README](docs/README.jp.md)
- [PRD (English)](docs/PRD.md) — the full design, including the
  built-in source catalog
- [PRD (日本語)](docs/PRD.jp.md)
- [Changelog](CHANGELOG.md)

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
