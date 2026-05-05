//! Print the JSON Schema for `pathlint check --json` to stdout.
//!
//! Generated from the live `pathlint::lint::CheckOutcomeView`
//! type via `schemars` so the schema cannot drift from what the
//! formatter actually emits. Used by:
//!
//! - the `tests/check_schema.rs` drift gate (CI fails when the
//!   checked-in `schemas/check.schema.json` diverges from what
//!   this binary prints), and
//! - `release.yml`, which uploads the printed schema as a
//!   GitHub Release asset alongside `pathlint.schema.json`.
//!
//! Regenerate the checked-in copy with:
//!
//!     cargo run --bin gen_check_schema > schemas/check.schema.json

fn main() {
    let schema = schemars::schema_for!(pathlint::lint::CheckOutcomeView);
    let json =
        serde_json::to_string_pretty(&schema).expect("schemars output must serialize to JSON");
    println!("{json}");
}
