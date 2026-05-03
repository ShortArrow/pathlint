//! Presentation layer: pure formatters that turn domain values
//! (`Outcome` / `Diagnostic` / `Found`) into strings.
//!
//! Nothing in here writes to stdout. Callers (in `run.rs`) print
//! the returned string. Keeping the formatters pure makes them
//! unit-testable without an `assert_cmd`-style integration harness.

use crate::config::Relation;
use crate::doctor::{Diagnostic, Kind, Severity};
use crate::lint::{self, Diagnosis, Outcome, Status};
use crate::sort::{SortNote, SortPlan};
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

/// Render a `SortPlan` as a multi-line human-readable block. The
/// layout shows the *original* PATH on the left numbered column,
/// the *proposed* PATH on the right column, and a list of
/// "moved entries" with the reason each one was promoted. Notes
/// (e.g. unsatisfiable `prefer`) follow as fyi lines.
///
/// No trailing newline; callers add their own. Pure.
pub fn sort_human(plan: &SortPlan) -> String {
    let mut buf = String::new();
    if plan.is_noop() {
        buf.push_str("pathlint sort: PATH is already in a satisfying order.\n");
        for note in &plan.notes {
            push_sort_note(&mut buf, note);
        }
        // strip trailing newline so behaviour matches the other
        // human formatters.
        if buf.ends_with('\n') {
            buf.pop();
        }
        return buf;
    }

    buf.push_str("pathlint sort: proposed PATH order (--dry-run; not applied).\n\n");
    let width = plan.original.len().max(plan.sorted.len()).to_string().len();
    let original_header = "before".to_string();
    let sorted_header = "after".to_string();
    let col_w = plan
        .original
        .iter()
        .chain(plan.sorted.iter())
        .map(|s| s.len())
        .max()
        .unwrap_or(0)
        .max(original_header.len());

    buf.push_str(&format!(
        "  {:>w$}  {:<col$}    {:>w$}  {:<col$}\n",
        "#",
        original_header,
        "#",
        sorted_header,
        w = width,
        col = col_w,
    ));
    let rows = plan.original.len().max(plan.sorted.len());
    for i in 0..rows {
        let lhs = plan.original.get(i).cloned().unwrap_or_default();
        let rhs = plan.sorted.get(i).cloned().unwrap_or_default();
        buf.push_str(&format!(
            "  {:>w$}  {:<col$}    {:>w$}  {:<col$}\n",
            i,
            lhs,
            i,
            rhs,
            w = width,
            col = col_w,
        ));
    }

    if !plan.moves.is_empty() {
        buf.push_str("\nmoved:\n");
        for m in &plan.moves {
            buf.push_str(&format!(
                "  #{from} -> #{to}: {entry}\n      reason: {reason}\n",
                from = m.from,
                to = m.to,
                entry = m.entry,
                reason = m.reason,
            ));
        }
    }

    for note in &plan.notes {
        push_sort_note(&mut buf, note);
    }

    if buf.ends_with('\n') {
        buf.pop();
    }
    buf
}

fn push_sort_note(buf: &mut String, note: &SortNote) {
    match note {
        SortNote::UnsatisfiablePrefer { command, prefer } => {
            buf.push_str(&format!(
                "\nnote: `{command}` cannot be satisfied by reordering — no PATH entry matches `prefer = [{}]`. Install via one of those sources, or relax the rule.\n",
                prefer.join(", "),
            ));
        }
    }
}

/// Render a `SortPlan` as pretty-printed JSON. Schema is the
/// `SortPlan` value verbatim; `notes` carry a `kind` discriminator
/// so machine consumers can pattern-match. Stable through `0.0.x`.
pub fn sort_json(plan: &SortPlan) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(plan)
}

