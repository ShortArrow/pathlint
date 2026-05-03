# pathlint — Product Requirements Document

**Status:** 0.0.x in progress. Schema and CLI surface remain in
motion through 0.1.0; the current crate version is whatever
`Cargo.toml` (and the crates.io badge in the README) say.

---

## 1. Overview

`pathlint` is a CLI that answers four questions about the `PATH` you
actually have, not the one you wish you had.

**R1 — Resolve order.** Given a command, which installer's copy
wins? You declare `[[expect]] command = "x" prefer = ["cargo"]`, and
pathlint checks. This is the original use case and the spine of the
tool.

**R2 — Existence and shape.** Is the file pathlint resolved
actually executable, or did something replace `runex` with a
directory of the same name? Is the symlink broken? Adding
`kind = "executable"` to an `[[expect]]` makes pathlint verify
the resolved path is a real executable file on top of the source
check.

**R3 — PATH hygiene.** Even before any expectation is evaluated,
the `PATH` itself is often a mess: duplicates, dangling directories,
8.3 short names, entries that could be written more concisely.
`pathlint doctor` lints the PATH on its own.

**R4 — Provenance.** `pathlint where <command>` reports the
resolved binary's full path, the catalog sources it matches, and
the most plausible uninstall command (`mise uninstall cargo:lazygit`,
`cargo uninstall lazygit`, ...). For binaries served through mise's
plugin layer it also infers the upstream installer.

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
  another debugging tool. `pathlint check --explain` (0.0.7+) opts
  in to a multi-line breakdown that names the offending `avoid`
  source, lists the `prefer` candidates that didn't match, and
  points at `pathlint where <command>` for the uninstall hint.
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
  your call. `pathlint sort --dry-run` prints a recommended order
  but never applies it.
- **No `which` clone (R1 boundary).** pathlint does include resolve
  logic internally, but it doesn't aim to replace `where` /
  `type -a` / `Get-Command -All`. The R1 question is "is the right
  installer winning?", not "where does this resolve?". R4
  (`pathlint where`) surfaces the resolved path prominently, but
  with provenance, not as a generic which-clone.
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
pathlint check --explain              # multi-line NG breakdown (0.0.7+)
pathlint check --json                 # JSON array of every outcome (0.0.7+)
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
  line with details. Pass `--explain` to expand each NG line into
  six rows (`resolved:` / `matched sources:` / `prefer:` / `avoid:` /
  `diagnosis:` / `hint:`); the diagnosis sentence is variant-
  specific (NgWrongSource names the offending `avoid` source if
  any, NgUnknownSource says the path is outside every defined
  source, NgNotFound advises install / `optional = true`,
  NgNotExecutable carries the underlying reason).
- `--json` swaps the human output for a single pretty-printed
  array: each element has `command`, `status` (snake_case
  `Status` variant), optional `resolved` / `matched_sources` /
  `prefer` / `avoid`, and on failures a tagged `diagnosis` object
  with `kind` ∈ {`wrong_source`, `unknown_source`, `not_found`,
  `not_executable`, `config`} plus the matching payload fields
  (`matched`, `prefer_missed`, `avoid_hits`, `reason`, `message`).
  The JSON view is the single source of truth in machine
  pipelines, mirroring the human view exactly. `--explain` and
  `--json` are mutually exclusive.
- Exit code: `0` if no expectation has status `NG` or `not_found`
  (excluding `optional` and `severity = "warn"` rules), `1`
  otherwise. Same exit codes apply to `--json` output.
- **Per-rule severity (0.0.7+).** Each `[[expect]]` accepts an
  optional `severity` field with values `"error"` (default) or
  `"warn"`. `error` keeps 0.0.x semantics: NG escalates to exit 1.
  `warn` reports the same diagnostic with a `[warn]` tag and
  leaves the exit code at 0 — appropriate for CI nudges where a
  single rogue path should not block the build. The choice is
  per-rule; an `error` rule and a `warn` rule may coexist in the
  same `pathlint.toml`. The severity is surfaced in
  `check --json` for tooling.

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
- (0.0.6+) `--include <kind>[,<kind>...]` shows only the named
  kinds; `--exclude <kind>[,<kind>...]` suppresses them. The two
  flags are mutually exclusive. Filter values are the snake-case
  kind names (`duplicate` / `missing` / `shortenable` /
  `trailing_slash` / `case_variant` / `short_name` /
  `malformed` / `mise_activate_both`); an unknown name is
  reported as a config error (exit 2). The exit code reflects
  the *kept* set, so `--exclude malformed` genuinely lets a run
  pass even when the underlying analysis would have escalated.
