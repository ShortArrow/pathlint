# pathlint — Product Requirements Document

**Status:** Draft (pre-implementation).
**Target release:** 0.0.1 MVP.

---

## 1. Overview

`pathlint` is a CLI that **verifies the `PATH` environment variable
against declarative ordering rules** written in TOML. It answers:

> "Is the right copy of this command being resolved first?"

Rules look like "X must come before Y in PATH". The tool reports each
rule as **OK / NG / skip** (skip = neither side present), with the
exact indices of the offending entries when a rule fails.

The MVP is read-only (lint mode). A later version may propose or apply
a reordered PATH (sort/fix mode).

A single `pathlint.toml` is intended to work across **Windows, macOS,
Linux, and Termux** — rules can be tagged with `os = [...]` so that
the same file can hold Windows-specific rules (WindowsApps stubs,
chocolatey, Strawberry) alongside macOS-specific ones (Homebrew vs
system) and Linux/Termux ones (mise vs distro pkg).

## 2. Problem statement

PATH ordering bugs are common and quietly painful. They look different
on each platform but reduce to the same shape — "the wrong same-named
binary wins":

- **Windows.** A Microsoft Store stub for `python.exe` shadows the real
  install (mise, conda, asdf, manual). Strawberry Perl's `gcc` shadows
  a Rust/MSYS toolchain. A leftover `WindowsPowerShell\v1.0` entry
  resolves to `pwsh.exe` instead of PowerShell 7.
- **macOS.** `/usr/bin/python3` (the system Apple-provided one) shadows
  Homebrew or pyenv. `/usr/local/bin` vs `/opt/homebrew/bin` ordering
  matters when both intel and arm brews are present.
- **Linux.** A distro-packaged `node` shadows nvm or mise. `/snap/bin`
  shadows `~/.cargo/bin`. `/usr/games` ahead of `~/bin` shadows local
  scripts.
- **Termux.** `~/bin` after `$PREFIX/bin` means user scripts can't
  override `pkg install`-supplied tools.

`which X` shows what wins but not what should win. There is no
declarative way to encode "what should win" so that CI, dotfiles, or
doctor scripts can check it. `pathlint` fills that gap.

## 3. Goals

- **Declarative rules.** A `pathlint.toml` file with `[[rule]]` entries
  expresses "X must precede Y" in plain TOML.
- **One file, all OSes.** Rules can carry an `os = [...]` filter so a
  single `pathlint.toml` works across Windows, macOS, Linux, and
  Termux. Untagged rules apply everywhere.
- **Substring + case-insensitive match.** Rule keys do not need to be
  exact paths. Case-insensitive matching is consistent across OSes
  (Windows is naturally case-insensitive; on Unix we trade strictness
  for portability of the same rules file).
- **OS-aware path sources.** `--target process|user|machine` selects
  what `PATH` to read. On Windows `user` and `machine` come from the
  registry; on Unix they fall back to `process` with a warning.
- **Honest exit codes.** `0` = clean, `1` = at least one rule failed,
  `2` = config / I/O error.
