//! Drift gate for the checked-in `schemas/check.schema.json`.
//!
//! The schema is generated from
//! `pathlint::lint::CheckOutcomeView` via schemars and committed
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
fn check_schema_required_excludes_skip_serializing_if_fields() {
    // 0.0.17 H fix (codex): the checked-in schema used to mark
    // `prefer` / `avoid` as required even though
    // CheckOutcomeView::From<&Outcome> applies
    // skip_serializing_if = "Vec::is_empty" on both. Result:
    // every Ok outcome with no prefer/avoid violated its own
    // schema. This gate keeps the two views aligned by asserting
    // those fields stay out of `required`.
    let schema = schemars::schema_for!(pathlint::lint::CheckOutcomeView);
    let json = serde_json::to_value(&schema).expect("schema must serialize");
    let required = json
        .get("required")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let required: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
    for field in ["prefer", "avoid", "reason", "diagnosis", "resolved"] {
        assert!(
            !required.contains(&field),
            "schema marks `{field}` as required but runtime uses skip_serializing_if; \
             update the field attribute or runtime to keep them aligned (required: {required:?})"
        );
    }
}

#[test]
fn checked_in_check_schema_matches_generator() {
    let mut actual_schema = schemars::schema_for!(pathlint::lint::CheckOutcomeView);
    let metadata = actual_schema
        .schema
        .metadata
        .get_or_insert_with(Default::default);
    metadata.id = Some(
        "https://raw.githubusercontent.com/ShortArrow/pathlint/main/schemas/check.schema.json"
            .to_string(),
    );
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
