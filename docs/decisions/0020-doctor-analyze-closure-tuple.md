# ADR-0020: `doctor::analyze` accepts an open-ended closure tuple as new detectors land

- **Status**: Accepted (superseded by ADR-0007 as of 0.0.27)
- **Date**: 2026-05-06 (fs_list_dir in 0.0.19) → 2026-05-09 (is_writable_dir in 0.0.21); recorded retroactively in 0.0.32 (2026-05-31)
- **Release**: 0.0.19 (7th positional closure) → 0.0.21 (8th positional closure)
- **Category**: 1. Public API surface (also touches 3. Cross-cutting concern for the closure-injection style)

## Context

`pathlint::doctor::analyze` is the entry point for the
PATH-hygiene linter (`pathlint doctor`). Each detector inside
it that needs to reach the host environment does so through
a caller-supplied closure rather than calling `std::env::var`
or `std::fs::*` directly — the same closure-injection style
ADR-0002 introduced for `PathEntry::from_raw`.

As new detectors landed across 0.0.x, each new "I need to
talk to the host" capability added a new closure parameter
to `analyze`. By 0.0.21 the signature had grown to:

```rust
pub fn analyze<F, G, H, I, J, K, L, M>(
    entries: &[PathEntry],
    sources: &[SourceMerged],
    relations: &[RelationMerged],
    os: Os,
    fs_exists: F,         // 0.0.4
    env_lookup: G,        // 0.0.4
    fs_canonicalize: H,   // 0.0.x baseline
    /* ...four more... */
    fs_list_dir: L,       // 0.0.19 (this ADR's first event)
    is_writable_dir: M,   // 0.0.21 (this ADR's second event)
) -> Vec<Diagnostic>
where
    F: Fn(&str) -> bool,
    G: Fn(&str) -> Option<String>,
    /* ...six more bounds... */
```

Each new detector that needed a host capability extended the
positional closure list:

- **0.0.19**: `DuplicateButShadowed` needed to enumerate
  executables in each PATH dir to detect command shadowing
  across dirs. Required a `fs_list_dir: Fn(&str) ->
  Vec<String>` closure.

- **0.0.21**: `WriteablePathDir` needed to check whether a
  PATH dir is writable by users other than the owner.
  Required an `is_writable_dir: Fn(&str) -> bool` closure
  (Unix others-write bit; Windows DACL probe via
  `GetEffectiveRightsFromAclW`).

The `analyze_real` production wrapper kept being updated to
match — each new closure parameter got a default
`*_real` function wired in, so CLI callers were unaffected.
But embedders writing their own `analyze` call had to
extend their argument list each release.

By 0.0.21 the signature carried 8 closures + 3
`Vec`/slice arguments + the `Os` enum, and clippy
fired `too_many_arguments` (silenced with
`#[allow(clippy::too_many_arguments)]`).

The 2026-05-17 codex audit flagged this as a CA H
finding — argument explosion in the public surface. The
fix shipped as ADR-0007 in 0.0.27: the layered `*Deps`
carriers absorbed the closure list into a typed bag.

This ADR records the **0.0.19 and 0.0.21 decisions in
their own context** — at the time, why was extending the
positional closure list the right call, and what
alternatives were rejected? It's important for posterity
because ADR-0007 supersedes only the *resulting*
signature; the *reasoning* that produced the 0.0.19 and
0.0.21 extensions is its own decision.

## Decision

When 0.0.19 added `DuplicateButShadowed` and 0.0.21 added
`WriteablePathDir`, the implementing closure was added as
a new positional parameter on `analyze` (and a new field on
no struct, because no struct existed yet to absorb them).
`analyze_real` got a corresponding default wiring; CLI
callers were unaffected.

The decision in both releases was:
**extend the positional closure list rather than refactor
the signature now**. Each addition was a focused, single-
detector change; the refactor work to introduce a typed
carrier (eventually ADR-0007 in 0.0.27) was held back to
a release that could afford the broader scope.

