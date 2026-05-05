//! pathlint library — verifies that commands on PATH resolve from the expected installer.
//!
//! # Public API surface (0.0.15+)
//!
//! The supported library surface is **nine modules**, each
//! described below by a few representative symbols. The
//! authoritative contract is `tests/public_api.rs`, which
//! imports every symbol pathlint promises here and fails the
//! build if any is moved or removed.
//!
//! - [`config`]: TOML schema. Headlines: `Config`, `Expectation`,
//!   `SourceDef`, `Relation`, `Severity`, `Kind`.
//! - [`lint`]: core PATH evaluation. Headlines: `evaluate`,
//!   `exit_code`, `Outcome`, `Status`, `Diagnosis`,
//!   `CheckOutcomeView`. Resolver closures take
//!   `&str -> Option<std::path::PathBuf>` (0.0.16+).
//! - [`trace`]: provenance lookup. Headlines: `locate`,
//!   `TraceOutcome`, `Found`, `Provenance`, `UninstallHint`.
//! - [`sort`]: PATH repair proposals. Headlines: `sort_path`,
//!   `SortPlan`, `EntryMove`, `SortNote`.
//! - [`doctor`]: PATH hygiene. Headlines: `analyze`,
//!   `analyze_real`, `Diagnostic`, `Filter`,
//!   plus the `Kind` / `Severity` enums.
//! - [`catalog`]: built-in source catalog. Headlines: `builtin`,
//!   `builtin_relations`, `merge_with_user`,
//!   `merge_with_user_relations`, `check_acyclic`,
//!   `version_check`, `embedded_version`.
//! - [`source_match`]: path → source matching. Headlines: `find`,
//!   `names_only`, `validate_sources`, `Match`, `SourceWarning`.
//! - [`os_detect`]: runtime OS dispatch. Headlines: `Os`,
//!   `os_filter_applies`.
//! - [`expand`]: env-var expansion + slash normalisation.
//!   Headlines: `expand_env`, `normalize`, `expand_and_normalize`.
//!
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
