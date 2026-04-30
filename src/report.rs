//! Format `Outcome`s into human-readable lines.

use crate::lint::{Outcome, Status};

#[derive(Debug, Clone, Copy)]
pub struct Style {
    pub no_glyphs: bool,
    pub verbose: bool,
    pub quiet: bool,
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
    let detail = detail_line(o);
    if let Some(d) = detail {
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
