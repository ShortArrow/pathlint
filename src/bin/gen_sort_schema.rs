//! Print the JSON Schema for `pathlint sort --json` to stdout.
//!
//! Generated from the live `pathlint::sort::SortPlan` type via
//! `schemars`. Used by:
//!
//! - the `tests/sort_schema.rs` drift gate, and
//! - `release.yml`, which uploads the printed schema as a
//!   GitHub Release asset alongside the other schemas.
//!
//! Regenerate the checked-in copy with:
//!
//!     cargo run --bin gen_sort_schema > schemas/sort.schema.json

fn main() {
    let schema = schemars::schema_for!(pathlint::sort::SortPlan);
    let json =
        serde_json::to_string_pretty(&schema).expect("schemars output must serialize to JSON");
    println!("{json}");
}
