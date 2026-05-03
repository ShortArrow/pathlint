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

use crate::config::{Relation, SourceDef};
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

/// Best-guess provenance derived from `[[relation]]` declarations
/// rather than catalog `[source.<name>]` entries. Today only fires
/// for binaries served through wrapper installers such as mise's
/// plugin system. Computed from `Relation::ServedByVia` (matching
/// the host source path + a `prefix-*` glob on the next path
/// segment) using the `installer_token` as the human-facing
/// installer name.
#[derive(Debug, PartialEq, Eq, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Provenance {
    /// A binary served via a wrapper installer's plugin layer.
    /// `installer` is the upstream tool name (`cargo`, `npm`, ...)
    /// taken from the matched relation's `installer_token` (or
    /// `guest_provider` if absent); `plugin_segment` is the raw
    /// path segment so the user can verify with the installer's
    /// own tooling.
    MiseInstallerPlugin {
        installer: String,
        plugin_segment: String,
    },
}

/// Compute the `where` answer for a command. The resolver is the
/// same closure used by `lint::evaluate`, so production code wires
/// this to the real PATH resolver.
pub fn locate<R>(
    command: &str,
    sources: &BTreeMap<String, SourceDef>,
    relations: &[Relation],
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

    // Provenance comes from `[[relation]] kind = "served_by_via"`:
    // when the resolved path lives under a relation's `host` source
    // and the next segment matches `guest_pattern`, attribute it.
    let provenance = infer_provenance_from_relations(&haystack, sources, relations, os);

    let uninstall = match &provenance {
        Some(prov) => uninstall_for_provenance(prov, os),
        None => derive_uninstall(&resolution.full_path, &matched, sources, os),
    };

    // Push every alias_of parent to the end when a child is also
    // matched — the catch-all is least informative.
    let matched = rank_aliases_last(matched, relations);

    WhereOutcome::Found(Found {
        command: command.to_string(),
        resolved: resolution.full_path,
        matched_sources: matched,
        uninstall,
        provenance,
    })
}

/// For every `[[relation]] kind = "alias_of"`, push the parent to
/// the end of `matched` when at least one declared child is also
/// present. Pure: stable ordering for non-parent entries. The
/// previous mise-only special case becomes one application of this
/// generic rule.
fn rank_aliases_last(matched: Vec<String>, relations: &[Relation]) -> Vec<String> {
    let parents: Vec<&str> = relations
        .iter()
        .filter_map(|r| match r {
            Relation::AliasOf { parent, children } => {
                let any_child_matched = children.iter().any(|c| matched.contains(c));
                if any_child_matched {
                    Some(parent.as_str())
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect();
    if parents.is_empty() {
        return matched;
    }
    let (deferred, others): (Vec<String>, Vec<String>) = matched
        .into_iter()
        .partition(|m| parents.contains(&m.as_str()));
    others.into_iter().chain(deferred).collect()
}

fn derive_uninstall(
    resolved: &std::path::Path,
    matched: &[String],
    sources: &BTreeMap<String, SourceDef>,
    os: Os,
) -> UninstallHint {
    if matched.is_empty() {
        return UninstallHint::NoSource;
    }
    // Walk matched sources in order; the first one with an uninstall
    // template wins. That's why ranking by specificity matters.
    //
    // The `{bin}` substitution goes through `format::quote_for` so
    // hostile bin names like `$(rm -rf ~)` cannot escape the
    // template even when the user copy-pastes the result. The
    // surrounding template text (the part the catalog author
    // controls) is not escaped — that is the "trust the catalog
    // author" boundary documented in PRD §12.
    let bin = bin_stem(resolved);
    let quoted_bin = crate::format::quote_for(os, &bin);
    for name in matched {
        let Some(def) = sources.get(name) else {
            continue;
        };
        if let Some(template) = &def.uninstall_command {
            return UninstallHint::Command {
                command: template.replace("{bin}", &quoted_bin),
            };
        }
    }
    UninstallHint::NoTemplate {
        source: matched[0].clone(),
    }
}

/// Walk every `Relation::ServedByVia` and try to attribute the
/// resolved path to the relation's installer. The path must
/// (a) live under the relation's `host` source's per-OS path, and
/// (b) the segment immediately after that prefix must match the
/// relation's `guest_pattern` (today only `prefix-*` glob is
/// supported, which is what every built-in uses).
///
/// Returns the first matching relation's provenance. The relation
/// list comes from the merged catalog so a user override can both
/// add new wrappers and shadow built-ins by ordering.
fn infer_provenance_from_relations(
    normalized_haystack: &str,
    sources: &BTreeMap<String, SourceDef>,
    relations: &[Relation],
    os: Os,
) -> Option<Provenance> {
    for rel in relations {
        let Relation::ServedByVia {
            host,
            guest_pattern,
            guest_provider,
            installer_token,
        } = rel
        else {
            continue;
        };
        let Some(host_def) = sources.get(host) else {
            continue;
        };
        let Some(host_raw) = host_def.path_for(os) else {
            continue;
        };
        let host_needle = expand_and_normalize(host_raw);
        if host_needle.is_empty() {
            continue;
        }
        let Some(after) = find_after_needle(normalized_haystack, &host_needle) else {
            continue;
        };
        let after = after.strip_prefix('/').unwrap_or(after);
        let Some(segment) = after.split('/').next() else {
            continue;
        };
        if let Some(_rest) = match_glob_prefix(guest_pattern, segment) {
            let installer = installer_token
                .clone()
                .unwrap_or_else(|| guest_provider.clone());
            return Some(Provenance::MiseInstallerPlugin {
                installer,
                plugin_segment: segment.to_string(),
            });
        }
    }
    None
}

/// Return the haystack slice starting just past the first occurrence
/// of `needle`, or `None` when the needle is absent.
fn find_after_needle<'h>(haystack: &'h str, needle: &str) -> Option<&'h str> {
    haystack.find(needle).map(|i| &haystack[i + needle.len()..])
}

/// Tiny glob matcher that handles `prefix-*` patterns — the only
/// shape every built-in `served_by_via` uses today. Returns the
/// captured suffix (`*` part) so callers can echo it back. `None`
/// when the segment does not match or the captured suffix is empty
/// (a bare `prefix-` is not a real plugin name).
fn match_glob_prefix<'a>(pattern: &str, segment: &'a str) -> Option<&'a str> {
    let prefix = pattern.strip_suffix('*')?;
    let rest = segment.strip_prefix(prefix)?;
    if rest.is_empty() { None } else { Some(rest) }
}

