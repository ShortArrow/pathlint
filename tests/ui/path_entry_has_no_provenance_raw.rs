//! Compile-fail snippet pinning ADR-0008's invariant: `PathEntry`
//! has no `provenance_raw` field, no `with_provenance` method, and
//! no `effective_raw_for_user_intent` method. Those concerns moved
//! to `pathlint::Attribution` when the 0.0.28 split landed.
//!
//! If a future refactor re-introduces any of these on `PathEntry`,
//! this snippet starts to compile and trybuild reports the test as
//! failing-to-fail, surfacing the regression at CI time.
//!
//! See [ADR-0026](../../docs/decisions/0026-trybuild-for-negative-invariants.md)
//! for why trybuild is the negative-invariant pin mechanism, and
//! [ADR-0008](../../docs/decisions/0008-attribution-type-split.md) for the
//! invariant itself.

fn main() {
    let pe = pathlint::path_entry::PathEntry::from_raw("/x", |_| -> Option<String> { None });
    let _ = pe.provenance_raw;
    let _ = pe.with_provenance("y".to_string());
    let _ = pe.effective_raw_for_user_intent();
}
