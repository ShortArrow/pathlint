# ADR-0014: source naming convention — `<provenance>_<scope>` and `os_baseline_*` split

- **Status**: Accepted
- **Date**: 2026-05-05 (decision); recorded retroactively in 0.0.32 (2026-05-31)
- **Release**: 0.0.14
- **Category**: 7. Persistence / data format (the catalog's source names are a wire-format the user types in `pathlint.toml`)

## Context

pathlint's catalog identifies installer locations by name
(`cargo`, `mise`, `winget`, …). Pre-0.0.14, two name shapes had
crept in:

- **PascalCase singletons.** `WindowsApps` was the only
  PascalCase source name; every other source was lowercase. It
  reflected the Windows directory name (`%LocalAppData%\Microsoft\WindowsApps`)
  rather than a normalised installer label.

- **`system_*` family.** `system_windows`, `system_linux`,
  `system_macos` named the OS-baseline binary directories
  (`%SystemRoot%\System32`, `/usr/bin`, etc.). The
  prefix collided with the `os_detect::Os` family which the lib
  already used for runtime OS dispatch — `system_linux` could
  reasonably be misread as "Linux-specific lookup engine" rather
  than "Linux's `/usr/bin` baseline".

Two operational pressures also accumulated:

- On Arch and openSUSE Tumbleweed, `/usr/sbin` is symlinked to
  `/usr/bin`; on traditional distros (Debian / Fedora) the two
  are separate directories and `which ls` reports
  `/usr/sbin/ls`. A `[[expect]] prefer = ["pacman"]` rule that
  treats only `/usr/bin` as the package-manager domain misses
  the Arch case. The catalog needed a separate source label
  the user could include alongside the package manager:
  `prefer = ["pacman", "os_baseline_linux_sbin"]`. (See
  user memory `Arch /usr/sbin first in PATH`.)

- The naming was load-bearing enough that the user's
  `pathlint.toml` referenced these strings directly. Whatever
  convention the catalog committed to, every embedder had to
  rewrite their config to match.

The 0.0.14 cut was already a major reshape (R5 `sort` was
introduced, `where` was being renamed to `trace`, JSON shapes
were unified — see ADR-0016 and ADR-0019). Bundling the source
naming convention into the same cut keeps the migration cost
concentrated.

## Decision

Adopt **`<provenance>_<scope>` snake_case** uniformly for every
built-in source name, and **split `/usr/sbin` into a sibling
source**:

- `WindowsApps` → `windows_apps`
- `system_windows` → `os_baseline_windows`
- `system_linux` → `os_baseline_linux`
- `system_macos` → `os_baseline_macos`
- **New**: `os_baseline_linux_sbin` for `/usr/sbin`

The pattern is: each name has a provenance prefix (`os_baseline_`,
`windows_`, the installer name itself, …) and a scope suffix
describing what the source targets within that provenance. The
prefix tells the reader where the source came from; the suffix
tells them what role it plays.

No aliases. Users must update their `pathlint.toml`. The
0.0.14 CHANGELOG carries a `sed` snippet for mechanical
migration:

```sh
sed -i \
  -e 's/WindowsApps/windows_apps/g' \
  -e 's/system_windows/os_baseline_windows/g' \
  -e 's/system_macos/os_baseline_macos/g' \
  -e 's/system_linux/os_baseline_linux/g' \
  pathlint.toml
```

## Alternatives considered

- **A. Keep both names via aliases (`system_linux` as alias of
  `os_baseline_linux`).** Rejected because the alias surface
  would have to live in the catalog file itself; the catalog
  is the source of truth for source names and an alias would
  duplicate the row. Unlike the CLI surface (where
  `pathlint where` was kept as a visible alias of `pathlint
  trace`, see ADR-0019), source names appear in
  `pathlint.toml`, which is the user's own file and which can
  be migrated mechanically with `sed`. A 6-release runway for
  catalog names would protect the wrong constituency at the
  wrong cost.

- **B. Keep `WindowsApps` PascalCase (match the directory
  name).** Rejected. Every other catalog source is lowercase
  snake_case; one PascalCase outlier in a long list of
  consistent names is a parse-time foot-gun. The directory's
  actual name (`%LocalAppData%\Microsoft\WindowsApps`) is
  preserved in the source's `windows = ...` path; the source
  *label* the user types in `pathlint.toml` does not need to
  reproduce filesystem casing.

- **C. Leave `/usr/sbin` inside `os_baseline_linux` (one source
  covering both directories).** Rejected because pathlint's
  matching is substring-based (see ADR-0011): a single source
  with `linux = "/usr/bin"` cannot also match `/usr/sbin`
  without making the substring more permissive (e.g.
  `linux = "/usr"`), which would over-match every Linux PATH
  entry rooted in `/usr`. Splitting into two sources keeps the
  substring narrow and lets the user opt in to either layout
  by naming both sources in `prefer`.

- **D. Use a different separator (`os-baseline-linux` /
  `os.baseline.linux`).** Rejected because TOML key syntax and
  Rust identifier conventions both favour snake_case;
  hyphens would force `[source."os-baseline-linux"]` quoting
  in user TOML files, and dots would conflict with TOML's
  table-path syntax. snake_case is unambiguous in both.

## Consequences

- **Positive.** Every catalog source name follows one
  convention. New built-in sources slot in without naming
  debate; user-defined sources have a clear pattern to follow.

- **Positive.** `os_baseline_linux_sbin` makes the Arch /
  openSUSE Tumbleweed case expressible in `pathlint.toml`
  without forcing every Linux user to set
  `prefer = ["pacman", "os_baseline_linux"]` even when the
  binary is in `/usr/sbin` rather than `/usr/bin`.

- **Positive.** The `os_baseline_` prefix is a recognisable
  signal that the source describes OS-provided binaries
  rather than a third-party installer — relevant for the
  `Conflict` doctor detector which flags an installer's
  binary winning over the OS baseline when the user expected
  the opposite.

- **Negative.** Every pre-0.0.14 `pathlint.toml` becomes a
  config error on 0.0.14+ until the user runs the `sed`
  migration. Acceptable given pathlint's
  early-adopter user base at the time and the small CLI
  surface (each user had perhaps 5–10 source references at
  most).

- **Negative.** The catalog now has two near-identical Linux
  baselines (`os_baseline_linux` and `os_baseline_linux_sbin`)
  rather than one, which doubles the maintenance cost when
  Linux distros change their layout conventions. Mitigation:
  both rows are 2-line definitions in
  `plugins/os_baseline.toml`; adding a third (`/sbin` on the
  bare-bones Termux-aarch64 layout, say) is cheap.

- **Follow-up.** None. ADR-0017 (lib surface narrowing in
  0.0.15/0.0.17) is the next decision in this chain;
  ADR-0019 covers the parallel CLI alias deprecation runway.
