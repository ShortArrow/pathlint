//! PATH-hygiene checks. Independent of `[[expect]]` evaluation.
//!
//! Each diagnostic carries a severity:
//!
//! * `Error` — the entry is malformed enough that the OS cannot use
//!   it as a directory (e.g. embedded NUL, illegal chars).
//! * `Warn` — the entry works, but is suspicious (duplicate,
//!   missing directory, 8.3 shortname, shortenable with an env var,
//!   trailing slash, case-variant duplicate).
//!
//! Doctor pure-functions take a list of PATH entry strings and return
//! `Vec<Diagnostic>`. The CLI layer formats them and decides the exit
//! code.

use std::collections::BTreeMap;
use std::env;
use std::path::Path;

use crate::expand;
use crate::os_detect::Os;

/// Real-world `fs_exists` for `analyze`: hits the filesystem.
pub fn fs_exists_real(path: &str) -> bool {
    Path::new(path).exists()
}

/// Real-world `env_lookup` for `analyze`: reads the process env.
pub fn env_lookup_real(var: &str) -> Option<String> {
    env::var(var).ok()
}

/// Convenience: production wiring of `analyze` that uses the real
/// filesystem and process env. Equivalent to
/// `analyze(entries, os, fs_exists_real, env_lookup_real)`.
pub fn analyze_real(entries: &[String], os: Os) -> Vec<Diagnostic> {
    analyze(entries, os, fs_exists_real, env_lookup_real)
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Warn,
    Error,
}

/// Discriminated union of every doctor diagnostic kind. The
/// `kind` field is the discriminator and the variant payload is
/// flattened alongside it for JSON consumers — e.g. `Shortenable`
/// emits `{"kind":"shortenable","suggestion":"..."}` rather than
/// nesting the suggestion under a wrapper.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Kind {
    Duplicate {
        first_index: usize,
    },
    Missing,
    Shortenable {
        suggestion: String,
    },
    TrailingSlash,
    CaseVariant {
        canonical: String,
    },
    ShortName,
    Malformed {
        reason: String,
    },
    /// PATH exposes both `mise/shims/` and `mise/installs/`
    /// directories at the same time. Usually means `mise activate`
    /// is configured in both shim and PATH-rewrite modes, or stale
    /// entries from a past configuration are still in PATH.
    /// `shim_indices` and `install_indices` list which entries fall
    /// in each layer; the `Diagnostic.index` points at the first
    /// shim entry for sort stability.
    MiseActivateBoth {
        shim_indices: Vec<usize>,
        install_indices: Vec<usize>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Diagnostic {
    pub index: usize,
    pub entry: String,
    pub severity: Severity,
    /// Flattened so the discriminator (`kind`) and any per-variant
    /// payload sit at the top level next to `index` / `entry` /
    /// `severity`.
    #[serde(flatten)]
    pub kind: Kind,
}

/// Stable kebabless name for a `Kind` variant. Used by
/// `pathlint doctor --include` / `--exclude`. Returning a `&'static
/// str` (not formatting the Debug output) means the names are part
/// of the public CLI surface and survive struct-field changes.
pub fn kind_name(kind: &Kind) -> &'static str {
    match kind {
        Kind::Duplicate { .. } => "duplicate",
        Kind::Missing => "missing",
        Kind::Shortenable { .. } => "shortenable",
        Kind::TrailingSlash => "trailing_slash",
        Kind::CaseVariant { .. } => "case_variant",
        Kind::ShortName => "short_name",
        Kind::Malformed { .. } => "malformed",
        Kind::MiseActivateBoth { .. } => "mise_activate_both",
    }
}

/// Every name `kind_name` can return. Used for CLI input validation
/// and help text.
pub fn all_kind_names() -> &'static [&'static str] {
    &[
        "duplicate",
        "missing",
        "shortenable",
        "trailing_slash",
        "case_variant",
        "short_name",
        "malformed",
        "mise_activate_both",
    ]
}

/// User intent for `pathlint doctor --include` / `--exclude`.
/// Pure data: holds two snake_case kind-name lists. The semantics
/// are "include-when-non-empty, otherwise exclude-when-non-empty,
/// otherwise pass-through". `--include` / `--exclude` are mutually
/// exclusive at the CLI layer (clap `conflicts_with`); this struct
/// does not re-enforce that.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

