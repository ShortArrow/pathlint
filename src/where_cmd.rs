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

use serde::Serialize;

use crate::config::SourceDef;
use crate::expand::{expand_and_normalize, normalize};
use crate::os_detect::Os;
use crate::resolve::Resolution;
use crate::source_match;

#[derive(Debug, PartialEq, Eq)]
pub enum WhereOutcome {
    /// The command resolved; here is everything we know about it.
    Found(Found),
    /// The command did not resolve from PATH at all.
    NotFound,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Found {
    pub command: String,
    pub resolved: PathBuf,
    /// Sources matched against the resolved path. The most specific
    /// (longest path) source comes first; the catch-all `mise`
    /// follows after `mise_shims` / `mise_installs`.
    pub matched_sources: Vec<String>,
    /// Best-guess uninstall hint or a reason none could be produced.
    pub uninstall: UninstallHint,
    /// Extra provenance hint inferred from path heuristics — e.g.
    /// "this lives under `mise/installs/cargo-foo/...` so it's a
    /// cargo install reached *through* mise". `None` when nothing
    /// more specific than `matched_sources` can be said.
    pub provenance: Option<Provenance>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UninstallHint {
    /// A concrete shell command the user can run.
    Command { command: String },
    /// We matched a source but it has no `uninstall_command` template.
    NoTemplate { source: String },
    /// No source matched the resolved path at all.
    NoSource,
}

/// Best-guess provenance derived from path-segment heuristics rather
/// than catalog `[source.<name>]` entries. Today only fires for
/// binaries served through mise's plugin system.
#[derive(Debug, PartialEq, Eq, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Provenance {
    /// A binary mise installed via a third-party installer plugin.
    /// `installer` is the upstream tool name (`cargo`, `npm`, ...);
    /// `plugin_segment` is the raw mise plugin segment so the user
    /// can verify with `mise plugins ls`.
    MiseInstallerPlugin {
        installer: &'static str,
        plugin_segment: String,
    },
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
    let matched = source_match::names_only(&haystack, sources, os);

    // Plugin-aware provenance gets a chance BEFORE the generic
    // catalog-based uninstall lookup, because for mise plugins we
    // can produce a sharper hint than the catalog can. This only
    // fires when the resolved path is under `mise_installs`.
    let provenance = if matched.iter().any(|s| s == "mise_installs") {
        infer_mise_plugin_provenance(&haystack, sources, os)
    } else {
        None
    };

    let uninstall = match &provenance {
        Some(prov) => uninstall_for_provenance(prov),
        None => derive_uninstall(&resolution.full_path, &matched, sources),
    };

    // The ranking already has the most specific match first; we
    // also surface the catch-all `mise` last among the mise family.
    let matched = rank_mise_alias_last(matched);

    WhereOutcome::Found(Found {
        command: command.to_string(),
        resolved: resolution.full_path,
        matched_sources: matched,
        uninstall,
        provenance,
    })
}

/// Push the catch-all `mise` source to the end of `matched` when a
/// more specific `mise_*` sibling is also present, otherwise return
/// the input unchanged. Pure: takes ownership and returns the new
/// vector. Stable: relative order of every other element is
/// preserved (we walk the input once and re-emit, deferring
/// `"mise"`).
fn rank_mise_alias_last(matched: Vec<String>) -> Vec<String> {
    let has_specific = matched
        .iter()
        .any(|s| s.starts_with("mise_") && s != "mise");
    if !has_specific {
        return matched;
    }
    let (mise_alias, others): (Vec<String>, Vec<String>) =
        matched.into_iter().partition(|s| s == "mise");
    others.into_iter().chain(mise_alias).collect()
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
            return UninstallHint::Command {
                command: template.replace("{bin}", &bin),
            };
        }
    }
    UninstallHint::NoTemplate {
        source: matched[0].clone(),
    }
}

/// Look at a normalized path that contains the mise_installs prefix
/// and try to read the next segment as a "<installer>-<rest>"
/// plugin name. Returns `None` for runtime layouts like
/// `installs/python/3.14/bin/python` where the segment is just the
/// language name.
fn infer_mise_plugin_provenance(
    normalized_haystack: &str,
    sources: &BTreeMap<String, SourceDef>,
    os: Os,
) -> Option<Provenance> {
    // We need the actual `mise_installs` per-OS path to know where
    // the plugin segment starts. If the user removed it from the
    // catalog, just bail.
    let installs_def = sources.get("mise_installs")?;
    let installs_raw = installs_def.path_for(os)?;
    let needle = expand_and_normalize(installs_raw);
    let after = normalized_haystack
        .find(&needle)
        .map(|i| &normalized_haystack[i + needle.len()..])?;
    // After the `mise/installs` substring we expect a slash then
    // the segment, then another slash. Strip the leading slash.
    let after = after.strip_prefix('/')?;
    let segment = after.split('/').next()?;
    classify_mise_segment(segment)
}

const MISE_PLUGIN_PREFIXES: &[(&str, &str)] = &[
    ("cargo-", "cargo"),
    ("npm-", "npm"),
    ("pipx-", "pipx"),
    ("go-", "go"),
    ("aqua-", "aqua"),
];

