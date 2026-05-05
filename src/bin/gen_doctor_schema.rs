//! Print the JSON Schema for `pathlint doctor --json` to stdout.
//!
//! Generated from the live `pathlint::doctor::Diagnostic` type via
//! `schemars` so the schema cannot drift from what the formatter
//! actually emits. Used by:
//!
//! - the `tests/doctor_schema.rs` drift gate (CI fails when the
//!   checked-in `schemas/doctor.schema.json` diverges from what
//!   this binary prints), and
//! - `release.yml`, which uploads the printed schema as a
//!   GitHub Release asset alongside the other schemas.
//!
//! Regenerate the checked-in copy with:
//!
//!     cargo run --bin gen_doctor_schema > schemas/doctor.schema.json

fn main() {
    let schema = schemars::schema_for!(pathlint::doctor::Diagnostic);
    let json =
        serde_json::to_string_pretty(&schema).expect("schemars output must serialize to JSON");
    println!("{json}");
}
