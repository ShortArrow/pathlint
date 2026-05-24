# ADR-0007: layered `*Deps` carriers + per-function production wrappers

- **Status**: Accepted
- **Date**: 2026-05-24
- **Release**: 0.0.27
- **Category**: 2. Module boundary / dependency direction — also touches 1. Public API surface

## Context

`doctor::analyze` had drifted into a positional-closure signature
that the codex 2026-05-17 audit flagged as CA H severity:

> `doctor::analyze` はすでに dependency bag を引数列で表現している
> 段階です。 `#[allow(clippy::too_many_arguments)]` が示す通り、
> detector を足すたびに public API が壊れる形になっています。
> `AnalyzeDeps` か trait object へまとめるのを 0.1 前の最優先に
> 置くべきです。

The same shape was creeping into `lint::evaluate` (resolver +
shape_check positional closures) and was implicit in `trace::locate`
(resolver only) and `sort::sort_path` (no closures yet, but every
internal call inside the function read the live env through the
wrapper `source_match::names_only` instead of a closure-injected
oracle).

ADR-0006 had also left a Follow-up open: 0.0.26 closed the env
injection at the library *boundary* (the `*_with` family on
`expand` and `source_match`), but the four headline public entry
points still let the wrappers leak the process env. Step 3 of the
0.0.25-0.1.0 roadmap promised to close that loop by threading
`env_lookup` through each entry point's internal matchers.

Both findings point at the same fix: stop passing the closures
positionally, bundle them into a typed carrier, and reuse one
carrier per public entry point.

## Decision

Introduce a **layered carrier**:

- `pathlint::CommonDeps` at the crate root holds the single
  closure that every entry point eventually consults
  (`env_lookup`). It is the *base layer*.
- Per-function carriers (`doctor::AnalyzeDeps`,
  `lint::EvaluateDeps`, `trace::LocateDeps`, `sort::SortDeps`)
  embed `CommonDeps` as a field and add their own
  function-specific closures on top. They are the *function
  layer*.
- Each carrier exposes a `production()` constructor that wires
  the `_real` closures so the CLI binary keeps its short-form
  call sites. `evaluate_real`, `locate_real`, and `sort_path_real`
  are added so all four entry points have a production wrapper
  with the same shape; the pre-existing `analyze_real` stays
  unchanged.

Closures inside the carriers are type-erased through `Box<dyn>`
with an explicit `'a` lifetime:

```rust
pub type EnvLookupFn<'a> = Box<dyn Fn(&str) -> Option<String> + 'a>;
pub type ResolverFn<'a> = Box<dyn FnMut(&str) -> Option<PathBuf> + 'a>;
// ... etc.

pub struct AnalyzeDeps<'a> {
    pub common: CommonDeps<'a>,
    pub fs_exists: FsBoolFn<'a>,
    pub fs_list_dir: FsListDirFn<'a>,
    pub is_writable_dir: FsBoolFn<'a>,
}

impl AnalyzeDeps<'static> {
    pub fn production() -> Self { /* wires `_real` helpers */ }
}

pub fn analyze(
    entries: &[PathEntry],
    sources: &BTreeMap<String, SourceDef>,
    relations: &[Relation],
    os: Os,
    deps: AnalyzeDeps<'_>,
) -> Vec<Diagnostic> { /* ... */ }
```

The `'a` lifetime accepts both `'static` production closures and
borrowed test closures (e.g. one returned by `fs_list_map(&listing)`
that captures `&listing`).

The carriers are *moved* into the entry point (not borrowed). Per
the planning discussion: there is no use case in pathlint for
calling `analyze(..)` twice with the same carrier, and moving keeps
the call site to one line at the surface level. Borrowing would
have been a fine alternative if a caller wanted to reuse the bag,
but adding `&` back later is purely additive — moving today does
not foreclose it.

Internal callers inside the lib (`doctor::matched_entries_for_source`,
`doctor::add_relation_conflict_diagnostics`, `lint::evaluate_one`,
`trace::locate`'s provenance walk, `sort::sort_path`'s indexer)
all switch from `source_match::*` (the wrappers) to
`source_match::*_with`, threading the closure from `deps.common.env_lookup`.
This closes ADR-0006 Follow-up for these four modules. The CLI
binary's `enforce_source_validation` still calls `validate_sources`
(without `_with`) because it always wants the production env.

## Alternatives considered

- **A. Status quo (positional closures).** Rejected outright by
  the codex audit. Every detector addition forces a public-API
  break, and the `#[allow(clippy::too_many_arguments)]` annotation
  was a tell that the shape was wrong.

- **B. Flat per-function carriers (no shared `CommonDeps`).**
  Considered first. Rejected because every per-function carrier
  needs an env oracle (the matching layer reads it transitively
  via `source_match::*_with`), and four parallel `env_lookup`
  fields scattered across four structs is the kind of duplication
  that creeps until a future contributor "fixes" it inconsistently.
  The layered design pays one extra struct (`CommonDeps`) and one
  field accessor (`deps.common.env_lookup`) to keep "the env oracle
  lives here" mechanically singular.

