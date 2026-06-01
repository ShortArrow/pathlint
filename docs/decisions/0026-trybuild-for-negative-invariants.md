# ADR-0026: adopt `trybuild` as the dev-dependency for compile-fail negative-invariant tests

- **Status**: Accepted
- **Date**: 2026-06-01
- **Release**: 0.0.33
- **Category**: 6. External dependency (+8. Process / governance for the testing-infra policy)

## Context

The 2026-05-31 codex 6-axis audit (recorded in CHANGELOG 0.0.30
Notes) flagged a TDD M finding:

> The negative invariant "`PathEntry` has no `provenance_raw` /
> `with_provenance` / `effective_raw_for_user_intent`" is only
> stated in a comment, not directly pinned by a compile-fail or
> equivalent negative test: see
> [tests/public_api.rs](tests/public_api.rs:170) and especially
> the comment at line 177. If someone re-added a provenance field
> while leaving `raw`/`expanded` intact, this test would still
> pass.

The invariant itself comes from ADR-0008 (0.0.28): when the
`Attribution` type was split out of `PathEntry`, three surfaces
moved off `PathEntry`:

- field `provenance_raw: Option<String>`
- method `with_provenance(self, String) -> Self`
- method `effective_raw_for_user_intent(&self) -> &str`

Today's pin is purely textual:

```rust
// tests/public_api.rs:177
// PathEntry no longer has provenance_raw / with_provenance /
// effective_raw_for_user_intent. Those moved to Attribution.
```

A refactor that re-introduced any of these on `PathEntry` (for
instance, "let's put the provenance field back to avoid the
`Attribution` wrapper overhead") would compile, run, and ship.
The CHANGELOG entry for 0.0.28 + ADR-0008 are the only durable
record of the invariant; nothing in the build pipeline enforces
it.

Rust doesn't have first-class compile-fail tests in the standard
test framework. The conventional Rust solution is the
[`trybuild`](https://crates.io/crates/trybuild) crate, which:

- accepts `.rs` snippets under a path the test points at,
- runs `rustc` on each snippet expecting it to fail,
- compares the produced `stderr` against a checked-in `.stderr`
  file, and
- reports test failure if a snippet starts to compile or if the
  diagnostic shifts unexpectedly.

`trybuild` is widely used in the Rust ecosystem (serde, anyhow,
clap, schemars all use it for derive-macro UI tests). The
dev-dependency model means production binaries are unaffected —
`trybuild` and its transitive deps only ship to anyone running
`cargo test`.

## Decision

Add `trybuild = "1"` to pathlint's `dev-dependencies` and
establish the following test infrastructure:

- `tests/ui/` directory holds one `.rs` snippet per negative
  invariant. Each snippet pairs with a `.stderr` file of the
  expected diagnostic.
- `tests/ui_compile_fail.rs` runs `trybuild::TestCases::new().compile_fail("tests/ui/*.rs")`
  inside one `#[test]`. CI runs this alongside the rest of
  `cargo test`.
- The first snippet is `tests/ui/path_entry_has_no_provenance_raw.rs`,
  pinning the ADR-0008 invariant.
- `tests/ui/*.stderr` files are checked in; regenerate with
  `TRYBUILD=overwrite cargo test --test ui_compile_fail` after a
  rustc upgrade or after intentionally moving the surface that a
  snippet pins against.

Future negative invariants follow the same pattern: add one
`.rs` snippet under `tests/ui/`, regenerate the `.stderr`,
commit both. No further infrastructure work.

## Alternatives considered

- **A. Keep the comment-only pin (M downgrade).** Rejected
  because comments are not build-time enforceable; a refactor
  that re-introduced the moved surface would silently break the
  invariant. ADR-0013's audit explicitly flagged this as M
  because the documented invariant has no machine-checked
  counterpart. Downgrade by ADR would document the limitation
  without removing it.

- **B. Hand-rolled macro trick inside an existing `#[test]`.**
  Rejected as fragile and surface-specific. The PathEntry
  invariant could be approximated with
  ```rust
  fn _pin() {
      let pe: PathEntry = todo!();
      // const _: () = if std::mem::size_of_val(&pe.provenance_raw) > 0 { ... };
  }
  ```
  but the trick varies per surface (field vs method vs trait
  impl) and produces non-obvious error messages. Future
  negative invariants would each need their own trick rather
  than reusing one mechanism.

- **C. Use a different crate (`compiletest_rs`, `compiletest-rs`,
  `assert_compiles_with`).** Rejected because `trybuild` is the
  most actively-maintained Rust-ecosystem standard for the
  exact use case (compile-fail tests with stderr diffing).
  `compiletest_rs` was last updated 2023, has rustc-internal
  dependencies, and is primarily used by the compiler test
  suite itself; `assert_compiles_with` lacks `.stderr` diffing.
  Adopting `trybuild` aligns with serde / anyhow / clap /
  schemars convention.

- **D. Move the invariant into a runtime check (panic if
  `mem::size_of::<PathEntry>() > expected`).** Rejected
  because runtime checks fail late (at test execution rather
  than compile time) and produce uninformative panic
  messages. The whole point of the negative invariant is "this
  code should not compile"; runtime enforcement is the wrong
  mechanism.

- **E. Wait until pathlint accumulates several negative
  invariants, then adopt `trybuild`.** Rejected because the
  marginal cost of adopting `trybuild` for the first
  invariant is small (one dev-dep + one test file + one
  snippet); deferring until "we have enough" is itself a
  judgment that reverses every time someone files a new
  negative invariant. Better to establish the infrastructure
  now so future contributors have the template.

## Consequences

- **Positive.** The ADR-0008 invariant is build-time enforced.
  A refactor that re-introduced any of `provenance_raw` /
  `with_provenance` / `effective_raw_for_user_intent` on
  `PathEntry` would fail `cargo test --test ui_compile_fail`
  before the change could merge.

- **Positive.** The infrastructure is reusable. Future
  negative invariants (e.g. "module `path_source` is not
  reachable from embedders without `#[doc(hidden)] pub`",
  "type `Attribution` cannot be constructed with an empty
  `raw`") slot into `tests/ui/` without further infrastructure
  work.

- **Positive.** `trybuild` is dev-only. Production binaries
  carry no extra weight. Embedders depending on pathlint as a
  library do not pull `trybuild` transitively.

- **Negative.** Initial `cargo test` after the first
  `trybuild` adoption pays a one-time cost: `trybuild` and
  its deps compile (~10 seconds on a warm cache, longer on a
  cold one). Subsequent runs use the incremental cache.

- **Negative.** `tests/ui/*.stderr` files can drift across
  rustc versions if the compiler changes diagnostic wording.
  CI runs on stable Rust; local developers on a different
  toolchain might see snapshot mismatches. Mitigation: the
  `.stderr` regeneration command is one line and documented
  in `tests/ui_compile_fail.rs` rustdoc.

- **Negative.** dev-dependency count rises from 1 (`tempfile`)
  to 2 (`tempfile` + `trybuild`). Modest, but worth recording.

- **Follow-up.** None scheduled. The pattern is now available;
  future negative invariants follow the established template.
  If `trybuild` ships a 2.x release with breaking changes, the
  decision to migrate goes through Cat 6 with a superseding
  ADR.
