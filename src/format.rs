//! Presentation layer: pure formatters that turn domain values
//! (`Outcome` / `Diagnostic` / `Found`) into strings.
//!
//! Nothing in here writes to stdout. Callers (in `run.rs`) print
//! the returned string. Keeping the formatters pure makes them
//! unit-testable without an `assert_cmd`-style integration harness.

use crate::doctor::{Diagnostic, Kind, Severity};
use crate::where_cmd::{Found, Provenance, UninstallHint, WhereOutcome};

/// Render a single doctor diagnostic into a multi-line block
/// (header line + indented detail). The trailing newline is
/// omitted; the caller decides whether to add one.
pub fn doctor_line(d: &Diagnostic, entries: &[String]) -> String {
    if let Kind::MiseActivateBoth {
        shim_indices,
        install_indices,
    } = &d.kind
    {
        return doctor_mise_activate_both(entries, shim_indices, install_indices);
    }

    let tag = match d.severity {
        Severity::Error => "[ERR] ",
        Severity::Warn => "[warn]",
    };
    let detail = match &d.kind {
        Kind::Duplicate { first_index } => format!(
            "duplicate of entry #{first} ({first_path})",
            first = first_index,
            first_path = entries.get(*first_index).cloned().unwrap_or_default(),
        ),
        Kind::Missing => "directory does not exist".into(),
        Kind::Shortenable { suggestion } => format!("could be written as {suggestion}"),
        Kind::TrailingSlash => "trailing slash; some shells handle this oddly".into(),
        Kind::CaseVariant { canonical } => {
            format!("case / slash variant of {canonical}; OS treats them as one directory")
        }
        Kind::ShortName => "Windows 8.3 short name in PATH; long-name form is more portable".into(),
        Kind::Malformed { reason } => format!("malformed entry: {reason}"),
        Kind::MiseActivateBoth { .. } => unreachable!("handled by early return above"),
    };
    format!(
        "{tag} #{idx:>3} {entry}\n      {detail}",
        idx = d.index,
        entry = d.entry
    )
}

/// MiseActivateBoth gets its own multi-line layout that
/// enumerates every shim and install entry below the header.
fn doctor_mise_activate_both(
    entries: &[String],
    shim_indices: &[usize],
    install_indices: &[usize],
) -> String {
    let mut buf = String::from(
        "[warn] mise activate exposes both shim and install layers (PATH order matters)\n",
    );
    buf.push_str("      shims:\n");
    for &i in shim_indices {
        let entry = entries.get(i).cloned().unwrap_or_default();
        buf.push_str(&format!("        #{i:>3} {entry}\n"));
    }
    buf.push_str("      installs:\n");
    for &i in install_indices {
        let entry = entries.get(i).cloned().unwrap_or_default();
        buf.push_str(&format!("        #{i:>3} {entry}\n"));
    }
    buf.pop(); // strip the trailing newline so the caller adds its own
    buf
}

/// Render a `Found` outcome from `pathlint where` as a multi-line
/// human block. Order: command header, resolved path, sources,
/// optional provenance, uninstall hint. No trailing newline.
pub fn where_human(found: &Found) -> String {
    let mut buf = String::new();
    buf.push_str(&found.command);
    buf.push('\n');
    buf.push_str(&format!("  resolved: {}\n", found.resolved.display()));
    if found.matched_sources.is_empty() {
        buf.push_str("  sources:  (no source matched)\n");
    } else {
        buf.push_str(&format!(
            "  sources:  {}\n",
            found.matched_sources.join(", ")
        ));
    }
    if let Some(Provenance::MiseInstallerPlugin {
        installer,
        plugin_segment,
    }) = &found.provenance
    {
        buf.push_str(&format!(
            "  provenance: {installer} (via mise plugin `{plugin_segment}`)\n"
        ));
    }
    match &found.uninstall {
        UninstallHint::Command { command } => {
            buf.push_str(&format!("  hint:     {command}"));
        }
        UninstallHint::NoTemplate { source } => {
            buf.push_str(&format!(
                "  hint:     (no uninstall template for source `{source}`)"
            ));
        }
        UninstallHint::NoSource => {
            buf.push_str("  hint:     (no source matched — pathlint cannot guess)");
        }
    }
    buf
}

