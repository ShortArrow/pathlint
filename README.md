# pathlint

[![crates.io](https://img.shields.io/crates/v/pathlint.svg)](https://crates.io/crates/pathlint)
[![CI](https://github.com/ShortArrow/pathlint/actions/workflows/ci.yml/badge.svg)](https://github.com/ShortArrow/pathlint/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/crates/l/pathlint.svg)](#license)

> Verify that each command on `PATH` resolves from the installer you expect.

> **⚠ Pre-alpha (0.0.x).** Schema and CLI surface are still moving;
> until 0.1.0 lands, both minor and patch releases may break the
> TOML schema or the CLI. The 0.0.2 binary is functional — just
> don't bake it into anything load-bearing yet.

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

The 0.0.x line ships a working `pathlint` / `pathlint init` /
`pathlint catalog list`. The TOML schema and CLI surface are still
moving, but the resolve / match / report pipeline is in place and
covered by tests. See [docs/PRD.md](docs/PRD.md) for the full design.

## What pathlint *won't* tell you

`pathlint` is **path-prefix based**: it resolves the command, looks at
the resolved binary's full path, and asks "does any defined source's
per-OS path appear in it as a substring?". That makes it fast (no
package-manager calls, no network), but it leaves blind spots you
should know about:

- **AUR / Homebrew tap / `make install` / any custom prefix.** If a
  binary lands somewhere not listed in your `[source.<name>]` entries,
  `pathlint` reports `NG (unknown source)` even when the install is
  legitimate. Add a `[source.my_prefix]` for it, or accept that
  pathlint can't tell that case apart from a real misordering.
- **Symlinked system dirs.** On Arch / openSUSE TW / Solus,
  `/usr/sbin → /usr/bin`. `which ls` reports `/usr/sbin/ls`, so the
  built-in `apt` / `pacman` / `dnf` source (`/usr/bin`) doesn't match.
  Add `[source.usr_sbin] linux = "/usr/sbin"` to your `pathlint.toml`
  if you hit this.
- **Which package owns this binary.** `pathlint` does not call
  `dpkg -S` / `rpm -qf` / `pacman -Qo` / `brew which-formula`. That's
  intentional in 0.0.x for speed and offline correctness; revisiting
  is on the 0.2 list.

The full set of known limitations and future trade-offs lives in
[docs/PRD.md §14, §16](docs/PRD.md).

## Usage

```sh
# Check the current process PATH against ./pathlint.toml
pathlint                          # = pathlint check

# Check the User-only or Machine-only PATH (Windows registry)
pathlint --target user
pathlint --target machine

# Verbose: also show n/a expectations and the resolved PATH
pathlint --verbose

# Drop a starter pathlint.toml in the current directory
pathlint init
pathlint init --emit-defaults     # also embeds the full source catalog

# Inspect every known source (built-in + user-defined)
pathlint catalog list             # paths for the running OS
pathlint catalog list --all       # every per-OS field
pathlint catalog list --names-only
```

## `pathlint.toml` (minimal example)

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

## Working with mise

mise serves binaries from two distinct places, and pathlint exposes
each as its own source so rules can be specific:

- **`mise_shims`** — `$HOME/.local/share/mise/shims/<bin>` on Unix,
  `$LocalAppData/mise/shims/<bin>` on Windows. This is the layer
  shells front-load when you run `mise activate`. It's the
  recommended source to reference in `prefer` for most rules.
- **`mise_installs`** — `$HOME/.local/share/mise/installs/<tool>/<ver>/bin/<bin>`.
  Hit when `mise activate` rewrites PATH directly (no shims), or
  when a plugin (`cargo-*`, `npm-*`, ...) ships its bin under
  `installs/<plugin>/<ver>/bin`.
- **`mise`** — catch-all that matches both layers. Useful when you
  don't care which mise mode is in use; rules written before 0.0.3
  keep working unchanged.

```toml
# Strict: only accept mise's shim layer.
[[expect]]
command = "python"
prefer  = ["mise_shims"]

# Looser: anything mise serves is fine.
[[expect]]
command = "node"
prefer  = ["mise"]
```

If you set `MISE_DATA_DIR` or `XDG_DATA_HOME` to a non-standard
location, override the three sources in your `pathlint.toml`:

```toml
[source.mise]
unix = "/data/tools/mise"

[source.mise_shims]
unix = "/data/tools/mise/shims"

[source.mise_installs]
unix = "/data/tools/mise/installs"
```

## Installation

```sh
# From crates.io
cargo install pathlint

# From source (latest main)
cargo install --git https://github.com/ShortArrow/pathlint

# Pre-built binaries
# https://github.com/ShortArrow/pathlint/releases
# Linux x86_64 / Windows x86_64 / macOS x86_64 / macOS aarch64
```

## Documentation

- [日本語 README](docs/README.jp.md)
- [PRD (English)](docs/PRD.md) — the full design, including the
  built-in source catalog
- [PRD (日本語)](docs/PRD.jp.md)
- [Release process](docs/RELEASE.md) — how to cut a new version
- [リリース手順 (日本語)](docs/RELEASE.jp.md)
- [Changelog](CHANGELOG.md)

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
