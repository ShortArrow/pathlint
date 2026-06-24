# pathlint — Product Requirements Document

🌐 **English** | [日本語](PRD.jp.md)

**Status:** 0.0.x in progress. Schema and CLI surface remain in
motion through 0.1.0; the current crate version is whatever
`Cargo.toml` (and the crates.io badge in the README) say. The
0.0.x → 0.1.0 graduation criteria are listed in §3.1; design
decisions accumulate in [`docs/decisions/`](decisions/).

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

**R3 — PATH hygiene + selfcheck.** Even before any expectation
is evaluated, the `PATH` itself is often a mess: duplicates,
dangling directories, 8.3 short names, entries that could be
written more concisely. `pathlint lint` lints the PATH on its
own (new name in 0.0.34, per ADR-0028; previously this was
`pathlint doctor`). `pathlint doctor` now answers a different
question — is pathlint itself functional in this environment?
(binary on PATH, `pathlint.toml` parseable, env vars readable).

**R4 — Provenance.** `pathlint trace <command>` reports the
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
`flatpak`, `windows_apps`, ...). Users only have to write their
**expectations**; sources are looked up by name.

## 2. Problem statement

The same command name often comes from different installers, and you
care which one wins:

- I ran `cargo install runex` on this machine, but the binary that
  actually fires is the older one in `WinGet/Links` — same name,
  different file.
- `python` should come from `mise`, not from the Microsoft Store
  `windows_apps` stub.
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
  points at `pathlint trace <command>` for the uninstall hint.
- **R2 (existence and shape).** When a command resolves to a path,
  the path must point at an actually-executable file. Symlinks
  must be alive; "executable" must mean it. Today only `not_found`
  is reported; the rest is 0.0.4+.
- **R3 (PATH hygiene + selfcheck).** Two sibling commands since
  0.0.34 (ADR-0028):
  - `pathlint lint` — even with no `[[expect]]` written, flags
    duplicates, dangling directories, 8.3 short names, env-var-
    shortenable entries, shadowed commands across PATH dirs,
    relative entries, world-writable directories, and malformed
    entries that would never resolve. Inherits the 12 detector
    kinds previously emitted by `pathlint doctor` (0.0.13–0.0.33).
  - `pathlint doctor` — checks pathlint itself is functional in
    this environment: binary self-locate on PATH,
    `pathlint.toml` discovery + parse, and `env_lookup`
    operational (`PATH`, `PATHEXT` on Windows, `HOME` /
    `USERPROFILE` for config search). Does not inspect PATH for
    anomalies.
- **R4 (provenance).** Given a resolved binary, name the installer
  it most plausibly came from, and the corresponding uninstall
  command. Useful when the user can't remember whether they ran
  `cargo install` or `mise use cargo:tool` six months ago.

### 3.1 Graduation to 0.1.0

The 0.0.x → 0.1.0 bump is gated on the following criteria. None
of them is "implemented enough"; each is a concrete pin a
reviewer can verify. ADR-0005 records the pre-1.0 BREAKING
licence that this gate retires.

1. **Public API freeze (lib).** The 10 modules listed in
   `tests/public_api.rs` keep their surfaces for ≥ 2 consecutive
   releases without a `### Breaking` entry in `CHANGELOG.md`.
2. **CLI surface freeze.** `pathlint <subcommand>` and the
   global flag set match the table in §11 for ≥ 2 consecutive
   releases.
3. **Schemars 1.0 migration evaluated.** Either migrated, or an
   ADR rejects the migration for 0.1.0 with a written reason.
4. **Trust model documented.** [`docs/SECURITY.md`](SECURITY.md)
   describes every boundary, with sanitisation pointers into
   code, and is kept in sync with the implementation.
5. **ADR completeness.** Every release in the 0.0.x line whose
   `### Breaking` section in `CHANGELOG.md` names a publicly
   visible type or function has at least one ADR linked from
   the corresponding `docs/decisions/NNNN-*.md` file.
6. **Documentation parity.** EN ↔ JP PRD diff is < 50 lines of
   semantic content (table-of-contents-only and link-only diffs
   excluded).
7. **No open H severity codex audit findings.** Either resolved
   or downgraded with an ADR that explains why the H rating no
   longer applies.

