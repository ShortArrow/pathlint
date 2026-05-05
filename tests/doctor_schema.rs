//! Drift gate for the checked-in `schemas/doctor.schema.json`.
//!
//! Generated from `pathlint::doctor::Diagnostic` via schemars.
//! Whenever the doctor diagnostic shape changes, this test fails
//! until someone runs:
//!
//!     cargo run --bin gen_doctor_schema > schemas/doctor.schema.json
//!
//! See tests/schema.rs for the parallel TOML config gate and
//! tests/check_schema.rs for `check --json`.

use std::fs;

#[test]
fn checked_in_doctor_schema_matches_generator() {
    let actual_schema = schemars::schema_for!(pathlint::doctor::Diagnostic);
    let actual =
        serde_json::to_string_pretty(&actual_schema).expect("schemars must serialize to JSON");
    let on_disk = fs::read_to_string("schemas/doctor.schema.json").expect(
        "schemas/doctor.schema.json must exist; run `cargo run --bin gen_doctor_schema > schemas/doctor.schema.json`",
    );

    let actual_normalized = actual.replace("\r\n", "\n");
    let on_disk_normalized = on_disk.replace("\r\n", "\n");
    assert_eq!(
        actual_normalized.trim_end(),
        on_disk_normalized.trim_end(),
        "checked-in schemas/doctor.schema.json is out of date. \
         Regenerate with: cargo run --bin gen_doctor_schema > schemas/doctor.schema.json"
    );
}