- (0.0.7+) `--json` swaps the human view for a JSON array. Each
  element has `index`, `entry`, `severity` (`"warn"` / `"error"`),
  the discriminator `kind`, and any per-kind payload fields
  (`suggestion` for shortenable, `canonical` for case_variant,
  `first_index` for duplicate, `reason` for malformed, and
  `shim_indices` / `install_indices` for mise_activate_both).
  Schema is stable through 0.0.x and parallels `check --json` /
  `where --json`, completing the 3-way machine-readable surface.
  The include / exclude filters still apply; `--quiet` is ignored
  in JSON mode (the output is intended to be complete).

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
- a `provenance:` line when a `[[relation]] kind = "served_by_via"`
  declaration matches: the resolved path lives under the relation's
  `host` source and the next path segment matches the relation's
  `guest_pattern`. The relation's `installer_token` (or
  `guest_provider` as fallback) becomes the installer label, and
  the raw segment is preserved so the user can verify with the
  installer's own tooling.

  Before 0.0.10 this was a hard-coded `MISE_PLUGIN_PREFIXES` table
  inside `where_cmd.rs`; 0.0.10 reads `plugins/<name>.toml` instead,
  so users can extend wrapper attribution by adding a relation to
  `pathlint.toml`.
- a single best-guess uninstall command. When provenance is
  present the hint is `<installer> uninstall '<rest>'` (or, for
  mise plugins, `mise uninstall <installer>:'<rest>'`) with a
  "best-guess; verify" caveat. Otherwise the hint comes from the
  matched source's `uninstall_command` template.

The `{bin}` substitution and the mise plugin segment go through
`format::quote_for(os, _)` (0.0.10+) so a hostile PATH entry like
`/.../installs/cargo-$(rm -rf ~)/bin` cannot inject shell code into
a copy-paste of the output. The escape is single-quote based on
POSIX shells and PowerShell-style on Windows.

The uninstall hint is a string the user runs themselves; pathlint
never executes it. When neither provenance nor the catalog can
suggest a command the output says so explicitly rather than
guessing.

Plugin provenance is a relation-driven label — a R4-only signal,
never a source match. `prefer = ["cargo"]` in `[[expect]]` will
NOT match a binary under `mise/installs/cargo-foo/...` unless the
user explicitly defines a `[source.X]` for that prefix.

(0.0.6+) `--json` switches the output to a single
machine-readable object. The schema is stable for `0.0.x`:

```json
{
  "found": true,
  "command": "lazygit",
  "resolved": "/home/u/.local/share/mise/installs/cargo-lazygit/0.61/bin/lazygit",
  "matched_sources": ["mise_installs", "mise"],
  "uninstall": {
    "kind": "command",
    "command": "mise uninstall cargo:lazygit  (best-guess; verify with `mise plugins ls`)"
  },
  "provenance": {
    "kind": "mise_installer_plugin",
    "installer": "cargo",
    "plugin_segment": "cargo-lazygit"
  }
}
```

`uninstall.kind` is `"command"`, `"no_template"` (carries
`source`), or `"no_source"`. `provenance` is `null` when no
heuristic fired. NotFound emits `{ "command": "...", "found":
false }` and exits 1.

Naming: `where` overlaps with Windows `where.exe`, but pathlint's
output is provenance-focused and clearly distinct in style. If the
overlap proves too confusing in practice the name will be revisited
before 0.1.0.

### 7.8 `pathlint sort` (R5 — repair, implemented in 0.0.8 as
read-only)

- Computes a PATH order that satisfies every applicable
  expectation. Read-only: prints a before / after diff (default)
  or a `SortPlan` JSON object (`--json`). pathlint never rewrites
  PATH itself — pair the output with a shell snippet, registry
  edit, or dotfiles diff to apply.
