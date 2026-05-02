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

/// Find every source whose path is a substring of `haystack`.
/// Returned ranked: longest needle first (the most specific match
/// leads). Sources whose `path_for(os)` is missing or whose needle
/// expands to an empty string are skipped.
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
        if haystack.contains(&needle) {
            hits.push(Match {
                name: name.clone(),
                needle_len: needle.len(),
            });
        }
    }
    hits.sort_by_key(|h| std::cmp::Reverse(h.needle_len));
    hits
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
}
