//! Match a resolved path against the merged source catalog.
//!
//! Pure function that asks: "for this path, which `[source.<name>]`
//! entries point at a directory the path lives inside?" Used by
//! both `lint::evaluate` (to decide whether `prefer` / `avoid`
//! intersects the matches) and `where_cmd::locate` (to rank
//! sources by specificity before picking an uninstall hint).
//!
//! The single entry point is `find` — callers consume the ranked
//! list directly, or strip the rank with `names_only` when
//! ordering does not matter.

use std::collections::BTreeMap;

use crate::config::SourceDef;
use crate::expand::expand_and_normalize;
use crate::os_detect::Os;

/// One source matched against the haystack. `needle_len` is the
/// length of the (expanded, normalised) source path, used as a
/// proxy for specificity — longer means "more deeply rooted, so
/// most likely the thing that owns this binary".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub name: String,
    pub needle_len: usize,
}

/// Find every source whose path appears in `haystack` aligned to a
/// path-segment boundary (the character after the match must be `/`
/// or the end of the haystack). Returned ranked: longest needle
/// first (the most specific match leads). Sources whose
/// `path_for(os)` is missing or whose needle expands to an empty
/// string are skipped.
///
/// Boundary alignment fixes a 0.0.9 footgun where
/// `/home/u/.cargo/bin` was reported as a match for paths under
/// `/home/u/.cargo/binx/...` purely because `contains` is byte-wise.
pub fn find(haystack: &str, sources: &BTreeMap<String, SourceDef>, os: Os) -> Vec<Match> {
    let mut hits: Vec<Match> = Vec::new();
    for (name, def) in sources {
        let Some(raw) = def.path_for(os) else {
            continue;
        };
        let needle = expand_and_normalize(raw);
        if needle.is_empty() {
            continue;
        }
        if needle_aligned_to_boundary(haystack, &needle) {
            hits.push(Match {
                name: name.clone(),
                needle_len: needle.len(),
            });
        }
    }
    hits.sort_by_key(|h| std::cmp::Reverse(h.needle_len));
    hits
}

/// True iff `needle` occurs in `haystack` aligned to a path
/// segment boundary. "Aligned" means either:
/// - the needle ends with `/` (it already encodes its own trailing
///   boundary, so the match cannot land mid-segment), or
/// - the byte right after the match is past the end of haystack
///   or is a `/`.
///
/// This rules out the `/cargo/bin` vs `/cargo/binx/rg` collision
/// while still accepting fragment-style needles like
/// `Microsoft/WindowsApps` that some built-ins use intentionally.
fn needle_aligned_to_boundary(haystack: &str, needle: &str) -> bool {
    if needle.ends_with('/') {
        return haystack.contains(needle);
    }
    haystack.match_indices(needle).any(|(start, _)| {
        let end = start + needle.len();
        let after = &haystack[end..];
        after.is_empty() || after.starts_with('/')
    })
}

/// One source flagged by [`validate_sources`] as too broad to be
/// safe. `name` is the catalog key (e.g. `evil`), `needle` is the
/// expanded path that triggered the warning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceWarning {
    pub name: String,
    pub needle: String,
    pub reason: SourceWarningReason,
}

/// Why a source's needle was rejected. Open enum so future kinds
/// (e.g. UNC paths, drive letters without separators) can be added
/// without breaking existing match arms.
///
/// Note: relative needles like `Microsoft/WindowsApps` are allowed
/// — several built-in sources (e.g. `WindowsApps`) intentionally
/// match by path fragment to flag the Microsoft Store stub layer
/// no matter where it appears in PATH. The boundary check in
/// `find` keeps fragments from over-matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceWarningReason {
    /// Needle expands to `/`, `\`, or another path that would
    /// match every PATH entry on the system.
    RootPath,
    /// Needle is too short (under 3 bytes) to be a meaningful
    /// directory and would over-attribute PATH entries.
    NeedleTooShort,
}