Criteria 1, 2, 5, 6, 7 are mechanical (countable). 3 and 4 are
narrative gates. The graduation verification record is itself an
ADR (planned), written at the moment the criteria audit passes;
its number depends on what else has shipped by then (ADR-0009
through ADR-0011 were assigned to other backlog drainage entries
in 0.0.30, so the verification record will land at a later
number).

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
  (`pathlint trace`) surfaces the resolved path prominently, but
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

The shape of the §3 / §4 boundary — OS layout knowledge as first-class,
tool meta as declarative-only catalog entries, no modelling of tool
runtime behavior — is anchored in
[ADR-0032](decisions/0032-scope-os-knowledge-tool-meta-declaration.md).
That ADR is the canonical rejection target for future "should pathlint
know what mise / asdf / volta currently has active?" requests.

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
| R3 — PATH hygiene | `pathlint lint` (formerly `pathlint doctor`) | implemented (0.0.3); renamed in 0.0.34 per ADR-0028 |
| R3' — selfcheck | `pathlint doctor` | implemented (0.0.34) |
| R4 — provenance | `pathlint trace <command>` | implemented (0.0.4) |

`pathlint init` and `pathlint catalog list` are infrastructure
subcommands (configuration scaffolding, catalog inspection); they
serve every role but don't belong to any one of them.

### 7.1 `pathlint [OPTIONS]` (= `pathlint check`)

`check` is the default subcommand; bare `pathlint` runs it.

```
pathlint                              # = pathlint check
pathlint --target user                # explicit target
pathlint --config ./other.toml
pathlint --verbose                    # also show n/a expectations and resolved PATH
pathlint --quiet                      # only print failures
pathlint check --explain              # multi-line NG breakdown (0.0.7+)
pathlint check --json                 # JSON array of every outcome (0.0.7+)
```

- `--target` default is `process`. `user` / `machine` are accepted
  everywhere but only meaningful on Windows; on Unix they print a
  one-line warning and fall back to `process`.
- `--config` default resolution order:
  1. `--config <path>` if given.
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

### 7.5 `pathlint lint` and `pathlint doctor` (0.0.34+, ADR-0028)

R3 splits into two sibling commands as of 0.0.34. The
responsibility split was driven by Round 1 dotfiles dogfooding
revealing that the 0.0.33 `doctor` did two unrelated jobs (PATH
hygiene vs pathlint selfcheck) under one name.

#### `pathlint lint` (PATH hygiene)

Inherits the 12 detector kinds previously emitted by
`pathlint doctor` (0.0.13–0.0.33). The `Diagnostic` JSON shape,
the kind enum, the `--include` / `--exclude` filter UX, the
`--json` output array, the schema (`schemas/doctor.schema.json`,
shared with the new doctor surface), and the exit-code semantics
are all preserved verbatim — only the CLI verb changes from
`doctor` to `lint`.

#### `pathlint doctor` (selfcheck)

Three checks only (ADR-0028):
1. Binary self-locate — running pathlint binary is on PATH.
2. `pathlint.toml` discovery + parse — found via cwd or
   `$XDG_CONFIG_HOME`, and the parser succeeds. Semantic
   validation (does `[source.x] path` resolve? does
   `[[expect]] command` match a catalog entry?) is not checked
   here; that is scoped for a future release.
3. `env_lookup` operational — `PATH`, plus `PATHEXT` on Windows,
   `HOME`/`USERPROFILE` for config search.

Selfcheck kinds (additive to the shared schema):
`binary_not_in_path`, `config_parse_error`, `config_not_found`
(info severity — running without a config is legitimate),
`env_lookup_failed`. The `Severity` enum gains `info` as a new
discriminant alongside `warn` / `error`.

