# ADR-0027: lib has two intentional env-read boundaries; the `_with` family is the injection seam, the wrapper family is the CLI-convenience surface

- **Status**: Accepted
- **Date**: 2026-06-01
- **Release**: 0.0.33
- **Category**: 3. Cross-cutting concern (+4. Trust / security boundary)

## Context

The 2026-05-31 codex 6-axis audit (recorded in CHANGELOG 0.0.30
Notes) flagged an FP M finding:

> Yes, there are still direct `std::env::var` calls inside the
> library that do not flow through `CommonDeps::env_lookup`.
> They are no longer on the main `doctor`/`lint`/`trace`/`sort`
> call graphs, but they still exist in public/helper wrappers
> and infra boundaries:
> - [src/source_match.rs:58](src/source_match.rs:58), [161](src/source_match.rs:161), [235](src/source_match.rs:235)
> - [src/expand.rs:95](src/expand.rs:95)
> - [src/resolve.rs:30](src/resolve.rs:30), [94](src/resolve.rs:94)
> - [src/path_source.rs:51](src/path_source.rs:51), [69](src/path_source.rs:69), [147](src/path_source.rs:147), [211](src/path_source.rs:211)
>
> So the original H is materially reduced, but "all env access
> inside the lib goes through `CommonDeps`" is still not true.

Reading the call graph in detail reveals two distinct categories
of `std::env::var` use that the codex finding mechanically
lumped together:

**Category A — Wrapper convenience functions** (4 sites:
`source_match::find` / `validate_sources` / `names_only` and
`expand::expand_and_normalize`). Each of these has a parallel
`_with` variant that takes a closure (`source_match::find_with`
etc, established by ADR-0006 in 0.0.26). The wrapper *is* a
one-line call to the `_with` form, passing `|v| std::env::var(v).ok()`.
Internal callers (`doctor`, `lint`, `trace`, `sort`) all use the
`_with` variant exclusively per ADR-0007 (0.0.27). The wrappers
exist for embedders and integration tests that don't need
deterministic env injection.

**Category B — Infrastructure boundary functions** (6 sites:
`resolve::split_path`, `resolve::pathext_list`, and four sites
in `path_source` reading PATH via `std::env::var("PATH")`). These
are the actual lib→OS env-reading points. They *do* take or
internally produce env access; pushing the closure injection one
level up would just relocate the `std::env::var` call to the
caller (typically `src/bin/pathlint/run.rs`) without changing
the boundary.

The codex finding is real but the cleanup is not "eliminate
every `std::env::var`" — that would either ship a BREAKING
change (deleting wrapper functions per Category A, forcing
every embedder to call `_with` directly) or play whack-a-mole
(moving Category B's env reads to the caller doesn't reduce the
trust surface, it just renames it).

The non-ad-hoc cleanup is **documenting the architecture**:
make explicit that pathlint has exactly two env-read boundaries
and that the wrapper/`_with` split is the injection seam, not a
bug.

## Decision

pathlint has **two intentional env-read boundaries**:

### Boundary 1 — Source catalog resolution

Functions: `source_match::find` / `validate_sources` /
`names_only` and `expand::expand_and_normalize`. These read
env vars while resolving a catalog source's per-OS path
(`%LocalAppData%`, `$HOME`, etc.) against a candidate haystack.

Each function has a parallel `_with` variant (`find_with`,
`validate_sources_with`, `names_only_with`,
`expand_and_normalize_with`) that takes a closure
`Fn(&str) -> Option<String>`. The closure is the injection
seam: embedders and tests construct one explicitly; the wrapper
form supplies `|v| std::env::var(v).ok()` for production use.

**Internal lib callers use the `_with` variant exclusively**
(verified by `doctor::analyze` / `lint::evaluate` /
`trace::locate` / `sort::sort_path` threading `CommonDeps::env_lookup`
through every recursive call). The wrappers exist for the
public surface so embedders without deterministic-env needs
can call one-line APIs.

### Boundary 2 — PATH entry construction

Functions: `resolve::split_path`, `resolve::pathext_list`, and
the `path_source::read_path` family (`read_process`,
`read_user`, `read_machine`). These read `std::env::var("PATH")`
(and on Windows, `std::env::var("PATHEXT")`) to materialise a
`Vec<Attribution>` of PATH entries.

Embedders that want env-deterministic PATH construction can
either:
- supply pre-constructed `Vec<Attribution>` directly to
  `lint::evaluate_real` / `trace::locate_real` / etc, bypassing
  `path_source` entirely; or
- construct each `Attribution` via
  `Attribution::new(PathEntry::from_raw(raw, closure))` using
  the closure-receiving `PathEntry::from_raw` form (ADR-0002).

The `resolve::split_path` form is a convenience for in-process
PATH splitting; the `path_source::read_path` form is the OS
infrastructure boundary. Both are documented as such; neither
is hidden behind a `_with` wrapper because there is no
single-function injection point that would meaningfully reduce
the env-read surface.