/// Walk every source in `sources` and report needles that are too
/// permissive to be safe. A hostile or careless catalog override
/// (`[source.evil] unix = "/"`) would otherwise mark every PATH
/// entry as belonging to that source.
///
/// Pure. Returns warnings instead of erroring directly so the
/// caller can choose whether to fail (`run.rs` does, with exit 2)
/// or report-and-continue.
pub fn validate_sources(sources: &BTreeMap<String, SourceDef>, os: Os) -> Vec<SourceWarning> {
    let mut warnings = Vec::new();
    for (name, def) in sources {
        let Some(raw) = def.path_for(os) else {
            continue;
        };
        let needle = expand_and_normalize(raw);
        if needle.is_empty() {
            continue;
        }
        let reason = classify_needle(&needle);
        if let Some(reason) = reason {
            warnings.push(SourceWarning {
                name: name.clone(),
                needle,
                reason,
            });
        }
    }
    warnings
}

fn classify_needle(needle: &str) -> Option<SourceWarningReason> {
    if needle == "/" || needle == "\\" {
        return Some(SourceWarningReason::RootPath);
    }
    // Windows drive-root forms: `C:\`, `C:/`, `C:` — anything that
    // boils down to "the entire C: volume" is just as broad as
    // bare `/` on Unix.
    if is_windows_drive_root(needle) {
        return Some(SourceWarningReason::RootPath);
    }
    if needle.len() < 3 {
        return Some(SourceWarningReason::NeedleTooShort);
    }
    None
}

fn is_windows_drive_root(needle: &str) -> bool {
    let bytes = needle.as_bytes();
    let drive_letter = bytes
        .first()
        .map(|b| b.is_ascii_alphabetic())
        .unwrap_or(false);
    if !drive_letter || bytes.get(1) != Some(&b':') {
        return false;
    }
    match &bytes[2..] {
        [] => true,                    // `C:`
        [b'/' | b'\\'] => true,        // `C:/` / `C:\`
        _ => false,
    }
}

