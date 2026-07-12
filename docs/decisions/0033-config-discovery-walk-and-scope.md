# ADR-0033: config discovery walks to the `.git` boundary; `--scope` selects the layer

- **Status**: Accepted
- **Date**: 2026-07-12
- **Release**: 0.0.41
- **Category**: 5. Architectural style (+8. Process / governance)

## Context

A repo-local `pathlint.toml` has been first-class since early 0.0.x:
`locate_rules` (the CLI-layer discovery helper every subcommand
shares) checks `./pathlint.toml` before the user-global XDG
location, and only an explicit `--config <path>` outranks it.
ADR-0032 made the policy explicit — the project config in the
user's repository is *the* way to declare binary-resolution
expectations, and pathlint reads it wherever it runs.

Two gaps remained:

1. **Monorepo blindness.** Discovery looked at the cwd only. A
   `pathlint.toml` at the repository root was invisible from
   `repo/packages/foo/`, forcing either `cd` gymnastics or a
   relative `--config ../../pathlint.toml` whose depth varies per
   working directory — brittle in CI matrices and in editors that
   set the cwd per package.
2. **No layer selection.** Because the repo-local file wins
   automatically, there was no way to say "use my user-global
   config for this one invocation" (e.g. comparing behaviour) or
   "repo-local only; if this repo declares nothing, run with the
   empty config rather than my personal rules" (e.g. CI that must
   not depend on the runner's home directory).

The 0.0.39 and 0.0.40 release notes both promised this pair for the
next feature release, with the design recorded in this ADR.

## Decision

### Walk to the `.git` boundary

When the cwd has no `pathlint.toml`, discovery searches parent
directories — but only up to and including the first ancestor that
contains a `.git` entry (a directory in a normal checkout, a file
in a linked worktree), and not at all when no `.git` exists
anywhere above the cwd. The cwd hit keeps returning its familiar
relative spelling (`pathlint.toml`); walked hits return absolute
paths.

The `.git` boundary rule is load-bearing in both directions:

- **Stop at the repo root** so a config in a *parent* of the
  repository (someone's scratch directory, a vendored checkout
  inside another repo) can never leak into an unrelated project.
- **No `.git`, no walk** so a stray `pathlint.toml` in the home
  directory or a parent temp directory can never win by accident —
  the user-global layer already has exactly one blessed location
  (XDG), and the walk must not invent a second, implicit one.

### `--scope=auto|local|global`

A new global option selects which layers discovery may read:

| value | layers searched |
|---|---|
| `auto` (default) | cwd → walk to `.git` → XDG user-global |
| `local` | cwd → walk only; no XDG fallthrough |
| `global` | XDG only; repo-local files ignored |

`auto` reproduces the pre-0.0.41 precedence exactly (plus the walk,
which only fires where discovery previously found nothing), so the
flag is additive — no existing invocation changes behaviour and no
`### Breaking` entry is needed. When `--scope=local` finds nothing,
each subcommand's existing "no config" path applies unchanged
(`check` runs with the empty config, `doctor` reports its
info-severity not-found diagnostic); the flag narrows the search
list without inventing a new failure mode.

An explicit `--config <path>` always wins over `--scope`, mirroring
how git's `--file` outranks `--local` / `--global`.

`pathlint init --scope=global` writes the starter file into the
user-global location (creating `$XDG_CONFIG_HOME/pathlint/` if
needed) instead of the cwd. The lib's `init::run(dir, opts, os)`
already takes the target directory, so this is CLI-side wiring
only.

### What stays out

`--scope=system` is **reserved, not implemented**. pathlint has no
system-wide config location on any OS today; inventing
`/etc/pathlint/` + `%ProgramData%` equivalents would add a new
trust boundary (system-owned file feeding a user process) for a
use case nobody has asked for. If field demand arrives, a separate
ADR defines the locations and their threat model, and the enum
gains the variant then — additively.

## Alternatives considered

- **A. Walk to the filesystem root** (cargo's `Cargo.toml` model).
  Rejected. Cargo can afford it because a workspace manifest is
  meaningless outside a project tree; a PATH-linting config is
  not — `~/pathlint.toml` left over from an experiment would
  silently apply to every repo under the home directory. The XDG
  location is the one blessed user-global path; the walk must not
  create a second, implicit one.

- **B. Environment-variable opt-in** (`PATHLINT_DISCOVERY=walk`).
  Rejected. The additive default already protects existing
  invocations, so gating the walk behind an env var would trade a
  zero-cost default for permanent configuration-surface noise —
  and env-var-driven behaviour is exactly what makes PATH problems
  hard to reproduce, the disease pathlint exists to diagnose.

- **C. A `pathlint global <subcommand>` command family.** Rejected.
  Duplicates every subcommand behind a second spelling
  (`global init`, `global check`, ...) for what is a *parameter*
  of discovery, not a different operation. One axis, one flag.

- **D. Ship `--scope=system` now for symmetry.** Rejected — see
  "What stays out". Symmetry with git's `--system` is not worth a
  new trust boundary with zero demand.

- **E. No walk; document `--config ../../pathlint.toml`.**
  Rejected. The relative depth changes with the working directory,
  which is precisely what varies across CI jobs, editor tasks, and
  humans; a discovery rule that depends on where you stand is not
  a rule. The monorepo case is the second-most-common layout after
  single-root repos and deserves first-class behaviour.

## Consequences

- **Positive.** Monorepo users get root-config discovery from any
  subdirectory, with the same file the repo already commits.
- **Positive.** The flag makes the discovery layers explicit and
  scriptable; CI can pin `--scope=local` and become independent of
  the runner's home directory without wiring `--config` through
  every call site.
- **Positive.** Zero library-surface change: `ScopeArg`, the walk,
  and the init target-directory switch all live in the binary
  crate (`src/bin/pathlint/`). The lib public-API freeze streak
  (graduation criterion 1) continues uninterrupted.
- **Negative.** Discovery now stats `.git` in each ancestor of the
  cwd (bounded by the boundary rule). This is a read-only
  filesystem probe consistent with the read-only stance (ADR-0009)
  and is unmeasurable next to process startup, but it is new I/O
  that did not exist before.
- **Negative.** `doctor`'s selfcheck reports the config the same
  discovery finds, so its "config discovery" wording in the PRD
  and README had to be updated to include the walk — one more
  place where discovery semantics are described and must be kept
  in sync (the shared `locate_rules` helper keeps the *behaviour*
  single-sourced).
- **Neutral.** A cwd hit still reports the relative
  `pathlint.toml` spelling in `-v` output, so existing logs and
  tests keyed on that string are unaffected; only walked hits
  print absolute paths.

## Related ADRs

- **ADR-0009** (read-only stance) — the walk adds filesystem reads
  only; nothing is written outside `init`'s explicit job.
- **ADR-0028** (doctor/lint split) — `doctor`'s selfcheck reuses
  the same discovery helper, so the walk and `--scope`
  automatically apply to what selfcheck reports.
- **ADR-0032** (scope: OS knowledge + tool meta) — declared the
  project-local config first-class; this ADR closes the monorepo
  gap that declaration left open (its Follow-up section sketched
  exactly this design).
