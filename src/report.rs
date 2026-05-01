//! Format `Outcome`s into human-readable lines.

use crate::lint::{Outcome, Status};

#[derive(Debug, Clone, Copy)]
pub struct Style {
    pub no_glyphs: bool,
    pub verbose: bool,
    pub quiet: bool,
    /// `--explain`: when true, NG outcomes get a multi-line breakdown
    /// of resolved / matched / prefer / avoid / diagnosis / hint
    /// instead of the one-line detail.
    pub explain: bool,
}

pub fn render(outcomes: &[Outcome], style: Style) -> String {
    let mut buf = String::new();
    for o in outcomes {
        if style.quiet && !is_failure(&o.status) {
            continue;
        }
        if !style.verbose && matches!(o.status, Status::NotApplicable) {
            continue;
        }
        buf.push_str(&render_one(o, style));
        buf.push('\n');
    }
    buf
}

fn render_one(o: &Outcome, style: Style) -> String {
    let tag = status_tag(&o.status, style.no_glyphs);
    let mut line = format!("{tag} {command}", command = o.command);

    // --explain mode swaps the one-line detail for the multi-line
    // breakdown. Only failure statuses produce explain content;
    // anything else (Ok / Skip / NotApplicable) falls through to
    // the regular detail line so behaviour is unchanged.
    if style.explain && is_failure(&o.status) {
        for ex in explain_lines(o) {
            line.push('\n');
            line.push_str("    ");
            line.push_str(&ex);
        }
        return line;
    }

    if let Some(d) = detail_line(o) {
        line.push('\n');
        line.push_str("    ");
        line.push_str(&d);
    }
    line
}

fn detail_line(o: &Outcome) -> Option<String> {
    match &o.status {
        Status::Ok => o.resolved.as_ref().map(|p| {
            let sources = if o.matched_sources.is_empty() {
                String::from("(no source matched)")
            } else {
                format!("source: {}", o.matched_sources.join(", "))
            };
            format!("{} — {}", p.display(), sources)
        }),
        Status::NgWrongSource => Some(format!(
            "resolved: {} — matched sources: [{}], prefer: [{}], avoid: [{}]",
            o.resolved
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "<unresolved>".into()),
            o.matched_sources.join(", "),
            o.prefer.join(", "),
            o.avoid.join(", "),
        )),
        Status::NgUnknownSource => Some(format!(
            "resolved: {} — matched no defined source; prefer: [{}]",
            o.resolved
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "<unresolved>".into()),
            o.prefer.join(", "),
        )),
        Status::NgNotFound => Some("not found on PATH".into()),
        Status::NgNotExecutable(reason) => Some(format!(
            "resolved: {} — not executable: {reason}",
            o.resolved
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "<unresolved>".into()),
        )),
        Status::Skip => Some("optional; not on PATH".into()),
        Status::NotApplicable => Some("excluded by os filter".into()),
        Status::ConfigError(msg) => Some(msg.clone()),
    }
}

fn status_tag(s: &Status, no_glyphs: bool) -> &'static str {
    match (s, no_glyphs) {
        (Status::Ok, false) => "[OK]  ",
        (
            Status::NgWrongSource
            | Status::NgUnknownSource
            | Status::NgNotFound
            | Status::NgNotExecutable(_),
            false,
        ) => "[NG]  ",
        (Status::Skip, false) => "[skip]",
        (Status::NotApplicable, false) => "[n/a] ",
        (Status::ConfigError(_), false) => "[ERR] ",
        (Status::Ok, true) => "OK   ",
        (
            Status::NgWrongSource
            | Status::NgUnknownSource
            | Status::NgNotFound
            | Status::NgNotExecutable(_),
            true,
        ) => "NG   ",
        (Status::Skip, true) => "skip ",
        (Status::NotApplicable, true) => "n/a  ",
        (Status::ConfigError(_), true) => "ERR  ",
    }
}

pub fn is_failure(status: &Status) -> bool {
    matches!(
        status,
        Status::NgWrongSource
            | Status::NgUnknownSource
            | Status::NgNotFound
            | Status::NgNotExecutable(_)
    )
}

pub fn has_config_error(outcomes: &[Outcome]) -> bool {
    outcomes
        .iter()
        .any(|o| matches!(o.status, Status::ConfigError(_)))
}

