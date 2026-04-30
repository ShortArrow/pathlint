//! R4 — `pathlint where <command>` provenance.
//!
//! Given a command name, resolve it against the merged catalog and
//! produce a structured answer: where it lives, which sources match,
//! and the most plausible uninstall hint.
//!
//! Pure where possible; the resolver is injected so tests don't need
//! to touch the real PATH.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::config::SourceDef;
use crate::expand::{expand_and_normalize, normalize};
use crate::os_detect::Os;
use crate::resolve::Resolution;

#[derive(Debug, PartialEq, Eq)]
pub enum WhereOutcome {
    /// The command resolved; here is everything we know about it.
    Found(Found),
    /// The command did not resolve from PATH at all.
    NotFound,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Found {
    pub command: String,
    pub resolved: PathBuf,
    /// Sources matched against the resolved path. The most specific
    /// (longest path) source comes first; the catch-all `mise`
    /// follows after `mise_shims` / `mise_installs`.
    pub matched_sources: Vec<String>,
    /// Best-guess uninstall hint or a reason none could be produced.
    pub uninstall: UninstallHint,
}

#[derive(Debug, PartialEq, Eq)]
pub enum UninstallHint {
    /// A concrete shell command the user can run.
    Command(String),
    /// We matched a source but it has no `uninstall_command` template.
    NoTemplate { source: String },
    /// No source matched the resolved path at all.
    NoSource,
}

/// Compute the `where` answer for a command. The resolver is the
/// same closure used by `lint::evaluate`, so production code wires
/// this to the real PATH resolver.
pub fn locate<R>(
    command: &str,
    sources: &BTreeMap<String, SourceDef>,
    os: Os,
    mut resolver: R,
) -> WhereOutcome
where
    R: FnMut(&str) -> Option<Resolution>,
{
    let Some(resolution) = resolver(command) else {
        return WhereOutcome::NotFound;
    };

    let haystack = normalize(&resolution.full_path.to_string_lossy());
    let mut matched = matched_sources_ranked(&haystack, sources, os);
    let uninstall = derive_uninstall(&resolution.full_path, &matched, sources);

    // The ranking already has the most specific match first; we
    // also surface the catch-all `mise` last among the mise family.
    rank_mise_alias_last(&mut matched);

    WhereOutcome::Found(Found {
        command: command.to_string(),
        resolved: resolution.full_path,
        matched_sources: matched,
        uninstall,
    })
}

/// Like `lint::matched_sources` but ranked by needle length
/// descending — the longest, most specific path wins the lead spot.
fn matched_sources_ranked(
    haystack: &str,
    sources: &BTreeMap<String, SourceDef>,
    os: Os,
) -> Vec<String> {
    let mut hits: Vec<(usize, String)> = Vec::new();
    for (name, def) in sources {
        let Some(raw) = def.path_for(os) else {
            continue;
        };
        let needle = expand_and_normalize(raw);
        if needle.is_empty() {
            continue;
        }
        if haystack.contains(&needle) {
            hits.push((needle.len(), name.clone()));
        }
    }
    hits.sort_by_key(|h| std::cmp::Reverse(h.0));
    hits.into_iter().map(|(_, n)| n).collect()
}

fn rank_mise_alias_last(matched: &mut [String]) {
    // Move plain "mise" to the back of the matched list IF a more
    // specific mise_* source is also present. Keeps the lead source
    // useful (mise_shims is more informative than mise alone).
    let has_specific = matched
        .iter()
        .any(|s| s.starts_with("mise_") && s != "mise");
    if !has_specific {
        return;
    }
    if let Some(pos) = matched.iter().position(|s| s == "mise") {
        let removed = matched[pos].clone();
        // Shift elements after `pos` left by one, then write at
        // the end. We're using a slice so we can't push; the
        // caller already gave us a Vec via DerefMut above.
        for i in pos..matched.len() - 1 {
            matched[i] = matched[i + 1].clone();
        }
        let last = matched.len() - 1;
        matched[last] = removed;
    }
}

fn derive_uninstall(
    resolved: &std::path::Path,
    matched: &[String],
    sources: &BTreeMap<String, SourceDef>,
) -> UninstallHint {
    if matched.is_empty() {
        return UninstallHint::NoSource;
    }
    // Walk matched sources in order; the first one with an uninstall
    // template wins. That's why ranking by specificity matters.
    let bin = bin_stem(resolved);
    for name in matched {
        let Some(def) = sources.get(name) else {
            continue;
        };
        if let Some(template) = &def.uninstall_command {
            return UninstallHint::Command(template.replace("{bin}", &bin));
        }
    }
    UninstallHint::NoTemplate {
        source: matched[0].clone(),
    }
}

fn bin_stem(path: &std::path::Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_default()
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

    fn src_with_uninstall(unix: &str, template: &str) -> SourceDef {
        SourceDef {
            unix: Some(unix.into()),
            uninstall_command: Some(template.into()),
            ..Default::default()
        }
    }

    fn cat(entries: &[(&str, SourceDef)]) -> BTreeMap<String, SourceDef> {
        entries
            .iter()
            .map(|(n, d)| (n.to_string(), d.clone()))
            .collect()
    }

    fn resolution(p: &str) -> Resolution {
        Resolution {
            full_path: PathBuf::from(p),
        }
    }

    #[test]
    fn not_found_when_resolver_returns_none() {
        let out = locate("ghost", &BTreeMap::new(), Os::Linux, |_| None);
        assert_eq!(out, WhereOutcome::NotFound);
    }

    #[test]
    fn cargo_install_renders_cargo_uninstall_hint() {
        let sources = cat(&[(
            "cargo",
            src_with_uninstall("/home/u/.cargo/bin", "cargo uninstall {bin}"),
        )]);
        let out = locate("lazygit", &sources, Os::Linux, |_| {
            Some(resolution("/home/u/.cargo/bin/lazygit"))
        });
        match out {
            WhereOutcome::Found(f) => {
                assert_eq!(f.matched_sources, vec!["cargo".to_string()]);
                assert_eq!(
                    f.uninstall,
                    UninstallHint::Command("cargo uninstall lazygit".into())
                );
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn most_specific_source_wins_lead_spot() {
        // mise_installs path is a substring of mise; the more
        // specific source must come first in matched_sources and
        // be the one that supplies the uninstall hint.
        let sources = cat(&[
            ("mise", src("/home/u/.local/share/mise")),
            (
                "mise_installs",
                src_with_uninstall("/home/u/.local/share/mise/installs", "mise uninstall {bin}"),
            ),
        ]);
        let out = locate("lazygit", &sources, Os::Linux, |_| {
            Some(resolution(
                "/home/u/.local/share/mise/installs/cargo-lazygit/0.61/bin/lazygit",
            ))
        });
        match out {
            WhereOutcome::Found(f) => {
                assert_eq!(f.matched_sources[0], "mise_installs");
                // `mise` alias is at the back when a more specific
                // mise_* match is present.
                assert_eq!(f.matched_sources.last().unwrap(), "mise");
                assert_eq!(
                    f.uninstall,
                    UninstallHint::Command("mise uninstall lazygit".into())
                );
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn no_template_when_source_has_none() {
        // aqua is in the catalog but has no uninstall_command.
        let sources = cat(&[("aqua", src("/home/u/.local/share/aquaproj-aqua"))]);
        let out = locate("aqua_tool", &sources, Os::Linux, |_| {
            Some(resolution(
                "/home/u/.local/share/aquaproj-aqua/cache/foo/aqua_tool",
            ))
        });
        match out {
            WhereOutcome::Found(f) => {
                assert_eq!(
                    f.uninstall,
                    UninstallHint::NoTemplate {
                        source: "aqua".into()
                    }
                );
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn no_source_when_resolved_outside_catalog() {
        // The resolved path doesn't match any catalog entry — pathlint
        // can't speak to provenance.
        let sources = cat(&[("cargo", src("/home/u/.cargo/bin"))]);
        let out = locate("orphan", &sources, Os::Linux, |_| {
            Some(resolution("/opt/local-stuff/bin/orphan"))
        });
        match out {
            WhereOutcome::Found(f) => {
                assert!(f.matched_sources.is_empty());
                assert_eq!(f.uninstall, UninstallHint::NoSource);
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn windows_extension_is_stripped_from_bin_token() {
        // The {bin} substitution should use the file stem, not the
        // full filename. cargo uninstall lazygit, not
        // `cargo uninstall lazygit.exe`.
        let sources = cat(&[(
            "cargo",
            src_with_uninstall("/home/u/.cargo/bin", "cargo uninstall {bin}"),
        )]);
        let out = locate("lazygit", &sources, Os::Linux, |_| {
            Some(resolution("/home/u/.cargo/bin/lazygit.exe"))
        });
        match out {
            WhereOutcome::Found(f) => {
                assert_eq!(
                    f.uninstall,
                    UninstallHint::Command("cargo uninstall lazygit".into())
                );
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }
}
