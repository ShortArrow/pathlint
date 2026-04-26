# pathlint — Product Requirements Document

**Status:** Draft (pre-implementation).
**Target release:** 0.0.1 MVP.

---

## 1. Overview

`pathlint` is a CLI that checks **"is each command being resolved
from the installer I expect?"** against a TOML manifest.

You declare:

> "`runex` should come from `cargo`, not from `winget`."

`pathlint` then resolves `runex` against the actual `PATH`, looks up
where the winning binary lives, and matches that location to the
**source label** ("cargo" / "winget" / ...) you defined.

The output is one line per expectation: **OK / NG / skip / n/a**.
Failures show the actual resolved location and which source label it
matched (or didn't).

A single `pathlint.toml` works across **Windows, macOS, Linux, and
Termux** — sources can declare their location per-OS, and each
expectation can carry an `os = [...]` filter.

`pathlint` ships with a built-in catalog of well-known sources
(`cargo`, `mise`, `volta`, `winget`, `choco`, `scoop`, `brew_arm`,
`brew_intel`, `apt`, `pacman`, `pkg`, `flatpak`, `WindowsApps`, ...).
Users only have to write their **expectations**; sources are looked
up by name.

## 2. Problem statement

The same command name often comes from different installers, and you
care which one wins:

- I ran `cargo install runex` on this machine, but the binary that
  actually fires is the older one in `WinGet/Links` — same name,
  different file.
- `python` should come from `mise`, not from the Microsoft Store
  `WindowsApps` stub.
- `node` should come from `volta`, not the system `apt` install.
- On macOS `gcc` should come from Homebrew, not from `/usr/bin/gcc`
  (which used to be a clang shim).

`which` tells you what wins; nothing tells you what *should* win in a
form you can commit to a dotfiles repo and check on every machine.

`pathlint` makes that intent explicit and verifiable.

## 3. Goals

- **Declarative expectations.** A `pathlint.toml` file with `[[expect]]`
  entries says "command X should resolve from source S".
- **Source labels, not paths.** Users speak in installer names
  (`cargo`, `mise`, `winget`, `brew_arm`, `apt`) instead of typing
  raw paths. Path patterns are looked up from a catalog.
- **Built-in catalog with override.** pathlint ships defaults for the
  popular installers; users redefine `[source.X]` only when they want
  to override or add a new one.
- **One file, all OSes.** Each `[[expect]]` may have an `os = [...]`
  filter, and each `[source.X]` may declare per-OS paths
  (`windows = ...`, `unix = ...`, etc.). The same `pathlint.toml`
  drives Windows, macOS, Linux, and Termux.
- **Substring + case-insensitive match.** Source paths are matched
  against the resolved binary path as substrings, after env-var
  expansion and slash normalization.
- **Honest exit codes.** `0` = clean, `1` = at least one expectation
  failed, `2` = config / I/O error.
- **Useful failure output.** Each failing expectation shows the
  command, its resolved full path, and which source it matched (or
  the `prefer` / `avoid` mismatch).
- **No mutation in MVP.** Read-only; `--apply` / `sort` are deferred.

## 4. Non-goals (MVP)

- **No PATH rewriting / persisting.** Sort/fix is later.
- **No editing of `.bashrc`, `$PROFILE`, or registry.** The output
  tells you what is wrong; how to fix it is your call.
- **No `which` clone.** `pathlint` does include resolve logic
  internally, but it does not aim to replace `where` / `type -a` /
  `Get-Command -All`. The interesting question pathlint answers is
  "is the right one winning?", not "where does this resolve?".
- **No package management.** `pathlint` does not install missing
  tools to satisfy an expectation.
- **No deep launchd / PAM / `/etc/environment` parsing.** Read what
  the process actually sees (`getenv("PATH")`) plus, on Windows, the
  two registry locations. Other layers are out of scope.

## 5. Target users

- Dotfiles maintainers wanting their `doctor` step to catch source
  drift on every machine they own — desktop Windows, work macOS,
  WSL, a Termux phone.
- Developers iterating on a tool they `cargo install` themselves who
  want to be sure their build, not the released winget/brew copy, is
  what runs.
- CI pipelines that bootstrap a developer environment and want to
  fail loudly when a wrong installer wins.

## 6. User stories

- I write `pathlint.toml` with five lines of `[[expect]]` for the
  commands I actually care about — no source definitions, since the
  built-ins cover them. `pathlint check` then runs the right subset
  on each OS.
- A linter run prints every expectation and its status; failures
  show me the actual resolved path and which `prefer` / `avoid` rule
  was violated.
- I override `[source.mise]` in my `pathlint.toml` because I keep
  mise in a non-standard directory.
- (post-MVP) I run `pathlint sort --target user --dry-run` and see a
  diff of how PATH would be reordered to satisfy every expectation.

## 7. Functional requirements (MVP)

### 7.1 `pathlint [OPTIONS]` (= `pathlint check`)

`check` is the default subcommand; bare `pathlint` runs it.

```
pathlint                              # = pathlint check
pathlint --target user                # explicit target
pathlint --rules ./other.toml
pathlint --verbose                    # also show n/a expectations and resolved PATH
pathlint --quiet                      # only print failures
```

- `--target` default is `process`. `user` / `machine` are accepted
  everywhere but only meaningful on Windows; on Unix they print a
  one-line warning and fall back to `process`.
- `--rules` default resolution order:
  1. `--rules <path>` if given.
  2. `./pathlint.toml` if present.
  3. `$XDG_CONFIG_HOME/pathlint/pathlint.toml` (or
     `$HOME/.config/pathlint/pathlint.toml`).
- For each `[[expect]]`:
  1. If its `os` filter excludes the current OS → status `n/a`.
  2. Resolve `command` against the chosen PATH (using `PATHEXT` on
     Windows, executable-bit on Unix).
  3. If not resolvable → status `not_found` (counts as failure
     unless `optional = true`).
  4. Look up the resolved full path against every defined `[source.X]`.
     The matched source name(s), if any, are recorded.
  5. **OK** if at least one matched source is in `prefer` and none of
     the matched sources is in `avoid`.
  6. **NG** otherwise — print the actual resolved path and the
     mismatch reason.
- One status line per expectation. Failures get a second indented
  line with details.
- Exit code: `0` if no expectation has status `NG` or `not_found`
  (excluding `optional`), `1` otherwise.

### 7.2 Source catalog merge

- pathlint embeds a built-in source catalog (see §9).
- The user's `pathlint.toml` may include any number of
  `[source.<name>]` entries:
  - Same `<name>` as a built-in → user overrides the per-OS paths
    field-by-field.
  - New `<name>` → added to the catalog.
- An expectation may reference any source name from the merged
  catalog. Referring to an undefined source is a config error.

### 7.3 `pathlint init` (planned, not MVP)

- Emits a starter `pathlint.toml` in the current directory with a
  small set of example `[[expect]]` entries for the current OS.
- `pathlint init --emit-defaults` writes the entire built-in source
  catalog into the file as well, so the user can edit / remove any
  entry. Off by default to keep the file short.

### 7.4 `pathlint sort` (post-MVP)

- Computes a PATH order that satisfies every applicable expectation,
  prints it (`--dry-run` default) or applies it via OS-appropriate
  APIs (`--apply`, Windows registry / shell-rc insertion). Out of
  scope for 0.0.x.

## 8. `pathlint.toml` schema

```toml
# ---- [[expect]]: per-command expectations ----

# Untagged: applies on every OS where the named sources are defined.
[[expect]]
command = "runex"
prefer  = ["cargo"]            # at least one matched source must be in this list
avoid   = ["winget"]           # no matched source may be in this list
os      = ["windows", "macos", "linux", "termux"]   # optional; default = all

[[expect]]
command = "python"
prefer  = ["mise"]
avoid   = ["WindowsApps", "choco"]
os      = ["windows"]

[[expect]]
command = "python"
prefer  = ["mise", "pkg"]
os      = ["termux"]

[[expect]]
command = "gcc"
prefer  = ["mingw", "msys"]
avoid   = ["strawberry"]
os      = ["windows"]

[[expect]]
command = "git"
optional = true                # if not on PATH at all, skip silently
prefer  = ["winget", "apt", "brew_arm", "brew_intel"]


# ---- [source.<name>]: how to recognize a source on disk ----

# Override a built-in (mise installed under D:\tools\mise on this machine):
[source.mise]
windows = "D:/tools/mise"

# Define a new source not in the built-in catalog:
[source.my_dotfiles_bin]
unix = "$HOME/dotfiles/bin"
```

### 8.1 Match semantics

For each `[source.X]`, the per-OS path string (after env-var
expansion and slash normalization) is checked against the resolved
binary path. **Substring + case-insensitive** match.

- A command is *matched against a source* iff the resolved binary's
  full path contains the source's per-OS path as a substring.
- A command may match **zero, one, or many** sources. Many is fine
  (e.g. `mise/installs/python/3.x/bin/python.exe` matches both
  `[source.mise]` and `[source.python_install]` if both are defined).
- Status decision uses the **set** of matched source names:
  - **OK**: at least one is in `prefer` AND none is in `avoid`.
  - **NG (wrong source)**: matched at least one source, but it is
    not in `prefer`, or it is in `avoid`.
  - **NG (unknown source)**: resolved path matched zero sources, and
    `prefer` is non-empty. (To allow "any source is fine, just exist",
    leave `prefer` empty and use `avoid` only.)
  - **NG (not found)**: command not on PATH, and `optional = false`
    (default).
  - **n/a**: `os` filter excludes the current OS.

### 8.2 Environment variable expansion

Source paths and PATH entries are expanded uniformly before matching:

- `%VAR%` (Windows-style) is expanded.
- `$VAR` and `${VAR}` (POSIX-style) are expanded.
- Leading `~` is expanded to the home directory.
- Unexpanded `%VAR%` / `$VAR` are kept verbatim (no error).

Both styles are accepted on every OS, so the same `pathlint.toml`
works under Windows pwsh, macOS bash, and Termux fish.

Slash normalization: `\` and `/` are converted to a single
representation (`/`) before substring comparison. So
`mise\\shims` (in a TOML literal) and `mise/shims` are equivalent.

### 8.3 OS identifiers

The `os` field on `[[expect]]` and the per-OS keys on `[source.X]`
accept these strings:

| value | matches when |
|---|---|
| `"windows"` | running on Windows (`cfg!(windows)`) |
| `"macos"` | running on macOS (`cfg!(target_os = "macos")`) |
| `"linux"` | running on Linux **and not** Termux |
| `"termux"` | running on Termux (detected via `PREFIX` env var pointing inside `/data/data/com.termux/files`) |
| `"unix"` | macOS or Linux or Termux (convenience alias) |

Termux is split out because its filesystem layout is fundamentally
different from generic Linux (no `/usr/bin`; everything lives under
`$PREFIX`). A source like `apt` (which means `/usr/bin`) should not
fire on Termux.

## 9. Built-in source catalog

pathlint embeds a default catalog, equivalent to the following TOML.
Every entry can be overridden field-by-field in the user's
`pathlint.toml`.

```toml
# ---- Cross-OS user-installed binaries ----

[source.cargo]
description = "binaries from `cargo install`"
windows = "$UserProfile/.cargo/bin"
unix    = "$HOME/.cargo/bin"

[source.go]
description = "binaries from `go install`"
windows = "$UserProfile/go/bin"
unix    = "$HOME/go/bin"

[source.npm_global]
windows = "$AppData/npm"
unix    = "$HOME/.npm-global/bin"

[source.pip_user]
windows = "$AppData/Python"
unix    = "$HOME/.local/bin"

[source.user_bin]
windows = "$UserProfile/bin"
unix    = "$HOME/bin"

[source.user_local_bin]
unix    = "$HOME/.local/bin"

# ---- Polyglot version managers ----

[source.mise]
windows = "$LocalAppData/mise"
unix    = "$HOME/.local/share/mise"

[source.volta]
windows = "$LocalAppData/Volta"
unix    = "$HOME/.volta/bin"

[source.aqua]
windows = "$LocalAppData/aquaproj-aqua"
unix    = "$HOME/.local/share/aquaproj-aqua"

[source.asdf]
unix    = "$HOME/.asdf/shims"

# ---- Windows-only package managers ----

[source.winget]
windows = "$LocalAppData/Microsoft/WinGet"

[source.choco]
windows = "$ProgramData/chocolatey"

[source.scoop]
windows = "$UserProfile/scoop"

[source.WindowsApps]
description = "Microsoft Store stub layer"
windows = "Microsoft/WindowsApps"

[source.strawberry]
windows = "Strawberry"

[source.mingw]
windows = "mingw"

[source.msys]
windows = "msys"

# ---- macOS-only package managers ----

[source.brew_arm]
description = "Homebrew on Apple Silicon"
macos = "/opt/homebrew"

[source.brew_intel]
description = "Homebrew on Intel macOS"
macos = "/usr/local"

[source.macports]
macos = "/opt/local"

# ---- Linux-only package managers ----

[source.apt]
linux = "/usr/bin"

[source.pacman]
linux = "/usr/bin"

[source.dnf]
linux = "/usr/bin"

[source.flatpak]
linux = "/var/lib/flatpak/exports/bin"

[source.snap]
linux = "/snap/bin"

# ---- Termux ----

[source.pkg]
description = "Termux pkg installs"
termux = "$PREFIX/bin"

[source.termux_user_bin]
termux = "$PREFIX/../home/bin"

# ---- OS baseline (catch-all "system PATH" sources) ----

[source.system_windows]
windows = "$SystemRoot/System32"

[source.system_macos]
macos = "/usr/bin"

[source.system_linux]
linux = "/usr/bin"
```

Notes:

- `apt` / `pacman` / `dnf` all point at `/usr/bin` because that is
  where their installed binaries land. They are aliases of "the
  Linux distro" from pathlint's perspective; users typically pick
  whichever name reads best in their `pathlint.toml`.
- `brew_arm` and `brew_intel` are split because `/opt/homebrew/bin`
  vs `/usr/local/bin` ordering on a single Mac is itself a typical
  source of bugs.
- `WindowsApps` and `strawberry` are listed primarily so they can
  appear in `avoid = [...]` lists.

## 10. Path sources (`--target`)

| `--target` | Windows | macOS / Linux / Termux |
|---|---|---|
| `process` | `GetEnvironmentVariable("PATH")` | `getenv("PATH")` |
| `user` | `HKCU\Environment\Path` (registry) | warn + fall back to `process` |
| `machine` | `HKLM\System\CurrentControlSet\Control\Session Manager\Environment\Path` | warn + fall back to `process` |

`process` is the union of Machine and User on Windows. On Unix the
"Machine vs User" distinction does not exist at the registry level —
`pathlint` does not parse `~/.bashrc`, `~/.zshrc`,
`/etc/environment`, launchd plists, or PAM in MVP.

## 11. CLI surface

```
pathlint [OPTIONS] [COMMAND]

Commands:
  check    Lint PATH against expectations (default)
  help     Print help

Options (global):
      --target <process|user|machine>  default: process
      --rules <path>                   default: search ./, then $XDG_CONFIG_HOME/pathlint/
  -v, --verbose                        print every expectation incl. n/a, plus the resolved PATH
  -q, --quiet                          only print failures
      --color <auto|always|never>      default: auto
      --no-glyphs                      ASCII-only output
  -h, --help
  -V, --version
```

`pathlint init` and `pathlint sort` are reserved for post-MVP.

## 12. Non-functional requirements

- **Single Rust binary.** No runtime deps beyond the OS itself.
- **Cross-platform first-class.** Windows, macOS, Linux all run in CI.
  Termux runs from `cargo install` on the device — no prebuilt
  Termux binary, mirroring `dotfm`'s policy.
- **Startup time.** `pathlint check` < 50 ms on a warm cache for a
  PATH of ~100 entries and ~20 expectations.
- **Stable exit codes.** `0` clean, `1` expectation failure, `2`
  config / I/O error.
- **Encoding.** All paths are treated as UTF-8 strings on every OS;
  rare non-UTF-8 PATH entries are reported with a warning and skipped.
- **Built-in catalog versioning.** The catalog is embedded at compile
  time; bumps to it are noted in the changelog so users know when
  defaults change.

## 13. Distribution

- crates.io publish once 0.0.1 ships.
- GitHub Releases workflow shipping `x86_64-{linux,windows,darwin}`
  and `aarch64-darwin` archives, mirroring `dotfm`. Termux users
  build from source.
- (post-MVP) Homebrew formula, scoop manifest, AUR PKGBUILD.

## 14. Out of scope

- PATH editing / persistence (deferred to post-MVP `sort` mode).
- `which` over function/alias resolution — only file-on-PATH lookup.
- Shell-config patching (`.bashrc`, `$PROFILE` rewriting).
- Detecting *which package* a binary belongs to (we look at the path
  prefix only; no `dpkg -S` / `rpm -qf` / `brew which-formula`).
- Parsing of `/etc/environment`, PAM, launchd plists, systemd unit
  `Environment=`, etc.

## 15. Success metrics

- The reference dotfiles (`ShortArrow/dotfiles`) replaces its
  `windows/Test-PathOrder.ps1` with a `pathlint check` invocation in
  `windows/doctor.ps1`, with a 5-line `pathlint.toml` of just
  `[[expect]]` entries (no `[source.*]` overrides).
- A user can write a useful `pathlint.toml` in under a minute by
  copy/edit from the README — including at least one OS-tagged
  expectation.
- A failing run names the command, the actual resolved path, and
  the mismatched source clearly enough to fix without further
  debugging tools.

## 16. Open questions

- **Multiple installs of the same source.** `mise` puts binaries in
  both `mise/shims/` and `mise/installs/<lang>/<ver>/bin/`. The
  current rule treats both as "from mise". Is that good enough, or
  should sources be split into `mise_shims` / `mise_installs`?
- **Catalog distribution.** Should the embedded catalog be exposed
  via `pathlint catalog list` for discovery? Trivial to add but adds
  a subcommand.
- **`prefer` ordering.** Currently `prefer = ["mise", "volta"]` is
  treated as a set ("any of these is OK"). Should the order
  additionally express preference for `sort`? Out of MVP.
- **Catalog versioning.** When pathlint updates a built-in source
  path (e.g. winget changes its layout), users on an old binary may
  silently get wrong matches. A `catalog_version = N` in the embedded
  catalog and a `--require-catalog >= N` flag could help.
- **macOS launchd / `eval $(brew shellenv)`.** PATH set by these
  paths may differ from `process`. Out of MVP.

## 17. Relationship to other tools

- **`which` / `where.exe` / `type -a` / `Get-Command -All`**: tell
  you what wins. `pathlint` tells you whether the right one wins.
- **`dotfm doctor`**: `pathlint check` is intended to be invoked from
  a `dotfm.toml` `[tools.<name>.doctor]` script.
- **`PATH.txt` / `DiffPath.ps1` (in `ShortArrow/dotfiles`)**: those
  check *whether expected entries exist* in `PATH`; `pathlint` checks
  *which installer the resolved binary actually came from*. The two
  are complementary.
- **Package managers (mise, brew, choco, pkg, ...)**: `pathlint` does
  not manage installations; it tells you whether the order they
  produced is what you wanted.
