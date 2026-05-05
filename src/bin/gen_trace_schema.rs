//! Print the JSON Schema for `pathlint trace --json` to stdout.
//!
//! Generated from the live `pathlint::trace::TraceJsonOutput`
//! type via `schemars`. Used by:
//!
//! - the `tests/trace_schema.rs` drift gate, and
//! - `release.yml`, which uploads the printed schema as a
//!   GitHub Release asset alongside the other schemas.
//!
//! Regenerate the checked-in copy with:
//!
//!     cargo run --bin gen_trace_schema > schemas/trace.schema.json

fn main() {
    let schema = schemars::schema_for!(pathlint::trace::TraceJsonOutput);
    let json =
        serde_json::to_string_pretty(&schema).expect("schemars output must serialize to JSON");
    println!("{json}");
}
