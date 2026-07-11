//! `PathEntry`: one PATH entry observed at a single source.
//!
//! A PATH entry has two semantic forms that detectors and resolvers
//! care about for different reasons:
//!
//! * **raw** — the string as stored at the source this entry came
//!   from. On Windows registry that means `%LocalAppData%\WindowsApps`
//!   for a `REG_EXPAND_SZ` value; on Unix that means `~/.local/bin`
//!   or `$HOME/bin` if the shell did not expand it before exporting
//!   `PATH`. Detectors that reason about *what the user typed* (e.g.
//!   `Shortenable`, `RelativePathEntry` for unresolved variables)
//!   need the raw form so they don't suggest a shortening the user
//!   already wrote.
//!
//! * **expanded** — the result of [`crate::expand::expand_env`] on
//!   `raw`. Detectors that reason about *the directory on disk*
//!   (`Missing`, `WriteablePathDir`, the resolver) need the expanded
//!   form because the filesystem doesn't know what `%LocalAppData%`
//!   means.
//!
//! pathlint computes both at the [`crate::path_source`] boundary, so
//! everything downstream picks its side from the type and never has
//! to ask "is this already expanded?" at runtime.
//!
//! Cross-source overlay (the case where one process-target entry's
//! `raw` was an OS-expanded literal of a `%VAR%` form stored in the
//! Windows registry) is *not* a property of `PathEntry`. That
//! information lives on [`crate::Attribution`], which wraps a
//! `PathEntry` together with an optional `provenance_raw` recovered
//! from cross-source matching.
//!
//! # Examples
//!
//! ```
//! use pathlint::path_entry::PathEntry;
//!
//! // Construction from raw runs `expand_env_with` once. The closure
//! // is the only env oracle — pathlint never reads the process
//! // environment from this constructor.
//! let e = PathEntry::from_raw("/usr/bin", |_| -> Option<String> { None });
//! assert_eq!(e.raw, "/usr/bin");
//! assert_eq!(e.expanded, "/usr/bin");
//!
//! // The closure decides what `$VAR` / `%VAR%` / `~` resolve to.
//! let e = PathEntry::from_raw("$VAR/bin", |k| {
//!     (k == "VAR").then(|| "/x".to_string())
//! });
//! assert_eq!(e.expanded, "/x/bin");
//!
//! // Unresolved variables stay verbatim.
//! let e = PathEntry::from_raw("$NOPE/bin", |_| None);
//! assert_eq!(e.expanded, "$NOPE/bin");
//! ```

use crate::expand;

/// One PATH entry as observed at a single source. Pure data:
/// `raw` is the on-source form, `expanded` is `expand::expand_env`
/// applied once at the boundary. 0.0.28 restored `PathEntry` to
/// this two-field shape — cross-source overlay moved to
/// [`crate::Attribution`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathEntry {
    /// As stored at the source this entry came from. Preserves
    /// `%VAR%` / `$VAR` / `${VAR}` / a leading `~`. Pure
    /// construction — never side-effects.
    pub raw: String,
    /// `expand::expand_env(&raw)`. Computed once at the boundary
    /// (`path_source::read_path` for production, `from_raw` for
    /// callers building a `PathEntry` directly). If a variable is
    /// unresolved the raw form is left verbatim by `expand_env`'s
    /// contract.
    pub expanded: String,
}

impl PathEntry {
    /// Build a `PathEntry` from a raw string by running
    /// [`crate::expand::expand_env_with`] exactly once with the
    /// caller-supplied env lookup. The intended construction path
    /// for every caller — keeps the raw/expanded duality consistent
    /// and makes env injection uniform across the lib.
    ///
    /// pathlint never reads the process environment from this
    /// constructor: the closure is the only oracle. Production
    /// callers (`path_source::read_path`, `resolve::split_path`)
    /// pass `|v| std::env::var(v).ok()` so the constructor still
    /// reflects the host env in production. Tests and lib
    /// embedders pass deterministic closures so behaviour is
    /// independent of whatever vars happen to exist on the host.
    pub fn from_raw<V>(raw: impl Into<String>, env_lookup: V) -> Self
    where
        V: Fn(&str) -> Option<String>,
    {
        let raw = raw.into();
        let expanded = expand::expand_env_with(&raw, &env_lookup);
        Self { raw, expanded }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_raw_preserves_literal_path() {
        let e = PathEntry::from_raw("/usr/bin", |_| -> Option<String> { None });
        assert_eq!(e.raw, "/usr/bin");
        assert_eq!(e.expanded, "/usr/bin");
    }

    #[test]
    fn from_raw_keeps_raw_for_unresolved_var() {
        // `$THIS_VAR_...` is unlikely to be in the test process env;
        // expand_env_with + closure returning None returns the input
        // verbatim in that case.
        let e = PathEntry::from_raw(
            "$THIS_VAR_DOES_NOT_EXIST_PROBABLY_XYZ/bin",
            |_| -> Option<String> { None },
        );
        assert!(e.raw.starts_with('$'));
        assert!(e.raw.contains("THIS_VAR_DOES_NOT_EXIST_PROBABLY_XYZ"));
        // With a None lookup, expanded === raw (verbatim).
        assert_eq!(e.expanded, e.raw);
    }

    /// 0.0.23: PathEntry::from_raw must consult only the supplied
    /// `env_lookup` — never `std::env::var` directly. Pre-injection,
    /// the constructor read the live process env, which made tests
    /// non-deterministic and lib embedders unable to substitute
    /// their own oracle.
    #[test]
    fn from_raw_uses_caller_env_lookup() {
        let e = PathEntry::from_raw("$STUB/bin", |k| {
            (k == "STUB").then(|| "/from-closure".to_string())
        });
        assert_eq!(e.raw, "$STUB/bin");
        assert_eq!(e.expanded, "/from-closure/bin");
    }

    // 0.0.28: effective_raw_for_user_intent / with_provenance /
    // provenance_raw tests moved to `crate::attribution_tests`
    // alongside the `Attribution` type that now owns those concepts.
}
