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
    let mut actual_schema = schemars::schema_for!(pathlint::config::Config);
    let metadata = actual_schema
        .schema
        .metadata
        .get_or_insert_with(Default::default);
    metadata.id = Some(
        "https://raw.githubusercontent.com/ShortArrow/pathlint/main/schemas/pathlint.schema.json"
            .to_string(),
    );
    let actual =
        serde_json::to_string_pretty(&actual_schema).expect("schemars must serialize to JSON");
    let on_disk = fs::read_to_string("schemas/pathlint.schema.json")
        .expect("schemas/pathlint.schema.json must exist; run `cargo run --bin gen_schema > schemas/pathlint.schema.json`");

    // Compare line-endings-insensitive. Git's `core.autocrlf` on
    // Windows checks the schema file out with CRLF, while the
    // generator always emits LF — without normalization, the drift
    // gate would always fail on Windows runners. Whitespace inside
    // each line is still load-bearing (any real schema shift
    // changes byte content, not line-ending style).
    let actual_normalized = actual.replace("\r\n", "\n");
    let on_disk_normalized = on_disk.replace("\r\n", "\n");
    assert_eq!(
        actual_normalized.trim_end(),
        on_disk_normalized.trim_end(),
        "checked-in schemas/pathlint.schema.json is out of date. \
         Regenerate with: cargo run --bin gen_schema > schemas/pathlint.schema.json"
    );
}