/// Render a structured, multi-line diagnosis for a single Outcome.
/// Used by `pathlint check --explain` to expand the one-line detail
/// into resolved / matched / prefer / avoid / diagnosis / hint rows.
///
/// Returned as a `Vec<String>` of detail lines (no leading indent,
/// no header — the caller wraps each line with the same `    `
/// prefix used by `detail_line`). Pure function: no I/O, no
/// allocation outside the returned strings.
pub fn explain_lines(o: &Outcome) -> Vec<String> {
    let mut lines = Vec::new();
    let resolved = o
        .resolved
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "<unresolved>".into());
    let none_marker = || String::from("(none)");
    let join_or_none = |v: &[String]| {
        if v.is_empty() {
            none_marker()
        } else {
            v.join(", ")
        }
    };

    match &o.status {
        Status::NgWrongSource => {
            lines.push(format!("resolved:        {resolved}"));
            lines.push(format!(
                "matched sources: {}",
                join_or_none(&o.matched_sources)
            ));
            lines.push(format!("prefer:          {}", join_or_none(&o.prefer)));
            lines.push(format!("avoid:           {}", join_or_none(&o.avoid)));
            lines.push(format!(
                "diagnosis:       {}",
                wrong_source_diagnosis(&o.matched_sources, &o.prefer, &o.avoid)
            ));
            lines.push(format!(
                "hint:            run `pathlint where {}` for uninstall guidance.",
                o.command
            ));
        }
        Status::NgUnknownSource => {
            lines.push(format!("resolved:        {resolved}"));
            lines.push("matched sources: (none — path is outside every defined source)".into());
            lines.push(format!("prefer:          {}", join_or_none(&o.prefer)));
            lines.push(format!("avoid:           {}", join_or_none(&o.avoid)));
            lines.push(
                "diagnosis:       command resolves from a directory not declared in any \
                    [source.<name>]. Either add a source for that directory or remove the \
                    directory from PATH."
                    .into(),
            );
            lines.push(format!(
                "hint:            run `pathlint where {}` to see the full path; \
                add `[source.X]` matching it if you want this case to pass.",
                o.command
            ));
        }
        Status::NgNotFound => {
            lines.push(format!("command:         {}", o.command));
            lines.push(format!("prefer:          {}", join_or_none(&o.prefer)));
            lines.push("diagnosis:       command not found on any PATH entry.".into());
            lines.push(
                "hint:            install it via one of the prefer sources, \
                or pass `optional = true` if the rule should accept absence."
                    .into(),
            );
        }
        Status::NgNotExecutable(reason) => {
            lines.push(format!("resolved:        {resolved}"));
            lines.push(format!(
                "matched sources: {}",
                join_or_none(&o.matched_sources)
            ));
            lines.push(format!("diagnosis:       not executable — {reason}"));
            lines.push(
                "hint:            another file/directory of the same name shadows the binary, \
                or the file lost its +x bit / became a broken symlink."
                    .into(),
            );
        }
        Status::ConfigError(msg) => {
            lines.push(format!("config error:    {msg}"));
            lines.push(
                "hint:            check spelling against `pathlint catalog list --names-only`."
                    .into(),
            );
        }
        // Non-failure statuses don't get an explain block; the
        // caller falls back to detail_line for these.
        Status::Ok | Status::Skip | Status::NotApplicable => {}
    }
    lines
}

