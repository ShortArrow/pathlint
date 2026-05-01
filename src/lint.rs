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
    /// R2 — resolved path failed `kind` shape check (directory,
    /// broken symlink, missing exec bit, etc.). Carries a short
    /// human-readable reason.
    NgNotExecutable(String),
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

/// Pure-data view of *why* an outcome failed. Derived from
/// `Outcome` by `diagnose`; kept separate so the presentation
/// layer renders strings from a structured value rather than from
/// raw `Outcome` fields. `serde::Serialize` so the same value can
/// drive `check --json`.
///
/// Variants name the failure mode; the fields are the load-bearing
/// facts callers need: which sources were missed (`prefer_missed`),
/// which `avoid` names were hit (`avoid_hits`), the reason the
/// shape check rejected the file, etc. The struct does *not*
/// carry `command` / `resolved` — those live on `Outcome` and the
/// caller pairs them up.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Diagnosis {
    /// Resolved path matched some sources, but none of the
    /// `prefer` set — or it matched a source listed under `avoid`.
    /// `avoid_hits` is the intersection of `matched ∩ avoid`;
    /// non-empty means the rule explicitly forbids this source.
    /// `prefer_missed` is `prefer` itself (every name the user
    /// hoped for); rendering decides whether to show it.
    WrongSource {
        matched: Vec<String>,
        prefer_missed: Vec<String>,
        avoid_hits: Vec<String>,
    },
    /// Path lies outside every defined `[source.<name>]`. No
    /// source name matched at all.
    UnknownSource { prefer: Vec<String> },
    /// Command was not on PATH, and the rule was not optional.
    NotFound { prefer: Vec<String> },
    /// `kind = "executable"` shape check rejected the resolved file.
    /// `reason` is the short human-readable cause from `lint`
    /// (`"is a directory"`, `"broken symlink"`, …).
    NotExecutable {
        reason: String,
        matched: Vec<String>,
    },
    /// `[[expect]]` referenced a source name that is not defined.
    Config { message: String },
}

/// Derive the `Diagnosis` for an outcome — the *why* behind a
/// failing status. Pure: takes only the outcome.
///
/// Returns `None` for non-failure statuses (`Ok` / `Skip` /
/// `NotApplicable`). Callers typically render a `Diagnosis` into
/// human or JSON form; treating the value as the single source of
/// truth keeps the two views in sync.
pub fn diagnose(o: &Outcome) -> Option<Diagnosis> {
    match &o.status {
        Status::Ok | Status::Skip | Status::NotApplicable => None,
        Status::NgWrongSource => {
            let avoid_hits: Vec<String> = o
                .matched_sources
                .iter()
                .filter(|m| o.avoid.iter().any(|a| a == *m))
                .cloned()
                .collect();
            Some(Diagnosis::WrongSource {
                matched: o.matched_sources.clone(),
                prefer_missed: o.prefer.clone(),
                avoid_hits,
            })
        }
        Status::NgUnknownSource => Some(Diagnosis::UnknownSource {
            prefer: o.prefer.clone(),
        }),
        Status::NgNotFound => Some(Diagnosis::NotFound {
            prefer: o.prefer.clone(),
        }),
        Status::NgNotExecutable(reason) => Some(Diagnosis::NotExecutable {
            reason: reason.clone(),
            matched: o.matched_sources.clone(),
        }),
        Status::ConfigError(msg) => Some(Diagnosis::Config {
            message: msg.clone(),
        }),
    }
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
    let mut status = decide(&matched, &expect.prefer, &expect.avoid);

    // R2 shape check. Only run when the source check already passed —
    // a `prefer` mismatch is a louder failure than a shape one and
    // we don't want to drown the user in two diagnostics for the
    // same expectation. The shape check only escalates an OK status
    // into a NG, never the other way around.
    if matches!(status, Status::Ok) {
        if let Some(kind) = expect.kind {
            if let Err(reason) = check_shape(&resolution.full_path, kind) {
                status = Status::NgNotExecutable(reason);
            }
        }
    }

    Outcome {
        command: expect.command.clone(),
        status,
        resolved: Some(resolution.full_path),
        matched_sources: matched,
        prefer: expect.prefer.clone(),
        avoid: expect.avoid.clone(),
    }
}

/// Verify a resolved path matches the expected `kind`. Today only
/// handles `Kind::Executable`. Returns Err with a short human-
/// readable reason on mismatch.
fn check_shape(path: &std::path::Path, kind: crate::config::Kind) -> Result<(), String> {
    use crate::config::Kind;
    match kind {
        Kind::Executable => check_executable(path),
    }
}

