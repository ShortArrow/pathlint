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
    /// generic parameter into every signature that carries the deps.
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

/// Per-entry cross-source carrier. Wraps a single-source
/// [`crate::path_entry::PathEntry`] together with an optional
/// `provenance_raw`: the `raw` form observed at *another* source
/// whose `expanded` matches this one. The only place that sets
/// `provenance_raw` today is
/// [`crate::path_source::reconcile_process_with_registry`], which
/// recovers the registry `%VAR%` form for entries the OS expanded
/// on its way into `getenv("PATH")` (the `--target process`
/// overlay).
///
/// 0.0.28 split this out of `PathEntry` so the carrier inside the
/// lib has one clean concept per type: `PathEntry` is "what one
/// source said this entry is"; `Attribution` is "that observation
/// plus whatever cross-source hint we have".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribution {
    /// The entry as observed at the source that originally
    /// produced it. Carries `raw` (on-source form) and `expanded`
    /// (`expand::expand_env`-once form).
    pub observed: path_entry::PathEntry,
    /// Cross-source overlay. `Some(reg_raw)` when this process
    /// entry was produced by the OS expanding a registry-stored
    /// `%VAR%` form (HKCU\Environment\Path or HKLM\...). `None`
    /// otherwise: `--target user` / `--target machine` (already
    /// raw at source), Unix / macOS (no registry), process
    /// entries with no expanded match in registry, or REG_SZ
    /// entries whose `raw == observed.raw` (no expansion happened).
    pub provenance_raw: Option<String>,
}

impl Attribution {
    /// Wrap a freshly-observed `PathEntry`; `provenance_raw` is
    /// `None`. Use this when you have one source's view of the
    /// entry and no cross-source hint to apply yet.
    pub fn new(observed: path_entry::PathEntry) -> Self {
        Attribution {
            observed,
            provenance_raw: None,
        }
    }

    /// Chainable setter for the cross-source overlay. Used by the
    /// `path_source` reconciler when a process entry's `expanded`
    /// matches an HKCU / HKLM entry whose `raw` differs (because
    /// the OS expanded a `%VAR%`). Idempotent.
    pub fn with_provenance(mut self, registry_raw: String) -> Self {
        self.provenance_raw = Some(registry_raw);
        self
    }

    /// Return the form the user authored, falling back to the
    /// observed `raw` when no overlay applies. Use this from
    /// detectors that reason about user intent (`Shortenable`,
    /// `Malformed`, `TrailingSlash`, `ShortName`) and from human
    /// rendering of `Diagnostic.entry`. Filesystem-side detectors
    /// (`Missing`, `WriteablePathDir`) keep using `observed.expanded`.
    pub fn effective_raw_for_user_intent(&self) -> &str {
        self.provenance_raw.as_deref().unwrap_or(&self.observed.raw)
    }
}

#[cfg(test)]
mod attribution_tests {
    use super::*;
    use path_entry::PathEntry;

    /// 0.0.28: with no provenance overlay, the accessor returns
    /// the observed raw form. This is the path every non-Windows
    /// entry and every `--target user` / `--target machine` entry
    /// takes.
    #[test]
    fn effective_raw_for_user_intent_falls_back_to_raw_when_none() {
        let pe = PathEntry::from_raw(
            r"C:\Users\me\AppData\Local\Microsoft\WindowsApps",
            |_| -> Option<String> { None },
        );
        let a = Attribution::new(pe);
        assert!(a.provenance_raw.is_none());
        assert_eq!(
            a.effective_raw_for_user_intent(),
            r"C:\Users\me\AppData\Local\Microsoft\WindowsApps",
        );
    }

    /// 0.0.28: when the path_source reconciler attaches a registry
    /// raw form via `with_provenance`, the accessor returns that
    /// form so raw-aware detectors and the human renderer see the
    /// `%VAR%` shape the user actually wrote in the registry.
    #[test]
    fn effective_raw_for_user_intent_returns_provenance_when_present() {
        let pe = PathEntry::from_raw(
            r"C:\Users\me\AppData\Local\Microsoft\WindowsApps",
            |_| -> Option<String> { None },
        );
        let a = Attribution::new(pe)
            .with_provenance(r"%LocalAppData%\Microsoft\WindowsApps".to_string());
        assert_eq!(
            a.provenance_raw.as_deref(),
            Some(r"%LocalAppData%\Microsoft\WindowsApps"),
        );
        assert_eq!(
            a.effective_raw_for_user_intent(),
            r"%LocalAppData%\Microsoft\WindowsApps",
        );
    }
}