fn wrong_source_diagnosis(matched: &[String], prefer: &[String], avoid: &[String]) -> String {
    let in_avoid: Vec<&String> = matched
        .iter()
        .filter(|m| avoid.iter().any(|a| a == *m))
        .collect();
    if !in_avoid.is_empty() {
        let names: Vec<&str> = in_avoid.iter().map(|s| s.as_str()).collect();
        return format!(
            "resolved path matched `avoid` source(s) [{}]; rule forbids these.",
            names.join(", ")
        );
    }
    if prefer.is_empty() {
        // Should not happen for NgWrongSource, but stay defensive.
        return "resolved path matched a source the rule rejects.".into();
    }
    format!(
        "resolved path matched [{}], none of which are in `prefer` [{}].",
        matched.join(", "),
        prefer.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn ng_wrong_source(matched: &[&str], prefer: &[&str], avoid: &[&str]) -> Outcome {
        Outcome {
            command: "rg".into(),
            status: Status::NgWrongSource,
            resolved: Some(PathBuf::from("/usr/local/bin/rg")),
            matched_sources: matched.iter().map(|s| s.to_string()).collect(),
            prefer: prefer.iter().map(|s| s.to_string()).collect(),
            avoid: avoid.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn explain_lines_empty_for_ok_status() {
        let o = Outcome {
            command: "rg".into(),
            status: Status::Ok,
            resolved: Some(PathBuf::from("/home/u/.cargo/bin/rg")),
            matched_sources: vec!["cargo".into()],
            prefer: vec!["cargo".into()],
            avoid: vec![],
        };
        assert!(explain_lines(&o).is_empty());
    }

    #[test]
    fn explain_lines_wrong_source_lists_resolved_matched_prefer_avoid() {
        let o = ng_wrong_source(&["scoop"], &["cargo"], &[]);
        let lines = explain_lines(&o);
        assert_eq!(lines[0], "resolved:        /usr/local/bin/rg");
        assert_eq!(lines[1], "matched sources: scoop");
        assert_eq!(lines[2], "prefer:          cargo");
        assert_eq!(lines[3], "avoid:           (none)");
        assert!(lines[4].starts_with("diagnosis:"));
        assert!(lines[4].contains("none of which are in `prefer`"));
        assert!(lines[5].starts_with("hint:"));
        assert!(lines[5].contains("pathlint where rg"));
    }

    #[test]
    fn explain_lines_wrong_source_calls_out_avoid_hit_explicitly() {
        // When matched intersects avoid, the diagnosis should name
        // the offending avoid source — that's the load-bearing fact.
        let o = ng_wrong_source(&["winget"], &["cargo"], &["winget"]);
        let lines = explain_lines(&o);
        let diagnosis = lines.iter().find(|l| l.starts_with("diagnosis:")).unwrap();
        assert!(
            diagnosis.contains("matched `avoid` source(s) [winget]"),
            "diagnosis: {diagnosis}"
        );
    }

    #[test]
    fn explain_lines_unknown_source_says_path_outside_every_source() {
        let o = Outcome {
            command: "rg".into(),
            status: Status::NgUnknownSource,
            resolved: Some(PathBuf::from("/opt/custom/bin/rg")),
            matched_sources: vec![],
            prefer: vec!["cargo".into()],
            avoid: vec![],
        };
        let lines = explain_lines(&o);
        assert!(
            lines
                .iter()
                .any(|l| l.contains("path is outside every defined source")),
        );
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("hint:") && l.contains("[source.X]")),
        );
    }

    #[test]
    fn explain_lines_not_found_advises_install_or_optional() {
        let o = Outcome {
            command: "ghost".into(),
            status: Status::NgNotFound,
            resolved: None,
            matched_sources: vec![],
            prefer: vec!["cargo".into()],
            avoid: vec![],
        };
        let lines = explain_lines(&o);
        assert!(lines.iter().any(|l| l.contains("not found on any PATH")));
        assert!(lines.iter().any(|l| l.contains("optional = true")));
    }

    #[test]
    fn explain_lines_not_executable_carries_reason_and_shadow_hint() {
        let o = Outcome {
            command: "rg".into(),
            status: Status::NgNotExecutable("is a directory".into()),
            resolved: Some(PathBuf::from("/tmp/rg")),
            matched_sources: vec!["custom".into()],
            prefer: vec!["custom".into()],
            avoid: vec![],
        };
        let lines = explain_lines(&o);
        assert!(
            lines
                .iter()
                .any(|l| l.contains("not executable — is a directory"))
        );
        assert!(lines.iter().any(|l| l.contains("shadows the binary")));
    }

    fn style(explain: bool) -> Style {
        Style {
            no_glyphs: false,
            verbose: false,
            quiet: false,
            explain,
        }
    }

    #[test]
    fn render_without_explain_keeps_one_line_detail() {
        let outcomes = vec![ng_wrong_source(&["scoop"], &["cargo"], &[])];
        let out = render(&outcomes, style(false));
        // existing one-line detail format
        assert!(out.contains("matched sources: [scoop]"), "out: {out}");
        // and only one detail row (header + one indented line)
        let n_lines = out.trim_end().lines().count();
        assert_eq!(n_lines, 2, "out:\n{out}");
    }

    #[test]
    fn render_with_explain_emits_multiline_breakdown() {
        let outcomes = vec![ng_wrong_source(&["scoop"], &["cargo"], &[])];
        let out = render(&outcomes, style(true));
        // header + 6 explain rows = 7 lines
        let n_lines = out.trim_end().lines().count();
        assert_eq!(n_lines, 7, "out:\n{out}");
        assert!(out.contains("    resolved:        /usr/local/bin/rg"));
        assert!(out.contains("    diagnosis:"));
        assert!(out.contains("    hint:            run `pathlint where rg`"));
    }

    #[test]
    fn render_explain_skips_ok_outcomes_unchanged() {
        let ok = Outcome {
            command: "rg".into(),
            status: Status::Ok,
            resolved: Some(PathBuf::from("/home/u/.cargo/bin/rg")),
            matched_sources: vec!["cargo".into()],
            prefer: vec!["cargo".into()],
            avoid: vec![],
        };
        let out = render(&[ok], style(true));
        assert!(out.contains("[OK]"), "out: {out:?}");
        assert!(out.contains("rg"), "out: {out:?}");
        assert!(out.contains("source: cargo"));
        // Just header + detail = 2 lines, no explain block.
        assert_eq!(out.trim_end().lines().count(), 2);
    }

    #[test]
    fn explain_lines_config_error_quotes_the_underlying_message() {
        let o = Outcome {
            command: "rg".into(),
            status: Status::ConfigError("undefined source name: typo".into()),
            resolved: None,
            matched_sources: vec![],
            prefer: vec![],
            avoid: vec![],
        };
        let lines = explain_lines(&o);
        assert!(lines[0].contains("undefined source name: typo"));
        assert!(lines[1].contains("catalog list"));
    }
}
