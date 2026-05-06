//! Drift gate for the checked-in `schemas/sort.schema.json`.
//!
//! Generated from `pathlint::sort::SortPlan` via schemars.
//! Whenever the SortPlan / EntryMove / SortNote shape changes,
//! this test fails until someone runs:
//!
//!     cargo run --bin gen_sort_schema > schemas/sort.schema.json

use std::fs;

#[test]
fn checked_in_sort_schema_matches_generator() {
    let mut actual_schema = schemars::schema_for!(pathlint::sort::SortPlan);
    let metadata = actual_schema
        .schema
        .metadata
        .get_or_insert_with(Default::default);
    metadata.id = Some(
        "https://raw.githubusercontent.com/ShortArrow/pathlint/main/schemas/sort.schema.json"
            .to_string(),
    );
    let actual =
        serde_json::to_string_pretty(&actual_schema).expect("schemars must serialize to JSON");
    let on_disk = fs::read_to_string("schemas/sort.schema.json").expect(
        "schemas/sort.schema.json must exist; run `cargo run --bin gen_sort_schema > schemas/sort.schema.json`",
    );

    let actual_normalized = actual.replace("\r\n", "\n");
    let on_disk_normalized = on_disk.replace("\r\n", "\n");
    assert_eq!(
        actual_normalized.trim_end(),
        on_disk_normalized.trim_end(),
        "checked-in schemas/sort.schema.json is out of date. \
         Regenerate with: cargo run --bin gen_sort_schema > schemas/sort.schema.json"
    );
}
