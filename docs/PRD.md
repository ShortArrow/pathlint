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

## 2. Problem statement

PATH ordering bugs are common and quietly painful:

- A Microsoft Store stub for `python.exe` shadows the real install
  (mise, conda, asdf, manual ...).
- Strawberry Perl's `gcc` shadows a Rust/MSYS toolchain.
- A leftover `WindowsPowerShell\v1.0` entry resolves to `pwsh.exe`
  instead of PowerShell 7.
- `cargo install`-deployed binaries in `%UserProfile%\.cargo\bin` lose
  to a same-named tool earlier in the Machine PATH.

`which X` shows what wins but not what should win. There is no
declarative way to encode "what should win" so that CI / dotfiles /
doctor scripts can check it.

`pathlint` fills that gap.

## 3. Goals

- **Declarative rules.** A `pathlint.toml` file with `[[rule]]` entries
  expresses "X must precede Y" in plain TOML.
- **Substring + case-insensitive match.** Rule keys do not need to be
  exact paths; "mise\\shims" matches any entry containing that
  substring after env-var expansion. This is intentional: the same
  rule should work across machines where `%UserProfile%` differs.
- **OS-aware sources.** `--target process|user|machine` selects what
  `PATH` to read. On Windows `user` and `machine` come from the
  registry; on Linux only `process` is meaningful.
- **Honest exit codes.** `0` = all rules satisfied or skipped, `1` =
  at least one rule failed.
