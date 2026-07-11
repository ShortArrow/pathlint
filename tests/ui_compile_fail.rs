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
//! trybuild is the pin mechanism because a comment-only invariant
//! rots silently and a hand-rolled macro cannot assert
//! non-compilation; a snippet that must fail to compile is the
//! strongest guard available for a "this API must not exist" claim.

#[test]
fn negative_invariants_stay_uncompilable() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