/// Convenience: just the names from `find`, in rank order. Callers
/// that don't need the specificity score reach for this.
pub fn names_only(haystack: &str, sources: &BTreeMap<String, SourceDef>, os: Os) -> Vec<String> {
    find(haystack, sources, os)
        .into_iter()
        .map(|m| m.name)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(unix: &str) -> SourceDef {
        SourceDef {
            unix: Some(unix.into()),
            ..Default::default()
        }
    }

    fn cat(entries: &[(&str, SourceDef)]) -> BTreeMap<String, SourceDef> {
        entries
            .iter()
            .map(|(n, d)| (n.to_string(), d.clone()))
            .collect()
    }

    #[test]
    fn find_returns_empty_when_no_source_matches() {
        let sources = cat(&[("cargo", src("/home/u/.cargo/bin"))]);
        let out = find("/usr/local/bin/rg", &sources, Os::Linux);
        assert!(out.is_empty());
    }

    #[test]
    fn find_skips_sources_with_no_path_for_current_os() {
        let def = SourceDef {
            windows: Some("WinGet".into()),
            ..Default::default()
        };
        let sources = cat(&[("winget", def)]);
        let out = find("/home/u/.cargo/bin/rg", &sources, Os::Linux);
        assert!(out.is_empty());
    }

    #[test]
    fn find_ranks_longer_needle_first() {
        // mise_installs path is more specific than mise itself; a
        // binary served from installs/python/.../bin must lead with
        // the longer needle.
        let sources = cat(&[
            ("mise", src("/home/u/.local/share/mise")),
            ("mise_installs", src("/home/u/.local/share/mise/installs")),
        ]);
        let out = find(
            "/home/u/.local/share/mise/installs/python/3.14/bin/python",
            &sources,
            Os::Linux,
        );
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].name, "mise_installs", "longer needle should lead");
        assert_eq!(out[1].name, "mise");
        assert!(out[0].needle_len > out[1].needle_len);
    }

    #[test]
    fn find_skips_empty_needles() {
        // A SourceDef with `unix = ""` should not match anything,
        // because `String::contains("")` is trivially true.
        let sources = cat(&[("empty", src(""))]);
        let out = find("/anywhere/at/all", &sources, Os::Linux);
        assert!(out.is_empty(), "empty needle must not match");
    }

    #[test]
    fn names_only_strips_specificity_but_keeps_order() {
        let sources = cat(&[
            ("mise", src("/home/u/.local/share/mise")),
            ("mise_installs", src("/home/u/.local/share/mise/installs")),
        ]);
        let out = names_only(
            "/home/u/.local/share/mise/installs/python/3.14/bin/python",
            &sources,
            Os::Linux,
        );
        assert_eq!(out, vec!["mise_installs".to_string(), "mise".to_string()]);
    }

    // ---- 0.0.10: path-segment boundary + validate_sources -----------

    #[test]
    fn find_does_not_match_partial_segment() {
        // A naive substring matcher claims `/home/u/.cargo/bin` is
        // inside `/home/u/.cargo/binx/rg` even though `binx` is a
        // different directory. 0.0.10 enforces the boundary so the
        // segment after the needle must end (`/`) or be the end of
        // the haystack.
        let sources = cat(&[("cargo", src("/home/u/.cargo/bin"))]);
        let out = find("/home/u/.cargo/binx/rg", &sources, Os::Linux);
        assert!(
            out.is_empty(),
            "needle ending mid-segment must not match: {out:?}"
        );
    }

    #[test]
    fn find_matches_when_needle_ends_haystack_exactly() {
        let sources = cat(&[("cargo", src("/home/u/.cargo/bin"))]);
        let out = find("/home/u/.cargo/bin", &sources, Os::Linux);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "cargo");
    }

    #[test]
    fn find_matches_when_needle_is_followed_by_separator() {
        let sources = cat(&[("cargo", src("/home/u/.cargo/bin"))]);
        let out = find("/home/u/.cargo/bin/rg", &sources, Os::Linux);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "cargo");
    }

    #[test]
    fn validate_sources_rejects_root_path() {
        let sources = cat(&[("evil", src("/"))]);
        let warnings = validate_sources(&sources, Os::Linux);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].name, "evil");
    }

    #[test]
    fn validate_sources_rejects_windows_drive_root() {
        let def = SourceDef {
            windows: Some("C:\\".into()),
            ..Default::default()
        };
        let sources = cat(&[("evil_drive", def)]);
        let warnings = validate_sources(&sources, Os::Windows);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].reason, SourceWarningReason::RootPath);
    }

    #[test]
    fn validate_sources_rejects_bare_drive_letter() {
        let def = SourceDef {
            windows: Some("d:".into()),
            ..Default::default()
        };
        let sources = cat(&[("evil_d", def)]);
        let warnings = validate_sources(&sources, Os::Windows);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].reason, SourceWarningReason::RootPath);
    }

    #[test]
    fn validate_sources_rejects_too_short_needle() {
        // Two-char needles like `.x` after expand are too easy to
        // collide accidentally with PATH entries. Reject them.
        let sources = cat(&[("ev", src(".x"))]);
        let warnings = validate_sources(&sources, Os::Linux);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn validate_sources_accepts_normal_paths() {
        let sources = cat(&[
            ("cargo", src("/home/u/.cargo/bin")),
            ("apt", src("/usr/bin")),
        ]);
        let warnings = validate_sources(&sources, Os::Linux);
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    }

    #[test]
    fn validate_sources_skips_sources_without_path_for_os() {
        // A windows-only source running on Linux must not trigger a
        // false positive — there's no needle to validate at all.
        let def = SourceDef {
            windows: Some("WinGet".into()),
            ..Default::default()
        };
        let sources = cat(&[("winget", def)]);
        let warnings = validate_sources(&sources, Os::Linux);
        assert!(warnings.is_empty());
    }
}