- **Useful failure output.** Each failing rule prints the indices of
  the offending entries and a brief reason ("'chocolatey\\bin' at #42
  precedes 'mise\\shims' at #49").
- **No mutation in MVP.** Read-only; `--apply` / `sort` are deferred.

## 4. Non-goals (MVP)

- **No PATH rewriting / persisting.** Sort/fix is later.
- **No pretty-printing of the entire PATH** beyond what is needed to
  contextualize a failure.
- **No shell-completion installation.** `pathlint completions <shell>`
  may land later.
- **No package management.** `pathlint` does not install missing tools
  to satisfy a rule.

## 5. Target users

- Dotfiles maintainers wanting their `doctor` step to catch PATH drift.
- Developers debugging "why does this Python run instead of that one"
  in a way that survives reboots.
- CI pipelines that bootstrap a developer environment and want to
  fail loudly on PATH ordering regressions.

## 6. User stories

- I write `pathlint.toml` once with the few rules I actually care
  about, commit it to my dotfiles, and `pathlint check` runs it on
  every machine.
- A linter run prints every rule and its status; failures show me
  which entry beats which entry.
- I can ask `pathlint check --target user` to verify only my user PATH
  before doing `setx PATH ...`.
- (post-MVP) I run `pathlint sort --target user --dry-run` and see a
  diff of what would change.

## 7. Functional requirements (MVP)

### 7.1 `pathlint check [--target <process|user|machine>] [--rules <path>]`

- Default `--target` is `process` (`$env:PATH` / `$PATH` after expansion).
- Default `--rules` resolution order:
  1. explicit `--rules <path>`
  2. `./pathlint.toml`
  3. `$XDG_CONFIG_HOME/pathlint/pathlint.toml` (or
     `$HOME/.config/pathlint/pathlint.toml`)
- Reads the rules, evaluates each, prints one line per rule plus
  details for failures.
- Exit code: `0` if no rule has status `fail`, `1` otherwise.

### 7.2 `pathlint which <command>` (MVP)

- Resolves the command across PATH using OS rules
  (`PATHEXT` on Windows, `+x` on Unix).
- Prints the winning path first, then any shadowed copies further down
  PATH, with a short `[shadowed]` annotation. The point is to make the
  "first wins, the rest are reachable but unused" relationship visible.
- Exit code: `0` if at least one match, `1` otherwise.

### 7.3 `pathlint.toml` schema

```toml
# Each [[rule]] asserts: at least one entry containing `before` must come
# earlier in PATH than every entry containing any of `after`.

[[rule]]
name   = "mise shims override system tools"
before = "mise\\shims"
after  = ["chocolatey\\bin", "Strawberry\\c\\bin"]

[[rule]]
name   = "user cargo bin precedes Strawberry's gcc/perl"
before = ".cargo\\bin"
after  = ["Strawberry"]
```

Match semantics:

- Substring, case-insensitive, against each PATH entry **after**
  environment-variable expansion.
- A rule is **OK** if every `after` match comes after the first
  `before` match.
- A rule is **fail** if any `after` match comes before all `before`
  matches, OR if `after` has matches but `before` has none.
- A rule is **skip** if neither side is present in PATH.

### 7.4 Path source resolution

- `process`: read `$env:PATH` (or `$PATH` on Unix).
- `user` (Windows only): read
  `HKCU\Environment\Path` from the registry.
- `machine` (Windows only): read
  `HKLM\System\CurrentControlSet\Control\Session Manager\Environment\Path`.
- On Unix, `--target user|machine` fall back to `process` with a warning.

### 7.5 Output

- Default: one line per rule (`OK` / `NG` / `skip`), failures get a
  second indented line with details.
- `--verbose`: also dump expanded PATH entries.
- `--quiet`: only print failures.

## 8. Non-functional requirements

- **Single Rust binary.** No runtime deps beyond the OS.
- **Cross-platform.** Windows (primary), macOS, Linux. Termux is
  source-build only (same policy as `dotfm`).
- **Startup time.** `pathlint check` < 50 ms on a warm cache for a
  PATH of ~100 entries and ~20 rules.
- **Stable exit codes.** `0` clean, `1` rule failure, `2` config
  parse / I/O error.

## 9. Distribution

- crates.io publish once 0.0.1 ships.
- GitHub Releases workflow shipping `x86_64-{linux,windows,darwin}`
  and `aarch64-darwin` archives, mirroring `dotfm`.
- (post-MVP) Homebrew / scoop / winget formulae.

## 10. Out of scope

- PATH editing / persistence (deferred to post-MVP `sort` mode).
- `which` over function/alias resolution — only file-on-PATH lookup.
- Shell-config patching (`.bashrc`, `$PROFILE` rewriting).
- Detecting *missing* commands beyond what rule evaluation produces as
  a side effect.

## 11. Success metrics

- The reference dotfiles (`ShortArrow/dotfiles`) replaces its
  `windows/Test-PathOrder.ps1` with a `pathlint check` invocation in
  `windows/doctor.ps1`, and the rules-file lives in the same repo.
- A user can write a 5-rule `pathlint.toml` in under a minute by
  copy/edit from the README.
- A failing run names every offending pair clearly enough to fix
  without further debugging tools.

## 12. Open questions

- **Negative match (`before_not`)** for "the user `go\\bin`, not the
  system one". Not in MVP; revisit when a real second-rule conflict
  hits.
- **Windows-only env-var expansion.** Currently planned to expand all
  `%VAR%` (Windows-style) and `$VAR` / `${VAR}` (POSIX-style)
  uniformly; might want OS-specific behavior.
- **Shell completions.** `clap_complete` is cheap to add but not part
  of MVP.
- **macOS launchd / Linux PAM PATH sources.** Process-level PATH on
  those OSes is the union of many things; do we surface them?

## 13. Relationship to other tools

- **`which` / `where.exe`**: same domain (find where a command
  resolves) but no notion of "should". `pathlint which` complements
  rather than replaces them.
- **`dotfm doctor`**: `pathlint check` is intended to be invoked from
  a `dotfm.toml` `[tools.windows.doctor]` script (or its successor),
  not to replace `dotfm`.
- **`PATH.txt` / `DiffPath.ps1` (in `ShortArrow/dotfiles`)**: those
  check *whether expected entries exist*; `pathlint` checks *whether
  the order is right*. The two are complementary.