- **C. A single `Deps` struct with `Option<...>` fields for every
  function-specific closure.** Rejected. Either the type system
  lets callers pass `None` for fields that the called function
  needs (and the runtime crashes), or the type system rejects
  every call site that misses a field (and we're back at per-function
  shape, just with worse ergonomics).

- **D. Layered carriers (chosen).** See *Decision*.

- **E. A `Context` trait with associated types and a `RealContext`
  unit struct.** Rejected because Rust's GAT story is still
  evolving and using associated types here would require nightly
  features or awkward workarounds. The trait path also pushes the
  decision of "what fields exist?" into trait implementations,
  which makes `tests/public_api.rs` harder to pin (the public
  contract becomes the trait's associated types, not a flat
  struct shape).

- **F. Generic closure fields on the carrier
  (`AnalyzeDeps<V, F, L, W>` with `Fn` bounds).** Considered at
  length and partly implemented before being rejected. The Rust
  compiler cannot infer `for<'a>` bounds on a closure type when
  the closure is moved into a generic struct field at the
  *construction* site (the user error
  `closure with signature ... must implement FnMut<(&'1 str,)>,
  for any lifetime '1...but it actually implements FnMut<(&'2 str,)>,
  for some specific lifetime '2`). Working around this would
  force every call site to use an explicit type annotation
  (`|x: &str| -> Option<PathBuf> { ... }`) on every closure passed
  to a carrier, which is exactly the kind of mechanical ad-hoc
  workaround `non-ad-hoc` planning was meant to avoid. Cost of
  the alternative: every test fixture, every external embedder
  example. Cost of `Box<dyn>` (chosen): one virtual call per
  closure invocation in a code path dominated by filesystem I/O.

- **G. Builder pattern (`AnalyzeDeps::new().fs_exists(...).run(...)`).**
  Rejected because it gives up the static guarantee that the
  carrier is fully populated. A forgotten `.is_writable_dir(...)`
  becomes a runtime error or a panic. The struct-literal form
  forces the carrier to be complete at construction time.

## Consequences

- **Positive.** `doctor::analyze`, `lint::evaluate`, `trace::locate`,
  and `sort::sort_path` are now uniform: each takes its dedicated
  `*Deps<'a>` carrier and has a `production()` constructor. Adding
  a new detector / matcher / resolver only needs an additive field
  on the carrier and an update to `*Deps::production()`. No more
  public-API break per detector.

- **Positive.** The `Box<dyn>` strategy gives identical ergonomics
  across all four carriers. Tests, embedders, and the production
  binary all build their carriers the same way. `clippy::type_complexity`
  is satisfied through type aliases (`EnvLookupFn`, etc.), not
  through `#[allow]`.

- **Positive.** ADR-0006 Follow-up is closed for the four
  headline public entry points. Every public entry point in
  `lib.rs` reaches `source_match::*_with` (the env-aware form)
  end to end. The `enforce_source_validation` call in
  `bin/pathlint/run.rs` is left on the non-`_with` wrapper
  deliberately — it is binary-side and always wants the
  production env, so adding a closure threading there would be
  noise.

- **Positive.** `#[allow(clippy::too_many_arguments)]` was the
  only `clippy::*` allow in the codebase for an API-shape lint;
  removing it lets a future contributor know that the shape is
  *meant* to stay narrow.

- **Negative.** This release lights up criterion 1 of PRD §3.1
  (the "≥ 2 consecutive releases without `### Breaking`" gate).
  Four entry points break at once, which is the worst single
  release for that gate but the fewest total releases needed —
  the alternative was four sequential breaking releases that
  each ate one criterion-1 reset. The plan also calls out
  Step 5 as additive-only to rebuild the streak.

- **Negative.** Dynamic dispatch in the hot path. The cost is
  one virtual call per closure invocation per detector iteration.
  Pathlint's hot paths are filesystem I/O (`fs_exists_real` does
  a `metadata` call, `fs_list_dir_real` walks a directory); the
  closure overhead is below the measurement floor.

- **Negative.** The `'a` lifetime parameter leaks into every
  type alias and every function signature. It buys flexibility
  (tests can pass closures that borrow from the stack) at the
  cost of one extra generic parameter in 5 type aliases and 4
  carriers. The `<'static>` impl block lets the production path
  keep a clean `AnalyzeDeps::production()` call.

- **Follow-up.** `bin/pathlint/run.rs::enforce_source_validation`
  is the last call site inside the repo that uses a non-`_with`
  wrapper. It is unambiguously binary-side, so leaving it on
  `validate_sources` is correct today, but if a future change
  makes that function reachable from a library entry point we
  will need to thread `env_lookup` there too. ADR-0006 marked
  the same call site; this ADR confirms it stays out of scope.

- **Follow-up.** Step 4 (`PathEntry` / `Attribution` split) is
  next on the roadmap. It is an unrelated BREAKING change and
  intentionally not bundled with this one — different concept,
  different review surface, different ADR.
