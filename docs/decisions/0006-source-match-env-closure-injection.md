# ADR-0006: `_with` env-lookup closures on `expand_and_normalize` and `source_match`

- **Status**: Accepted
- **Date**: 2026-05-23
- **Release**: 0.0.26
- **Category**: 3. Cross-cutting concern (env injection policy) — also touches 1. Public API surface

## Context

ADR-0002 (0.0.23) made `PathEntry::from_raw` take an env-lookup
closure and committed the project to the policy that *the lib reads
`std::env::var` only at infrastructure boundary points*. The
constructor and `expand::expand_env_with` were rolled into that
policy; ADR-0002 closed with a Follow-up that named
`resolve::split_path` and `source_match::find` as remaining
direct readers and slated them for Step 2 of the 0.0.25-0.1.0
roadmap.

By the time 0.0.26 started, the actual state of those two call
sites was:

- `resolve::split_path` already takes the closure path through
  `PathEntry::from_raw(raw, |v| std::env::var(v).ok())` at line 29
  of `src/resolve.rs`. It had been quietly fixed during 0.0.23
  itself, but ADR-0002 was written before that confirmation
  landed.
- `resolve::pathext_list` already routes through
  `expand::pathext_raw(|v| std::env::var(v).ok())`.
- `expand::expand_and_normalize(input)` was still reading the
  process env via `expand_env(input)` (line 67-69 of
  `src/expand.rs`).
- `source_match::find`, `source_match::validate_sources`, and
  `source_match::names_only` all called `expand_and_normalize`
  internally to resolve the catalog source's `unix` / `windows`
  / `macos` path — so they too leaked the process env into the
  needle that was matched against PATH entries.

Codex's 2026-05-17 6-axis audit (commit `33471a5`) flagged this
as an FP H severity finding: *env injection is established as
project policy but not closed library-wide; new embedders cannot
predict which entry points are env-pure*. The audit was the
trigger for finishing the Follow-up in 0.0.26.

The codex audit also surfaced the matching docs drift: PRD §10.1
still said "the only two places in the lib that read
`std::env::var` are `path_source::read_path` and
`resolve::split_path`", which had been true at 0.0.23 but
overlooked the `source_match` layer.

## Decision

Add `_with` variants to four functions, keep the existing four
as thin wrappers that bake in `|v| std::env::var(v).ok()`:

| Module | New | Existing (wrapper) |
|---|---|---|
| `expand` | `expand_and_normalize_with<V>(input, env_lookup) -> String` | `expand_and_normalize(input)` |
| `source_match` | `find_with<V>(haystack, sources, os, env_lookup) -> Vec<Match>` | `find(haystack, sources, os)` |
| `source_match` | `validate_sources_with<V>(sources, os, env_lookup) -> Vec<SourceWarning>` | `validate_sources(sources, os)` |
| `source_match` | `names_only_with<V>(haystack, sources, os, env_lookup) -> Vec<String>` | `names_only(haystack, sources, os)` |

`source_match::find_with` resolves the catalog source's path via
`expand_and_normalize_with(raw, &env_lookup)` instead of the live
`expand_and_normalize`. `validate_sources_with` and
`names_only_with` chain through the same closure.

`normalize` is *not* given a `_with` form: it is a pure
case-and-slash transformation that never touches the env.
Inventing `normalize_with(input, env_lookup)` would inflate the
public surface for no observable benefit.

The result: an embedder that exclusively calls the `_with`
variants can run pathlint's matching layer with a deterministic
oracle and never touch `std::env::var`. Production CLI wiring is
unaffected — the wrappers exist precisely so the binary keeps
behaving the same way without explicit threading.

## Alternatives considered

- **A. Directly change the existing `find` / `validate_sources` /
  `names_only` / `expand_and_normalize` signatures to take the
  closure.** Rejected: this is a BREAKING change to every caller,
  including the binary's `lint`, `trace`, `sort`, and `doctor`
  modules. ADR-0005 lets the project make BREAKING changes in
  the 0.0.x line, but criterion 1 of PRD §3.1 (the graduation
  gate) requires two consecutive releases without a `### Breaking`
  entry naming a public symbol. Spending the BREAKING budget on
  a closure migration that the wrapper pattern can solve with
  zero breakage is wasteful. The same pattern was rejected for
  identical reasons in ADR-0002 (`expand_env` / `expand_env_with`).
