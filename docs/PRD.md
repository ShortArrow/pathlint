# pathlint — Product Requirements Document

**Status:** 0.0.x in progress.
**Target release:** 0.0.3 is the latest working version. Schema and
CLI surface remain in motion through 0.1.0.

---

## 1. Overview

`pathlint` is a CLI that answers four questions about the `PATH` you
actually have, not the one you wish you had.

**R1 — Resolve order.** Given a command, which installer's copy
wins? You declare `[[expect]] command = "x" prefer = ["cargo"]`, and
pathlint checks. This is the original use case and the spine of the
tool.

**R2 — Existence and shape (planned).** Is the file pathlint
resolved actually executable, or did something replace `runex` with
a directory of the same name? Is the symlink broken? Today pathlint
only reports `not_found`; richer shape checks live in 0.0.4+.

**R3 — PATH hygiene.** Even before any expectation is evaluated,
the `PATH` itself is often a mess: duplicates, dangling directories,
8.3 short names, entries that could be written more concisely.
`pathlint doctor` lints the PATH on its own.

**R4 — Provenance (planned).** Once the resolved binary's full path
is in hand, where did it come from — and how would I uninstall it?
Today the matched-source list is internal data exposed only via
`check`. A `pathlint where <command>` subcommand (0.0.4+) will
surface it directly, including the most plausible uninstall command
(`mise uninstall cargo:lazygit`, `cargo uninstall lazygit`, ...).

A single `pathlint.toml` covers all four roles across **Windows,
macOS, Linux, and Termux** — sources declare their location per-OS,
and each `[[expect]]` may carry an `os = [...]` filter.

`pathlint` ships with a built-in catalog of well-known sources
(`cargo`, `mise`, `mise_shims`, `mise_installs`, `volta`, `winget`,
`choco`, `scoop`, `brew_arm`, `brew_intel`, `apt`, `pacman`, `pkg`,
`flatpak`, `WindowsApps`, ...). Users only have to write their
**expectations**; sources are looked up by name.

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

Across all four roles (R1 – R4):

- **Declarative.** Whatever pathlint cares about is expressible in a
  `pathlint.toml` that lives in a dotfiles repo. Nothing is hidden
  in invocation flags only.
- **Source labels, not paths.** Users speak in installer names
  (`cargo`, `mise_shims`, `winget`, `brew_arm`, `apt`) — the path
  patterns come from a catalog so the same TOML works on every
  machine.
- **Built-in catalog with override.** pathlint ships defaults for the
  popular installers; users redefine `[source.X]` only when they want
  to override or add a new one.
- **One file, all OSes.** Each `[[expect]]` may carry an `os = [...]`
  filter, and each `[source.X]` may declare per-OS paths
  (`windows = ...`, `unix = ...`, etc.). The same `pathlint.toml`
  drives Windows, macOS, Linux, and Termux.
- **Substring + case-insensitive match.** Source paths are matched
  against the resolved binary path as substrings, after env-var
  expansion and slash normalization.
- **Honest exit codes.** `0` = clean, `1` = at least one expectation
  failed, `2` = config / I/O error. R3 (`doctor`) and R4 (`where`)
  follow the same scale.
- **Read-only.** pathlint never mutates PATH, registry, dotfiles,
  or installed packages. It tells you what's there; you act.

Per-role:

- **R1 (resolve order).** A failing expectation shows the command,
  its resolved full path, the matched source(s), and the
  `prefer` / `avoid` mismatch. It must be enough to fix without
  another debugging tool.
- **R2 (existence and shape).** When a command resolves to a path,
  the path must point at an actually-executable file. Symlinks
  must be alive; "executable" must mean it. Today only `not_found`
  is reported; the rest is 0.0.4+.
- **R3 (PATH hygiene).** Even with no `[[expect]]` written,
  `pathlint doctor` flags duplicates, dangling directories,
  8.3 short names, env-var-shortenable entries, and malformed
  entries that would never resolve.
- **R4 (provenance).** Given a resolved binary, name the installer
  it most plausibly came from, and the corresponding uninstall
  command. Useful when the user can't remember whether they ran
  `cargo install` or `mise use cargo:tool` six months ago.

## 4. Non-goals

The roles above also imply specific *non-roles*:

- **No PATH rewriting / persisting.** pathlint does not mutate the
  process PATH, the Windows registry, `.bashrc`, `$PROFILE`, or
  any other shell config. It tells you what's wrong; how to fix is
  your call. (A `pathlint sort` post-MVP would print a recommended
  order without applying it.)
- **No `which` clone (R1 boundary).** pathlint does include resolve
  logic internally, but it doesn't aim to replace `where` /
  `type -a` / `Get-Command -All`. The R1 question is "is the right
  installer winning?", not "where does this resolve?". R4
  (`pathlint where`, planned) will surface the resolved path
  prominently, but with provenance, not as a generic which-clone.
- **No future install simulation.** pathlint answers about the
  PATH and binaries you have *now*. It does not predict where a
  future `cargo install` would land, what order the next mise
  activate would produce, or whether a planned install is "safe".
  This is intentional — predicting installer behaviour requires
  modelling each installer, which would balloon the trust surface.
- **No package management.** pathlint does not install or remove
  packages to satisfy an expectation. R4 may *suggest* an
  uninstall command (a string for the user to run); it never runs
  one.
- **No deep environment parsing.** Reads what the process actually
  sees (`getenv("PATH")`) plus, on Windows, the two registry
  locations. `/etc/environment`, PAM, launchd plists, systemd
  unit `Environment=`, `eval "$(brew shellenv)"` — out of scope.
- **No package-manager queries (0.1.x).** pathlint does not call
  `dpkg -S` / `rpm -qf` / `pacman -Qo` / `brew which-formula`.
  Path-prefix matching is fast and offline; the trade-off is that
  AUR / `make install` / custom prefixes are invisible until the
  user adds a `[source.<name>]`. Revisiting in 0.2 (see §16).

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

Mapping subcommands to roles (see §1):

| Role | Subcommand | Status |
|---|---|---|
| R1 — resolve order | `pathlint check` (default) | implemented (0.0.2) |
| R2 — existence and shape | reuses `[[expect]]` with a `kind` field, exposed in `check` | implemented (0.0.4) |
| R3 — PATH hygiene | `pathlint doctor` | implemented (0.0.3) |
| R4 — provenance | `pathlint where <command>` | implemented (0.0.4) |

`pathlint init` and `pathlint catalog list` are infrastructure
subcommands (configuration scaffolding, catalog inspection); they
serve every role but don't belong to any one of them.

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

### 7.3 `pathlint init` (implemented)

- Emits a starter `pathlint.toml` in the current directory with a
  small set of example `[[expect]]` entries for the current OS.
- `pathlint init --emit-defaults` writes the entire built-in source
  catalog into the file as well, so the user can edit / remove any
  entry. Off by default to keep the file short.
- Refuses to overwrite an existing file (exit 1) unless `--force`
  is passed.

### 7.4 `pathlint catalog list` (implemented)

- Prints every source in the merged catalog (built-ins plus user
  overrides / additions).
- Default output is the path applicable to the running OS;
  `--all` shows every per-OS field; `--names-only` strips paths and
  descriptions for shell pipelines.

### 7.5 `pathlint doctor` (implemented)

- Lints the PATH itself, independent of `[[expect]]`.
- **Error** (exits 1): malformed entries — embedded NUL, NTFS-
  illegal chars on Windows. The OS cannot use these as directories
  so they're escalated.
- **Warn** (exits 0):
  - Duplicate entries (after env-var expansion / slash normalize).
  - Missing directories.
  - Trailing slashes.
  - Windows 8.3 short names (`PROGRA~1`).
  - Case- / slash-variant duplicates (same normalized form,
    different verbatim).
  - Shortenable entries — could be written using a known env var
    (`%LocalAppData%` / `%UserProfile%` / `$HOME` etc.); the
    suggestion preserves the original case + slash style.
  - (0.0.5+) `MiseActivateBoth` — PATH exposes both `mise/shims/`
    and `mise/installs/` simultaneously. Usually means
    `mise activate` is configured in both shim and PATH-rewrite
    modes, or stale entries from a past configuration are still
    in PATH. Output enumerates every shim and install entry so
    the user can pick which to remove.
- `--quiet` hides warns; errors always print.

### 7.6 `[[expect]] kind = "executable"` (R2, implemented in 0.0.4)