impl Filter {
    /// Filter a slice of diagnostics by kind name. The returned
    /// vector borrows from the input. Pure: no allocations beyond
    /// the references.
    ///
    /// Semantics:
    /// - both empty → pass everything through
    /// - `include` non-empty → keep only diagnostics whose kind is listed
    /// - `exclude` non-empty (and `include` empty) → drop listed kinds
    pub fn apply<'a>(&self, diags: &'a [Diagnostic]) -> Vec<&'a Diagnostic> {
        diags
            .iter()
            .filter(|d| {
                let name = kind_name(&d.kind);
                if !self.include.is_empty() {
                    self.include.iter().any(|s| s == name)
                } else if !self.exclude.is_empty() {
                    !self.exclude.iter().any(|s| s == name)
                } else {
                    true
                }
            })
            .collect()
    }
}

/// Reject any name in `filter` that isn't a valid `Kind` discriminator.
/// Returns `Err` carrying a one-line message naming the offending
/// name and the valid set, suitable for surfacing as exit code 2.
pub fn validate_filter_names(filter: &Filter) -> Result<(), String> {
    let known: std::collections::BTreeSet<&'static str> =
        all_kind_names().iter().copied().collect();
    for name in filter.include.iter().chain(filter.exclude.iter()) {
        if !known.contains(name.as_str()) {
            return Err(format!(
                "unknown doctor kind `{name}`; valid values: {}",
                all_kind_names().join(", ")
            ));
        }
    }
    Ok(())
}

/// Does the (already-filtered) set of diagnostics contain at least
/// one `Severity::Error`? This is the single source of truth for
/// `pathlint doctor`'s exit code 1 — an excluded `Malformed`
/// diagnostic must not escalate, which is why we check the kept
/// set rather than the raw `analyze` output.
pub fn has_error(diags: &[&Diagnostic]) -> bool {
    diags.iter().any(|d| d.severity == Severity::Error)
}

/// Run every PATH-hygiene check and return a flat list of
/// diagnostics. Pure: I/O is reached via the injected `fs_exists`
/// (used by the missing-directory check) and `env_lookup` (used by
/// the shortenable-with-an-env-var check). Production passes
/// `fs_exists_real` / `env_lookup_real`; tests pass deterministic
/// stubs. See `analyze_real` for the production wiring.
pub fn analyze<F, V>(entries: &[String], os: Os, fs_exists: F, env_lookup: V) -> Vec<Diagnostic>
where
    F: Fn(&str) -> bool,
    V: Fn(&str) -> Option<String>,
{
    let mut out = Vec::new();
    for (i, entry) in entries.iter().enumerate() {
        if let Some(d) = check_malformed(i, entry) {
            out.push(d);
            // If the entry is malformed, skip the other checks for it
            // — they're going to be noisy or wrong.
            continue;
        }
        if let Some(d) = check_missing(i, entry, &fs_exists) {
            out.push(d);
        }
        if let Some(d) = check_trailing_slash(i, entry) {
            out.push(d);
        }
        if os == Os::Windows {
            if let Some(d) = check_short_name(i, entry) {
                out.push(d);
            }
        }
        if let Some(d) = check_shortenable(i, entry, os, &env_lookup) {
            out.push(d);
        }
    }
    // Pair-wise checks need every entry's normalized form.
    let normalized: Vec<String> = entries
        .iter()
        .map(|e| expand::normalize(&expand::expand_env(e)))
        .collect();
    add_duplicate_diagnostics(&normalized, entries, &mut out);
    add_case_variant_diagnostics(entries, &mut out);
    add_mise_activate_both_diagnostic(&normalized, entries, &mut out);
    out
}