- Algorithm: for each `[[expect]]` whose `os` filter applies,
  every PATH entry is classified as **preferred** (matches the
  rule's `prefer`), **avoided** (matches `avoid`), or neutral.
  `avoid` wins when an entry matches both sets, mirroring
  `lint::decide`. The plan then concatenates three buckets in
  order: preferred entries, neutral entries, avoided entries.
  Each bucket preserves the entries' original relative order
  unless a `[[relation]] kind = "prefer_order_over"` (0.0.10+)
  applies — those reorder entries **within** the same bucket but
  never cross bucket boundaries. The diff only contains moves the
  user actually needs to think about. Rules with both `prefer` and
  `avoid` empty do not
  contribute. Entries matching no defined source stay in their
  bucket.
- When `prefer` cannot be satisfied by reordering (no PATH entry
  matches any of the listed sources), the plan emits a
  `SortNote::UnsatisfiablePrefer` listing the command and the
  prefer set — the only fix is to install via one of those
  sources or relax the rule.
- Always exits 0; `sort` is a *suggestion* command, not a
  pass / fail check. Use `pathlint check` for go / no-go.
- `--apply` is not shipped in 0.0.8. PRD §4 forbids PATH
  mutation; revisiting `--apply` is on the post-1.0 list and
  would live behind an explicit flag.

## 8. `pathlint.toml` schema

```toml
# ---- [[expect]]: per-command expectations ----

# Untagged: applies on every OS. Add `os = [...]` to restrict it.
# (pathlint does NOT auto-skip rules whose preferred sources happen
# to lack a per-OS path on the current OS — the rule still runs.)
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

The default catalog ships as one TOML file per package manager
under `plugins/`. `build.rs` concatenates them into a single
embedded string at compile time. Adding a package manager means
adding a TOML file there and listing its name in
`plugins/_index.toml`.

The current set, grouped:

| Group | Plugins / sources |
|---|---|
| Generic user dirs | `user_bin`, `user_local_bin` |
| Language toolchains | `cargo`, `go`, `npm_global`, `pip_user` |
| Polyglot version managers | `mise` / `mise_shims` / `mise_installs`, `volta`, `aqua`, `asdf` |
| Windows package managers | `winget`, `choco`, `scoop` |
| Windows-specific | `WindowsApps`, `strawberry`, `mingw`, `msys` |
| macOS package managers | `brew_arm`, `brew_intel`, `macports` |
| Linux package managers | `apt`, `pacman`, `dnf`, `flatpak`, `snap` |
| Termux | `pkg`, `termux_user_bin` |
| OS baseline | `system_windows`, `system_macos`, `system_linux` |

Run `pathlint catalog list` to dump the resolved catalog with
each source's per-OS path, including any overrides the user
added. The TOML for any individual plugin is
in `plugins/<name>.toml` in the source tree.

**Source path constraints (0.0.10+):** every `[source.<name>]`
per-OS path is validated at startup before `check`, `doctor`,
`where`, and `sort` consume the catalog. A source whose expanded
needle is `/`, `\`, or shorter than 3 bytes is rejected with
exit 2. Relative needles like `Microsoft/WindowsApps` (used by
fragment-style built-ins) are still accepted; the `find` boundary
check keeps them from over-matching across path segments.

Notes on the design:

- `apt` / `pacman` / `dnf` all point at `/usr/bin` because that is
  where their installed binaries land. They are aliases of "the
  Linux distro" from pathlint's perspective; users pick whichever
  name reads best in their `pathlint.toml`.
- `brew_arm` and `brew_intel` are split because `/opt/homebrew/bin`
  vs `/usr/local/bin` ordering on a single Mac is itself a typical
  source of bugs.
- `WindowsApps` and `strawberry` are listed primarily so they can
  appear in `avoid = [...]` lists.

### 9.1 Relations between sources (0.0.9+)

Plugins can declare structural relationships between sources using
`[[relation]]` blocks. Users can declare their own in
`pathlint.toml` to extend the same vocabulary to custom sources.
Run `pathlint catalog relations` to dump the merged list (use
`--json` for tooling).

Five `kind`s are recognised:

- **`alias_of`** — a parent source is a catch-all over more
  specific children. Matching the parent does not exclude matching
  the children. `pathlint where` pushes the parent to the end of
  the matched-sources list when at least one child also matched.
  Used for `mise` over `mise_shims` / `mise_installs`.
- **`conflicts_when_both_in_path`** — two or more sources that
  shouldn't be active in PATH at once. `pathlint doctor` raises
  `diagnostic` (a `Kind` snake_case name) when more than one of
  them appears. (0.0.10 still uses the hard-coded `mise_activate_both`
  detector; relation-driven doctor is the 0.0.11 list.)
- **`served_by_via`** — `host` serves binaries originally from
  `guest_provider` via paths matching `guest_pattern`. The
  optional `installer_token` field (0.0.10+) names the installer
  for human-facing output when it differs from the source name —
  e.g. `guest_provider = "pip_user"` but `installer_token = "pipx"`
  because the user runs `mise uninstall pipx:black`.
  `pathlint where` consumes this directly.
- **`depends_on`** — `target` is a hard prerequisite of `source`.
  Reads "`source` depends on `target`". Example: `paru` depends on
  `pacman`, so uninstalling `paru` does not remove pacman-managed
  binaries. Surfaced by `pathlint where`.
- **`prefer_order_over`** (0.0.10+) — `earlier` should appear in
  PATH before `later`. Consumed by `pathlint sort` to break ties
  inside the preferred / neutral / avoided buckets. Bucket
  boundaries are not crossed: a `prefer_order_over` cannot rescue
  an avoided source. Forms a directed edge for the cycle check.

Example (built into `plugins/mise.toml`):

```toml
[[relation]]
kind = "alias_of"
parent = "mise"
children = ["mise_shims", "mise_installs"]

[[relation]]
kind = "conflicts_when_both_in_path"
sources = ["mise_shims", "mise_installs"]
diagnostic = "mise_activate_both"

[[relation]]
kind = "served_by_via"
host = "mise_installs"
guest_pattern = "cargo-*"
guest_provider = "cargo"
installer_token = "cargo"
```

`served_by_via`, `depends_on`, and `prefer_order_over` describe
directed edges; pathlint checks that the merged graph is acyclic
when running `pathlint catalog relations`. A cycle is a
configuration error (exit 2). `alias_of` and
`conflicts_when_both_in_path` are symmetric and never participate
in the DAG check.

In 0.0.9 the relation list was purely descriptive. In 0.0.10
`pathlint where` reads `served_by_via` + `alias_of` directly (the
old `MISE_PLUGIN_PREFIXES` table is gone) and `pathlint sort` reads
`prefer_order_over`. Doctor still uses the hard-coded
`mise_activate_both` substring detector; migrating it to read
`conflicts_when_both_in_path` is the 0.0.11 list.

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
    list       list every known source (built-in + user)
    relations  list declared [[relation]] between sources
  doctor   Lint the PATH itself (duplicates, missing dirs, etc.)
  where    Show where a command resolves from + uninstall hint
  sort     Propose a PATH order satisfying every [[expect]] rule
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

`pathlint sort` is a read-only proposal (see §7.8). `--apply` is
held back by PRD §4's "no PATH mutation" policy and is on the
post-1.0 list.

`pathlint catalog relations` prints the source relations declared
by built-in plugins and any user `[[relation]]` blocks (see §9.1).

## 12. Non-functional requirements

- **Single Rust binary.** No runtime deps beyond the OS itself.
- **Cross-platform first-class.** Windows, macOS, Linux all run in CI.
  Termux runs from `cargo install` on the device — no prebuilt
  Termux binary, mirroring `dotfm`'s policy.
- **Startup time.** `pathlint check` < 50 ms on a warm cache for a
  PATH of ~100 entries and ~20 expectations.
- **Stable exit codes.** `0` clean, `1` expectation failure, `2`
  config / I/O error.
- **Encoding.** All paths are treated as UTF-8 strings on every OS.
  If `PATH` is not valid UTF-8, pathlint reads it as if empty; a
  warning + per-entry skip is a future improvement. (0.0.10+)
  Human output of `pathlint where` runs every attacker-controlled
  string through `format::strip_control_chars`, replacing ASCII
  control bytes (0x00–0x08, 0x0B–0x1F, 0x7F) with `?` so a hostile
  PATH segment cannot rewrite the terminal. `\t` and `\n` are
  preserved. Doctor / catalog list still emit raw strings; that is
  the 0.0.11 list.
- **Trust boundary for shell strings (0.0.10+).** `pathlint where`
  emits commands the user might copy-paste. The `{bin}`
  substitution and the mise plugin segment are quoted via
  `format::quote_for(os, _)` (POSIX single-quote on Unix-likes,
  PowerShell single-quote on Windows). Catalog template *bodies*
  themselves (the `uninstall_command = "..."` string) are not
  re-quoted — they come from the catalog author or user config and
  pathlint trusts them.
- **Built-in catalog versioning.** The catalog is embedded at compile
  time; bumps to it are called out in the GitHub Release notes so
  users know when defaults change. 0.0.10 bumps `catalog_version`
  to `3` because relation interpretation changed (where / sort now
  read the relations).

## 13. Distribution

- Published on crates.io as `pathlint`.
- GitHub Releases ship `x86_64-{linux,windows,darwin}` and
  `aarch64-darwin` archives. Termux users build from source via
  `cargo install pathlint`.
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