#### Behaviour shared with the 0.0.33 doctor (now under `lint`):
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
  - `Conflict` — two or more sources that should not coexist in
    PATH have all matched. The diagnostic name comes from a
    `Relation::ConflictsWhenBothInPath` declaration (built-in or
    user-supplied via `pathlint.toml`). Output enumerates each
    source's matched PATH entries under a numbered `group #N:`
    block. Built-in coverage as of 0.0.11: `mise_activate_both`
    (mise shim + install layers active simultaneously). Users
    can declare new conflicts without touching pathlint by adding
    `[[relation]] kind = "conflicts_when_both_in_path"` blocks.
    Before 0.0.11 this was a hard-coded `mise/shims` vs
    `mise/installs` substring check; relation-driven detection
    needs the relation's declared sources to actually match the
    current PATH (so a user override that points
    `[source.mise_shims]` somewhere else changes when the
    diagnostic fires — usually the right behaviour).
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
  `diagnostic` + `groups` for conflict). Before 0.0.11 the
  conflict variant was emitted as `kind="mise_activate_both"`
  with `shim_indices` / `install_indices`; that shape is
  retired. The schema parallels `check --json` /
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

### 7.7 `pathlint trace <command>` (R4, implemented in 0.0.4; plugin provenance in 0.0.5)

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
    "command": "mise uninstall cargo:'lazygit'  (best-guess; verify with `mise plugins ls`)"
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
- **Relation consumption (0.0.12+).** `pathlint sort` consumes
  only the `prefer_order_over` relation kind. The other four
  kinds (`alias_of`, `conflicts_when_both_in_path`,
  `served_by_via`, `depends_on`) describe the source graph but
  do not influence the sort order. Future ordering rules — e.g.
  promoting `mise_installs` ahead of `mise_shims` by default —
  would need a new relation kind, not a reinterpretation of an
  existing one. This avoids the trap where adding a
  `served_by_via` for a new wrapper installer silently changes
  PATH ordering recommendations.

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
avoid   = ["windows_apps", "choco"]
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

### 8.4 JSON Schema for editors (shipped in 0.0.11)