fn check_malformed(index: usize, entry: &str) -> Option<Diagnostic> {
    if entry.contains('\0') {
        return Some(Diagnostic {
            index,
            entry: entry.to_string(),
            severity: Severity::Error,
            kind: Kind::Malformed {
                reason: "embedded NUL byte".into(),
            },
        });
    }
    if cfg!(windows) {
        // PATH separator is ;, so ; cannot appear in an entry. Other
        // illegal-on-NTFS characters: <>"|?* and control chars.
        for c in entry.chars() {
            let illegal =
                matches!(c, '<' | '>' | '"' | '|' | '?' | '*') || (c.is_control() && c != '\t');
            if illegal {
                return Some(Diagnostic {
                    index,
                    entry: entry.to_string(),
                    severity: Severity::Error,
                    kind: Kind::Malformed {
                        reason: format!("illegal character {c:?} in path"),
                    },
                });
            }
        }
    }
    None
}

fn check_missing<F>(index: usize, entry: &str, fs_exists: &F) -> Option<Diagnostic>
where
    F: Fn(&str) -> bool,
{
    let expanded = expand::expand_env(entry);
    if fs_exists(&expanded) {
        return None;
    }
    Some(Diagnostic {
        index,
        entry: entry.to_string(),
        severity: Severity::Warn,
        kind: Kind::Missing,
    })
}

fn check_trailing_slash(index: usize, entry: &str) -> Option<Diagnostic> {
    if entry.len() <= 1 {
        return None;
    }
    let last = entry.chars().last().unwrap();
    if last != '/' && last != '\\' {
        return None;
    }
    // Allow root-level slashes ("/", "C:/", "C:\\").
    if entry == "/" || entry.ends_with(":/") || entry.ends_with(":\\") {
        return None;
    }
    Some(Diagnostic {
        index,
        entry: entry.to_string(),
        severity: Severity::Warn,
        kind: Kind::TrailingSlash,
    })
}

fn check_short_name(index: usize, entry: &str) -> Option<Diagnostic> {
    // Windows 8.3 short names contain "~<digit>" before a slash or end.
    // Heuristic: any segment matching <up-to-6 chars>~<digit>+ .
    for segment in entry.split(['/', '\\']) {
        if looks_like_8dot3(segment) {
            return Some(Diagnostic {
                index,
                entry: entry.to_string(),
                severity: Severity::Warn,
                kind: Kind::ShortName,
            });
        }
    }
    None
}

fn looks_like_8dot3(segment: &str) -> bool {
    let bytes = segment.as_bytes();
    let Some(tilde) = bytes.iter().position(|&b| b == b'~') else {
        return false;
    };
    if tilde == 0 || tilde > 6 {
        return false;
    }
    let after = &bytes[tilde + 1..];
    if after.is_empty() {
        return false;
    }
    // Read run of digits.
    let mut digits = 0;
    while digits < after.len() && after[digits].is_ascii_digit() {
        digits += 1;
    }
    if digits == 0 {
        return false;
    }
    // Whatever follows the digit run must be either end-of-segment or
    // the file-extension dot — NOT a regular ident character. That
    // way "lib~1.so" / "PROGRA~1" trip the check while "foo~bar" or
    // "FILE_~_NAME" don't.
    matches!(after.get(digits), None | Some(b'.'))
}

fn check_shortenable<V>(index: usize, entry: &str, os: Os, env_lookup: &V) -> Option<Diagnostic>
where
    V: Fn(&str) -> Option<String>,
{
    // Skip if the entry is already using an env var.
    if entry.contains('%') || entry.contains('$') {
        return None;
    }
    // Match on normalized form (lowercased + slash-unified) but reuse
    // the raw entry's tail so the suggestion preserves the user's
    // capitalization and slash style.
    let normalized_entry = expand::normalize(entry);
    for (var, prefer_style) in candidate_vars(os) {
        let Some(raw) = env_lookup(var) else {
            continue;
        };
        if raw.is_empty() {
            continue;
        }
        let normalized_var = expand::normalize(&raw);
        if !normalized_entry.starts_with(&normalized_var) {
            continue;
        }
        // The raw entry begins with the same prefix length (in chars)
        // because normalize is char-preserving — only case and slashes
        // change. Cut the same number of bytes off the raw entry.
        let suffix = entry.get(normalized_var.len()..).unwrap_or("");
        let suggestion = match prefer_style {
            VarStyle::Percent => format!("%{var}%{suffix}"),
            VarStyle::Dollar => format!("${var}{suffix}"),
        };
        return Some(Diagnostic {
            index,
            entry: entry.to_string(),
            severity: Severity::Warn,
            kind: Kind::Shortenable { suggestion },
        });
    }
    None
}

