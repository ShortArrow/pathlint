//! Drift gate for the checked-in `schemas/check.schema.json`.
//!
//! The schema is generated from
//! `pathlint::format::CheckOutcomeView` via schemars and committed
//! to the repo so editors / Schema Store consumers can fetch it by
//! raw URL. Whenever `CheckOutcomeView` (or its nested `Status` /
//! `Diagnosis` / `Severity` types) changes, this test fails until
//! someone runs:
//!
//!     cargo run --bin gen_check_schema > schemas/check.schema.json
//!
//! and commits the new bytes. By design — the checked-in schema
//! must always match what schemars would produce against the
//! current formatter.

use std::fs;

#[test]
fn checked_in_check_schema_matches_generator() {
    let actual_schema = schemars::schema_for!(pathlint::format::CheckOutcomeView);
    let actual =
        serde_json::to_string_pretty(&actual_schema).expect("schemars must serialize to JSON");
    let on_disk = fs::read_to_string("schemas/check.schema.json")
        .expect("schemas/check.schema.json must exist; run `cargo run --bin gen_check_schema > schemas/check.schema.json`");

    // Compare line-endings-insensitive — same rationale as
    // tests/schema.rs (Windows checks the file out CRLF, generator
    // emits LF).
    let actual_normalized = actual.replace("\r\n", "\n");
    let on_disk_normalized = on_disk.replace("\r\n", "\n");
    assert_eq!(
        actual_normalized.trim_end(),
        on_disk_normalized.trim_end(),
        "checked-in schemas/check.schema.json is out of date. \
         Regenerate with: cargo run --bin gen_check_schema > schemas/check.schema.json"
    );
}