## Alternatives considered

(These are the alternatives considered *at the time*,
0.0.19/21. ADR-0007 later considered a broader set
when designing the eventual replacement.)

- **A. Introduce a `DoctorDeps` struct in 0.0.19 to bag
  the four-then-five closures.** Rejected at the time
  because:
  - 0.0.19 was a focused detector-addition release; the
    refactor would have widened the BREAKING surface
    beyond what the detector itself caused.
  - The eventual carrier shape was not yet clear —
    would it embed a shared env oracle? Would it have a
    `production()` constructor? Should it be per-function
    (`AnalyzeDeps`) or shared (one big `Deps`)? These
    questions took until 2026-05-17 (codex audit) and
    2026-05-24 (ADR-0007's plan) to answer.

- **B. Introduce a `DoctorDeps` struct in 0.0.21 (after
  one more closure-addition release).** Rejected at the
  time for the same reason: the questions in (A) were
  still open. Better to keep extending the positional
  list until the design space was clear.

- **C. Add a trait object `&dyn DoctorContext` carrying
  all the closure-equivalent methods.** Rejected
  because trait objects forbid generic closures on the
  trait methods; every detector that wanted to inject a
  test-time fake would have to construct a trait
  implementer rather than just pass a closure.

- **D. Refuse to add `fs_list_dir` and
  `is_writable_dir`; implement the detectors with the
  existing closures.** Rejected because neither
  detector was implementable without the new capability:
  `DuplicateButShadowed` genuinely needs directory
  enumeration; `WriteablePathDir` genuinely needs DACL
  / mode-bit probing. Refusing the closures would have
  meant refusing the detectors.

- **E. Side-channel the new closures (a `thread_local!`
  or `LazyLock<RefCell>`-style global).** Rejected as a
  testability anti-pattern: global state would prevent
  parallel test execution and would hide the
  dependency from the signature.

## Consequences

- **Positive at the time.** Each detector landed in a
  focused release. 0.0.19 was "add `DuplicateButShadowed`";
  0.0.21 was "add `WriteablePathDir`". Both were
  reviewable in one sitting; both had small surface
  motion outside the new detector code.

- **Positive at the time.** `analyze_real` absorbed the
  CLI-side cost. CLI callers (which were the only
  callers at the time) saw no change.

- **Negative at the time.** The positional list was
  approaching the maintainability ceiling (8 closures,
  clippy lint suppressed). Embedders writing their own
  `analyze` calls had to extend the argument list each
  release, and the order-positional shape made each
  extension a BREAKING change even when the closure was
  optional.

- **Negative in retrospect.** The 2026-05-17 codex
  audit flagged the resulting argument explosion as a
  CA H finding. Holding back the refactor accumulated
  cost: when the carrier shape finally landed in 0.0.27
  (ADR-0007), it had to break four public entry points
  simultaneously rather than one (`doctor::analyze`,
  `lint::evaluate`, `trace::locate`, `sort::sort_path`).

- **Negative in retrospect.** The closure-list ceiling
  was a leading indicator that the carrier was needed;
  the 0.0.21 release could have introduced a per-
  function `DoctorDeps` struct without the broader
  cross-function unification, and the eventual ADR-0007
  unification would have been less BREAKING. The
  trade-off at 0.0.21 ("focused release vs. early
  refactor") biased toward focus; with hindsight a
  smaller mid-cycle refactor would have spread the
  BREAKING cost more evenly.

- **Follow-up.** ADR-0007 (0.0.27) introduces the
  layered `*Deps` carriers across all four public entry
  points. `doctor::analyze`'s positional closure list
  collapses into a single `AnalyzeDeps<'_>` field,
  retiring the design recorded here. **This ADR's
  Status moves to "Accepted (superseded by ADR-0007 as
  of 0.0.27)"** per the README §Supersession rule.