#[derive(Clone, Copy)]
enum VarStyle {
    Percent,
    Dollar,
}

fn candidate_vars(os: Os) -> &'static [(&'static str, VarStyle)] {
    // Order matters: the first match wins, so list the most specific
    // (deepest) prefix first.
    match os {
        Os::Windows => &[
            ("LocalAppData", VarStyle::Percent),
            ("AppData", VarStyle::Percent),
            ("ProgramFiles(x86)", VarStyle::Percent),
            ("ProgramFiles", VarStyle::Percent),
            ("ProgramData", VarStyle::Percent),
            ("UserProfile", VarStyle::Percent),
            ("SystemRoot", VarStyle::Percent),
        ],
        _ => &[("HOME", VarStyle::Dollar)],
    }
}

fn add_duplicate_diagnostics(normalized: &[String], raw: &[String], out: &mut Vec<Diagnostic>) {
    let mut first_seen: BTreeMap<&str, usize> = BTreeMap::new();
    for (i, n) in normalized.iter().enumerate() {
        if n.is_empty() {
            continue;
        }
        if let Some(&first) = first_seen.get(n.as_str()) {
            out.push(Diagnostic {
                index: i,
                entry: raw[i].clone(),
                severity: Severity::Warn,
                kind: Kind::Duplicate { first_index: first },
            });
        } else {
            first_seen.insert(n.as_str(), i);
        }
    }
}

fn add_mise_activate_both_diagnostic(
    normalized: &[String],
    raw: &[String],
    out: &mut Vec<Diagnostic>,
) {
    // Look for entries that contain `mise/shims` vs `mise/installs`.
    // We deliberately don't mine the catalog here — these substrings
    // are the well-known mise layout, and depending on the catalog
    // (which the user can override) for a hygiene check would be
    // surprising.
    let mut shim_indices: Vec<usize> = Vec::new();
    let mut install_indices: Vec<usize> = Vec::new();
    for (i, n) in normalized.iter().enumerate() {
        if n.contains("/mise/shims") {
            shim_indices.push(i);
        }
        // `/mise/installs` matches both the parent dir and any
        // `installs/<runtime>/<ver>/bin` underneath it. Both forms
        // count as "the install layer is present".
        if n.contains("/mise/installs") {
            install_indices.push(i);
        }
    }
    if shim_indices.is_empty() || install_indices.is_empty() {
        return;
    }
    // Anchor the diagnostic at the first shim entry; sort stays
    // stable that way regardless of how the layers are interleaved.
    let anchor = shim_indices[0];
    out.push(Diagnostic {
        index: anchor,
        entry: raw[anchor].clone(),
        severity: Severity::Warn,
        kind: Kind::MiseActivateBoth {
            shim_indices,
            install_indices,
        },
    });
}