fn classify_mise_segment(segment: &str) -> Option<Provenance> {
    for (prefix, installer) in MISE_PLUGIN_PREFIXES {
        if let Some(rest) = segment.strip_prefix(prefix) {
            if !rest.is_empty() {
                return Some(Provenance::MiseInstallerPlugin {
                    installer,
                    plugin_segment: segment.to_string(),
                });
            }
        }
    }
    None
}

fn uninstall_for_provenance(prov: &Provenance) -> UninstallHint {
    match prov {
        Provenance::MiseInstallerPlugin {
            installer,
            plugin_segment,
        } => {
            // Strip the `<installer>-` prefix to recover the upstream
            // plugin id. We can't reliably split it further (e.g. the
            // mise convention `cargo:owner/repo` vs the segment
            // `cargo-owner-repo` is lossy), so we hand the user a
            // best-guess command and tell them to verify.
            let rest = plugin_segment
                .strip_prefix(&format!("{installer}-"))
                .unwrap_or(plugin_segment);
            UninstallHint::Command {
                command: format!(
                    "mise uninstall {installer}:{rest}  (best-guess; verify with `mise plugins ls`)"
                ),
            }
        }
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
                    UninstallHint::Command {
                        command: "cargo uninstall lazygit".into()
                    }
                );
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn most_specific_source_wins_lead_spot() {
        // mise_installs path is a substring of mise; the more
        // specific source must come first in matched_sources.
        // The path uses a `cargo-` plugin segment, so 0.0.5+
        // plugin attribution kicks in and supplies the uninstall
        // hint instead of the generic `mise_installs` template.
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
                // Plugin attribution: the cargo- prefix wins over
                // the generic mise_installs uninstall_command.
                match &f.uninstall {
                    UninstallHint::Command { command } => {
                        assert!(
                            command.contains("mise uninstall cargo:lazygit"),
                            "uninstall: {command}"
                        );
                    }
                    other => panic!("expected Command, got {other:?}"),
                }
                assert!(matches!(
                    f.provenance,
                    Some(Provenance::MiseInstallerPlugin {
                        installer: "cargo",
                        ..
                    })
                ));
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
                    UninstallHint::Command {
                        command: "cargo uninstall lazygit".into()
                    }
                );
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    // ---- Plugin attribution (R4 / 0.0.5+) ---------------------

    fn mise_sources() -> BTreeMap<String, SourceDef> {
        cat(&[
            ("mise", src("/home/u/.local/share/mise")),
            ("mise_installs", src("/home/u/.local/share/mise/installs")),
        ])
    }

    #[test]
    fn npm_plugin_segment_yields_npm_provenance() {
        let out = locate("gemini", &mise_sources(), Os::Linux, |_| {
            Some(resolution(
                "/home/u/.local/share/mise/installs/npm-google-gemini-cli/0.40.0/gemini",
            ))
        });
        match out {
            WhereOutcome::Found(f) => {
                assert!(matches!(
                    &f.provenance,
                    Some(Provenance::MiseInstallerPlugin {
                        installer: "npm",
                        plugin_segment,
                    }) if plugin_segment == "npm-google-gemini-cli"
                ));
                match &f.uninstall {
                    UninstallHint::Command { command } => {
                        assert!(command.starts_with("mise uninstall npm:google-gemini-cli"));
                    }
                    other => panic!("expected Command, got {other:?}"),
                }
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn runtime_segment_does_not_get_plugin_provenance() {
        // `python` is a mise runtime, not a plugin install. No
        // installer prefix to detect, so provenance stays None.
        let out = locate("python", &mise_sources(), Os::Linux, |_| {
            Some(resolution(
                "/home/u/.local/share/mise/installs/python/3.14/bin/python",
            ))
        });
        match out {
            WhereOutcome::Found(f) => {
                assert!(f.provenance.is_none());
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn unknown_plugin_prefix_does_not_attribute() {
        // A plugin installed via a prefix we don't recognize stays
        // unattributed — pathlint shouldn't guess.
        let out = locate("xyz", &mise_sources(), Os::Linux, |_| {
            Some(resolution(
                "/home/u/.local/share/mise/installs/exotic-thing/0.1/bin/xyz",
            ))
        });
        match out {
            WhereOutcome::Found(f) => {
                assert!(f.provenance.is_none());
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn provenance_only_fires_for_mise_installs_paths() {
        // A plain cargo install (no mise involved) must not falsely
        // pick up MiseInstallerPlugin provenance even though the
        // binary stem starts with `cargo-`. The trigger is the
        // mise_installs path match, not the bin name.
        let sources = cat(&[(
            "cargo",
            src_with_uninstall("/home/u/.cargo/bin", "cargo uninstall {bin}"),
        )]);
        let out = locate("cargo-lazygit", &sources, Os::Linux, |_| {
            Some(resolution("/home/u/.cargo/bin/cargo-lazygit"))
        });
        match out {
            WhereOutcome::Found(f) => {
                assert!(f.provenance.is_none());
                assert_eq!(
                    f.uninstall,
                    UninstallHint::Command {
                        command: "cargo uninstall cargo-lazygit".into()
                    }
                );
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }
}
