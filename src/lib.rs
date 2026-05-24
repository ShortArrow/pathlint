//! pathlint library — verifies that commands on PATH resolve from the expected installer.
//!
//! # Public API surface (0.0.15+)
//!
//! The supported library surface is **ten modules**, each
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
//!   `analyze_real`, `fs_list_dir_real` (the production wrapper
//!   for the 0.0.19 `fs_list_dir` closure parameter),
//!   `Diagnostic`, `Filter`, plus the `Kind` / `Severity` enums.
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
//! - [`path_entry`]: PATH entry duality (raw + expanded). Headlines:
//!   `PathEntry`, `PathEntry::from_raw`. 0.0.23+.
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
//!
//! # Quick example
//!
//! Evaluate one expectation against an in-process PATH without
//! reading a `pathlint.toml` from disk:
//!
//! ```
//! use pathlint::config::{Config, Expectation, Severity};
//! use pathlint::lint;
//! use pathlint::os_detect::Os;
//! use std::path::PathBuf;
//!
//! // Caller-supplied resolver. Production wiring would use a
//! // `which`-style PATH walk; this stub pretends `rg` lives in
//! // `~/.cargo/bin`. The closure boundary intentionally takes
//! // and returns standard-library types only — pathlint never
//! // exposes its own internal resolver type to embedders.
//! let resolver = |cmd: &str| -> Option<PathBuf> {
//!     match cmd {
//!         "rg" => Some(PathBuf::from("/home/me/.cargo/bin/rg")),
//!         _ => None,
//!     }
//! };
//!
//! let cfg = Config::default();
//! let expectations = vec![Expectation {
//!     command: "rg".into(),
//!     prefer: vec!["cargo".into()],
//!     avoid: vec![],
//!     os: None,
//!     optional: false,
//!     kind: None,
//!     severity: Severity::Error,
//! }];
//!
//! let sources = pathlint::catalog::merge_with_user(&cfg.source);
//! let deps = lint::EvaluateDeps {
//!     common: pathlint::CommonDeps { env_lookup: Box::new(|_v: &str| -> Option<String> { None }) },
//!     resolver: Box::new(resolver),
//!     shape_check: Box::new(lint::check_shape_filesystem), // R2 shape check; unused here
//! };
//! let outcomes = lint::evaluate(&expectations, &sources, Os::current(), deps);
//!
//! // outcomes[0].status is Status::Ok because the resolver picked
//! // a path under cargo's built-in source.
//! assert_eq!(outcomes.len(), 1);
//! ```
//!
//! For the full pipeline (read `pathlint.toml`, walk `$PATH`,
//! print to stdout, set the exit code) see the binary at
//! `src/bin/pathlint/run.rs` — the library is shaped so the binary
//! is a thin orchestration on top of the same primitives.

pub mod catalog;
pub mod config;
pub mod doctor;
pub mod expand;
pub mod lint;
pub mod os_detect;
pub mod path_entry;
pub mod sort;
pub mod source_match;
pub mod trace;

// Internal modules — `#[doc(hidden)] pub` for the ones the
// `pathlint` binary in `src/bin/pathlint/` needs to call across
// the lib/bin boundary, `pub(crate)` for everything strictly
// internal to the lib. Neither tier appears on docs.rs and
// neither is part of the library contract.
//
// 0.0.17 moved `cli` and `run` out of the lib entirely; they now
// live in `src/bin/pathlint/` alongside `main.rs`. The binary
// still consumes some lib internals (formatter, presentation,
// init template, OS PATH reader, command resolver) — those stay
// `#[doc(hidden)] pub` so Cargo can route the call across the
// crate boundary without putting them on the public surface.
#[doc(hidden)]
pub mod catalog_view;
#[doc(hidden)]
pub mod format;
#[doc(hidden)]
pub mod init;
#[doc(hidden)]
pub mod path_source;
#[doc(hidden)]
pub mod report;
#[doc(hidden)]
pub mod resolve;
pub(crate) mod shell_quote;

/// Shared dependency carrier embedded in every per-function `*Deps`
/// (see [`doctor::AnalyzeDeps`], [`lint::EvaluateDeps`],
/// [`trace::LocateDeps`], [`sort::SortDeps`]).
///
/// Today this only holds `env_lookup`, but the four
/// per-function carriers all reach for the same env oracle, so
/// concentrating it here keeps the "how does pathlint read the
/// process env?" decision in a single place. Future cross-cutting
/// concerns (logging hook, deterministic clock, etc.) belong here
/// too.
///
/// Production callers usually start from [`CommonDeps::production`]
/// and pass the resulting value into one of the per-function
/// carriers. Embedders supplying a deterministic env oracle
/// construct the struct directly.
///
/// 0.0.27+.
/// `Fn(&str) -> Option<String>` env oracle, type-erased through a
/// trait object. Used as the type of `CommonDeps::env_lookup` and
/// (transitively) every per-function `*Deps`. The `'a` lifetime lets
/// the closure borrow from the test stack; production wiring uses
/// `'static`. 0.0.27+.
pub type EnvLookupFn<'a> = Box<dyn Fn(&str) -> Option<String> + 'a>;

pub struct CommonDeps<'a> {
    /// The env oracle every `source_match::*_with` call inside the
    /// lib eventually consults. Type-erased through a trait object so
    /// each per-function `*Deps` can embed it without leaking another
    /// generic parameter — see ADR-0007 §Alternatives for why Box<dyn>
    /// beat the generic-closure variants.
    pub env_lookup: EnvLookupFn<'a>,
}

impl CommonDeps<'static> {
    /// Production wiring: env_lookup reads the live process
    /// environment via [`doctor::env_lookup_real`].
    pub fn production() -> Self {
        CommonDeps {
            env_lookup: Box::new(doctor::env_lookup_real),
        }
    }
}
