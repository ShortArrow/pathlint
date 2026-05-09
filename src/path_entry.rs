//! `PathEntry`: one PATH entry carrying both raw and env-expanded forms.
//!
//! A PATH entry has two distinct semantic forms that detectors and
//! resolvers care about for different reasons:
//!
//! * **raw** — the string as stored at the source. On Windows that
//!   means `%LocalAppData%\WindowsApps` for a `REG_EXPAND_SZ` registry
//!   value; on Unix that means `~/.local/bin` or `$HOME/bin` if the
//!   shell did not expand it before exporting `PATH`. Detectors that
//!   reason about *what the user typed* (e.g. `Shortenable`,
//!   `RelativePathEntry` for unresolved variables) need the raw form
//!   so they don't suggest a shortening the user already wrote.
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
//! # Examples
//!
//! ```
//! use pathlint::path_entry::PathEntry;
//!
//! // Construction from raw runs `expand_env` once.
//! let e = PathEntry::from_raw("/usr/bin");
//! assert_eq!(e.raw, "/usr/bin");
//! assert_eq!(e.expanded, "/usr/bin");
//!
//! // Unresolved variables stay verbatim in `expanded` (matches
//! // `expand_env`'s contract).
//! let e = PathEntry::from_raw("$THIS_VAR_DOES_NOT_EXIST_PROBABLY_XYZ/bin");
//! assert!(e.raw.contains('$'));
//! ```

use crate::expand;

/// One PATH entry as it flows from the source down to detectors and
/// resolvers. See the module docs for the semantic split between
/// `raw` and `expanded`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathEntry {
    /// As stored at the source. Preserves `%VAR%` / `$VAR` /
    /// `${VAR}` / a leading `~`. Pure construction — never
    /// side-effects.
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
    /// `expand::expand_env` once. The intended construction path for
    /// every caller — keeps the raw/expanded duality consistent.
    pub fn from_raw(raw: impl Into<String>) -> Self {
        let raw = raw.into();
        let expanded = expand::expand_env(&raw);
        Self { raw, expanded }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_raw_preserves_literal_path() {
        let e = PathEntry::from_raw("/usr/bin");
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
}
