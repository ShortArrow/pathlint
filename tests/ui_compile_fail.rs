//! trybuild harness for compile-fail tests under `tests/ui/`. Each
//! snippet pins a negative invariant: code that *should not compile*.
//! If any snippet starts to compile (because a future refactor
//! re-introduced the surface the snippet tests against), trybuild
//! reports the test as failing.
//!
//! Run locally with:
//!
//!     cargo test --test ui_compile_fail
//!
//! To regenerate `tests/ui/*.stderr` after a rustc upgrade:
//!
//!     TRYBUILD=overwrite cargo test --test ui_compile_fail
//!
//! See [ADR-0026](../docs/decisions/0026-trybuild-for-negative-invariants.md)
//! for the adoption rationale and the rejected alternatives
//! (hand-rolled macro tricks, `compiletest_rs`, comment-only pins).

#[test]
fn negative_invariants_stay_uncompilable() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