### The wrapper / `_with` pattern is the design

The wrapper form (Category A) takes no closure; the `_with`
form takes one. **The wrappers are not removed** even though
they call `std::env::var` directly:

- Removing them would BREAK every embedder calling
  `source_match::find(...)`, `validate_sources(...)`,
  `names_only(...)`, or `expand_and_normalize(...)`. The
  migration is mechanical but pathlint deliberately wants the
  convenient form available.
- The CHANGELOG 0.0.26 entry establishing `_with` variants
  framed them as **additive** — the existing wrappers stay,
  the `_with` variants are added alongside. That framing is
  ADR-0006's contract.
- Internal lib code uses `_with` exclusively (verified
  callgraph); the wrappers only get called from `src/bin/pathlint/`
  and from embedders / integration tests choosing the
  convenience form.

This is the close-out of the codex M finding: the residual
`std::env::var` calls are intentional architecture, not
unfinished work. The injection seam (the `_with` family)
exists; the wrapper convenience surface (Category A) is what
the codex flagged. Documenting the distinction is the fix.

## Alternatives considered

- **A. Delete the wrapper functions (Category A); ship `_with`
  only.** Rejected because:
  - It's a BREAKING release; every embedder calling
    `source_match::find` etc would have to migrate to passing
    a closure explicitly. The migration is mechanical (`f(a, b, c)`
    becomes `f_with(a, b, c, |v| std::env::var(v).ok())`),
    but it forces work on every embedder for no functional
    benefit.
  - ADR-0006's contract was "additive — wrappers stay". Removing
    them now would reverse that without a load-bearing
    reason.
  - The convenience wrappers exist because some embedders
    *do* want process env; making them write
    `|v| std::env::var(v).ok()` 4× per file is noise.

- **B. Unify Category A and Category B under one big `Deps`
  carrier extending `CommonDeps`.** Rejected because the two
  boundaries answer different questions:
  - Category A is "how do we resolve a catalog source path?"
    (answer: read env vars referenced by the path string).
  - Category B is "what's on `PATH` right now?" (answer: read
    `PATH` itself, then split, then expand each entry).
  - Forcing both into one `Deps` carrier would conflate
    "lookup any env var" with "read the specific
    PATH/PATHEXT names". The current shape (`env_lookup` is
    generic; `path_source::read_path` is specialised) is the
    cleaner split.

- **C. Move every `std::env::var` to `src/bin/pathlint/`
  (zero env reads in the lib).** Rejected as fictitious
  reduction. Category B's reads would shift to the caller;
  the lib boundary surface (the functions the caller has to
  call) would be the same. The trust-boundary surface depends
  on *what the lib's public surface reads*, not on which
  file contains the syscall.

- **D. Accept the codex M finding as a permanent caveat
  (no ADR).** Rejected because the audit explicitly noted
  that this finding should be either resolved or downgraded
  with a written reason — the ADR-0013 graduation audit
  pattern. Documenting the boundary in an ADR closes the
  finding while preserving the current architecture.

- **E. Add an integration test that asserts "no
  `std::env::var` outside Category A and B sites".**
  Rejected because the assertion would become brittle: any
  new lib code that reads env would either fail the test
  (forcing the developer to update the allowlist) or quietly
  evade detection (the assertion could be lint-scoped but not
  semantic). Documentation + code review is the practical
  enforcement; an automated check provides marginal value
  for high maintenance cost.

## Consequences

- **Positive.** The codex M finding 2 closes. The
  architecture is now ADR-documented and a future reader (or
  auditor) understands why the residual `std::env::var`
  calls exist.

- **Positive.** Future env-read sites have a decision template:
  identify which boundary the new site belongs to, ensure the
  `_with` variant is established alongside, and document the
  wrapper if added. ADR-0006's pattern + ADR-0027's
  formalisation together cover the case.

- **Positive.** SECURITY.md gains a row covering env_lookup's
  return values as untrusted bytes (committed in the same
  release as this ADR; see CHANGELOG 0.0.33). The trust-
  boundary table now correctly reflects that the closure's
  *transport* is in-process code while its *payload* is
  external OS state.

- **Negative.** This ADR does not eliminate `std::env::var`
  from the lib. A future codex audit that mechanically
  scans for `std::env::var` will surface the same 10 sites;
  the response is "see ADR-0027". The cost is a recurring
  audit-time clarification, not unresolved technical debt.

- **Negative.** If a future change wants the wrapper functions
  removed (e.g. a sandbox-only build of pathlint that refuses
  any `std::env::var`), this ADR is the rejected-alternatives
  citation; the superseding ADR would have to address each
  rejection point.

- **Follow-up.** The next codex audit re-run (whenever it
  happens) should confirm M finding 2 is closed. If the
  audit re-raises the finding, the response is the same: the
  architecture is intentional, see ADR-0027. The audit
  template's "M findings carried forward" list is updated to
  empty by CHANGELOG 0.0.33.