fn uninstall_for_provenance(prov: &Provenance, os: Os) -> UninstallHint {
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
            //
            // `installer` comes from the in-tree relation table
            // (trusted) so it is interpolated as-is. `rest` is
            // attacker-controlled (it's a path segment) and must go
            // through quote_for.
            let rest = plugin_segment
                .strip_prefix(&format!("{installer}-"))
                .unwrap_or(plugin_segment);
            let quoted_rest = crate::format::quote_for(os, rest);
            UninstallHint::Command {
                command: format!(
                    "mise uninstall {installer}:{quoted_rest}  (best-guess; verify with `mise plugins ls`)"
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

    /// Built-in mise relations re-stated for unit tests so each
    /// case is self-contained. Production wiring uses
    /// `catalog::merge_with_user_relations(&cfg.relations)`.
    fn mise_relations() -> Vec<Relation> {
        vec![
            Relation::AliasOf {
                parent: "mise".into(),
                children: vec!["mise_shims".into(), "mise_installs".into()],
            },
            Relation::ServedByVia {
                host: "mise_installs".into(),
                guest_pattern: "cargo-*".into(),
                guest_provider: "cargo".into(),
                installer_token: Some("cargo".into()),
            },
            Relation::ServedByVia {
                host: "mise_installs".into(),
                guest_pattern: "npm-*".into(),
                guest_provider: "npm_global".into(),
                installer_token: Some("npm".into()),
            },
            Relation::ServedByVia {
                host: "mise_installs".into(),
                guest_pattern: "pipx-*".into(),
                guest_provider: "pip_user".into(),
                installer_token: Some("pipx".into()),
            },
        ]
    }

    #[test]
    fn not_found_when_resolver_returns_none() {
        let out = locate("ghost", &BTreeMap::new(), &[], Os::Linux, |_| None);
        assert_eq!(out, WhereOutcome::NotFound);
    }

    #[test]
    fn cargo_install_renders_cargo_uninstall_hint() {
        let sources = cat(&[(
            "cargo",
            src_with_uninstall("/home/u/.cargo/bin", "cargo uninstall {bin}"),
        )]);
        let out = locate("lazygit", &sources, &[], Os::Linux, |_| {
            Some(resolution("/home/u/.cargo/bin/lazygit"))
        });
        match out {
            WhereOutcome::Found(f) => {
                assert_eq!(f.matched_sources, vec!["cargo".to_string()]);
                assert_eq!(
                    f.uninstall,
                    UninstallHint::Command {
                        command: "cargo uninstall 'lazygit'".into()
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
        let out = locate("lazygit", &sources, &mise_relations(), Os::Linux, |_| {
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
                            command.contains("mise uninstall cargo:'lazygit'"),
                            "uninstall: {command}"
                        );
                    }
                    other => panic!("expected Command, got {other:?}"),
                }
                match &f.provenance {
                    Some(Provenance::MiseInstallerPlugin { installer, .. }) => {
                        assert_eq!(installer, "cargo");
                    }
                    other => panic!("expected MiseInstallerPlugin, got {other:?}"),
                }
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn no_template_when_source_has_none() {
        // aqua is in the catalog but has no uninstall_command.
        let sources = cat(&[("aqua", src("/home/u/.local/share/aquaproj-aqua"))]);
        let out = locate("aqua_tool", &sources, &[], Os::Linux, |_| {
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
        let out = locate("orphan", &sources, &[], Os::Linux, |_| {
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
        let out = locate("lazygit", &sources, &[], Os::Linux, |_| {
            Some(resolution("/home/u/.cargo/bin/lazygit.exe"))
        });
        match out {
            WhereOutcome::Found(f) => {
                assert_eq!(
                    f.uninstall,
                    UninstallHint::Command {
                        command: "cargo uninstall 'lazygit'".into()
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
        let out = locate(
            "gemini",
            &mise_sources(),
            &mise_relations(),
            Os::Linux,
            |_| {
                Some(resolution(
                    "/home/u/.local/share/mise/installs/npm-google-gemini-cli/0.40.0/gemini",
                ))
            },
        );
        match out {
            WhereOutcome::Found(f) => {
                match &f.provenance {
                    Some(Provenance::MiseInstallerPlugin {
                        installer,
                        plugin_segment,
                    }) => {
                        assert_eq!(installer, "npm");
                        assert_eq!(plugin_segment, "npm-google-gemini-cli");
                    }
                    other => panic!("expected MiseInstallerPlugin, got {other:?}"),
                }
                match &f.uninstall {
                    UninstallHint::Command { command } => {
                        assert!(
                            command.starts_with("mise uninstall npm:'google-gemini-cli'"),
                            "uninstall: {command}"
                        );
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
        let out = locate(
            "python",
            &mise_sources(),
            &mise_relations(),
            Os::Linux,
            |_| {
                Some(resolution(
                    "/home/u/.local/share/mise/installs/python/3.14/bin/python",
                ))
            },
        );
        match out {
            WhereOutcome::Found(f) => {
                assert!(f.provenance.is_none());
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn pipx_plugin_uses_installer_token_distinct_from_guest_provider() {
        // 0.0.10: served_by_via.installer_token decouples the source
        // name (pip_user, what `pathlint catalog list` shows) from
        // the human-facing installer (`pipx`, what mise's CLI uses).
        let out = locate(
            "black",
            &mise_sources(),
            &mise_relations(),
            Os::Linux,
            |_| {
                Some(resolution(
                    "/home/u/.local/share/mise/installs/pipx-black/24.0/bin/black",
                ))
            },
        );
        match out {
            WhereOutcome::Found(f) => match &f.provenance {
                Some(Provenance::MiseInstallerPlugin {
                    installer,
                    plugin_segment,
                }) => {
                    assert_eq!(
                        installer, "pipx",
                        "installer_token must override guest_provider"
                    );
                    assert_eq!(plugin_segment, "pipx-black");
                }
                other => panic!("expected MiseInstallerPlugin, got {other:?}"),
            },
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn unknown_plugin_prefix_does_not_attribute() {
        // A plugin installed via a prefix we don't recognize stays
        // unattributed — pathlint shouldn't guess.
        let out = locate("xyz", &mise_sources(), &mise_relations(), Os::Linux, |_| {
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
        let out = locate(
            "cargo-lazygit",
            &sources,
            &mise_relations(),
            Os::Linux,
            |_| Some(resolution("/home/u/.cargo/bin/cargo-lazygit")),
        );
        match out {
            WhereOutcome::Found(f) => {
                assert!(f.provenance.is_none());
                assert_eq!(
                    f.uninstall,
                    UninstallHint::Command {
                        command: "cargo uninstall 'cargo-lazygit'".into()
                    }
                );
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }
}
