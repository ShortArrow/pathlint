//! `PathEntry`: one PATH entry carrying both raw and env-expanded forms,
//! plus an optional cross-source provenance overlay.
//!
//! A PATH entry has two distinct semantic forms that detectors and
//! resolvers care about for different reasons:
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
//! # Observed vs. provenance
//!
//! `raw` / `expanded` describe a single entry as observed at one
//! source. There is one Windows case where two sources disagree:
//! `--target process` reads `getenv("PATH")`, but the OS has already
//! expanded `REG_EXPAND_SZ` registry values before handing them to
//! the child process — so `raw` on a process entry is always a
//! literal even if HKCU has `%LocalAppData%\...`.
//!
//! 0.0.24 introduces `provenance_raw: Option<String>` to fix this
//! single mismatch without changing the meaning of `--target`. When
//! the [`crate::path_source`] reconciler can match a process entry's
//! `expanded` with an HKCU or HKLM `expanded`, it copies the
//! registry's `raw` into `provenance_raw`. Detectors that reason
//! about user intent then go through
//! [`PathEntry::effective_raw_for_user_intent`], which prefers
//! provenance over the observed raw. `provenance_raw` stays `None`
//! on every other code path:
//!
//! - `--target user` / `--target machine` (raw is already authoritative)
//! - Unix / macOS (no registry, nothing to overlay)
//! - process entries that don't match any registry entry (process-
//!   only injection or post-session mutation; we leave them alone
//!   to avoid false suppression)
//! - REG_SZ entries whose raw equals the process raw (no expansion
//!   happened, so the overlay would be redundant)
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

/// One PATH entry as it flows from the source down to detectors and
/// resolvers. See the module docs for the semantic split between
/// `raw` and `expanded`, and for the role of `provenance_raw`.
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
    /// Cross-source overlay: the raw form observed at a different
    /// source whose `expanded` matches this entry's `expanded`.
    /// Set only by the `path_source` reconciler when running with
    /// `--target process` on Windows (matching against HKCU / HKLM).
    /// `None` everywhere else. See the module docs for the full
    /// rationale.
    pub provenance_raw: Option<String>,
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
    ///
    /// `provenance_raw` is `None` on construction. The
    /// `path_source` reconciler may attach it later via
    /// [`Self::with_provenance`] when an overlay applies.
    pub fn from_raw<V>(raw: impl Into<String>, env_lookup: V) -> Self
    where
        V: Fn(&str) -> Option<String>,
    {
        let raw = raw.into();
        let expanded = expand::expand_env_with(&raw, &env_lookup);
        Self {
            raw,
            expanded,
            provenance_raw: None,
        }
    }

    /// Attach a provenance overlay. Used by the `path_source`
    /// reconciler when a process entry's `expanded` matches an
    /// HKCU / HKLM entry whose `raw` differs (because the OS
    /// expanded a `%VAR%`). Idempotent and chainable.
    pub fn with_provenance(mut self, registry_raw: String) -> Self {
        self.provenance_raw = Some(registry_raw);
        self
    }

    /// Return the form the user authored, falling back to the
    /// observed `raw` when no overlay applies. Use this from
    /// detectors that reason about user intent (`Shortenable`,
    /// `Malformed`, `TrailingSlash`, `ShortName`) and from human
    /// rendering of `Diagnostic.entry`. Filesystem-side detectors
    /// (`Missing`, `WriteablePathDir`) keep using `expanded`.
    pub fn effective_raw_for_user_intent(&self) -> &str {
        self.provenance_raw.as_deref().unwrap_or(&self.raw)
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

    /// 0.0.24: with no provenance overlay, the accessor returns the
    /// observed raw form. This is the path every non-Windows entry
    /// and every `--target user` / `--target machine` entry takes.
    #[test]
    fn effective_raw_for_user_intent_falls_back_to_raw_when_none() {
        let e = PathEntry::from_raw(
            r"C:\Users\me\AppData\Local\Microsoft\WindowsApps",
            |_| -> Option<String> { None },
        );
        assert!(e.provenance_raw.is_none());
        assert_eq!(
            e.effective_raw_for_user_intent(),
            r"C:\Users\me\AppData\Local\Microsoft\WindowsApps",
        );
    }

    /// 0.0.24: when the path_source reconciler attaches a registry
    /// raw form via `with_provenance`, the accessor returns that
    /// form so raw-aware detectors and the human renderer see the
    /// `%VAR%` shape the user actually wrote in the registry.
    #[test]
    fn effective_raw_for_user_intent_returns_provenance_when_present() {
        let e = PathEntry::from_raw(
            r"C:\Users\me\AppData\Local\Microsoft\WindowsApps",
            |_| -> Option<String> { None },
        )
        .with_provenance(r"%LocalAppData%\Microsoft\WindowsApps".to_string());
        assert_eq!(
            e.provenance_raw.as_deref(),
            Some(r"%LocalAppData%\Microsoft\WindowsApps"),
        );
        assert_eq!(
            e.effective_raw_for_user_intent(),
            r"%LocalAppData%\Microsoft\WindowsApps",
        );
    }
}
