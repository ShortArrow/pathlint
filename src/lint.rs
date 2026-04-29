//! Evaluate `[[expect]]` entries against the resolved PATH.
//!
//! Pure: takes the merged catalog, the OS, the PATH entries, and a
//! resolver function, then returns one `Outcome` per expectation.
//! Tests can swap the resolver for a deterministic stub.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::config::{Expectation, SourceDef};
use crate::expand::{expand_and_normalize, normalize};
use crate::os_detect::Os;
use crate::resolve::Resolution;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Ok,
    NgWrongSource,
    NgUnknownSource,
    NgNotFound,
    Skip, // optional + not on PATH
    NotApplicable,
    ConfigError(String),
}

#[derive(Debug, Clone)]
pub struct Outcome {
    pub command: String,
    pub status: Status,
    pub resolved: Option<PathBuf>,
    pub matched_sources: Vec<String>,
    pub prefer: Vec<String>,
    pub avoid: Vec<String>,
}

/// Evaluate every expectation. `resolver` is called for the ones that
/// pass the OS filter; in production it is a closure over the real
/// PATH-resolve function, in tests it is a stub.
pub fn evaluate<R>(
    expectations: &[Expectation],
    sources: &BTreeMap<String, SourceDef>,
    os: Os,
    mut resolver: R,
) -> Vec<Outcome>
where
    R: FnMut(&str) -> Option<Resolution>,
{
    expectations
        .iter()
        .map(|e| evaluate_one(e, sources, os, &mut resolver))
        .collect()
}

fn evaluate_one<R: FnMut(&str) -> Option<Resolution>>(
    expect: &Expectation,
    sources: &BTreeMap<String, SourceDef>,
    os: Os,
    resolver: &mut R,
) -> Outcome {
    if !os_filter_applies(expect, os) {
        return Outcome {
            command: expect.command.clone(),
            status: Status::NotApplicable,
            resolved: None,
            matched_sources: Vec::new(),
            prefer: expect.prefer.clone(),
            avoid: expect.avoid.clone(),
        };
    }

    if let Some(name) = first_undefined(&expect.prefer, &expect.avoid, sources) {
        return Outcome {
            command: expect.command.clone(),
            status: Status::ConfigError(format!("undefined source name: {name}")),
            resolved: None,
            matched_sources: Vec::new(),
            prefer: expect.prefer.clone(),
            avoid: expect.avoid.clone(),
        };
    }

    let resolution = resolver(&expect.command);
    let Some(resolution) = resolution else {
        let status = if expect.optional {
            Status::Skip
        } else {
            Status::NgNotFound
        };
        return Outcome {
            command: expect.command.clone(),
            status,
            resolved: None,
            matched_sources: Vec::new(),
            prefer: expect.prefer.clone(),
            avoid: expect.avoid.clone(),
        };
    };

    let haystack = normalize(&resolution.full_path.to_string_lossy());
    let matched = matched_sources(&haystack, sources, os);
    let status = decide(&matched, &expect.prefer, &expect.avoid);

    Outcome {
        command: expect.command.clone(),
        status,
        resolved: Some(resolution.full_path),
        matched_sources: matched,
        prefer: expect.prefer.clone(),
        avoid: expect.avoid.clone(),
    }
}

fn os_filter_applies(expect: &Expectation, os: Os) -> bool {
    match &expect.os {
        None => true,
        Some(tags) => tags.iter().any(|t| os.matches_tag(t)),
    }
}

fn first_undefined<'a>(
    prefer: &'a [String],
    avoid: &'a [String],
    sources: &BTreeMap<String, SourceDef>,
) -> Option<&'a str> {
    for name in prefer.iter().chain(avoid.iter()) {
        if !sources.contains_key(name) {
            return Some(name.as_str());
        }
    }
    None
}

fn matched_sources(
    normalized_full_path: &str,
    sources: &BTreeMap<String, SourceDef>,
    os: Os,
) -> Vec<String> {
    let mut hits = Vec::new();
    for (name, def) in sources {
        let Some(raw) = def.path_for(os) else {
            continue;
        };
        let needle = expand_and_normalize(raw);
        if needle.is_empty() {
            continue;
        }
        if normalized_full_path.contains(&needle) {
            hits.push(name.clone());
        }
    }
    hits
}