/// Render doctor diagnostics as a pretty-printed JSON array — the
/// machine-readable counterpart of `pathlint doctor`'s human view.
/// Each element carries `index`, `entry`, `severity`, the
/// discriminator `kind`, and any per-variant payload fields
/// (e.g. `suggestion` for shortenable, `canonical` for
/// case_variant, `shim_indices` / `install_indices` for
/// mise_activate_both).
///
/// The schema parallels `check --json`: top-level array, every
/// failure carries enough structured detail that CI consumers
/// don't need to parse human strings. Stable through `0.0.x`.
/// Render the relation list as a human-readable block. One relation
/// per stanza, grouped only by declaration order. No trailing
/// newline.
pub fn relations_human(relations: &[Relation]) -> String {
    if relations.is_empty() {
        return "no relations declared".to_string();
    }
    let mut buf = String::new();
    for (i, rel) in relations.iter().enumerate() {
        if i > 0 {
            buf.push('\n');
        }
        match rel {
            Relation::AliasOf { parent, children } => {
                buf.push_str(&format!("alias_of: `{parent}` → [{}]", children.join(", ")));
            }
            Relation::ConflictsWhenBothInPath {
                sources,
                diagnostic,
            } => {
                buf.push_str(&format!(
                    "conflicts_when_both_in_path: [{}] (diagnostic: `{diagnostic}`)",
                    sources.join(", "),
                ));
            }
            Relation::ServedByVia {
                host,
                guest_pattern,
                guest_provider,
            } => {
                buf.push_str(&format!(
                    "served_by_via: `{host}` serves `{guest_pattern}` from `{guest_provider}`",
                ));
            }
            Relation::DependsOn { source, target } => {
                buf.push_str(&format!("depends_on: `{source}` → `{target}`"));
            }
        }
    }
    buf
}

/// Pretty-print the relations as a JSON array. Each element carries
/// the `kind` discriminator the TOML schema uses.
pub fn relations_json(relations: &[Relation]) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(relations)
}

pub fn doctor_json(diags: &[&Diagnostic]) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(diags)
}

/// Render `check` outcomes as a pretty-printed JSON array — the
/// machine-readable counterpart of `--explain`. Each element
/// carries the per-expectation status, resolved path (when known),
/// the matched / prefer / avoid sets, and a tagged `diagnosis`
/// object derived from `lint::diagnose`.
///
/// Schema is stable through `0.0.x`. The `diagnosis` field uses a
/// `kind` discriminator (`"wrong_source"` / `"unknown_source"` /
/// `"not_found"` / `"not_executable"` / `"config"`) so consumers
/// can pattern-match instead of string-searching.
///
/// Pure: callers do the printing and exit-code mapping.
pub fn check_json(outcomes: &[Outcome]) -> Result<String, serde_json::Error> {
    let view: Vec<OutcomeView<'_>> = outcomes.iter().map(OutcomeView::from).collect();
    serde_json::to_string_pretty(&view)
}

#[derive(serde::Serialize)]
struct OutcomeView<'a> {
    command: &'a str,
    status: &'a Status,
    /// Per-rule severity copied from the Outcome. Always emitted
    /// (even for `error`, the default) so a downstream consumer
    /// gating on severity does not need a fallback for the absent
    /// case.
    severity: crate::config::Severity,
    #[serde(skip_serializing_if = "Option::is_none")]
    resolved: Option<String>,
    matched_sources: &'a [String],
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    prefer: &'a [String],
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    avoid: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    diagnosis: Option<Diagnosis>,
}

