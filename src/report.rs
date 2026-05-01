//! Format `Outcome`s into human-readable lines.

use crate::lint::{self, Diagnosis, Outcome, Status};

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
    // Non-failure statuses don't go through `Diagnosis`; their
    // detail is direct projection of the Outcome.
    match &o.status {
        Status::Ok => {
            return o.resolved.as_ref().map(|p| {
                let sources = if o.matched_sources.is_empty() {
                    String::from("(no source matched)")
                } else {
                    format!("source: {}", o.matched_sources.join(", "))
                };
                format!("{} — {}", p.display(), sources)
            });
        }
        Status::Skip => return Some("optional; not on PATH".into()),
        Status::NotApplicable => return Some("excluded by os filter".into()),
        _ => {}
    }
    // Failure statuses: render the one-liner from the Diagnosis so
    // the explain view and the default view share a single source
    // of truth.
    lint::diagnose(o).map(|d| detail_one_liner(o, &d))
}

fn detail_one_liner(o: &Outcome, diagnosis: &Diagnosis) -> String {
    let resolved = resolved_or_placeholder(o);
    match diagnosis {
        Diagnosis::WrongSource {
            matched,
            prefer_missed,
            avoid_hits: _,
        } => format!(
            "resolved: {resolved} — matched sources: [{}], prefer: [{}], avoid: [{}]",
            matched.join(", "),
            prefer_missed.join(", "),
            o.avoid.join(", "),
        ),
        Diagnosis::UnknownSource { prefer } => format!(
            "resolved: {resolved} — matched no defined source; prefer: [{}]",
            prefer.join(", "),
        ),
        Diagnosis::NotFound { .. } => "not found on PATH".into(),
        Diagnosis::NotExecutable { reason, .. } => {
            format!("resolved: {resolved} — not executable: {reason}")
        }
        Diagnosis::Config { message } => message.clone(),
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
/// Pure function. Goes through `lint::diagnose` so the human
/// rendering and the JSON rendering share a single source of truth
/// (the `Diagnosis` value). Non-failure statuses return an empty
/// vec; callers fall back to `detail_line`.
pub fn explain_lines(o: &Outcome) -> Vec<String> {
    let Some(diagnosis) = lint::diagnose(o) else {
        return Vec::new();
    };
    explain_lines_from(o, &diagnosis)
}

fn explain_lines_from(o: &Outcome, diagnosis: &Diagnosis) -> Vec<String> {
    let resolved = resolved_or_placeholder(o);
    match diagnosis {
        Diagnosis::WrongSource {
            matched,
            prefer_missed,
            avoid_hits,
        } => vec![
            format!("resolved:        {resolved}"),
            format!("matched sources: {}", join_or_none(matched)),
            format!("prefer:          {}", join_or_none(prefer_missed)),
            format!("avoid:           {}", join_or_none(&o.avoid)),
            format!(
                "diagnosis:       {}",
                wrong_source_sentence(matched, prefer_missed, avoid_hits)
            ),
            format!(
                "hint:            run `pathlint where {}` for uninstall guidance.",
                o.command
            ),
        ],
        Diagnosis::UnknownSource { prefer } => vec![
            format!("resolved:        {resolved}"),
            "matched sources: (none — path is outside every defined source)".into(),
            format!("prefer:          {}", join_or_none(prefer)),
            format!("avoid:           {}", join_or_none(&o.avoid)),
            "diagnosis:       command resolves from a directory not declared in any \
                [source.<name>]. Either add a source for that directory or remove the \
                directory from PATH."
                .into(),
            format!(
                "hint:            run `pathlint where {}` to see the full path; \
                add `[source.X]` matching it if you want this case to pass.",
                o.command
            ),
        ],
        Diagnosis::NotFound { prefer } => vec![
            format!("command:         {}", o.command),
            format!("prefer:          {}", join_or_none(prefer)),
            "diagnosis:       command not found on any PATH entry.".into(),
            "hint:            install it via one of the prefer sources, \
                or pass `optional = true` if the rule should accept absence."
                .into(),
        ],
        Diagnosis::NotExecutable { reason, matched } => vec![
            format!("resolved:        {resolved}"),
            format!("matched sources: {}", join_or_none(matched)),
            format!("diagnosis:       not executable — {reason}"),
            "hint:            another file/directory of the same name shadows the binary, \
                or the file lost its +x bit / became a broken symlink."
                .into(),
        ],
        Diagnosis::Config { message } => vec![
            format!("config error:    {message}"),
            "hint:            check spelling against `pathlint catalog list --names-only`.".into(),
        ],
    }
}

fn resolved_or_placeholder(o: &Outcome) -> String {
    o.resolved
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "<unresolved>".into())
}

fn join_or_none(v: &[String]) -> String {
    if v.is_empty() {
        "(none)".into()
    } else {
        v.join(", ")
    }
}

fn wrong_source_sentence(
    matched: &[String],
    prefer_missed: &[String],
    avoid_hits: &[String],
) -> String {
    if !avoid_hits.is_empty() {
        return format!(
            "resolved path matched `avoid` source(s) [{}]; rule forbids these.",
            avoid_hits.join(", ")
        );
    }
    if prefer_missed.is_empty() {
        return "resolved path matched a source the rule rejects.".into();
    }
    format!(
        "resolved path matched [{}], none of which are in `prefer` [{}].",
        matched.join(", "),
        prefer_missed.join(", ")
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