/// Render a NotFound `where` outcome — single line, no trailing
/// newline.
pub fn where_not_found(command: &str) -> String {
    format!("{command} — not found on PATH")
}

/// Convenience: render a complete `WhereOutcome` to a single
/// (multi-line) string suitable for `print!`. The caller still
/// chooses what exit code to use.
pub fn where_outcome(outcome: &WhereOutcome) -> String {
    match outcome {
        WhereOutcome::Found(f) => where_human(f),
        WhereOutcome::NotFound => {
            // We don't have the command name here from NotFound
            // alone; callers that need the original spelling reach
            // for `where_not_found` directly. For symmetry we
            // return an empty string so this branch is detectable.
            String::new()
        }
    }
}

/// Render the `where` outcome as pretty-printed JSON. The schema
/// is documented in PRD §7.7 and stable for `0.0.x`. Used by
/// `pathlint where --json`.
///
/// `command` is needed because `WhereOutcome::NotFound` carries no
/// data — the JSON shape `{"command":"...", "found":false}` keeps
/// the discriminator field consistent with the Found variant.
pub fn where_json(command: &str, outcome: &WhereOutcome) -> Result<String, serde_json::Error> {
    #[derive(serde::Serialize)]
    #[serde(untagged)]
    enum Out<'a> {
        NotFound {
            command: &'a str,
            found: bool,
        },
        Found {
            found: bool,
            #[serde(flatten)]
            inner: &'a Found,
        },
    }

    let payload = match outcome {
        WhereOutcome::NotFound => Out::NotFound {
            command,
            found: false,
        },
        WhereOutcome::Found(f) => Out::Found {
            found: true,
            inner: f,
        },
    };
    serde_json::to_string_pretty(&payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn entries(strs: &[&str]) -> Vec<String> {
        strs.iter().map(|s| s.to_string()).collect()
    }

    fn found_minimal() -> Found {
        Found {
            command: "rustc".into(),
            resolved: PathBuf::from("/home/u/.cargo/bin/rustc"),
            matched_sources: vec!["cargo".into()],
            uninstall: UninstallHint::Command {
                command: "cargo uninstall rustc".into(),
            },
            provenance: None,
        }
    }

    #[test]
    fn warn_diagnostic_has_warn_tag_and_indented_detail() {
        let d = Diagnostic {
            index: 3,
            entry: "/usr/bin".into(),
            severity: Severity::Warn,
            kind: Kind::Missing,
        };
        let out = doctor_line(&d, &entries(&[]));
        assert!(out.starts_with("[warn]"));
        assert!(out.contains("#  3 /usr/bin"));
        assert!(out.contains("      directory does not exist"));
    }

    #[test]
    fn error_diagnostic_uses_err_tag() {
        let d = Diagnostic {
            index: 0,
            entry: "C:\\foo|bar".into(),
            severity: Severity::Error,
            kind: Kind::Malformed {
                reason: "illegal character '|' in path".into(),
            },
        };
        let out = doctor_line(&d, &entries(&[]));
        assert!(out.starts_with("[ERR]"));
        assert!(out.contains("malformed entry: illegal character"));
    }

    #[test]
    fn duplicate_renders_first_index_with_back_reference() {
        let entries = entries(&["/usr/bin", "/foo/bar", "/usr/bin"]);
        let d = Diagnostic {
            index: 2,
            entry: "/usr/bin".into(),
            severity: Severity::Warn,
            kind: Kind::Duplicate { first_index: 0 },
        };
        let out = doctor_line(&d, &entries);
        assert!(
            out.contains("duplicate of entry #0 (/usr/bin)"),
            "out: {out}"
        );
    }

    #[test]
    fn shortenable_renders_suggestion_string() {
        let d = Diagnostic {
            index: 5,
            entry: "C:\\Users\\who\\.cargo\\bin".into(),
            severity: Severity::Warn,
            kind: Kind::Shortenable {
                suggestion: "%UserProfile%\\.cargo\\bin".into(),
            },
        };
        let out = doctor_line(&d, &entries(&[]));
        assert!(out.contains("could be written as %UserProfile%\\.cargo\\bin"));
    }

    #[test]
    fn where_human_minimal_has_command_resolved_sources_hint_in_order() {
        let out = where_human(&found_minimal());
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "rustc");
        assert!(lines[1].starts_with("  resolved: "));
        assert!(lines[2].starts_with("  sources:  cargo"));
        assert_eq!(lines[3], "  hint:     cargo uninstall rustc");
        // No trailing newline (caller adds one when printing).
        assert!(!out.ends_with('\n'));
    }

    #[test]
    fn where_human_includes_provenance_line_when_set() {
        let mut f = found_minimal();
        f.matched_sources = vec!["mise_installs".into(), "mise".into()];
        f.provenance = Some(Provenance::MiseInstallerPlugin {
            installer: "cargo",
            plugin_segment: "cargo-foo".into(),
        });
        f.uninstall = UninstallHint::Command {
            command: "mise uninstall cargo:foo".into(),
        };
        let out = where_human(&f);
        assert!(out.contains("provenance: cargo (via mise plugin `cargo-foo`)"));
    }

    #[test]
    fn where_human_uninstall_no_template_names_the_source() {
        let mut f = found_minimal();
        f.uninstall = UninstallHint::NoTemplate {
            source: "aqua".into(),
        };
        let out = where_human(&f);
        assert!(
            out.contains("(no uninstall template for source `aqua`)"),
            "out: {out}"
        );
    }

    #[test]
    fn where_human_uninstall_no_source_says_pathlint_cannot_guess() {
        let mut f = found_minimal();
        f.matched_sources = Vec::new();
        f.uninstall = UninstallHint::NoSource;
        let out = where_human(&f);
        assert!(out.contains("sources:  (no source matched)"), "out: {out}");
        assert!(
            out.contains("(no source matched — pathlint cannot guess)"),
            "out: {out}"
        );
    }

    #[test]
    fn where_not_found_is_single_line_with_em_dash() {
        let out = where_not_found("ghost");
        assert_eq!(out, "ghost — not found on PATH");
        assert!(!out.ends_with('\n'));
    }

    #[test]
    fn where_json_found_carries_kind_discriminators() {
        let out = where_json("rustc", &WhereOutcome::Found(found_minimal())).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["found"], true);
        assert_eq!(v["command"], "rustc");
        assert_eq!(v["uninstall"]["kind"], "command");
        assert_eq!(v["uninstall"]["command"], "cargo uninstall rustc");
        assert!(v["provenance"].is_null());
    }

    #[test]
    fn where_json_not_found_is_compact() {
        let out = where_json("ghost", &WhereOutcome::NotFound).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["found"], false);
        assert_eq!(v["command"], "ghost");
        assert!(v.get("resolved").is_none());
    }

    #[test]
    fn where_json_provenance_emits_kind_and_segment() {
        let mut f = found_minimal();
        f.provenance = Some(Provenance::MiseInstallerPlugin {
            installer: "cargo",
            plugin_segment: "cargo-foo".into(),
        });
        let out = where_json("foo", &WhereOutcome::Found(f)).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["provenance"]["kind"], "mise_installer_plugin");
        assert_eq!(v["provenance"]["installer"], "cargo");
        assert_eq!(v["provenance"]["plugin_segment"], "cargo-foo");
    }

    #[test]
    fn mise_activate_both_lists_each_layer_separately() {
        let entries = entries(&[
            "/home/u/.local/share/mise/shims",
            "/home/u/.local/share/mise/installs/python/3.14/bin",
            "/home/u/.local/share/mise/installs/node/25.9.0/bin",
        ]);
        let d = Diagnostic {
            index: 0,
            entry: entries[0].clone(),
            severity: Severity::Warn,
            kind: Kind::MiseActivateBoth {
                shim_indices: vec![0],
                install_indices: vec![1, 2],
            },
        };
        let out = doctor_line(&d, &entries);
        assert!(out.starts_with(
            "[warn] mise activate exposes both shim and install layers (PATH order matters)"
        ));
        // shim entry appears under the shims: header
        assert!(out.contains("shims:\n        #  0 /home/u/.local/share/mise/shims"));
        // both install entries appear under installs:
        assert!(out.contains("installs:\n        #  1"));
        assert!(out.contains("\n        #  2 /home/u/.local/share/mise/installs/node/25.9.0/bin"));
        // No trailing newline (caller adds its own)
        assert!(!out.ends_with('\n'));
    }
}
