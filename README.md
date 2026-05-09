# pathlint

🌐 **English** | [日本語](docs/README.jp.md)

[![crates.io](https://img.shields.io/crates/v/pathlint.svg)](https://crates.io/crates/pathlint)
[![CI](https://github.com/ShortArrow/pathlint/actions/workflows/ci.yml/badge.svg)](https://github.com/ShortArrow/pathlint/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/crates/l/pathlint.svg)](#license)

> Verify that each command on `PATH` resolves from the installer you expect.

> **⚠ Pre-alpha (0.0.x).** Schema and CLI surface are still moving;
> until 0.1.0 lands, both minor and patch releases may break the
> TOML schema or the CLI. The current 0.0.x binary is functional —
> just don't bake it into anything load-bearing yet.

---

## What it is

Most "PATH problems" come from one place: **the wrong copy of an
executable resolves first.** `which python` tells you what wins,
but not whether that's what *should* win in a form you can commit
to a dotfiles repo and check on every machine.

`pathlint` makes that intent explicit: write down "**`runex` should
come from `cargo`, not from `winget`**" once in a `pathlint.toml`,
and the tool checks it on every machine you own.

## Install

```sh
# From crates.io
cargo install pathlint

# From source (latest main)
cargo install --git https://github.com/ShortArrow/pathlint

# Pre-built binaries
# https://github.com/ShortArrow/pathlint/releases
# Linux x86_64 / Windows x86_64 / macOS x86_64 / macOS aarch64
```

## 60-second try

```sh
# Drop a starter pathlint.toml in the current directory
pathlint init

# Edit pathlint.toml: add one [[expect]] for a tool you actually
# care about. e.g. "rg should come from cargo, not from winget":
#
#   [[expect]]
#   command = "rg"
#   prefer  = ["cargo"]
#   avoid   = ["winget"]

# Run the check
pathlint                          # = pathlint check

# If something fails, ask why
pathlint check --explain
```

That's the loop. `[[expect]]` is the user-facing concept;
everything else is convenience around it.

## `pathlint.toml` (minimal example)

```toml
[[expect]]
command = "runex"
prefer  = ["cargo"]
avoid   = ["winget"]

[[expect]]
command = "python"
prefer  = ["mise"]
avoid   = ["windows_apps", "choco"]

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
they're all in the built-in catalog (`cargo`, `mise`, `volta`,
`aqua`, `winget`, `choco`, `scoop`, `brew_arm`, `brew_intel`,
`apt`, `pacman`, `dnf`, `pkg`, `flatpak`, `snap`, `windows_apps`,
and more). The whole file is the user's intent.

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

Add `severity = "warn"` to a rule to keep its NG visible without
blocking CI (exit stays 0; the line is tagged `[warn]` instead of
`[NG]`):

```toml
[[expect]]
command  = "rg"
prefer   = ["cargo"]
severity = "warn"   # 0.0.7+ — nudge, not a hard fail
```

Add `kind = "executable"` to also verify the resolved path is an
actual executable file — catches the case where a directory of the
same name shadows the binary, or where the file the symlink points
at has gone missing:

```toml
[[expect]]
command = "rustc"
prefer  = ["cargo"]
kind    = "executable"
```

## Where next

- `pathlint check --explain` — multi-line diagnosis when an
  expectation fails (resolved / matched / prefer / avoid /
  diagnosis / hint).
- `pathlint check --json` — machine-readable output for CI
  pipelines.
- `pathlint doctor` — lint PATH itself (duplicates, missing dirs,
  8.3 short names, env-var-shortenable entries, malformed entries,
  same-command-different-dir shadows). Independent of `[[expect]]`.
- `pathlint trace <command>` — show where a command resolves from,
  which sources match it, and the most plausible uninstall command.
  Plugin-aware for mise (see [Working with mise](#working-with-mise)).
- `pathlint sort --dry-run` — propose a PATH order that satisfies
  every applicable `[[expect]]` rule. Read-only by design; PRD §4
  forbids PATH mutation.
- `pathlint catalog list` — every known source with its per-OS
  path. `--names-only` for a compact listing; `--all` to see every
  per-OS field even when only one is active on the current host.
- `pathlint catalog relations` — declared relations between
  sources (alias / conflict / served-by-via / depends-on /
  prefer-order-over).
- `pathlint --target user` / `--target machine` — Windows-only;
  read PATH from the per-user or per-machine registry instead of
  the inherited process env. The other subcommands accept the same
  flag.
- `pathlint check --json | jq '.[] | select(.kind != "ok")'` and
  similar machine pipelines — every JSON-emitting subcommand has a
  stable `kind` discriminator (since 0.0.15).

Full design and rationale: [docs/PRD.md](docs/PRD.md) (English),
[docs/PRD.jp.md](docs/PRD.jp.md) (日本語).

## How it works

Two TOML concepts:

1. **`[[expect]]`** — per-command expectations. "command X should be
   resolved from source S." This is what users actually write.
2. **`[source.<name>]`** — how to recognize an installer on disk
   ("`cargo` lives at `~/.cargo/bin`"). pathlint ships built-in
   defaults for every popular installer; users only override when
   their layout is non-standard.

For each `[[expect]]`, pathlint resolves the command against the real
PATH, looks at where the winning binary lives, and matches that
location to the source labels.

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
  built-in `apt` / `pacman` / `dnf` source (`/usr/bin`) doesn't match
  alone. Reference the built-in `os_baseline_linux_sbin` source
  alongside the package manager:

  ```toml
  [[expect]]
  command = "ls"
  prefer = ["pacman", "os_baseline_linux_sbin"]
  ```
- **Which package owns this binary.** `pathlint` does not call
  `dpkg -S` / `rpm -qf` / `pacman -Qo` / `brew which-formula`. That's
  intentional in 0.0.x for speed and offline correctness; revisiting
  is on the 0.2 list.

The full set of known limitations and future trade-offs lives in
[docs/PRD.md §14, §16](docs/PRD.md).

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

`pathlint trace <command>` is plugin-aware: when the resolved
binary lives under `mise/installs/<segment>/...` and `<segment>`
starts with `cargo-` / `npm-` / `pipx-` / `go-` / `aqua-`, the
output adds a `provenance:` line and a `mise uninstall ...` hint
so you don't have to remember which plugin you used:

```
$ pathlint trace lazygit
lazygit
  resolved: ~/.local/share/mise/installs/cargo-jesseduffield-lazygit/0.61/bin/lazygit
  sources:  mise_installs, mise
  provenance: cargo (via mise plugin `cargo-jesseduffield-lazygit`)
  hint:     mise uninstall cargo:'jesseduffield-lazygit'  (best-guess; verify with `mise plugins ls`)
```

The provenance is a path heuristic — it never causes
`prefer = ["cargo"]` to match a mise-served binary. Source
labels stay catalog-driven; provenance is purely a `trace`
display.

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

## Operational details

The 0.0.x line ships six subcommands: `check` (default), `doctor`,
`trace`, `sort`, `init`, and `catalog` (with `list` and
`relations`). `pathlint where` is kept as a visible alias of
`pathlint trace`, and `--rules` is kept as a visible alias of
`--config`. Both aliases are slated for removal in a future
release; the exact timing is undecided and will be announced
ahead of the breaking version. The TOML schema and CLI surface
are still moving, but the resolve / match / report pipeline is
in place and covered by tests.

`pathlint --version` typically runs in well under 50 ms on a
modern host. Verify on your hardware with `scripts/bench.sh`,
which wraps `hyperfine` around `--version`, `--help`, and
`catalog list --names-only`.

### Editor support (JSON Schema)

`pathlint.toml` ships with a JSON Schema generated from the live
Rust types. Add this single line at the top of your config to get
autocomplete and inline validation in [Taplo] (the dominant TOML
LSP) and the [Even Better TOML][ebt] VS Code extension:

```toml
#:schema https://raw.githubusercontent.com/ShortArrow/pathlint/main/schemas/pathlint.schema.json
```

Pin to a specific release for reproducibility (replace `<TAG>`
with the version you want, e.g. `v0.0.21`):

```toml
#:schema https://github.com/ShortArrow/pathlint/releases/download/<TAG>/pathlint.schema.json
```

The schema is also attached as `pathlint.schema.json` to every
GitHub Release alongside the binaries. As of 0.0.15 each release
also ships `check.schema.json`, which describes the JSON shape of
`pathlint check --json` output for downstream consumers.

[Taplo]: https://taplo.tamasfe.dev/
[ebt]: https://marketplace.visualstudio.com/items?itemName=tamasfe.even-better-toml

### Pinning the catalog version

The built-in source catalog evolves: a new pathlint version may
change a source's per-OS path (because `winget` reshuffled its
layout, say). If you want a guarantee that your `pathlint.toml`
runs against a sufficiently fresh catalog, declare a minimum:

```toml
require_catalog = 1
```

When the running binary embeds an older catalog, pathlint exits
with code 2 and a message naming the gap, instead of silently
matching against stale rules. `pathlint catalog list` prints the
embedded version on its first line so you can pick a value.

The opposite direction is not enforced — running against a *newer*
catalog is always fine. Bumping `catalog_version` is reserved for
real path or semantics changes; adding a new source does not bump
it, so old rules don't break.

## Documentation

Each translated doc has a language switcher at the top.

- [PRD](docs/PRD.md) — full design, including the built-in source
  catalog
- [Architecture](docs/ARCHITECTURE.md) — 5-minute repo map for new
  contributors
- [Release](docs/RELEASE.md) — how to cut a new version
- [Changelog](CHANGELOG.md) — breaking-change migration notes
- [Releases](https://github.com/ShortArrow/pathlint/releases) —
  version history with auto-generated notes

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