impl<'a> From<&'a Outcome> for OutcomeView<'a> {
    fn from(o: &'a Outcome) -> Self {
        OutcomeView {
            command: &o.command,
            status: &o.status,
            severity: o.severity,
            resolved: o.resolved.as_ref().map(|p| p.display().to_string()),
            matched_sources: &o.matched_sources,
            prefer: &o.prefer,
            avoid: &o.avoid,
            diagnosis: lint::diagnose(o),
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

    fn check_outcome_ok() -> Outcome {
        Outcome {
            command: "rg".into(),
            status: Status::Ok,
            resolved: Some(PathBuf::from("/home/u/.cargo/bin/rg")),
            matched_sources: vec!["cargo".into()],
            prefer: vec!["cargo".into()],
            avoid: vec![],
            severity: crate::config::Severity::Error,
        }
    }

    fn check_outcome_wrong_source() -> Outcome {
        Outcome {
            command: "rg".into(),
            status: Status::NgWrongSource,
            resolved: Some(PathBuf::from("/usr/local/bin/rg")),
            matched_sources: vec!["scoop".into()],
            prefer: vec!["cargo".into()],
            avoid: vec![],
            severity: crate::config::Severity::Error,
        }
    }

    fn sort_plan_noop() -> SortPlan {
        SortPlan {
            original: vec!["/usr/bin".into(), "/home/u/.cargo/bin".into()],
            sorted: vec!["/usr/bin".into(), "/home/u/.cargo/bin".into()],
            moves: vec![],
            notes: vec![],
        }
    }

    fn sort_plan_swap() -> SortPlan {
        SortPlan {
            original: vec!["/usr/bin".into(), "/home/u/.cargo/bin".into()],
            sorted: vec!["/home/u/.cargo/bin".into(), "/usr/bin".into()],
            moves: vec![
                crate::sort::EntryMove {
                    entry: "/home/u/.cargo/bin".into(),
                    from: 1,
                    to: 0,
                    reason: "preferred source for `rg`".into(),
                },
                crate::sort::EntryMove {
                    entry: "/usr/bin".into(),
                    from: 0,
                    to: 1,
                    reason: "displaced by a preferred entry".into(),
                },
            ],
            notes: vec![],
        }
    }

    #[test]
    fn sort_human_noop_says_already_sorted() {
        let out = sort_human(&sort_plan_noop());
        assert!(out.contains("already in a satisfying order"), "out: {out}");
        assert!(!out.ends_with('\n'), "no trailing newline");
    }

    #[test]
    fn sort_human_swap_renders_both_columns_and_moved_section() {
        let out = sort_human(&sort_plan_swap());
        assert!(out.contains("before"), "out: {out}");
        assert!(out.contains("after"), "out: {out}");
        assert!(out.contains("--dry-run"), "must mention dry-run: {out}");
        assert!(out.contains("moved:"), "must list moves: {out}");
        assert!(out.contains("preferred source for `rg`"));
    }

    #[test]
    fn sort_human_unsatisfiable_note_appears_after_diff() {
        let mut plan = sort_plan_noop();
        plan.notes.push(SortNote::UnsatisfiablePrefer {
            command: "rg".into(),
            prefer: vec!["cargo".into()],
        });
        let out = sort_human(&plan);
        assert!(out.contains("`rg` cannot be satisfied"));
        assert!(out.contains("`prefer = [cargo]`"));
    }

    #[test]
    fn sort_json_serializes_plan_verbatim() {
        let out = sort_json(&sort_plan_swap()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["original"][0], "/usr/bin");
        assert_eq!(v["sorted"][0], "/home/u/.cargo/bin");
        assert_eq!(v["moves"][0]["from"], 1);
        assert_eq!(v["moves"][0]["to"], 0);
        assert_eq!(v["moves"][0]["entry"], "/home/u/.cargo/bin");
    }

    #[test]
    fn sort_json_note_carries_kind_discriminator() {
        let mut plan = sort_plan_noop();
        plan.notes.push(SortNote::UnsatisfiablePrefer {
            command: "rg".into(),
            prefer: vec!["cargo".into()],
        });
        let out = sort_json(&plan).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["notes"][0]["kind"], "unsatisfiable_prefer");
        assert_eq!(v["notes"][0]["command"], "rg");
    }

    #[test]
    fn doctor_json_emits_top_level_array_with_kind_discriminator() {
        let d_missing = Diagnostic {
            index: 3,
            entry: "/usr/bin".into(),
            severity: Severity::Warn,
            kind: Kind::Missing,
        };
        let d_short = Diagnostic {
            index: 5,
            entry: "C:\\Users\\who\\.cargo\\bin".into(),
            severity: Severity::Warn,
            kind: Kind::Shortenable {
                suggestion: "%UserProfile%\\.cargo\\bin".into(),
            },
        };
        let out = doctor_json(&[&d_missing, &d_short]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(v.is_array());
        assert_eq!(v[0]["index"], 3);
        assert_eq!(v[0]["entry"], "/usr/bin");
        assert_eq!(v[0]["severity"], "warn");
        assert_eq!(v[0]["kind"], "missing");
        // Missing has no payload — no extra fields beyond the four above.
        assert!(v[0].get("suggestion").is_none());
        assert_eq!(v[1]["kind"], "shortenable");
        assert_eq!(v[1]["suggestion"], "%UserProfile%\\.cargo\\bin");
    }

    #[test]
    fn doctor_json_malformed_carries_error_severity_and_reason() {
        let d = Diagnostic {
            index: 0,
            entry: "C:\\foo|bar".into(),
            severity: Severity::Error,
            kind: Kind::Malformed {
                reason: "illegal character '|' in path".into(),
            },
        };
        let out = doctor_json(&[&d]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["severity"], "error");
        assert_eq!(v[0]["kind"], "malformed");
        assert!(v[0]["reason"].as_str().unwrap().contains("illegal"));
    }

    #[test]
    fn doctor_json_duplicate_carries_first_index() {
        let d = Diagnostic {
            index: 2,
            entry: "/usr/bin".into(),
            severity: Severity::Warn,
            kind: Kind::Duplicate { first_index: 0 },
        };
        let out = doctor_json(&[&d]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["kind"], "duplicate");
        assert_eq!(v[0]["first_index"], 0);
    }

    #[test]
    fn doctor_json_mise_activate_both_carries_both_layers() {
        let d = Diagnostic {
            index: 0,
            entry: "/home/u/.local/share/mise/shims".into(),
            severity: Severity::Warn,
            kind: Kind::MiseActivateBoth {
                shim_indices: vec![0],
                install_indices: vec![1, 2],
            },
        };
        let out = doctor_json(&[&d]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["kind"], "mise_activate_both");
        assert_eq!(v[0]["shim_indices"][0], 0);
        assert_eq!(v[0]["install_indices"][0], 1);
        assert_eq!(v[0]["install_indices"][1], 2);
    }

    #[test]
    fn doctor_json_empty_diagnostics_yields_empty_array() {
        let out = doctor_json(&[]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(v.is_array());
        assert_eq!(v.as_array().unwrap().len(), 0);
    }

    #[test]
    fn check_json_emits_array_with_status_resolved_and_diagnosis() {
        let out = check_json(&[check_outcome_ok(), check_outcome_wrong_source()]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["command"], "rg");
        assert_eq!(v[0]["status"], "ok");
        assert_eq!(v[0]["resolved"], "/home/u/.cargo/bin/rg");
        assert!(
            v[0].get("diagnosis").is_none(),
            "ok must not carry diagnosis"
        );
        assert_eq!(v[1]["status"], "ng_wrong_source");
        assert_eq!(v[1]["diagnosis"]["kind"], "wrong_source");
        assert_eq!(v[1]["diagnosis"]["matched"][0], "scoop");
        assert_eq!(v[1]["diagnosis"]["prefer_missed"][0], "cargo");
    }

    #[test]
    fn check_json_omits_empty_prefer_and_avoid_for_ok() {
        let out = check_json(&[check_outcome_ok()]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        // prefer is non-empty for this outcome
        assert!(v[0].get("prefer").is_some());
        // avoid is empty -> skipped
        assert!(v[0].get("avoid").is_none(), "empty avoid leaked");
    }

    #[test]
    fn check_json_resolved_field_absent_when_outcome_has_no_path() {
        let not_found = Outcome {
            command: "ghost".into(),
            status: Status::NgNotFound,
            resolved: None,
            matched_sources: vec![],
            prefer: vec!["cargo".into()],
            avoid: vec![],
            severity: crate::config::Severity::Error,
        };
        let out = check_json(&[not_found]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(v[0].get("resolved").is_none(), "resolved leaked: {out}");
        assert_eq!(v[0]["diagnosis"]["kind"], "not_found");
    }

    #[test]
    fn check_json_skip_outcome_has_no_diagnosis() {
        let skip = Outcome {
            command: "tooly".into(),
            status: Status::Skip,
            resolved: None,
            matched_sources: vec![],
            prefer: vec![],
            avoid: vec![],
            severity: crate::config::Severity::Error,
        };
        let out = check_json(&[skip]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["status"], "skip");
        assert!(v[0].get("diagnosis").is_none());
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

    // ---- Relation formatters ----------------------------------------

    fn alias_of(parent: &str, children: &[&str]) -> Relation {
        Relation::AliasOf {
            parent: parent.into(),
            children: children.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn conflicts(sources: &[&str], diagnostic: &str) -> Relation {
        Relation::ConflictsWhenBothInPath {
            sources: sources.iter().map(|s| s.to_string()).collect(),
            diagnostic: diagnostic.into(),
        }
    }

    fn served(host: &str, pattern: &str, provider: &str) -> Relation {
        Relation::ServedByVia {
            host: host.into(),
            guest_pattern: pattern.into(),
            guest_provider: provider.into(),
        }
    }

    fn depends(source: &str, target: &str) -> Relation {
        Relation::DependsOn {
            source: source.into(),
            target: target.into(),
        }
    }

    #[test]
    fn relations_human_empty_says_no_relations() {
        let out = relations_human(&[]);
        assert_eq!(out, "no relations declared");
    }

    #[test]
    fn relations_human_renders_alias_of_with_arrow_and_children() {
        let rels = vec![alias_of("mise", &["mise_shims", "mise_installs"])];
        let out = relations_human(&rels);
        assert!(out.starts_with("alias_of:"), "out: {out}");
        assert!(out.contains("`mise`"), "out: {out}");
        assert!(out.contains("mise_shims, mise_installs"), "out: {out}");
    }

    #[test]
    fn relations_human_renders_conflicts_with_diagnostic_name() {
        let rels = vec![conflicts(
            &["mise_shims", "mise_installs"],
            "mise_activate_both",
        )];
        let out = relations_human(&rels);
        assert!(out.starts_with("conflicts_when_both_in_path:"));
        assert!(out.contains("mise_shims, mise_installs"));
        assert!(out.contains("`mise_activate_both`"));
    }

    #[test]
    fn relations_human_renders_served_by_via_with_pattern_and_provider() {
        let rels = vec![served("mise_installs", "cargo-*", "cargo")];
        let out = relations_human(&rels);
        assert!(out.starts_with("served_by_via:"));
        assert!(out.contains("`mise_installs`"));
        assert!(out.contains("`cargo-*`"));
        assert!(out.contains("`cargo`"));
    }

    #[test]
    fn relations_human_renders_depends_on_with_source_and_target() {
        let rels = vec![depends("paru", "pacman")];
        let out = relations_human(&rels);
        assert!(out.starts_with("depends_on:"));
        assert!(out.contains("`paru`"));
        assert!(out.contains("`pacman`"));
    }

    #[test]
    fn relations_human_separates_multiple_relations_by_newline() {
        // Each relation occupies one line; the joiner is a single
        // newline. No trailing newline (caller adds it).
        let rels = vec![alias_of("mise", &["mise_shims"]), depends("paru", "pacman")];
        let out = relations_human(&rels);
        assert_eq!(out.lines().count(), 2, "out:\n{out}");
        assert!(!out.ends_with('\n'));
    }

    #[test]
    fn relations_json_is_an_array_with_kind_discriminator() {
        let rels = vec![
            alias_of("mise", &["mise_shims"]),
            conflicts(&["a", "b"], "x"),
            served("h", "p-*", "g"),
            depends("a", "b"),
        ];
        let out = relations_json(&rels).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 4);
        assert_eq!(arr[0]["kind"], "alias_of");
        assert_eq!(arr[1]["kind"], "conflicts_when_both_in_path");
        assert_eq!(arr[2]["kind"], "served_by_via");
        assert_eq!(arr[3]["kind"], "depends_on");
    }

    #[test]
    fn relations_json_alias_of_carries_parent_and_children_at_top_level() {
        // The TOML schema and the JSON output mirror each other
        // (no nesting under `payload` etc.) — this is the contract
        // CI consumers depend on.
        let rels = vec![alias_of("mise", &["mise_shims", "mise_installs"])];
        let out = relations_json(&rels).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["parent"], "mise");
        assert_eq!(v[0]["children"][0], "mise_shims");
        assert_eq!(v[0]["children"][1], "mise_installs");
    }

    #[test]
    fn relations_json_served_by_via_keeps_pattern_field() {
        let rels = vec![served("mise_installs", "cargo-*", "cargo")];
        let out = relations_json(&rels).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["host"], "mise_installs");
        assert_eq!(v[0]["guest_pattern"], "cargo-*");
        assert_eq!(v[0]["guest_provider"], "cargo");
    }
}
