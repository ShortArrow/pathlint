//! Print the JSON Schema for `pathlint.toml` to stdout.
//!
//! Generated from the live `pathlint::config::Config` types via
//! `schemars` so the schema cannot drift from what the parser
//! accepts. Used by:
//!
//! - the `tests/schema.rs` drift gate (CI fails when the
//!   checked-in `schemas/pathlint.schema.json` diverges from
//!   what this binary prints), and
//! - `release.yml`, which uploads the printed schema as a
//!   GitHub Release asset for users / Schema Store registration.
//!
//! Regenerate the checked-in copy with:
//!
//!     cargo run --bin gen_schema > schemas/pathlint.schema.json

fn main() {
    let mut schema = schemars::schema_for!(pathlint::config::Config);
    let metadata = schema.schema.metadata.get_or_insert_with(Default::default);
    metadata.id = Some(
        "https://raw.githubusercontent.com/ShortArrow/pathlint/main/schemas/pathlint.schema.json"
            .to_string(),
    );
    let json =
        serde_json::to_string_pretty(&schema).expect("schemars output must serialize to JSON");
    println!("{json}");
}