- **Useful failure output.** Each failing rule prints the indices of
  the offending entries and a brief reason ("'chocolatey\\bin' at #42
  precedes 'mise\\shims' at #49").
- **No mutation in MVP.** Read-only; `--apply` / `sort` are deferred.

## 4. Non-goals (MVP)

- **No PATH rewriting / persisting.** Sort/fix is later.
- **No editing of `.bashrc`, `$PROFILE`, or registry.** The lint output
  tells you *what* needs fixing; *how* is the user's call.
- **No shell-completion installation.** `pathlint completions <shell>`
  may land later.
- **No package management.** `pathlint` does not install missing tools
  to satisfy a rule.
- **No deep launchd / PAM / `/etc/environment` parsing.** Read what the
  process actually sees (`getenv("PATH")`) plus, on Windows, the two
  registry locations. Other layers are out of scope.

## 5. Target users

- Dotfiles maintainers wanting their `doctor` step to catch PATH drift
  on every machine they own — desktop Windows, work macOS, WSL, a
  Termux phone.
- Developers debugging "why does this Python run instead of that one"
  in a way that survives reboots.
- CI pipelines that bootstrap a developer environment and want to
  fail loudly on PATH ordering regressions.

## 6. User stories

- I write `pathlint.toml` once with rules I care about — some tagged
  `os = ["windows"]`, some `os = ["macos", "linux", "termux"]`, some
  untagged — commit it to my dotfiles, and `pathlint check` evaluates
  the right subset on each machine.
- A linter run prints every rule and its status; failures show me
  which entry beats which entry.
- I can ask `pathlint check --target user` (Windows) to verify only my
  user PATH before doing `setx PATH ...`.
- On Termux I run `pathlint check` and it understands that
  `$PREFIX/bin` is the system equivalent of `/usr/bin`.
- (post-MVP) I run `pathlint sort --target user --dry-run` and see a
  diff of what would change.

## 7. Functional requirements (MVP)

### 7.1 `pathlint [OPTIONS]` (= `pathlint check`)

`check` is the default subcommand; bare `pathlint` runs it.

```
pathlint                              # = pathlint check
pathlint --target user                # explicit target
pathlint --rules ./other.toml
pathlint --verbose                    # also dump expanded PATH entries
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
- For each loaded rule, evaluate against the resolved PATH. Rules
  whose `os` filter excludes the current OS are silently skipped
  (and counted as `n/a` in `--verbose`).
- One status line per rule (`OK` / `NG` / `skip`). Failures get a
  second indented line with the offender details.
- Exit code: `0` if no rule has status `fail`, `1` otherwise.

### 7.2 `pathlint which <command>` (MVP)

- Resolves the command across PATH using OS rules: `PATHEXT` on
  Windows, the executable bit on Unix.
- Prints the winning path first, then any shadowed copies further down
  PATH, with a short `[shadowed]` annotation. The point is to make the
  "first wins, the rest are reachable but unused" relationship visible.
- Exit code: `0` if at least one match, `1` otherwise.

### 7.3 `pathlint init` (planned, not MVP)

- Emits a starter `pathlint.toml` in the current directory, populated
  with a small set of OS-tagged example rules for the current OS plus
  comments showing the others. Skipped for MVP; `pathlint init`
  may also accept `--os <list>` to seed from a different OS's defaults.

### 7.4 `pathlint sort` (post-MVP)

- Computes the topological order of PATH entries that satisfies every
  applicable rule, printing it (`--dry-run` default) or applying it
  via OS-appropriate APIs (`--apply`, Windows registry / shell-rc
  insertion). Out of scope for 0.0.x.

## 8. `pathlint.toml` schema

```toml
# Each [[rule]] asserts: at least one entry containing `before` must come
# earlier in PATH than every entry containing any of `after`.

# Untagged rule — applies on every OS.
[[rule]]
name   = "PowerShell 7 precedes legacy WindowsPowerShell"
before = "PowerShell\\7"
after  = ["WindowsPowerShell\\v1.0"]

# Windows-only rule.
[[rule]]
name   = "mise shims override system tools"
os     = ["windows"]
before = "mise\\shims"
after  = ["chocolatey\\bin", "Strawberry\\c\\bin"]

# Multi-OS rule, opting out of Termux.
[[rule]]
name   = "user cargo bin precedes distro tools"
os     = ["windows", "macos", "linux"]
before = ".cargo/bin"
after  = ["/usr/bin", "Strawberry"]

# Termux-specific rule.
[[rule]]
name   = "user bin precedes pkg-installed binaries"
os     = ["termux"]
before = "/data/data/com.termux/files/home/bin"
after  = ["/data/data/com.termux/files/usr/bin"]
```

### 8.1 Match semantics

- Substring, case-insensitive, against each PATH entry **after**
  environment-variable expansion (see §8.2).
- Forward and back slashes are normalized to a single representation
  (`\` -> `/`) before comparison, so `mise\shims` matches `mise/shims`
  and vice versa. This lets one rules file work cross-OS.
- A rule is **OK** if every `after` match comes after the first
  `before` match.
- A rule is **fail** if any `after` match comes before all `before`
  matches, OR if `after` has matches but `before` has none.
- A rule is **skip** if neither side is present in PATH.
- A rule is **n/a** if its `os` filter excludes the current OS;
  silently ignored unless `--verbose`.

### 8.2 Environment variable expansion

PATH entries are expanded uniformly before matching:

- `%VAR%` (Windows-style) is expanded.
- `$VAR` and `${VAR}` (POSIX-style) are expanded.
- Leading `~` is expanded to the home directory.
- Unexpanded `%VAR%` / `$VAR` are kept verbatim (no error).

Both styles are accepted on every OS. This means a rule can be
written `before = "$HOME/bin"` and it still works under Windows pwsh
where the entry is literally `%USERPROFILE%\bin`, because both
expansions converge to the same absolute path.

### 8.3 OS identifiers

The `os` field accepts these strings:

| value | matches when |
|---|---|
| `"windows"` | running on Windows (`cfg!(windows)`) |
| `"macos"` | running on macOS (`cfg!(target_os = "macos")`) |
| `"linux"` | running on Linux **and not** Termux |
| `"termux"` | running on Termux (detected via `PREFIX` env var pointing inside `/data/data/com.termux/files`) |
| `"unix"` | macOS or Linux or Termux (convenience alias) |

Termux is split out because its filesystem layout is fundamentally
different from generic Linux (no `/usr/bin`; everything lives under
`$PREFIX`). Rules that talk about `/usr/bin` should not fire on Termux.

## 9. Path sources

| `--target` | Windows | macOS / Linux / Termux |
|---|---|---|
| `process` | `GetEnvironmentVariable("PATH")` | `getenv("PATH")` |
| `user` | `HKCU\Environment\Path` (registry) | warn + fall back to `process` |
| `machine` | `HKLM\System\CurrentControlSet\Control\Session Manager\Environment\Path` | warn + fall back to `process` |

`process` is the union of Machine and User on Windows. On Unix the
"Machine vs User" distinction does not exist at the registry level —
`pathlint` does not parse `~/.bashrc`, `~/.zshrc`, `/etc/environment`,
launchd plists, or PAM in MVP. (`shellrc` source could be added later
if there is demand.)

## 10. CLI surface

```
pathlint [OPTIONS] [COMMAND]

Commands:
  check    Lint PATH against rules (default)
  which    Resolve a command across PATH and list shadowed copies
  help     Print help

Options (global):
      --target <process|user|machine>  default: process
      --rules <path>                   default: search ./, then $XDG_CONFIG_HOME/pathlint/
  -v, --verbose                        print every rule, including n/a, plus expanded PATH
  -q, --quiet                          only print failures
      --color <auto|always|never>      default: auto
      --no-glyphs                      ASCII-only output (default is ASCII anyway; opt-in glyphs come later)
  -h, --help
  -V, --version
```

## 11. Non-functional requirements

- **Single Rust binary.** No runtime deps beyond the OS itself.
- **Cross-platform first-class.** Windows, macOS, Linux all run in CI.
  Termux runs from `cargo install` on the device — no prebuilt
  Termux binary, mirroring `dotfm`'s policy.
- **Startup time.** `pathlint check` < 50 ms on a warm cache for a
  PATH of ~100 entries and ~20 rules.
- **Stable exit codes.** `0` clean, `1` rule failure, `2` config /
  I/O error.
- **Encoding.** All paths are treated as UTF-8 strings on every OS;
  rare non-UTF-8 PATH entries are reported with a warning and skipped.

## 12. Distribution

- crates.io publish once 0.0.1 ships.
- GitHub Releases workflow shipping `x86_64-{linux,windows,darwin}`
  and `aarch64-darwin` archives, mirroring `dotfm`. Termux users
  build from source.
- (post-MVP) Homebrew formula, scoop manifest, AUR PKGBUILD.

## 13. Out of scope

- PATH editing / persistence (deferred to post-MVP `sort` mode).
- `which` over function/alias resolution — only file-on-PATH lookup.
- Shell-config patching (`.bashrc`, `$PROFILE` rewriting).
- Detecting *missing* commands beyond what rule evaluation produces as
  a side effect.
- Parsing of `/etc/environment`, PAM, launchd plists, systemd unit
  `Environment=`, etc.

## 14. Success metrics

- The reference dotfiles (`ShortArrow/dotfiles`) replaces its
  `windows/Test-PathOrder.ps1` with a `pathlint check` invocation in
  `windows/doctor.ps1`, and the rules-file lives in the same repo
  (working on every OS the user owns, not just Windows).
- A user can write a 5-rule `pathlint.toml` in under a minute by
  copy/edit from the README — including at least one OS-tagged rule.
- A failing run names every offending pair clearly enough to fix
  without further debugging tools.

## 15. Open questions

- **`before_not` (negative match).** Needed when "user `go/bin`" must
  precede "system `Go/bin`" but both contain `go/bin`. Not in MVP;
  revisit when a real second-rule conflict hits.
- **`shellrc` source.** Should `--target shellrc` parse `.bashrc` /
  `.zshrc` for `export PATH=...` lines? Useful for "I committed a
  PATH change to shellrc but a fresh shell hasn't picked it up yet".
  Out of MVP.
- **Termux PATH conventions.** Should `os = ["termux"]` rules
  automatically rewrite `/usr/bin` -> `$PREFIX/bin` for the user, or
  is that surprising? Current preference: no rewriting; rules say
  literally what they mean.
- **macOS launchd.** `launchctl getenv PATH` may differ from
  `process` PATH in some apps. Out of MVP.
- **`pathlint sort` semantics.** If two rules conflict, which wins?
  Topological-sort cycle handling needs design before implementation.
- **Shell completions** via `clap_complete`. Cheap but post-MVP.

## 16. Relationship to other tools

- **`which` / `where.exe`**: same domain (find where a command
  resolves) but no notion of "should". `pathlint which` complements
  rather than replaces them.
- **`dotfm doctor`**: `pathlint check` is intended to be invoked from
  a `dotfm.toml` `[tools.<name>.doctor]` script, not to replace
  `dotfm`. The recommended layout: a single `pathlint.toml` lives in
  the dotfiles repo and is referenced by both Windows and Unix doctor
  scripts.
- **`PATH.txt` / `DiffPath.ps1` (in `ShortArrow/dotfiles`)**: those
  check *whether expected entries exist*; `pathlint` checks *whether
  the order is right*. The two are complementary.
- **Package managers (mise, brew, choco, pkg)**: `pathlint` does not
  manage installations; it tells you whether the order they produced
  is what you wanted.
