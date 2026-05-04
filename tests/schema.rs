//! Drift gate for the checked-in `schemas/pathlint.schema.json`.
//!
//! The schema is generated from the live Rust types via schemars,
//! and committed to the repo so editors / Schema Store can fetch it
//! by raw URL. Whenever the `Config` / `Expectation` / `SourceDef` /
//! `Relation` types change, this test fails until someone runs:
//!
//!     cargo run --bin gen_schema > schemas/pathlint.schema.json
//!
//! and commits the new bytes. That is by design — the checked-in
//! schema must always match what schemars would produce against
//! the current parser.

use std::fs;

#[test]
fn checked_in_schema_matches_generator() {
    let actual_schema = schemars::schema_for!(pathlint::config::Config);
    let actual =
        serde_json::to_string_pretty(&actual_schema).expect("schemars must serialize to JSON");
    let on_disk = fs::read_to_string("schemas/pathlint.schema.json")
        .expect("schemas/pathlint.schema.json must exist; run `cargo run --bin gen_schema > schemas/pathlint.schema.json`");

    // Tolerate trailing newlines from the redirected `cargo run`
    // output. Otherwise compare verbatim — even whitespace shifts
    // are real schema changes worth committing.
    assert_eq!(
        actual.trim_end(),
        on_disk.trim_end(),
        "checked-in schemas/pathlint.schema.json is out of date. \
         Regenerate with: cargo run --bin gen_schema > schemas/pathlint.schema.json"
    );
}
