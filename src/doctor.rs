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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub index: usize,
    pub entry: String,
    pub severity: Severity,
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
}
