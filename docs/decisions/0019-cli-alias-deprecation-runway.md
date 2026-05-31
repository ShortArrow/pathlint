# ADR-0019: 6-release deprecation runway for CLI renames (`where` → `trace`, `--rules` → `--config`)

- **Status**: Accepted
- **Date**: 2026-05-05 (decision); 2026-05-09 (removal); recorded retroactively in 0.0.32 (2026-05-31)
- **Release**: 0.0.14 (introduction) → 0.0.20 (stderr warning phase) → 0.0.22 (removal)
- **Category**: 5. Architectural style (CLI rename policy; also touches 8. Process / governance for the runway-length template)

## Context

The 0.0.14 cut renamed two CLI surfaces:

- `pathlint where <command>` → `pathlint trace <command>` (R4
  provenance lookup). The original name borrowed `which`'s
  spelling but conflicted with `where` on Windows (a builtin
  `cmd` command with different semantics). `trace` better
  captured the multi-source provenance walk pathlint actually
  performs.

- `--rules <path>` → `--config <path>` (global flag for
  `pathlint.toml` location). `--rules` reflected the original
  framing ("a file of expectation rules") but pathlint had
  grown source definitions, relation declarations, and
  catalog-version pins; `--config` matched the broadened
  surface.

Unlike catalog source names (which appear in user
`pathlint.toml` files that can be migrated with `sed` — see
ADR-0014), CLI surfaces appear in:

- shell rc files (`alias ptl='pathlint trace'`)
- dotfiles repositories shared across machines
- CI scripts that someone wrote 6 months ago and forgot about
- ad-hoc terminal history users `Ctrl-R` against

A clean break in 0.0.14 would have silently broken every one
of those, with no warning channel. Users would only learn
when their shell wrapped pathlint with an old name and got an
"unknown subcommand" error.

The lib world has Cargo's deprecation conventions
(`#[deprecated(...)]`); the CLI world has clap's
`visible_alias` attribute plus stderr warnings as the
equivalent runway mechanism.

## Decision

Adopt a **6-release deprecation runway** for the rename:

| Phase | Releases | Behaviour |
|---|---|---|
| 1. Introduction | 0.0.14 | New name (`trace`, `--config`) is canonical. Old name (`where`, `--rules`) remains via clap `visible_alias`. No stderr warning yet — the rename is fresh, no migration pressure. |
| 2. Quiet runway | 0.0.15 – 0.0.19 | Aliases still accepted, no warning. Users who notice the rename migrate at their own pace. |
| 3. Warning phase | 0.0.20 – 0.0.21 | Alias still accepted, but a one-line stderr warning fires on use: "warning: `pathlint where` is deprecated, use `pathlint trace`". Two releases of warning give users a visible deadline. |
| 4. Removal | 0.0.22 | Alias removed entirely. clap rejects with the standard "unknown argument" error and exits 2. |

The "6 releases" figure is the count from introduction (0.0.14)
to removal (0.0.22) inclusive. In a pre-1.0 line where each
0.0.x → 0.0.(x+1) bump is MAJOR-equivalent (see ADR-0005), 6
MAJOR-bumps is generous; calibrated against the dotfiles-repo
share-across-machines use case rather than a single-developer
local workflow.

The decision applies *only* to CLI surface renames where the
new spelling is the canonical form on day 1. It does not
apply to:

- CLI removals where no replacement exists (deal with case by
  case).
- TOML or JSON wire-format renames (mechanical migration via
  `sed` / `jq`; ADR-0014 covers catalog source names; ADR-0016
  covers JSON discriminator unification).
- Lib API renames (Rust's `#[deprecated]` is the channel; the
  Cargo ecosystem already has its own runway customs).

## Alternatives considered

- **A. Clean break in 0.0.14 (no aliases, no runway).**
  Rejected because pathlint sits in shell rc files and CI
  pipelines; a silent break would have produced GitHub issues
  shaped "pathlint stopped working" without users immediately
  recognising the rename was the cause. The cost of carrying
  6 releases of clap aliases is one line per alias in
  `src/bin/pathlint/cli.rs`; the cost of silent breakage to
  users is unbounded.

- **B. Permanent alias (keep `where` and `--rules` forever).**
  Rejected because the alias surface becomes load-bearing
  over time: docs, help text, and tutorials would have to
  choose one spelling, leaving readers confused about which
  is canonical. The whole point of the rename was to retire
  the old spelling; keeping it forever inverts the goal.

- **C. 2-release runway (introduce in 0.0.14, remove in
  0.0.16).** Rejected as too aggressive for a CLI surface
  that lives in shell rc files. dotfiles-repo users sync
  across machines on cadences ranging from hourly to
  monthly; a 2-release window forces the conscientious users
  to migrate ahead of the careless ones get an "unknown
  argument" surprise.

- **D. 12-release runway (introduce in 0.0.14, remove in
  0.0.26).** Rejected as overcautious. pathlint's audience
  at 0.0.14 was small enough to absorb a 6-release rather
  than 12-release deadline; the stderr warning phase
  (0.0.20-21) gave users two MAJOR-equivalent releases of
  visible notice. 12 releases would have meant the alias
  was live longer than several other parts of the CLI
  surface.

- **E. Use `#[deprecated]` semantics from the lib world
  instead of stderr warnings.** Rejected because there is
  no `#[deprecated]` equivalent for CLI surfaces in clap;
  the closest is `visible_alias` plus a hand-rolled
  warning. The stderr warning was custom but minimal: one
  `eprintln!` in the subcommand dispatch path.

## Consequences

- **Positive.** Users had two MAJOR-equivalent releases of
  visible warning (0.0.20-21) before the alias removal in
  0.0.22. A user who never updated pathlint between 0.0.13
  and 0.0.22 would still get the warning the first time
  they invoked `pathlint where` after upgrading.

- **Positive.** The runway is a *template*: a future CLI
  rename can follow the same 6-release pattern without
  re-litigating the policy. ADR-0019 is the citation; the
  PR introducing the next CLI rename can write
  "Following ADR-0019, this rename keeps `<old>` as a
  visible alias through 0.0.NN and removes it at 0.0.(NN+8)."

- **Positive.** The 0.0.22 CHANGELOG `### Breaking` entry
  cleanly carries the removal text and a migration
  instruction (`sed` snippet for shell rc files); the
  introduction-time runway is now historical context.

- **Negative.** Carrying the aliases for 6 releases meant
  the clap help output listed both spellings during
  0.0.14-21, doubling the line count of the subcommand
  list and the global flag table. Tolerable; the help
  output is short anyway.

- **Negative.** Two months elapsed between rename
  introduction (0.0.14, 2026-05-05) and removal (0.0.22,
  2026-05-09) at pathlint's then-development pace. Some
  users may have only noticed the rename at the warning
  phase; the runway concentrated migration into 0.0.20-21.
  Acceptable: that *is* the warning phase's purpose.

- **Follow-up.** No further CLI renames have happened in
  0.0.x. If one is proposed, the runway template here
  applies; if a shorter or longer cadence is wanted, a
  superseding ADR records the change.