fn check_executable(path: &std::path::Path) -> Result<(), String> {
    // metadata() follows symlinks. If that fails, the symlink is
    // dangling or the file vanished between resolve and now.
    let md = match std::fs::metadata(path) {
        Ok(md) => md,
        Err(_) => {
            return Err(if path.is_symlink() {
                "broken symlink".into()
            } else {
                "cannot stat".into()
            });
        }
    };
    if md.is_dir() {
        return Err("is a directory".into());
    }
    if !md.is_file() {
        return Err("not a regular file".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if md.permissions().mode() & 0o111 == 0 {
            return Err("not executable (no +x bit)".into());
        }
    }
    Ok(())
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
            kind: None,
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
            kind: None,
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
            kind: None,
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
            kind: None,
        }];
        let out = evaluate(&expectations, &BTreeMap::new(), Os::Linux, |_| None);
        assert_eq!(out[0].status, Status::NgNotFound);

        let optional = vec![Expectation {
            command: "runex".into(),
            prefer: vec![],
            avoid: vec![],
            os: None,
            optional: true,
            kind: None,
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
            kind: None,
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
            kind: None,
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
            kind: None,
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
            kind: None,
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
            kind: None,
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
            kind: None,
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
            kind: None,
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
            kind: None,
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
            kind: None,
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

    // ---- R2 kind = "executable" shape checks --------------------

    use crate::config::Kind;

    fn expect_with_kind(command: &str, source: &str, kind: Kind) -> Expectation {
        Expectation {
            command: command.into(),
            prefer: vec![source.into()],
            avoid: vec![],
            os: None,
            optional: false,
            kind: Some(kind),
        }
    }

    /// Build a SourceDef whose path is set on every OS, so the
    /// kind tests work regardless of which OS the test host runs on.
    fn src_anywhere(p: &str) -> SourceDef {
        SourceDef {
            windows: Some(p.into()),
            unix: Some(p.into()),
            ..Default::default()
        }
    }

    #[test]
    fn kind_executable_flags_directory_shadow() {
        // Source matches, but the resolved path is a directory of
        // the same name. R1 says OK, R2 must escalate to NG.
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("rogue_bin");
        std::fs::create_dir_all(&dir).unwrap();
        let dir_str = dir.to_string_lossy().into_owned();
        let sources = cat(&[("rogue", src_anywhere(&dir_str))]);
        let expectations = vec![expect_with_kind("rogue_bin", "rogue", Kind::Executable)];
        let out = evaluate(&expectations, &sources, Os::current(), |_| {
            Some(Resolution {
                full_path: dir.clone(),
            })
        });
        match &out[0].status {
            Status::NgNotExecutable(reason) => {
                assert!(reason.contains("directory"), "reason: {reason}");
            }
            other => panic!("expected NgNotExecutable, got {other:?}"),
        }
    }

    #[test]
    fn kind_executable_flags_missing_target() {
        // Resolver returns a path that doesn't exist on disk —
        // a file vanishing between resolve and shape-check, or a
        // dangling symlink. metadata() fails, R2 says NG.
        let sources = cat(&[("anywhere", src_anywhere("/no/such/place"))]);
        let expectations = vec![expect_with_kind("ghost", "anywhere", Kind::Executable)];
        let out = evaluate(&expectations, &sources, Os::current(), |_| {
            Some(resolved("/no/such/place/ghost"))
        });
        assert!(matches!(out[0].status, Status::NgNotExecutable(_)));
    }

    #[test]
    fn kind_unset_skips_shape_check_entirely() {
        // Even when the resolved path is bogus, no shape check
        // means the source-only outcome stands.
        let sources = cat(&[("anywhere", src_anywhere("/no/such/place"))]);
        let expectations = vec![Expectation {
            command: "ghost".into(),
            prefer: vec!["anywhere".into()],
            avoid: vec![],
            os: None,
            optional: false,
            kind: None,
        }];
        let out = evaluate(&expectations, &sources, Os::current(), |_| {
            Some(resolved("/no/such/place/ghost"))
        });
        assert_eq!(out[0].status, Status::Ok);
    }

    #[test]
    fn kind_executable_does_not_override_wrong_source() {
        // A source mismatch (NG already) must not be downgraded by
        // the shape check.
        let sources = cat(&[("good", src("/home/u/good")), ("bad", src("/home/u/bad"))]);
        let expectations = vec![Expectation {
            command: "x".into(),
            prefer: vec!["good".into()],
            avoid: vec!["bad".into()],
            os: None,
            optional: false,
            kind: Some(Kind::Executable),
        }];
        let out = evaluate(&expectations, &sources, Os::Linux, |_| {
            Some(resolved("/home/u/bad/x"))
        });
        // Stays NgWrongSource — the shape check is suppressed
        // because the source check already failed.
        assert!(matches!(out[0].status, Status::NgWrongSource));
    }

    // ---- diagnose() -------------------------------------------------

    fn outcome(status: Status, matched: &[&str], prefer: &[&str], avoid: &[&str]) -> Outcome {
        Outcome {
            command: "rg".into(),
            status,
            resolved: Some(PathBuf::from("/usr/local/bin/rg")),
            matched_sources: matched.iter().map(|s| s.to_string()).collect(),
            prefer: prefer.iter().map(|s| s.to_string()).collect(),
            avoid: avoid.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn diagnose_returns_none_for_non_failure_statuses() {
        for status in [Status::Ok, Status::Skip, Status::NotApplicable] {
            let o = outcome(status, &["cargo"], &["cargo"], &[]);
            assert!(
                diagnose(&o).is_none(),
                "status should yield None: {:?}",
                o.status
            );
        }
    }

    #[test]
    fn diagnose_wrong_source_collects_avoid_hits_when_intersection_non_empty() {
        let o = outcome(
            Status::NgWrongSource,
            &["winget", "scoop"],
            &["cargo"],
            &["winget"],
        );
        let d = diagnose(&o).unwrap();
        match d {
            Diagnosis::WrongSource {
                matched,
                prefer_missed,
                avoid_hits,
            } => {
                assert_eq!(matched, vec!["winget", "scoop"]);
                assert_eq!(prefer_missed, vec!["cargo"]);
                assert_eq!(avoid_hits, vec!["winget"]);
            }
            other => panic!("expected WrongSource, got {other:?}"),
        }
    }

    #[test]
    fn diagnose_wrong_source_with_no_avoid_overlap_returns_empty_avoid_hits() {
        let o = outcome(Status::NgWrongSource, &["scoop"], &["cargo"], &[]);
        let d = diagnose(&o).unwrap();
        match d {
            Diagnosis::WrongSource { avoid_hits, .. } => assert!(avoid_hits.is_empty()),
            other => panic!("expected WrongSource, got {other:?}"),
        }
    }

    #[test]
    fn diagnose_unknown_source_carries_only_prefer() {
        let o = outcome(Status::NgUnknownSource, &[], &["cargo"], &[]);
        let d = diagnose(&o).unwrap();
        assert!(
            matches!(d, Diagnosis::UnknownSource { ref prefer } if prefer == &["cargo".to_string()])
        );
    }

    #[test]
    fn diagnose_not_found_carries_prefer() {
        let o = outcome(Status::NgNotFound, &[], &["cargo", "winget"], &[]);
        let d = diagnose(&o).unwrap();
        assert!(matches!(d, Diagnosis::NotFound { ref prefer } if prefer.len() == 2));
    }

    #[test]
    fn diagnose_not_executable_keeps_reason_and_matched() {
        let o = outcome(
            Status::NgNotExecutable("is a directory".into()),
            &["custom"],
            &["custom"],
            &[],
        );
        let d = diagnose(&o).unwrap();
        match d {
            Diagnosis::NotExecutable { reason, matched } => {
                assert_eq!(reason, "is a directory");
                assert_eq!(matched, vec!["custom"]);
            }
            other => panic!("expected NotExecutable, got {other:?}"),
        }
    }

    #[test]
    fn diagnose_config_error_propagates_message() {
        let o = outcome(
            Status::ConfigError("undefined source name: typo".into()),
            &[],
            &[],
            &[],
        );
        let d = diagnose(&o).unwrap();
        assert!(matches!(d, Diagnosis::Config { ref message } if message.contains("typo")));
    }

    #[test]
    fn diagnosis_serializes_with_kind_discriminator() {
        let d = Diagnosis::WrongSource {
            matched: vec!["scoop".into()],
            prefer_missed: vec!["cargo".into()],
            avoid_hits: vec![],
        };
        let json = serde_json::to_value(&d).unwrap();
        assert_eq!(json["kind"], "wrong_source");
        assert_eq!(json["matched"][0], "scoop");
    }
}