- **B. A global trait object (`set_env_oracle(...)` mutating a
  static).** Rejected: globals make tests order-dependent and
  fight against pathlint's "pass dependencies explicitly"
  invariant. The same alternative was rejected in ADR-0002 for
  the same reasons.
- **C. A thread-local env oracle.** Rejected: still global state,
  just per-thread. Embedders calling pathlint from a thread pool
  would need to install the oracle on each worker, which is
  worse ergonomics than passing the closure.
- **D. Leave it alone (rely on ADR-0002's Follow-up note as a
  future-work marker).** Rejected: the audit flagged this as H
  severity, the graduation criteria expect closed env-injection
  scope, and the cost of the closing patch is small (one new
  function per layer, no BREAKING).
- **E. Push the closure threading further: change `lint::evaluate`,
  `trace::locate`, `sort::sort_path`, and `doctor::analyze` to
  take the env_lookup as a top-level parameter and let it flow
  to `source_match` without the production wiring still reading
  `std::env::var`.** Rejected for 0.0.26: that change is a
  BREAKING signature change on four headline functions, three of
  which already had BREAKING entries in 0.0.19 / 0.0.21 / 0.0.23.
  Doing it in 0.0.26 would re-burn the BREAKING budget for
  criteria 1 and 5. The plan-Step-3 work (`AnalyzeDeps`
  dependency bag) is a better vehicle for that consolidation:
  once a typed deps carrier exists, threading the env_lookup is
  an additive field on the bag rather than another positional
  argument.

## Consequences

- **Positive.** The lib's public boundary is closure-injectable
  end to end. PRD §10.1's claim that env is read only at
  documented boundaries is now accurate (the doc edit lands in
  the same PR as this ADR).
- **Positive.** Codex's FP H finding is closed. The same finding
  cannot recur on this scope because future audits can grep for
  `std::env::var` in `src/` and only find it in the documented
  boundary functions plus the wrapper bodies.
- **Positive.** The pattern stays consistent with ADR-0002: every
  injection-aware variant is named `_with`, takes
  `V: Fn(&str) -> Option<String>`, and is the "real" function;
  the bare name is a wrapper that reads the live env. New
  contributors learning the codebase only need to internalise
  one convention.
- **Negative.** The lib's public function surface goes from 4
  entry points in this area to 8. Wrappers exist precisely to
  avoid BREAKING for callers that don't care about injection,
  but every additional `pub fn` is one more thing to keep
  documented and tested. The README ADR index now has to make
  the wrapper-vs-real distinction clear.
- **Negative.** Internal callers (`lint::evaluate_one`,
  `trace::locate`, `sort::sort_path`, `doctor::matched_entries_for_source`,
  `doctor::add_relation_conflict_diagnostics`, the binary's
  `enforce_source_validation`) still go through the wrappers
  and so still read the process env in the production CLI. The
  "library-wide closed env injection" claim is therefore
  precisely **lib-boundary-wide**, not call-graph-wide. The
  follow-up below is where the call-graph closure work lands.
- **Follow-up — internal call-graph closure.** Step 3 of the
  0.0.25-0.1.0 roadmap (`doctor::analyze` dependency bag,
  Category 2) is the natural home for threading `env_lookup`
  through the internal call sites. Once `AnalyzeDeps` exists,
  `lint`, `trace`, `sort`, and `doctor` will all carry the
  env_lookup as a field on a deps carrier, and the wrappers
  here will only be used by external callers that don't bring
  their own oracle.
- **Follow-up — wrapper retirement.** The four wrappers
  (`expand_and_normalize`, `find`, `validate_sources`,
  `names_only`) are kept for source-compatibility. A future
  ADR may decide to retire them when 0.1.0's public-API freeze
  takes effect, but doing so is a BREAKING change and out of
  scope until then.
- **Follow-up — graduation criterion 5.** This ADR is the
  required link from the 0.0.26 CHANGELOG entry. Criterion 5
  says every public-symbol BREAKING entry must link an ADR;
  0.0.26 has no `### Breaking` section, so the criterion is
  vacuously satisfied for this release. The Added entries link
  back here regardless because the additive change is itself a
  policy decision worth recording.
