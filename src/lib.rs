//! pathlint library — verifies that commands on PATH resolve from the expected installer.
//!
//! # Public API surface (0.0.15+)
//!
//! The supported library surface is **nine modules**:
//!
//! - [`config`]: TOML schema (Config / Expectation / SourceDef / Relation / Severity / Kind).
//! - [`lint`]: core PATH evaluation (Outcome / Status / Diagnosis / evaluate / exit_code / CheckOutcomeView).
//! - [`trace`]: provenance lookup (TraceOutcome / Found / Provenance / UninstallHint / locate).
//! - [`sort`]: PATH repair proposals (SortPlan / EntryMove / SortNote / sort_path).
//! - [`doctor`]: PATH hygiene (Diagnostic / Kind / Severity / analyze / analyze_real / Filter).
//! - [`catalog`]: built-in source catalog (builtin / merge_with_user / merge_with_user_relations / check_acyclic).
//! - [`source_match`]: path → source matching (find / names_only / validate_sources / Match / SourceWarning).
//! - [`os_detect`]: runtime OS dispatch (Os / os_filter_applies).
//! - [`expand`]: env-var expansion + slash normalisation (expand_env / normalize / expand_and_normalize).
//!
//! `tests/public_api.rs` pins every symbol pathlint promises here.
//! Anything not exported through one of those modules — including
//! the CLI plumbing, presentation layer, registry reader, and
//! orchestration glue — is **internal** and may change between
//! patch releases without notice.
//!
//! Across 0.0.x the library surface is treated as additive-only
//! best-effort; intentional breaks land at `0.0.x → 0.0.(x+1)`
//! boundaries (Cargo's MAJOR-equivalent for `0.0.y`) and are
//! flagged in release notes.

pub mod catalog;
pub mod config;
pub mod doctor;
pub mod expand;
pub mod lint;
pub mod os_detect;
pub mod sort;
pub mod source_match;
pub mod trace;

// Internal modules. Two visibility tiers:
//
// * `pub(crate)` for everything that only needs to be reachable
//   inside the lib crate itself (formatter, presentation, embed
//   reader, init template). These do not appear on docs.rs and
//   do not figure in the library contract.
// * `#[doc(hidden)] pub` for `cli` and `run`, which are reached
//   from `src/main.rs`. Cargo treats `src/main.rs` as a separate
//   crate that depends on `pathlint`, so `pub(crate)` would hide
//   them from the binary. They remain hidden from docs.rs and
//   are explicitly NOT part of the supported library surface.
pub(crate) mod catalog_view;
pub(crate) mod format;
pub(crate) mod init;
pub(crate) mod path_source;
pub(crate) mod report;
pub(crate) mod resolve;

#[doc(hidden)]
pub mod cli;
#[doc(hidden)]
pub mod run;
