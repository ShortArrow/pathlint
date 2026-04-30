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

    #[test]
    fn lazygit_any_of_three_preferred_is_ok() {
        // PRD §8.1: prefer is a set; matching any one is OK.
        let sources = cat(&[
            ("cargo", src("/home/u/.cargo/bin")),
            ("winget", src_win("WinGet")),
            ("mise", src("/home/u/.local/share/mise")),
        ]);
        let expectations = vec![Expectation {
            command: "lazygit".into(),
            prefer: vec!["cargo".into(), "winget".into(), "mise".into()],
            avoid: vec![],
            os: None,
            optional: false,
        }];
        // Only the mise install path matches; cargo and winget do not.
        let out = evaluate(&expectations, &sources, Os::Linux, |_| {
            Some(resolved(
                "/home/u/.local/share/mise/installs/lazygit/0.42/bin/lazygit",
            ))
        });
        assert_eq!(out[0].status, Status::Ok);
        assert_eq!(out[0].matched_sources, vec!["mise".to_string()]);
    }

    #[test]
    fn multiple_sources_match_same_path_all_recorded() {
        // PRD §8.1 explicitly: a path may match many sources.
        let sources = cat(&[
            ("mise", src("/home/u/.local/share/mise")),
            ("python_install", src("/installs/python/")),
        ]);
        let expectations = vec![Expectation {
            command: "python".into(),
            prefer: vec!["mise".into()],
            avoid: vec![],
            os: None,
            optional: false,
        }];
        let out = evaluate(&expectations, &sources, Os::Linux, |_| {
            Some(resolved(
                "/home/u/.local/share/mise/installs/python/3.12/bin/python",
            ))
        });
        assert_eq!(out[0].status, Status::Ok);
        assert_eq!(out[0].matched_sources.len(), 2);
        assert!(out[0].matched_sources.contains(&"mise".to_string()));
        assert!(
            out[0]
                .matched_sources
                .contains(&"python_install".to_string())
        );
    }

    #[test]
    fn avoid_overrides_prefer_when_both_match() {
        // If the resolved path matches both a prefer source and an
        // avoid source, avoid wins (status NG).
        let sources = cat(&[
            ("mise", src("/home/u/.local/share/mise")),
            ("dangerous_subdir", src("/installs/python/3.10/")),
        ]);
        let expectations = vec![Expectation {
            command: "python".into(),
            prefer: vec!["mise".into()],
            avoid: vec!["dangerous_subdir".into()],
            os: None,
            optional: false,
        }];
        let out = evaluate(&expectations, &sources, Os::Linux, |_| {
            Some(resolved(
                "/home/u/.local/share/mise/installs/python/3.10/bin/python",
            ))
        });
        assert_eq!(out[0].status, Status::NgWrongSource);
    }

    #[test]
    fn mise_layered_match_shim_path_hits_mise_and_mise_shims() {
        // A binary served via mise's shim layer must match BOTH the
        // catch-all `mise` source and the more specific `mise_shims`,
        // never `mise_installs`. Tests rule co-existence after the
        // 0.0.3 split.
        let sources = cat(&[
            ("mise", src("/home/u/.local/share/mise")),
            ("mise_shims", src("/home/u/.local/share/mise/shims")),
            ("mise_installs", src("/home/u/.local/share/mise/installs")),
        ]);
        let expectations = vec![Expectation {
            command: "python".into(),
            prefer: vec!["mise_shims".into()],
            avoid: vec![],
            os: None,
            optional: false,
        }];
        let out = evaluate(&expectations, &sources, Os::Linux, |_| {
            Some(resolved("/home/u/.local/share/mise/shims/python"))
        });
        assert_eq!(out[0].status, Status::Ok);
        assert!(out[0].matched_sources.contains(&"mise".to_string()));
        assert!(out[0].matched_sources.contains(&"mise_shims".to_string()));
        assert!(
            !out[0]
                .matched_sources
                .contains(&"mise_installs".to_string())
        );
    }

    #[test]
    fn mise_layered_match_install_path_hits_mise_and_mise_installs() {
        // The install layer (per-runtime bin dirs). Used when mise is
        // activated by PATH-rewriting, or when a plugin lives in
        // `installs/<plugin>/<ver>/bin`.
        let sources = cat(&[
            ("mise", src("/home/u/.local/share/mise")),
            ("mise_shims", src("/home/u/.local/share/mise/shims")),
            ("mise_installs", src("/home/u/.local/share/mise/installs")),
        ]);
        let expectations = vec![Expectation {
            command: "python".into(),
            prefer: vec!["mise_installs".into()],
            avoid: vec![],
            os: None,
            optional: false,
        }];
        let out = evaluate(&expectations, &sources, Os::Linux, |_| {
            Some(resolved(
                "/home/u/.local/share/mise/installs/python/3.14/bin/python",
            ))
        });
        assert_eq!(out[0].status, Status::Ok);
        assert!(out[0].matched_sources.contains(&"mise".to_string()));
        assert!(
            out[0]
                .matched_sources
                .contains(&"mise_installs".to_string())
        );
        assert!(!out[0].matched_sources.contains(&"mise_shims".to_string()));
    }

    #[test]
    fn mise_alias_remains_for_backwards_compat() {
        // Existing rules written with prefer = ["mise"] keep working
        // even though they don't know about mise_shims / mise_installs.
        let sources = cat(&[
            ("mise", src("/home/u/.local/share/mise")),
            ("mise_shims", src("/home/u/.local/share/mise/shims")),
            ("mise_installs", src("/home/u/.local/share/mise/installs")),
        ]);
        let expectations = vec![Expectation {
            command: "python".into(),
            prefer: vec!["mise".into()],
            avoid: vec![],
            os: None,
            optional: false,
        }];
        let out_shim = evaluate(&expectations, &sources, Os::Linux, |_| {
            Some(resolved("/home/u/.local/share/mise/shims/python"))
        });
        let out_install = evaluate(&expectations, &sources, Os::Linux, |_| {
            Some(resolved(
                "/home/u/.local/share/mise/installs/python/3.14/bin/python",
            ))
        });
        assert_eq!(out_shim[0].status, Status::Ok);
        assert_eq!(out_install[0].status, Status::Ok);
    }
}
