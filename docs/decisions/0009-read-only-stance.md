# ADR-0009: pathlint is read-only on `PATH`, registry, and dotfiles

- **Status**: Accepted
- **Date**: 2026-05-31
- **Release**: 0.0.x (policy in force from 0.0.1; recorded retroactively in 0.0.30)
- **Category**: 5. Architectural style (also touches 4. Trust / security boundary)

## Context

`pathlint` reads four things — the `PATH` env var, `HKCU\Environment\Path`
and `HKLM\...\Environment\Path` on Windows, and the user's
`pathlint.toml` — and writes diagnostics. It never mutates the host.

This stance has been load-bearing since 0.0.1: the binary's name
ends in *lint*, the doctor / trace / sort outputs are
suggestions, and `pathlint sort` deliberately ships without an
`--apply` mode (the flag exists as `--dry-run` only; running
`sort` without it exits 2 with an explanatory message — see
0.0.14 CHANGELOG Breaking).

The stance is mentioned in three places (PRD §4 Non-goals,
SECURITY.md §Non-goals, the `sort` subcommand's clap docstring)
but has no dedicated ADR. ADR-0000's Known ADR backlog table
lists this as the first row, with the note that an ADR
"would crystallise the rejected `sort --apply` line and serve as
the anchor for any future 'stay read-only' call". This ADR is
that anchor.

The graduation criterion in [PRD §3.1 #5][grad5] requires every
CHANGELOG `### Breaking` entry naming a public symbol to link an
ADR. 0.0.14 carried `pathlint sort` without `--dry-run` exits 2
under Breaking, which the runtime check matches against the
read-only stance — formalising the stance lets that entry point
at this ADR rather than at PRD §4 prose.

[grad5]: ../PRD.md#31-graduation-to-010

## Decision

pathlint **never writes** any of the following:

- The `PATH` environment variable in the current process or any
  child.
- The Windows registry (`HKCU\Environment\Path`,
  `HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment\Path`,
  or any other key).
- Any file on disk other than its own stdout / stderr and clap's
  `pathlint init` template (which only creates a starter
  `pathlint.toml` when none exists and the user invoked `init`).
- Any executable, package, or installer state.

`pathlint sort` produces a proposal printed to stdout. It does
not call `setenv`, write to registry, edit shell rc files, or
shell out to `setx` / `reg add`. The `--apply` flag is not on
the public surface; if it ever ships, the launch will go through
a separate ADR that supersedes this one in part.

The read-only stance is a Category 4 (trust / security) boundary
too: by promising no host mutation, pathlint takes itself out of
several attack patterns (a hostile `pathlint.toml` cannot
weaponise the binary to rewrite `PATH`, install a backdoor,
or persist anything). SECURITY.md §Non-goals already records
this; the ADR is the policy citation those Non-goals point at.

## Alternatives considered

- **A. Ship `sort --apply` from day one.** Rejected because
  applying a PATH change correctly on every supported host is
  itself a major engineering surface: Windows registry write +
  WM_SETTINGCHANGE broadcast vs Unix shell rc file detection
  (bash, zsh, fish, pwsh, nu) vs Termux. pathlint's value in
  R3 / R4 is the *diagnosis*; the user's shell already knows how
  to write its own rc.

- **B. Allow opt-in mutation behind `--write` / `PATHLINT_ALLOW_WRITE`.**
  Rejected because the security and operational stories diverge
  once write is on the table: a user with a hostile
  `pathlint.toml` could weaponise the binary, and SECURITY.md's
  trust-boundary table would need a fundamentally different
  shape. The single bright line ("pathlint never writes")
  is easier to audit and easier for embedders to reason about.

- **C. Ship a separate `pathlint-apply` binary.** Rejected
  because it just relocates the same questions. The binary name
  would set the expectation, the security story would still
  apply, and the user has to install two binaries instead of
  one. If write support ever lands it goes through this ADR's
  successor, not into a sibling crate.

- **D. Let `pathlint init` overwrite existing `pathlint.toml`.**
  Rejected: `init` is read-only with respect to the *user's
  existing state* — it refuses to overwrite unless `--force` is
  passed, and even then it only touches `pathlint.toml` in the
  cwd. The `--force` flag is opt-in, mirrors `cargo new --force`,
  and stays inside this ADR's read-only spirit (no PATH /
  registry / shell rc mutation).

## Consequences

- **Positive.** The trust-boundary table in SECURITY.md needs
  exactly one row per untrusted input source. Without the
  read-only stance, every row would need a second column about
  what happens when that untrusted input flows into a write
  path. The current SECURITY.md size (≈ 130 lines) is feasible
  because of this stance.

- **Positive.** Embedders can wrap pathlint in any sandboxing
  scheme without worrying about side effects: no temp files
  beyond cargo's own build cache, no env writes, no registry
  hits beyond reads. The `*Deps` carriers introduced in 0.0.27
  make the env *read* explicit; there is no symmetric env-write
  surface to worry about.

- **Positive.** `pathlint sort --json` is a stable wire format
  that downstream tools (an editor plugin, a CI gate, a
  dotfiles installer) can consume and apply themselves. The
  decision about *how* to apply lives with the consumer, not in
  pathlint.

- **Negative.** Users who want one-step `pathlint sort --apply`
  must currently chain it through their shell (a snippet like
  `eval "$(pathlint sort --json | jq -r '...')"`). For one of
  R3's user stories this is friction worth accepting; for a
  hypothetical "team-wide PATH normaliser" use case it would
  not be enough, and that user case is what a future
  `sort --apply` ADR would have to weigh.

- **Negative.** The stance has to be re-affirmed in three
  places (PRD §4, SECURITY.md, this ADR) plus implicitly in
  every `--apply`-style feature request. The ADR removes the
  "implicit" qualifier — future requests can be answered with
  "see ADR-0009; opening one means superseding it".

- **Follow-up.** None planned. If a `sort --apply` ADR is ever
  written, its first task is to mark this ADR as
  `Superseded by ADR-NNNN` per the supersession rule in
  [README](README.md#supersession). The body of this ADR stays
  intact so the original rationale is preserved.