Today an `[[expect]]` only checks that `command` resolves and the
matched source is acceptable. The resolved path could still be:

- a directory (someone shadowed the binary with a folder of the
  same name)
- a broken symlink
- a regular file without execute permission
- a zero-byte file from a half-finished install

Adding `kind = "executable"` to an expectation would make pathlint
verify the resolved path actually points at an executable file
(symlinks followed, mode bit / NTFS reparse honored). On failure
the status becomes a new `NG (not_executable)` with the kind of
shape mismatch named.

Vocabulary stays minimal in 0.0.4: `executable` only. Distinguishing
"native binary" from "script" is OS-specific (Windows `.cmd` vs
`.exe`, Unix shebangs) and would balloon the matrix without
clear win.

### 7.7 `pathlint where <command>` (R4, implemented in 0.0.4; plugin provenance in 0.0.5)

Surfaces what `check` already computes internally: for the named
command, print

- the resolved full path (the one R1 evaluates against)
- every matched source, with the most specific listed first
- (0.0.5+) a `provenance:` line when the path is under
  `mise/installs/<segment>/...` and `<segment>` starts with
  `cargo-` / `npm-` / `pipx-` / `go-` / `aqua-`. The provenance
  carries both the installer name and the raw plugin segment, so
  the user can verify with `mise plugins ls`.
- a single best-guess uninstall command. When provenance is
  present (0.0.5+) the hint is `mise uninstall <installer>:<rest>`
  with a "best-guess; verify" caveat, because the segment-to-id
  mapping is lossy. Otherwise the hint comes from the matched
  source's `uninstall_command` template.

The uninstall hint is a string the user runs themselves; pathlint
never executes it. When neither provenance nor the catalog can
suggest a command the output says so explicitly rather than
guessing.

Plugin provenance is a path-segment heuristic — a R4-only label,
never a source match. `prefer = ["cargo"]` in `[[expect]]` will
NOT match a binary under `mise/installs/cargo-foo/...` unless the
user explicitly defines a `[source.X]` for that prefix.

Naming: `where` overlaps with Windows `where.exe`, but pathlint's
output is provenance-focused and clearly distinct in style. If the
overlap proves too confusing in practice the name will be revisited
before 0.1.0.

### 7.8 `pathlint sort` (post-MVP)

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

# Catch-all alias: matches anything served by mise (shims OR
# installs). Kept for rules that don't care which layer.
[source.mise]
description = "any binary served by mise (alias matching shims + installs)"
windows = "$LocalAppData/mise"
unix    = "$HOME/.local/share/mise"

# Recommended for most rules — mise's shim layer is what shells
# `mise activate` front-loads onto PATH.
[source.mise_shims]
description = "mise shim layer"
windows = "$LocalAppData/mise/shims"
unix    = "$HOME/.local/share/mise/shims"

# Per-runtime install dirs. Hit either when mise activates a
# runtime via PATH-rewriting (no shim layer in front), or when a
# plugin (cargo-*, npm-*, ...) ships its own bin under
# `installs/<plugin>/<ver>/bin`.
[source.mise_installs]
description = "mise per-runtime install dirs"
windows = "$LocalAppData/mise/installs"
unix    = "$HOME/.local/share/mise/installs"

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
  init     Write a starter pathlint.toml in the current directory
  catalog  Inspect the source catalog
  doctor   Lint the PATH itself
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

`pathlint sort` is reserved for post-MVP.

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

- crates.io publish from 0.0.2 onward.
- GitHub Releases workflow shipping `x86_64-{linux,windows,darwin}`
  and `aarch64-darwin` archives, mirroring `dotfm`. Termux users
  build from source.
- (post-MVP) Homebrew formula, scoop manifest, AUR PKGBUILD.

## 14. Out of scope

- PATH editing / persistence (deferred to post-MVP `sort` mode).
- `which` over function/alias resolution — only file-on-PATH lookup.
- Shell-config patching (`.bashrc`, `$PROFILE` rewriting).
- Detecting *which package* a binary belongs to (we look at the path
  prefix only; no `dpkg -S` / `rpm -qf` / `brew which-formula` /
  `pacman -Qo` / `paru -Qo`). This is the dominant correctness
  trade-off: AUR / `make install` / any custom prefix is invisible to
  pathlint until the user adds a `[source.<name>]` for that prefix.
  See §16 for revisiting in 0.2.
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