fn add_case_variant_diagnostics(raw: &[String], out: &mut Vec<Diagnostic>) {
    // Two PATH entries can have identical normalized form but differ
    // verbatim (case difference, mixed slashes). The plain Duplicate
    // diagnostic already covers exact-string duplicates; this one
    // catches "looks the same to the OS, looks different in the
    // file" cases so the user can decide whether to canonicalize.
    let mut buckets: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, entry) in raw.iter().enumerate() {
        let key = expand::normalize(&expand::expand_env(entry));
        if key.is_empty() {
            continue;
        }
        buckets.entry(key).or_default().push(i);
    }
    for indices in buckets.values() {
        if indices.len() < 2 {
            continue;
        }
        let first = indices[0];
        for &i in &indices[1..] {
            // Skip exact-verbatim duplicates — Duplicate covers them.
            if raw[i] == raw[first] {
                continue;
            }
            out.push(Diagnostic {
                index: i,
                entry: raw[i].clone(),
                severity: Severity::Warn,
                kind: Kind::CaseVariant {
                    canonical: raw[first].clone(),
                },
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(strs: &[&str]) -> Vec<String> {
        strs.iter().map(|s| s.to_string()).collect()
    }

    fn kinds(diags: &[Diagnostic]) -> Vec<&Kind> {
        diags.iter().map(|d| &d.kind).collect()
    }

    /// Test stubs for the closures `analyze` injects. Most tests
    /// don't care about either signal, so default to "every path
    /// exists" + "no env var defined" — that way `Missing` and
    /// `Shortenable` simply don't fire and noise stays low.
    fn fs_yes(_: &str) -> bool {
        true
    }
    fn fs_no(_: &str) -> bool {
        false
    }
    fn env_none(_: &str) -> Option<String> {
        None
    }
    fn env_map<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |k| {
            pairs
                .iter()
                .find(|(name, _)| *name == k)
                .map(|(_, v)| (*v).to_string())
        }
    }

    #[test]
    fn duplicate_detected_on_normalized_form() {
        let e = entries(&["/usr/bin", "/usr/local/bin", "/usr/bin"]);
        let diags = analyze(&e, Os::Linux, fs_yes, env_none);
        let dups: Vec<_> = diags
            .iter()
            .filter(|d| matches!(d.kind, Kind::Duplicate { .. }))
            .collect();
        assert_eq!(dups.len(), 1);
        assert_eq!(dups[0].index, 2);
    }

    #[test]
    fn missing_directory_detected() {
        // fs_no makes every path "missing" — drives the Missing path
        // without touching the real filesystem.
        let e = entries(&["/anywhere"]);
        let diags = analyze(&e, Os::Linux, fs_no, env_none);
        assert!(diags.iter().any(|d| matches!(d.kind, Kind::Missing)));
    }

    #[test]
    fn trailing_slash_detected_but_root_allowed() {
        let e = entries(&["/foo/", "/", "C:/"]);
        let diags = analyze(&e, Os::Linux, fs_yes, env_none);
        let trailing: Vec<_> = diags
            .iter()
            .filter(|d| matches!(d.kind, Kind::TrailingSlash))
            .collect();
        assert_eq!(trailing.len(), 1);
        assert_eq!(trailing[0].index, 0);
    }

    #[test]
    fn malformed_nul_is_error_severity() {
        let e = entries(&["/foo\0/bar"]);
        let diags = analyze(&e, Os::Linux, fs_yes, env_none);
        assert!(
            diags
                .iter()
                .any(|d| d.severity == Severity::Error && matches!(d.kind, Kind::Malformed { .. }))
        );
    }

    #[test]
    fn looks_like_8dot3_matches_typical_short_names() {
        assert!(looks_like_8dot3("PROGRA~1"));
        assert!(looks_like_8dot3("USERPR~2"));
        assert!(looks_like_8dot3("lib~1.so"));
    }

    #[test]
    fn looks_like_8dot3_rejects_normal_names() {
        assert!(!looks_like_8dot3("Program Files"));
        assert!(!looks_like_8dot3("foo~bar"));
        assert!(!looks_like_8dot3("file~name~here"));
        assert!(!looks_like_8dot3("~/.cargo/bin"));
    }

    #[test]
    fn shortenable_suggests_env_var_when_entry_starts_with_one() {
        // Inject UserProfile via env_map; analyze should pick it up
        // and emit a Shortenable suggestion that preserves the
        // original case and backslashes from the entry tail.
        let e = entries(&["C:\\Users\\Mixed\\GoLang\\bin"]);
        let diags = analyze(
            &e,
            Os::Windows,
            fs_yes,
            env_map(&[("UserProfile", "C:\\Users\\Mixed")]),
        );
        let s = diags
            .iter()
            .find_map(|d| match &d.kind {
                Kind::Shortenable { suggestion } => Some(suggestion.clone()),
                _ => None,
            })
            .expect("expected Shortenable");
        assert_eq!(s, "%UserProfile%\\GoLang\\bin");
    }

    #[test]
    fn shortenable_skipped_when_already_using_env_var() {
        // Pre-condition: even if HOME points at a prefix of the entry,
        // we don't suggest anything when the entry already uses $.
        let e = entries(&["$HOME/bin"]);
        let diags = analyze(&e, Os::Linux, fs_yes, env_map(&[("HOME", "/home/u")]));
        assert!(
            !diags
                .iter()
                .any(|d| matches!(d.kind, Kind::Shortenable { .. }))
        );
    }

    #[test]
    fn case_variant_picked_up_when_only_case_differs() {
        // No more temp-dir dance; fs_yes makes both paths "exist" so
        // Missing does not pollute the result, leaving CaseVariant
        // free to fire on platforms that case-fold.
        let e = entries(&["/Tmp/Pathlint_Case", "/tmp/pathlint_case"]);
        let diags = analyze(&e, Os::Linux, fs_yes, env_none);
        let case: Vec<_> = diags
            .iter()
            .filter(|d| matches!(d.kind, Kind::CaseVariant { .. }))
            .collect();
        assert!(!case.is_empty(), "diags: {diags:?}");
    }

    #[test]
    fn empty_entries_are_silently_ignored() {
        let e = entries(&[""]);
        let diags = analyze(&e, Os::Linux, fs_yes, env_none);
        // Empty entries are filtered upstream by `split_path`. If one
        // does sneak in, our checks must not blow up.
        let _ = kinds(&diags);
    }

    // ---- MiseActivateBoth (R3 / 0.0.5+) ------------------------

    #[test]
    fn mise_activate_both_fires_when_shim_and_install_coexist() {
        let e = entries(&[
            "/home/u/.local/share/mise/shims",
            "/home/u/.local/share/mise/installs/python/3.14/bin",
            "/usr/bin",
        ]);
        let diags = analyze(&e, Os::Linux, fs_yes, env_none);
        let mab: Vec<_> = diags
            .iter()
            .filter(|d| matches!(d.kind, Kind::MiseActivateBoth { .. }))
            .collect();
        assert_eq!(mab.len(), 1);
        if let Kind::MiseActivateBoth {
            shim_indices,
            install_indices,
        } = &mab[0].kind
        {
            assert_eq!(shim_indices, &vec![0]);
            assert_eq!(install_indices, &vec![1]);
        } else {
            panic!("kind mismatch");
        }
    }

    #[test]
    fn mise_activate_both_does_not_fire_when_only_shims_present() {
        let e = entries(&["/home/u/.local/share/mise/shims", "/usr/bin"]);
        let diags = analyze(&e, Os::Linux, fs_yes, env_none);
        assert!(
            !diags
                .iter()
                .any(|d| matches!(d.kind, Kind::MiseActivateBoth { .. }))
        );
    }

    #[test]
    fn mise_activate_both_does_not_fire_when_only_installs_present() {
        let e = entries(&[
            "/home/u/.local/share/mise/installs/python/3.14/bin",
            "/usr/bin",
        ]);
        let diags = analyze(&e, Os::Linux, fs_yes, env_none);
        assert!(
            !diags
                .iter()
                .any(|d| matches!(d.kind, Kind::MiseActivateBoth { .. }))
        );
    }

    #[test]
    fn mise_activate_both_collects_multiple_install_entries() {
        let e = entries(&[
            "/home/u/.local/share/mise/shims",
            "/home/u/.local/share/mise/installs/python/3.14/bin",
            "/home/u/.local/share/mise/installs/node/25.9.0/bin",
            "/usr/bin",
        ]);
        let diags = analyze(&e, Os::Linux, fs_yes, env_none);
        let kind = diags
            .iter()
            .find_map(|d| {
                if let Kind::MiseActivateBoth {
                    shim_indices,
                    install_indices,
                } = &d.kind
                {
                    Some((shim_indices.clone(), install_indices.clone()))
                } else {
                    None
                }
            })
            .expect("MiseActivateBoth must fire");
        assert_eq!(kind.0, vec![0]);
        assert_eq!(kind.1, vec![1, 2]);
    }

    // ---- Filter / validate / has_error ---------------------------

    fn diag(kind: Kind, severity: Severity) -> Diagnostic {
        Diagnostic {
            index: 0,
            entry: "/anywhere".into(),
            severity,
            kind,
        }
    }

    #[test]
    fn filter_default_passes_everything_through() {
        let diags = vec![
            diag(Kind::Missing, Severity::Warn),
            diag(Kind::TrailingSlash, Severity::Warn),
        ];
        let kept = Filter::default().apply(&diags);
        assert_eq!(kept.len(), 2);
    }

    #[test]
    fn filter_include_keeps_only_named_kinds() {
        let diags = vec![
            diag(Kind::Missing, Severity::Warn),
            diag(Kind::TrailingSlash, Severity::Warn),
            diag(Kind::Malformed { reason: "x".into() }, Severity::Error),
        ];
        let f = Filter {
            include: vec!["missing".into(), "malformed".into()],
            ..Default::default()
        };
        let kept = f.apply(&diags);
        let names: Vec<&'static str> = kept.iter().map(|d| kind_name(&d.kind)).collect();
        assert_eq!(names, vec!["missing", "malformed"]);
    }

    #[test]
    fn filter_exclude_drops_named_kinds_when_include_empty() {
        let diags = vec![
            diag(Kind::Missing, Severity::Warn),
            diag(Kind::TrailingSlash, Severity::Warn),
        ];
        let f = Filter {
            exclude: vec!["trailing_slash".into()],
            ..Default::default()
        };
        let kept = f.apply(&diags);
        assert_eq!(kept.len(), 1);
        assert!(matches!(kept[0].kind, Kind::Missing));
    }

    #[test]
    fn filter_include_takes_precedence_over_exclude_when_both_set() {
        // CLI layer enforces mutual exclusion; this guards the
        // semantic in case someone constructs a Filter directly.
        let diags = vec![
            diag(Kind::Missing, Severity::Warn),
            diag(Kind::TrailingSlash, Severity::Warn),
        ];
        let f = Filter {
            include: vec!["missing".into()],
            exclude: vec!["missing".into()],
        };
        let kept = f.apply(&diags);
        assert_eq!(kept.len(), 1);
        assert!(matches!(kept[0].kind, Kind::Missing));
    }

    #[test]
    fn validate_filter_names_accepts_valid() {
        let f = Filter {
            include: vec!["duplicate".into(), "malformed".into()],
            exclude: vec![],
        };
        assert!(validate_filter_names(&f).is_ok());
    }

    #[test]
    fn validate_filter_names_rejects_typo() {
        let f = Filter {
            include: vec!["duplicat".into()],
            exclude: vec![],
        };
        let err = validate_filter_names(&f).unwrap_err();
        assert!(err.contains("duplicat"));
        assert!(err.contains("duplicate"), "valid list must be listed");
    }

    #[test]
    fn validate_checks_exclude_too() {
        let f = Filter {
            include: vec![],
            exclude: vec!["nope".into()],
        };
        assert!(validate_filter_names(&f).is_err());
    }

    #[test]
    fn has_error_true_when_any_kept_is_error_severity() {
        let d_err = diag(Kind::Malformed { reason: "x".into() }, Severity::Error);
        let d_warn = diag(Kind::Missing, Severity::Warn);
        let kept: Vec<&Diagnostic> = vec![&d_warn, &d_err];
        assert!(has_error(&kept));
    }

    #[test]
    fn has_error_false_when_all_kept_are_warn() {
        let d1 = diag(Kind::Missing, Severity::Warn);
        let d2 = diag(Kind::TrailingSlash, Severity::Warn);
        let kept: Vec<&Diagnostic> = vec![&d1, &d2];
        assert!(!has_error(&kept));
    }

    #[test]
    fn has_error_respects_filtering_excluding_malformed_lets_run_pass() {
        // Regression guard: the whole point of the kept-set check
        // is that excluding `malformed` lets a run pass even when
        // the raw analysis would have escalated.
        let diags = vec![
            diag(Kind::Malformed { reason: "x".into() }, Severity::Error),
            diag(Kind::Missing, Severity::Warn),
        ];
        let f = Filter {
            exclude: vec!["malformed".into()],
            ..Default::default()
        };
        let kept = f.apply(&diags);
        assert!(!has_error(&kept), "excluded malformed must not escalate");
    }
}
