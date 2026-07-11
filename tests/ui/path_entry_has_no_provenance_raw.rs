//! Compile-fail snippet pinning the 0.0.28 type-split invariant:
//! `PathEntry` has no `provenance_raw` field, no `with_provenance`
//! method, and no `effective_raw_for_user_intent` method. Those
//! concerns moved to `pathlint::Attribution` when the split landed.
//!
//! If a future refactor re-introduces any of these on `PathEntry`,
//! this snippet starts to compile and trybuild reports the test as
//! failing-to-fail, surfacing the regression at CI time.

fn main() {
    let pe = pathlint::path_entry::PathEntry::from_raw("/x", |_| -> Option<String> { None });
    let _ = pe.provenance_raw;
    let _ = pe.with_provenance("y".to_string());
    let _ = pe.effective_raw_for_user_intent();
}