fn decide(matched: &[String], prefer: &[String], avoid: &[String]) -> Status {
    let in_avoid = matched.iter().any(|m| avoid.iter().any(|a| a == m));
    if in_avoid {
        return Status::NgWrongSource;
    }
    if prefer.is_empty() {
        return Status::Ok;
    }
    if matched.is_empty() {
        return Status::NgUnknownSource;
    }
    let in_prefer = matched.iter().any(|m| prefer.iter().any(|p| p == m));
    if in_prefer {
        Status::Ok
    } else {
        Status::NgWrongSource
    }
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

    fn src_win(win: &str) -> SourceDef {
        SourceDef {
            windows: Some(win.into()),
            ..Default::default()
        }
    }

    fn cat(entries: &[(&str, SourceDef)]) -> BTreeMap<String, SourceDef> {
        entries
            .iter()
            .map(|(n, d)| (n.to_string(), d.clone()))
            .collect()
    }

    fn resolved(p: &str) -> Resolution {
        Resolution {
            full_path: PathBuf::from(p),
        }
    }

    #[test]
    fn ok_when_resolved_under_preferred_source() {
        let sources = cat(&[("cargo", src("/home/u/.cargo/bin"))]);
        let expectations = vec![Expectation {
            command: "runex".into(),
            prefer: vec!["cargo".into()],
            avoid: vec![],
            os: None,
            optional: false,
        }];
        let out = evaluate(&expectations, &sources, Os::Linux, |_| {
            Some(resolved("/home/u/.cargo/bin/runex"))
        });
        assert_eq!(out[0].status, Status::Ok);
        assert_eq!(out[0].matched_sources, vec!["cargo".to_string()]);
    }

    #[test]
    fn ng_wrong_source_when_avoid_hits() {
        let sources = cat(&[
            ("cargo", src("/home/u/.cargo/bin")),
            ("winget", src_win("WinGet")),
        ]);
        let expectations = vec![Expectation {
            command: "runex".into(),
            prefer: vec!["cargo".into()],
            avoid: vec!["winget".into()],
            os: None,
            optional: false,
        }];
        let out = evaluate(&expectations, &sources, Os::Windows, |_| {
            Some(resolved(
                r"C:\Users\u\AppData\Local\Microsoft\WinGet\Links\runex.exe",
            ))
        });
        assert_eq!(out[0].status, Status::NgWrongSource);
        assert!(out[0].matched_sources.contains(&"winget".to_string()));
    }

    #[test]
    fn unknown_source_when_no_match_but_prefer_set() {
        let sources = cat(&[("cargo", src("/home/u/.cargo/bin"))]);
        let expectations = vec![Expectation {
            command: "runex".into(),
            prefer: vec!["cargo".into()],
            avoid: vec![],
            os: None,
            optional: false,
        }];
        let out = evaluate(&expectations, &sources, Os::Linux, |_| {
            Some(resolved("/usr/local/bin/runex"))
        });
        assert_eq!(out[0].status, Status::NgUnknownSource);
    }

    #[test]
    fn not_found_unless_optional() {
        let expectations = vec![Expectation {
            command: "runex".into(),
            prefer: vec![],
            avoid: vec![],
            os: None,
            optional: false,
        }];
        let out = evaluate(&expectations, &BTreeMap::new(), Os::Linux, |_| None);
        assert_eq!(out[0].status, Status::NgNotFound);

        let optional = vec![Expectation {
            command: "runex".into(),
            prefer: vec![],
            avoid: vec![],
            os: None,
            optional: true,
        }];
        let out = evaluate(&optional, &BTreeMap::new(), Os::Linux, |_| None);
        assert_eq!(out[0].status, Status::Skip);
    }

    #[test]
    fn os_filter_excludes() {
        let expectations = vec![Expectation {
            command: "runex".into(),
            prefer: vec![],
            avoid: vec![],
            os: Some(vec!["windows".into()]),
            optional: false,
        }];
        let out = evaluate(&expectations, &BTreeMap::new(), Os::Linux, |_| {
            panic!("resolver must not be called for n/a expectations")
        });
        assert_eq!(out[0].status, Status::NotApplicable);
    }

    #[test]
    fn config_error_on_undefined_source() {
        let expectations = vec![Expectation {
            command: "runex".into(),
            prefer: vec!["nonexistent".into()],
            avoid: vec![],
            os: None,
            optional: false,
        }];
        let out = evaluate(&expectations, &BTreeMap::new(), Os::Linux, |_| {
            panic!("must not resolve when config is invalid")
        });
        assert!(matches!(out[0].status, Status::ConfigError(_)));
    }

    #[test]
    fn empty_prefer_with_avoid_only_passes_when_avoid_misses() {
        let sources = cat(&[("winget", src_win("WinGet"))]);
        let expectations = vec![Expectation {
            command: "runex".into(),
            prefer: vec![],
            avoid: vec!["winget".into()],
            os: None,
            optional: false,
        }];
        let out = evaluate(&expectations, &sources, Os::Windows, |_| {
            Some(resolved(r"C:\Users\u\.cargo\bin\runex.exe"))
        });
        assert_eq!(out[0].status, Status::Ok);
    }
}