The TOML format itself has no built-in schema mechanism, but
Taplo (the dominant TOML LSP, also bundled in VS Code's "Even
Better TOML" extension) consumes JSON Schema. 0.0.11 ships:

1. `schemars` as a runtime dep, deriving `JsonSchema` on the
   live `Config` / `Expectation` / `SourceDef` / `Relation` /
   `Severity` / `Kind` types. The schema cannot drift from the
   parser because both come from the same Rust types.
2. `src/bin/gen_schema` prints the schema; `tests/schema.rs` is
   a CI drift gate that fails when the checked-in
   `schemas/pathlint.schema.json` diverges from what the
   generator currently emits.
3. The `release` workflow re-runs the generator on the tagged
   commit and uploads `pathlint.schema.json` as a GitHub
   Release asset (alongside the binaries and `SHA256SUMS`).

Two stable URLs for users to pin:

- **Latest main** (auto-updates per merge):
  `https://raw.githubusercontent.com/ShortArrow/pathlint/main/schemas/pathlint.schema.json`
- **Specific release** (frozen at tag — replace `<TAG>` with
  the version you want, e.g. `v0.0.13`):
  `https://github.com/ShortArrow/pathlint/releases/download/<TAG>/pathlint.schema.json`

Users opt in with a single line at the top of `pathlint.toml`:

```toml
#:schema https://raw.githubusercontent.com/ShortArrow/pathlint/main/schemas/pathlint.schema.json
```

A follow-up PR to https://www.schemastore.org/ matches
`pathlint.toml` by filename so Taplo / Even Better TOML resolve
the schema automatically without per-user opt-in. Schema Store
registration is tracked separately from the pathlint release
cycle.

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
| Windows-specific | `windows_apps`, `strawberry`, `mingw`, `msys` |
| macOS package managers | `brew_arm`, `brew_intel`, `macports` |
| Linux package managers | `apt`, `pacman`, `dnf`, `flatpak`, `snap` |
| Termux | `pkg`, `termux_user_bin` |
| OS baseline | `os_baseline_windows`, `os_baseline_macos`, `os_baseline_linux` |

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
- `windows_apps` and `strawberry` are listed primarily so they can
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
  the children. `pathlint trace` pushes the parent to the end of
  the matched-sources list when at least one child also matched.
  Used for `mise` over `mise_shims` / `mise_installs`.
- **`conflicts_when_both_in_path`** — two or more sources that
  shouldn't be active in PATH at once. `pathlint doctor` (0.0.11+)
  walks every relation and emits a `Kind::Conflict` diagnostic
  with the relation's `diagnostic` label and per-source matched
  PATH entries. Built-in coverage: `mise_activate_both`. Users
  add new conflicts by writing more relations, no code change
  needed.
- **`served_by_via`** — `host` serves binaries originally from
  `guest_provider` via paths matching `guest_pattern`. The
  optional `installer_token` field (0.0.10+) names the installer
  for human-facing output when it differs from the source name —
  e.g. `guest_provider = "pip_user"` but `installer_token = "pipx"`
  because the user runs `mise uninstall pipx:black`.
  `pathlint trace` consumes this directly.
- **`depends_on`** — `target` is a hard prerequisite of `source`.
  Reads "`source` depends on `target`". Example: `paru` depends on
  `pacman`, so uninstalling `paru` does not remove pacman-managed
  binaries. **Descriptive only** — the kind shows up in
  `pathlint catalog relations` output and participates in the
  cycle check, but no other subcommand currently consumes it.
  Surfacing it from `pathlint trace` (e.g. as a "you also need to
  uninstall X" hint) is post-1.0 work; until then the relation
  is data the user can grep, not a runtime signal.
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

In 0.0.9 the relation list was purely descriptive. 0.0.10
made `pathlint trace` read `served_by_via` + `alias_of` (the
old `MISE_PLUGIN_PREFIXES` table is gone) and `pathlint sort`
read `prefer_order_over`. 0.0.11 closes the loop: `pathlint
doctor` reads `conflicts_when_both_in_path` to fire
`Kind::Conflict` diagnostics. The whole relation graph is
relation-driven from this release onward; new conflict /
order / provenance behaviour can land entirely as TOML.

Each consumer reads exactly one or two kinds: `where` uses
`served_by_via` + `alias_of`, `sort` uses `prefer_order_over`
only (see §7.8), and `doctor` uses `conflicts_when_both_in_path`.
`depends_on` is currently descriptive — surfaced in the
`catalog relations` output but not consumed by any other
subcommand. This explicit map keeps "adding a relation" from
having unintended effects on commands that did not declare
themselves as consumers.

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

### 10.1 Path entry raw/expanded duality (0.0.23+)

A PATH entry has two semantic forms that detectors and resolvers
care about for different reasons:

- **raw** — the entry as stored at the source. On Windows that is
  the literal `%LocalAppData%\WindowsApps` for a `REG_EXPAND_SZ`
  registry value; on Unix that is `~/.local/bin` or `$HOME/bin` if
  the shell exported `PATH` without expanding the variable.
- **expanded** — `expand::expand_env(raw)`. The directory string
  the OS would actually consult on disk.

`pathlint` captures both at exactly one boundary point:
`pathlint::path_source::read_path` builds a
`pathlint::path_entry::PathEntry { raw, expanded }` for every
entry, and every consumer downstream picks its side from the type.
Detectors that reason about *what the user typed* — Shortenable
(must not suggest shortening an entry the user already shortened),
RelativePathEntry (an unresolved `$VAR/bin` is a config bug worth
surfacing) — read `entry.raw`. Detectors that reason about *the
directory on disk* — Missing, WriteablePathDir, DuplicateButShadowed,
PerSourceMissingRequired — and the `resolve` walker read
`entry.expanded`.

**Windows registry policy.** `winreg`'s
`RegKey::get_value::<String, _>` silently runs
`ExpandEnvironmentStringsW` on `REG_EXPAND_SZ` values, which would
strip the `%LocalAppData%` form before it ever reaches
`PathEntry::from_raw`. `pathlint` instead reads raw bytes via
`RegKey::get_raw_value` and decodes UTF-16 LE in
`path_source::decode_reg_string`. Invalid surrogate pairs are
replaced with `U+FFFD` (lossy decode); registry types other than
`REG_SZ` / `REG_EXPAND_SZ` (`REG_MULTI_SZ`, `REG_BINARY`,
`REG_DWORD`, …) produce an explicit warning and an empty PATH —
pathlint never panics on a hostile payload. The single
`expand_env` call lives in `PathEntry::from_raw`, so Windows and
Unix follow the exact same "raw at the source, expanded at the
boundary" rule.

**User-visible consequence.** `pathlint doctor --target user` /
`--target machine` on Windows now displays `%LocalAppData%`-style
entries verbatim in human and JSON output, matching what the user
has stored in the registry. Pre-0.0.23 the output was the
already-expanded form, which surprised users who recognised the
raw form they typed but not the expanded one the OS produced.

**Decoder failure policy.** `path_source::decode_reg_string` is
*lossy* on invalid UTF-16 surrogate pairs — the offending code
unit is replaced with `U+FFFD` rather than panicking — and
*rejects* registry value types other than `REG_SZ` /
`REG_EXPAND_SZ` (`REG_MULTI_SZ`, `REG_BINARY`, `REG_DWORD`, …) by
returning `Err`. In both error cases `read_path` surfaces a
`warning` and an empty `entries` vector, so pathlint never
panics on a hostile registry payload, never silently emits
diagnostics built from garbled bytes, and never quietly succeeds
on a registry value whose type pathlint doesn't understand.

**Env-lookup injection.** `PathEntry::from_raw(raw, env_lookup)`
takes a `Fn(&str) -> Option<String>` so the constructor reads
the env exclusively through the caller's closure — pathlint
never touches `std::env::var` from inside `from_raw`. The
infrastructure boundary points (`path_source::read_path` and
`resolve::split_path`) inject `|v| std::env::var(v).ok()`; lib
embedders and tests inject deterministic closures so behaviour
is independent of the host environment. The same closure flows
through `expand::expand_env_with`, which is the public-facing
form of the previous `expand::expand_env` (kept as a thin
wrapper over `expand_env_with` with the live process env).

0.0.26+ extends the closure-injection pattern across the rest of
the lib's public matching surface (ADR-0006). `expand` exposes
`expand_and_normalize_with(input, env_lookup)` alongside the
existing `expand_and_normalize(input)` wrapper, and `source_match`
exposes `find_with(haystack, sources, os, env_lookup)`,
`validate_sources_with(sources, os, env_lookup)`, and
`names_only_with(haystack, sources, os, env_lookup)` alongside
their wrappers. An embedder that calls the `_with` variants
exclusively can resolve catalog source paths without ever
reading the process env — closure injection is complete at the
lib boundary.

0.0.27+ closes the internal call-graph threading (ADR-0007). The
four headline entry points — `doctor::analyze`, `lint::evaluate`,
`trace::locate`, `sort::sort_path` — now take typed `*Deps<'_>`
carriers (`AnalyzeDeps`, `EvaluateDeps`, `LocateDeps`, `SortDeps`)
that embed a shared `CommonDeps` holding the env oracle. Every
internal matcher inside those entry points threads
`deps.common.env_lookup` to `source_match::*_with`, so an
embedder that constructs a deterministic carrier never touches
`std::env`. The only remaining wrapper call site in the repo is
`bin/pathlint/run::enforce_source_validation`, which is
binary-side and always wants the live env.

**Observed vs. provenance (0.0.24+, Windows process target; type split in 0.0.28).**
`PathEntry { raw, expanded }` describes a single entry as
observed at one source. There is one Windows case where two
sources disagree: `--target process` calls `getenv("PATH")`, but
the OS expands `REG_EXPAND_SZ` registry values via
`ExpandEnvironmentStringsW` before handing PATH to the child
process. So `raw` on a process entry is always a literal — even
when HKCU has `%LocalAppData%\Microsoft\WindowsApps`. The 0.0.23
raw-preservation fix protects `--target user` / `--target machine`
(which read the registry directly) but does nothing for the
default `--target process`.

0.0.24 introduced a cross-source overlay; 0.0.28 (ADR-0008) split
it out of `PathEntry` into its own carrier `pathlint::Attribution`:

```rust
pub struct Attribution {
    pub observed: PathEntry,
    pub provenance_raw: Option<String>,
}
```

On Windows process target, `path_source::read_process` reads HKCU
and HKLM raw at start-up, then a pure
`reconcile_process_with_registry` overlay sets `provenance_raw`
on each `Attribution` whose `observed.expanded` matches a
registry entry's `observed.expanded`. Detectors that reason
about user intent (`Shortenable`, `Malformed`, `TrailingSlash`,
`ShortName`, plus the human-facing `Diagnostic.entry`) go
through `Attribution::effective_raw_for_user_intent()`, which
prefers `provenance_raw` over `observed.raw`. Filesystem-side
detectors (`Missing`, `WriteablePathDir`, the resolver) keep
reading `attrib.observed.expanded` — the filesystem doesn't
care what the user typed.

The overlay's contract:

- Match condition: `expand::normalize` equality on the two
  `expanded` strings (case-insensitive + slash-unify), so
  `C:\Users\Me\X` and `c:/users/me/x` count as the same entry.
- Tie-break: HKCU before HKLM, then first occurrence within a
  source. Deterministic across runs.
- Skipped when no expanded match is found in either registry
  source. This is the codex-recommended safety stance —
  false-negative (literal stays literal, Shortenable still
  fires) is preferable to false-suppression on a runtime-injected
  PATH (`set PATH=...` in a child shell, `os.environ['PATH']`
  in a Python supervisor, etc.).
- Skipped when registry raw equals process raw verbatim. REG_SZ
  registry entries do not need overlays; only REG_EXPAND_SZ
  values whose `%VAR%` form was expanded by the OS get one.
- `provenance_raw` stays `None` on every other code path:
  `--target user` / `--target machine` (raw is already
  authoritative at the source), Unix / macOS (no registry to
  overlay), and process entries with no registry counterpart.

`--target` itself is unchanged: the three values still describe
*which source pathlint reads*. The overlay is a cross-source
hint that lets `process` recover the original raw form when it
can without renaming process to "the doctor display mode".

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
  trace    Show where a command resolves from + uninstall hint
  sort     Propose a PATH order satisfying every [[expect]] rule
  help     Print help

Options (global):
      --target <process|user|machine>  default: process
      --config <path>                   default: search ./, then $XDG_CONFIG_HOME/pathlint/
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

**Alias retirement (removed in 0.0.22).** `pathlint where` (alias
of `pathlint trace`) and `--rules` (alias of `--config`) lived as
clap `visible_alias` forms from 0.0.14 to 0.0.21, with a stderr
deprecation warning added in 0.0.20. The 0.0.22 BREAKING release
removed both. Migrate to `pathlint trace` and `--config`; clap
will reject the legacy spellings as unknown arguments otherwise.

## 12. Non-functional requirements

- **Single Rust binary.** No runtime deps beyond the OS itself.
- **Cross-platform first-class.** Windows, macOS, Linux all run in CI.
  Termux runs from `cargo install` on the device — no prebuilt
  Termux binary, mirroring `dotfm`'s policy.
- **Startup time.** `pathlint check` < 50 ms on a warm cache for a
  PATH of ~100 entries and ~20 expectations. Windows process target
  reads HKCU and HKLM at start-up for the 0.0.24 provenance
  overlay (one `RegQueryValueEx` per hive plus an `O(n*m)`
  expanded-equality reconcile, where `n` is process entries and
  `m` is registry entries — typically `m ≈ 30`); empirical cost
  is in the low single-digit milliseconds and stays inside the
  budget.
- **Stable exit codes.** `0` clean, `1` expectation failure, `2`
  config / I/O error.
- **Encoding.** All paths are treated as UTF-8 strings on every OS.
  If `PATH` is not valid UTF-8, pathlint reads it as if empty; a
  warning + per-entry skip is a future improvement. 0.0.11
  applies `format::strip_control_chars` (ASCII control bytes
  0x00–0x08, 0x0B–0x1F, 0x7F replaced with `?`; `\t` and `\n`
  preserved) to every human-mode renderer: `where`, `doctor`,
  `catalog list`, `catalog relations`, and `check`'s report.
  JSON output is unchanged — `serde_json` already escapes
  control bytes correctly.
- **Trust boundary for shell strings (0.0.10+).** `pathlint trace`
  emits commands the user might copy-paste. The `{bin}`
  substitution and the mise plugin segment are quoted via
  `format::quote_for(os, _)` (POSIX single-quote on Unix-likes,
  PowerShell single-quote on Windows). Catalog template *bodies*
  themselves (the `uninstall_command = "..."` string) are not
  re-quoted — they come from the catalog author or user config and
  pathlint trusts them.
- **Rules file DoS guards (0.0.11+).** `Config::from_path` now
  rejects `--config` and `pathlint.toml` paths whose final hop is
  not a regular file (block devices, multi-hop symlinks) and
  caps file size at 16 MiB before any byte is buffered. A single
  symlink hop to a regular file is still allowed so dotfiles
  managers continue to work. Failures surface as exit 2.
- **Built-in catalog versioning.** The catalog is embedded at compile
  time; bumps to it are called out in the GitHub Release notes so
  users know when defaults change. Bump history:
  - `0.0.10` → `catalog_version = 3` because relation
    interpretation changed (`trace` / `sort` now read the
    relations).
  - `0.0.11` keeps `catalog_version = 3`: doctor now reads
    relations too, but the relation TOML is unchanged and no
    built-in source path moved.
  - `0.0.14` → `catalog_version = 4` because source names were
    renamed (`WindowsApps` → `windows_apps`, `system_*` →
    `os_baseline_*`, plus the new `os_baseline_linux_sbin`
    entry). User TOMLs that referenced the old names by name
    must migrate (see §17).
  - `0.0.15` keeps `catalog_version = 4`: the embedded TOML is
    unchanged, but the builtin/user split was tightened so a
    user `pathlint.toml` declaring `catalog_version` is now a
    structural error rather than a post-parse one.

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

- **[R1] Symlinked system dirs.** *(Resolved in 0.0.14 by adding
  `os_baseline_linux_sbin = "/usr/sbin"` to the built-in catalog.)*
  On Arch, Solus, openSUSE TW etc., `/usr/sbin` is a symlink to
  `/usr/bin` and `which` reports `/usr/sbin/<cmd>`. The
  `apt` / `pacman` / `dnf` / `os_baseline_linux` sources still
  declare `linux = "/usr/bin"` only because that's where their
  packages land on traditional distros; users on a symlinked
  layout reference `os_baseline_linux_sbin` alongside the package
  manager:

  ```toml
  [[expect]]
  command = "ls"
  prefer = ["pacman", "os_baseline_linux_sbin"]
  ```

  Path-canonicalize was rejected as an alternative because it
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
  doctor` now emits a `Kind::Conflict { diagnostic =
  "mise_activate_both" }` diagnostic listing every shim group
  alongside every install group. Users still pick a mode for
  `[[expect]]` rules; pathlint does not auto-detect.)*
- **[R3] DuplicateButShadowed.** Same command basename exists as
  a real executable in two or more PATH dirs. The earlier dir
  wins; later dirs are shadowed. Always reported — duplicates
  are facts, not noise. Suppress per host with
  `--exclude duplicate_but_shadowed`.

  Complements the relation-driven `mise_activate_both` Conflict
  detector: that one fires when *named* sources (`mise_shims`
  and `mise_installs`) are both in PATH, regardless of whether
  the same command exists in both. DuplicateButShadowed fires
  when the *same command* exists in two PATH dirs, regardless of
  whether the dirs are named in any relation. Together they
  cover the two angles (named-source-pair conflicts vs unnamed
  command-name shadows).

  Why always reported: in mise activate's standard usage only one
  of `mise_shims` / `mise_installs` is on PATH at a time
  (`mise activate` exposes shims; `mise hook-env` exposes
  installs); both being on PATH is itself a misconfiguration the
  existing `mise_activate_both` Conflict detector covers from a
  different angle. Filtering out the same situation in a second
  detector would hide the same mistake. See §17.2 0.0.19 entry
  for the full design discussion. *(0.0.19+.)*
- **[R3] RelativePathEntry.** PATH entry expands to a relative
  path (`.`, `./bin`, bare `bin`, …). The shell resolves these
  against the current working directory at command-invocation
  time, so the binary that runs depends on where the user
  happens to be — almost always a security or portability
  footgun. Env vars are expanded first, so `$HOME/bin` does not
  fire when `HOME` is set; an unresolved `$VAR/bin` stays
  verbatim and fires (it is itself a config bug). "Absolute"
  is judged by the target Os, not the host: `/usr/bin` is
  absolute on Linux but not on Windows. Suppress per host with
  `--exclude relative_path_entry`. *(0.0.20+.)*
- **[R3] WriteablePathDir.** PATH entry resolves to a directory
  writable by users other than the owner. An attacker with shell
  access can drop a malicious binary that the user runs by
  typing a common command name. On Unix, the others-write bit
  (`mode & 0o002`) is the trigger. On Windows, the directory's
  DACL is queried and the detector fires when any of three
  well-known SIDs has effective `FILE_GENERIC_WRITE` or
  `FILE_APPEND_DATA`: **Everyone** (`S-1-1-0`),
  **Authenticated Users** (`S-1-5-11`), and **BUILTIN\\Users**
  (`S-1-5-32-545`). 0.0.21 covered Everyone only; 0.0.22 added
  the latter two so the detector catches the typical
  Windows-host case where writes are inherited through a group
  rather than granted to Everyone explicitly. Still
  approximation: per-user explicit grants outside these three
  groups, and DENY ACEs that don't propagate, are not modelled.
  Missing or unreadable dirs are skipped (the `Missing` detector
  covers them). Suppress per host with
  `--exclude writeable_path_dir`. *(0.0.21+; expanded 0.0.22.)*
- **[R3] macOS launchd / `eval $(brew shellenv)`.** PATH set by
  these paths may differ from `process`. Out of MVP and out of
  the 0.0.x line — flagged here as a 0.1.x candidate. Three
  implementation options on the table:

  1. **New `--target launchd` flag.** Adds a fourth `Target`
     variant alongside `process` / `user` / `machine`. `pathlint
     check --target launchd` would lint the launchd-visible PATH
     with the same rule set. Pros: integrates uniformly with
     check / doctor / where. Cons: launchctl spawn cost on every
     run; macOS-only; the `launchctl getenv PATH` output covers
     only the global env, not plist-bootstrapped daemon env.
  2. **Doctor-only diff diagnostic.** A new `Kind` variant fires
     when the user-shell PATH differs from `launchctl getenv
     PATH`. Pros: surfaces "iTerm vs launchd" drift without
     extending the target model. Cons: doctor's responsibility
     creeps from "lint the PATH itself" toward "lint the
     environment delta". The diagnostic shape would need to
     carry both PATHs which is bulkier than the current per-entry
     diagnostics.
  3. **Phased: start with option 2, extend to option 1 if needed.**
     Ship the read-only diff diagnostic in 0.1.x, observe whether
     users want to write `[[expect]]` rules against the
     launchd-visible PATH, and only then extend `Target`. Avoids
     committing to the target-flag surface on speculation.

  Implementation gates (need investigation before either option):
  - Stability of `launchctl` output across macOS versions
    (Sequoia changed several launchctl subcommands).
  - Whether `launchctl getenv` is the right oracle, or whether
    pathlint should also read user / system Launch Daemons /
    Agents plists.
  - Linux equivalent (systemd user units, `EnvironmentFile=`)
    and Windows equivalent (HKLM\SYSTEM\CurrentControlSet\Services
    `Environment` REG_MULTI_SZ) — same problem, different shape.
    macOS-first because the demand is loudest there.

  Schema-store registration (PRD §8.4 follow-up) and Renovate /
  Dependabot for the SHA-pinned actions (PRD §13) are tracked
  separately and do not block this work.

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
- **`pathlint where` vs `which`/`where.exe` confusion.**
  *(Resolved in 0.0.14 — `pathlint where` is renamed to
  `pathlint trace`. `where` is kept as a clap visible alias
  throughout the 0.0.x line.)*
- **`/usr/sbin` first on Arch / openSUSE TW.** *(Resolved in
  0.0.14 — built-in `os_baseline_linux_sbin` source. Add it to
  `prefer` instead of writing your own `[source.usr_sbin]`.)*

## 17. Change log

The release-by-release change log lives in
[`CHANGELOG.md`](../CHANGELOG.md) at the repository root, in
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format.
Each entry is grouped under `### Breaking`, `### Added`, or
`### Changed`, in reverse chronological order.

The 0.0.x line treats each `0.0.x → 0.0.(x+1)` bump as
MAJOR-equivalent (Cargo's pre-1.0 convention). Breaking changes
are allowed within 0.0.x and announced under `### Breaking` in
`CHANGELOG.md`. Whether and when 0.0.x graduates to 0.1.0 is
undecided.

PRD §17 used to host the cumulative log inline; that content moved
to `CHANGELOG.md` in the 0.0.22 → 0.0.23 timeframe so PRD stays
focused on the spec while release history lives next to the
project root where readers expect it.


## 18. Relationship to other tools

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
