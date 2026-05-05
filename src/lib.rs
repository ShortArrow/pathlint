//! pathlint library — verifies that commands on PATH resolve from the expected installer.
//!
//! Only [`config`] is part of the supported public API. The rest
//! of the modules below are marked `#[doc(hidden)]` and are
//! visible only because the binary and integration tests need to
//! reach them across the `pathlint` lib / bin boundary. Treat
//! anything outside `pathlint::config` as implementation detail
//! that may break across 0.0.x and 0.1.x releases without a major
//! version bump. The 0.1.0 semver surface is the TOML schema
//! (Config / Expectation / SourceDef / Relation) plus the
//! `pathlint` CLI binary.

pub mod config;

#[doc(hidden)]
pub mod catalog;
#[doc(hidden)]
pub mod catalog_view;
#[doc(hidden)]
pub mod cli;
#[doc(hidden)]
pub mod doctor;
#[doc(hidden)]
pub mod expand;
#[doc(hidden)]
pub mod format;
#[doc(hidden)]
pub mod init;
#[doc(hidden)]
pub mod lint;
#[doc(hidden)]
pub mod os_detect;
#[doc(hidden)]
pub mod path_source;
#[doc(hidden)]
pub mod report;
#[doc(hidden)]
pub mod resolve;
#[doc(hidden)]
pub mod run;
#[doc(hidden)]
pub mod sort;
#[doc(hidden)]
pub mod source_match;
#[doc(hidden)]
pub mod where_cmd;