Tagged with the role(s) each touches.

### R1 — resolve order

- **[R1] Symlinked system dirs.** On Arch, Solus, openSUSE TW etc.,
  `/usr/sbin` is a symlink to `/usr/bin`, and `which` reports
  `/usr/sbin/<cmd>`. The built-in `apt` / `pacman` / `dnf` /
  `system_linux` sources declare `linux = "/usr/bin"` only, so the
  substring miss makes pathlint report `NG (unknown source)` even
  though the binary is the distro one. Either the user adds
  `[source.usr_sbin] linux = "/usr/sbin"`, or the catalog grows a
  combined entry. Path-canonicalize is rejected for now because it
  silently changes which source label appears in the output and
  breaks shim-aware matching for mise / volta / asdf.
- **[R1] `prefer` ordering.** Currently `prefer = ["mise", "volta"]`
  is treated as a set ("any of these is OK"). Should the order
  additionally express preference for `sort`? Tied to the post-MVP
  `pathlint sort` design.

### R1 / R4 — installer identification

- **[R1, R4] Package-manager queries (0.2 candidate).** path-based
  matching misses AUR, Homebrew tap, `make install`, and anything
  else outside the prefixes listed in `[source.<name>]`. A future
  knob — perhaps `[source.X] owner_query = ["pacman", "-Qo"]` or an
  `[[expect]] via = "command"` opt-in — would let pathlint ask the
  package manager directly. Trade-off: ~50–100 ms per query,
  OS-specific output parsers, and a ring-of-trust issue (the
  queried binary must itself be trustworthy). Not for 0.1.x;
  revisit once we have field data on how often path-based matching
  falls short. R4 in particular benefits from this — uninstall
  hints get sharper when the package manager confirms ownership.
- **[R1, R4] mise plugin attribution.** A binary installed via
  mise's plugin system lives at `mise/installs/<plugin>/<ver>/bin/<bin>`,
  where `<plugin>` often encodes the upstream installer.
  *(Resolved in 0.0.5 — R4 emits a `provenance:` line and a
  `mise uninstall <installer>:<rest>` hint when the segment starts
  with `cargo-` / `npm-` / `pipx-` / `go-` / `aqua-`. R1's catalog
  is left untouched; this stays a pure provenance heuristic,
  never a source label, so `prefer = ["cargo"]` does NOT match a
  `mise/installs/cargo-foo/...` binary. Users who want such
  matching can still write a custom `[source.X]` for the
  `mise/installs/cargo-` substring.)*

### R3 — PATH hygiene

- **[R3] mise activate vs shims.** `mise activate` can either
  prepend `mise/shims/` to PATH or rewrite PATH with the
  per-runtime `installs/<lang>/<ver>/bin/` directly. *(0.0.5
  resolved the "warn when both layers coexist" half — `pathlint
  doctor` now emits a `MiseActivateBoth` diagnostic listing every
  shim entry alongside every install entry. Users still pick a
  mode for `[[expect]]` rules; pathlint does not auto-detect.)*
- **[R3] macOS launchd / `eval $(brew shellenv)`.** PATH set by
  these paths may differ from `process`. Out of MVP. R3 might
  expose this differently from R1: doctor could compare what the
  user sees vs what login services see, instead of pretending one
  PATH is "the" PATH.

### Cross-role / infrastructure

- **`MISE_DATA_DIR` / `XDG_DATA_HOME`.** mise honors both env
  vars for the location of its tree. The built-in catalog
  hardcodes the default `$LocalAppData/mise` (Windows) and
  `$HOME/.local/share/mise` (Unix). Users with a custom location
  override `[source.mise]` (and the two siblings) in their own
  `pathlint.toml`. Could be lifted to automatic discovery in 0.0.5+
  if it becomes a recurring papercut.

### Resolved

- **[R1] Multiple installs of the same source.** *(Resolved in
  0.0.3 — split into `mise`, `mise_shims`, `mise_installs`.)*
- **Catalog distribution.** *(Resolved in 0.0.x — `pathlint
  catalog list` ships.)*
- **Catalog versioning.** *(Resolved in 0.0.3 — `catalog_version`
  / `require_catalog`.)*

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
