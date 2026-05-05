//! Drift gate for the checked-in `schemas/trace.schema.json`.
//!
//! Generated from `pathlint::trace::TraceJsonOutput` via schemars.
//! Whenever the trace JSON shape changes, this test fails until
//! someone runs:
//!
//!     cargo run --bin gen_trace_schema > schemas/trace.schema.json

use std::fs;

#[test]
fn checked_in_trace_schema_matches_generator() {
    let actual_schema = schemars::schema_for!(pathlint::trace::TraceJsonOutput);
    let actual =
        serde_json::to_string_pretty(&actual_schema).expect("schemars must serialize to JSON");
    let on_disk = fs::read_to_string("schemas/trace.schema.json").expect(
        "schemas/trace.schema.json must exist; run `cargo run --bin gen_trace_schema > schemas/trace.schema.json`",
    );

    let actual_normalized = actual.replace("\r\n", "\n");
    let on_disk_normalized = on_disk.replace("\r\n", "\n");
    assert_eq!(
        actual_normalized.trim_end(),
        on_disk_normalized.trim_end(),
        "checked-in schemas/trace.schema.json is out of date. \
         Regenerate with: cargo run --bin gen_trace_schema > schemas/trace.schema.json"
    );
}
