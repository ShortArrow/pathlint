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

use crate::config::{Relation, SourceDef};
use crate::expand;
use crate::os_detect::Os;
use crate::source_match;

/// Real-world `fs_exists` for `analyze`: hits the filesystem.
pub fn fs_exists_real(path: &str) -> bool {
    Path::new(path).exists()
}

/// Real-world `env_lookup` for `analyze`: reads the process env.
pub fn env_lookup_real(var: &str) -> Option<String> {
    env::var(var).ok()
}

/// Convenience: production wiring of `analyze` that uses the real
/// filesystem and process env. `sources` and `relations` come from
/// the merged catalog; relation-driven conflict diagnostics
/// (`Relation::ConflictsWhenBothInPath`) fire from this set.
pub fn analyze_real(
    entries: &[String],
    sources: &BTreeMap<String, SourceDef>,
    relations: &[Relation],
    os: Os,
) -> Vec<Diagnostic> {
    analyze(
        entries,
        sources,
        relations,
        os,
        fs_exists_real,
        env_lookup_real,
    )
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
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
    /// Multiple sources that should not coexist in PATH have all
    /// fired at once. `diagnostic` is the snake_case label
    /// identifying the specific conflict (e.g. `mise_activate_both`)
    /// and comes from a `Relation::ConflictsWhenBothInPath`. Each
    /// element of `groups` lists the PATH entries that matched the
    /// corresponding source in the relation's `sources` array.
    /// `Diagnostic.index` points at the first entry of the first
    /// non-empty group for sort stability.
    Conflict {
        diagnostic: String,
        groups: Vec<Vec<usize>>,
    },
    /// A `[source.<name>]` declared in the merged catalog points
    /// at a per-OS path that does not exist on the filesystem.
    /// Common when a user's `pathlint.toml` declares a source for
    /// a tool they don't actually have installed (e.g.
    /// `[source.cargo] unix = "$HOME/.cargo/bin"` on a host
    /// without rust). 0.0.18+. `entry` is the expanded path that
    /// was checked; `Diagnostic.index` is fixed at `usize::MAX`
    /// because the diagnostic is per-source, not per-PATH-entry.
    PerSourceMissingRequired {
        source: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
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
/// `pathlint doctor --include` / `--exclude`. The names are part of
/// the public CLI surface — `Conflict` returns the runtime
/// `diagnostic` field so user-defined relations get filtered by
/// their declared name (e.g. `mise_activate_both`,
/// `arm_x86_homebrew_overlap`).
pub fn kind_name(kind: &Kind) -> &str {
    match kind {
        Kind::Duplicate { .. } => "duplicate",
        Kind::Missing => "missing",
        Kind::Shortenable { .. } => "shortenable",
        Kind::TrailingSlash => "trailing_slash",
        Kind::CaseVariant { .. } => "case_variant",
        Kind::ShortName => "short_name",
        Kind::Malformed { .. } => "malformed",
        Kind::Conflict { diagnostic, .. } => diagnostic.as_str(),
        Kind::PerSourceMissingRequired { .. } => "per_source_missing_required",
    }
}

/// Every static name `kind_name` can return for built-in detectors.
/// Used for CLI input validation and help text. Conflict diagnostics
/// declared by user relations are not enumerated here — the CLI
/// accepts any string and lets unmatched names fall through (the
/// existing pass-through Filter::apply semantics already covers
/// "kind name not produced anywhere", which becomes a no-op).
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
        "per_source_missing_required",
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

/// Reject any name in `filter` that isn't a valid `Kind` discriminator
/// or a user-declared `ConflictsWhenBothInPath` diagnostic name.
/// Returns `Err` carrying a one-line message naming the offending
/// name and the valid set, suitable for surfacing as exit code 2.
///
/// `extra_known` is the diagnostic-name list collected from the
/// merged relation set (see `user_diagnostic_names`). Without it,
/// `pathlint doctor --include foo_overlap` would be hard-rejected
/// even when the user declared `[[relation]] kind =
/// "conflicts_when_both_in_path" diagnostic = "foo_overlap"`.
pub fn validate_filter_names(filter: &Filter, extra_known: &[String]) -> Result<(), String> {
    let mut known: std::collections::BTreeSet<String> =
        all_kind_names().iter().map(|s| (*s).to_string()).collect();
    known.extend(extra_known.iter().cloned());
    for name in filter.include.iter().chain(filter.exclude.iter()) {
        if !known.contains(name) {
            let mut all: Vec<String> = known.iter().cloned().collect();
            all.sort();
            return Err(format!(
                "unknown doctor kind `{name}`; valid values: {}",
                all.join(", ")
            ));
        }
    }
    Ok(())
}

/// Collect the `diagnostic` strings declared by every
/// `ConflictsWhenBothInPath` relation in `relations`. Used by
/// `validate_filter_names` so user-defined conflict names flow
/// through `--include` / `--exclude` correctly. Pure.
pub fn user_diagnostic_names(relations: &[Relation]) -> Vec<String> {
    // 0.0.18: read conflict diagnostics via RelationIndex so this
    // call site no longer pattern-matches on the Relation sum type
    // directly.
    crate::catalog::RelationIndex::from_slice(relations)
        .iter_conflicts()
        .map(|(_sources, diagnostic)| diagnostic.to_string())
        .collect()
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
///
/// `sources` and `relations` together drive
/// `Kind::Conflict` diagnostics: every
/// `Relation::ConflictsWhenBothInPath` in `relations` fires when
/// at least two of its declared `sources` match the current PATH.
pub fn analyze<F, V>(
    entries: &[String],
    sources: &BTreeMap<String, SourceDef>,
    relations: &[Relation],
    os: Os,
    fs_exists: F,
    env_lookup: V,
) -> Vec<Diagnostic>
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
    add_relation_conflict_diagnostics(&normalized, entries, sources, relations, os, &mut out);
    add_per_source_missing_required_diagnostics(sources, os, &fs_exists, &env_lookup, &mut out);
    out
}

/// Detect declared `[source.<name>]` entries whose per-OS path
/// does not exist on the filesystem. 0.0.18+. The path is expanded
/// (`expand_env`) before checking so `$HOME` / `~` work the same
/// way the rest of the doctor pipeline does. Sources that are not
/// applicable to the current OS (no path defined for `os`) are
/// skipped — that is a config-time decision, not a hygiene one.
///
/// Built-in sources are also skipped: every host is missing most
/// of the catalog (you don't have winget on Linux, you don't have
/// brew on Termux), so flagging them would drown the user in
/// known-irrelevant warnings. Only sources the user supplied via
/// their own `pathlint.toml` are checked.
///
/// Pure: every fs hit goes through the injected `fs_exists`
/// closure, every env lookup through `env_lookup`. Tests stub both.
fn add_per_source_missing_required_diagnostics<F, V>(
    sources: &BTreeMap<String, SourceDef>,
    os: Os,
    fs_exists: &F,
    env_lookup: &V,
    out: &mut Vec<Diagnostic>,
) where
    F: Fn(&str) -> bool,
    V: Fn(&str) -> Option<String>,
{
    // Skip built-in catalog sources — most are deliberately missing
    // on any given host (no winget on Linux, no brew on Termux).
    // Only user-supplied sources should fire this detector.
    let builtin = crate::catalog::builtin();
    for (name, def) in sources {
        if builtin.contains_key(name) {
            // Treat as user override iff the def differs from the
            // built-in. Conservative: if any per-OS field changed,
            // assume the user opted in to checking this source.
            if let Some(builtin_def) = builtin.get(name) {
                if builtin_def == def {
                    continue;
                }
            }
        }
        let Some(raw) = def.path_for(os) else {
            continue;
        };
        // Apply the same env-var expansion as the rest of the
        // doctor pipeline so `$HOME` / `~` resolve consistently.
        // Reuse `expand::expand_env` directly; the entry-level
        // `check_missing` does the same dance for PATH entries.
        let expanded = expand_with_env(raw, env_lookup);
        if expanded.is_empty() {
            continue;
        }
        if fs_exists(&expanded) {
            continue;
        }
        out.push(Diagnostic {
            // Per-source diagnostics are not anchored to a PATH
            // index. usize::MAX is the sentinel — formatters render
            // it as "(catalog)" instead of an entry number.
            index: usize::MAX,
            entry: expanded,
            severity: Severity::Warn,
            kind: Kind::PerSourceMissingRequired {
                source: name.clone(),
            },
        });
    }
}

/// Tiny shim that runs `expand::expand_env` first, then resolves
/// any `$VAR` / `~` / `%VAR%` against the injected env_lookup so
/// tests can stub the environment without touching the process.
/// Used only by per-source missing detection so far; the existing
/// shortenable / check_missing call sites have their own paths.
fn expand_with_env<V>(raw: &str, env_lookup: &V) -> String
where
    V: Fn(&str) -> Option<String>,
{
    // Best-effort: replace literal $VAR and ~/ via env_lookup so
    // tests retain control. If `env_lookup` doesn't know a name,
    // leave the literal in place — fs_exists will reject it.
    let mut buf = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '~' && (chars.peek() == Some(&'/') || chars.peek().is_none()) {
            if let Some(home) = env_lookup("HOME").or_else(|| env_lookup("USERPROFILE")) {
                buf.push_str(&home);
                continue;
            }
        }
        if c == '$' {
            let mut name = String::new();
            while let Some(&nc) = chars.peek() {
                if nc.is_ascii_alphanumeric() || nc == '_' {
                    name.push(nc);
                    chars.next();
                } else {
                    break;
                }
            }
            if !name.is_empty() {
                if let Some(val) = env_lookup(&name) {
                    buf.push_str(&val);
                    continue;
                }
                buf.push('$');
                buf.push_str(&name);
                continue;
            }
            buf.push('$');
            continue;
        }
        buf.push(c);
    }
    buf
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

/// Walk every `Relation::ConflictsWhenBothInPath` and fire a
/// `Kind::Conflict` diagnostic when at least two of its declared
/// `sources` have matching PATH entries.
///
/// `groups[i]` lists the PATH indices matching the i-th source in
/// the relation's `sources` array. Sources with no matches still
/// occupy a slot in `groups` (as an empty Vec) so consumers can
/// align groups with the relation's sources by position.
///
/// Pure: every PATH lookup goes through `source_match::find` which
/// itself is pure. Diagnostics are anchored at the first index of
/// the first non-empty group for sort stability.
fn add_relation_conflict_diagnostics(
    normalized: &[String],
    raw: &[String],
    sources: &BTreeMap<String, SourceDef>,
    relations: &[Relation],
    os: Os,
    out: &mut Vec<Diagnostic>,
) {
    // 0.0.18: walk conflicts via RelationIndex so this call site
    // does not destructure the Relation enum.
    let index = crate::catalog::RelationIndex::from_slice(relations);
    for (src_names, diagnostic) in index.iter_conflicts() {
        let groups: Vec<Vec<usize>> = src_names
            .iter()
            .map(|name| matched_entries_for_source(name, normalized, sources, os))
            .collect();
        let active = groups.iter().filter(|g| !g.is_empty()).count();
        if active < 2 {
            continue;
        }
        let anchor = groups
            .iter()
            .find_map(|g| g.first().copied())
            .expect("at least two groups are non-empty");
        out.push(Diagnostic {
            index: anchor,
            entry: raw[anchor].clone(),
            severity: Severity::Warn,
            kind: Kind::Conflict {
                diagnostic: diagnostic.to_string(),
                groups,
            },
        });
    }
}

/// Indices of PATH entries (in normalized form) that the named
/// source matches under `os`. Pure: filters by
/// `source_match::find` against a single-source catalog so the
/// boundary check stays consistent with the rest of the
/// codebase. Returns an empty Vec when the source name is not in
/// the catalog or the source has no per-OS path on this OS.
fn matched_entries_for_source(
    source_name: &str,
    normalized: &[String],
    sources: &BTreeMap<String, SourceDef>,
    os: Os,
) -> Vec<usize> {
    let Some(def) = sources.get(source_name) else {
        return Vec::new();
    };
    let mut single = BTreeMap::new();
    single.insert(source_name.to_string(), def.clone());
    normalized
        .iter()
        .enumerate()
        .filter_map(|(i, n)| {
            let hit = source_match::find(n, &single, os);
            if hit.is_empty() { None } else { Some(i) }
        })
        .collect()
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

    fn empty_sources() -> BTreeMap<String, SourceDef> {
        BTreeMap::new()
    }

    fn unix_source(path: &str) -> SourceDef {
        SourceDef {
            unix: Some(path.into()),
            ..Default::default()
        }
    }

    /// Re-state the built-in mise relations + sources so each
    /// doctor test stays self-contained. Production wiring uses
    /// `catalog::merge_with_user_relations(&cfg.relations)`.
    ///
    /// The path uses an absolute literal (not `$HOME/...`) because
    /// `source_match::find` calls `expand_and_normalize` which reads
    /// the real process env, and unit tests cannot inject `HOME`
    /// without `std::env::set_var` (which would race with parallel
    /// tests). Production sources do use `$HOME` via the embedded
    /// catalog; that path is exercised by `tests/doctor.rs`.
    fn mise_sources_and_relations() -> (BTreeMap<String, SourceDef>, Vec<Relation>) {
        let mut sources = BTreeMap::new();
        sources.insert(
            "mise_shims".into(),
            unix_source("/home/u/.local/share/mise/shims"),
        );
        sources.insert(
            "mise_installs".into(),
            unix_source("/home/u/.local/share/mise/installs"),
        );
        let relations = vec![Relation::ConflictsWhenBothInPath {
            sources: vec!["mise_shims".into(), "mise_installs".into()],
            diagnostic: "mise_activate_both".into(),
        }];
        (sources, relations)
    }

    /// Helper: just the relations from `mise_sources_and_relations`.
    /// Tests that don't need to drive PATH-matching use this with
    /// `empty_sources()` so the relation walker walks but finds
    /// nothing — appropriate for tests that exercise other detectors.
    fn mise_relations() -> Vec<Relation> {
        mise_sources_and_relations().1
    }

    #[test]
    fn duplicate_detected_on_normalized_form() {
        let e = entries(&["/usr/bin", "/usr/local/bin", "/usr/bin"]);
        let diags = analyze(
            &e,
            &empty_sources(),
            &mise_relations(),
            Os::Linux,
            fs_yes,
            env_none,
        );
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
        let diags = analyze(
            &e,
            &empty_sources(),
            &mise_relations(),
            Os::Linux,
            fs_no,
            env_none,
        );
        assert!(diags.iter().any(|d| matches!(d.kind, Kind::Missing)));
    }

    #[test]
    fn trailing_slash_detected_but_root_allowed() {
        let e = entries(&["/foo/", "/", "C:/"]);
        let diags = analyze(
            &e,
            &empty_sources(),
            &mise_relations(),
            Os::Linux,
            fs_yes,
            env_none,
        );
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
        let diags = analyze(
            &e,
            &empty_sources(),
            &mise_relations(),
            Os::Linux,
            fs_yes,
            env_none,
        );
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
            &empty_sources(),
            &mise_relations(),
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
        let diags = analyze(
            &e,
            &empty_sources(),
            &mise_relations(),
            Os::Linux,
            fs_yes,
            env_map(&[("HOME", "/home/u")]),
        );
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
        let diags = analyze(
            &e,
            &empty_sources(),
            &mise_relations(),
            Os::Linux,
            fs_yes,
            env_none,
        );
        let case: Vec<_> = diags
            .iter()
            .filter(|d| matches!(d.kind, Kind::CaseVariant { .. }))
            .collect();
        assert!(!case.is_empty(), "diags: {diags:?}");
    }

    #[test]
    fn empty_entries_are_silently_ignored() {
        let e = entries(&[""]);
        let diags = analyze(
            &e,
            &empty_sources(),
            &mise_relations(),
            Os::Linux,
            fs_yes,
            env_none,
        );
        // Empty entries are filtered upstream by `split_path`. If one
        // does sneak in, our checks must not blow up.
        let _ = kinds(&diags);
    }

    // ---- mise_activate_both Conflict diagnostic ----------------

    fn match_mise_activate_both(d: &Diagnostic) -> Option<(&Vec<usize>, &Vec<usize>)> {
        if let Kind::Conflict { diagnostic, groups } = &d.kind {
            if diagnostic == "mise_activate_both" && groups.len() == 2 {
                return Some((&groups[0], &groups[1]));
            }
        }
        None
    }

    #[test]
    fn mise_activate_both_fires_when_shim_and_install_coexist() {
        let e = entries(&[
            "/home/u/.local/share/mise/shims",
            "/home/u/.local/share/mise/installs/python/3.14/bin",
            "/usr/bin",
        ]);
        let (sources, relations) = mise_sources_and_relations();
        let diags = analyze(&e, &sources, &relations, Os::Linux, fs_yes, env_none);
        let mab: Vec<_> = diags.iter().filter_map(match_mise_activate_both).collect();
        assert_eq!(mab.len(), 1);
        let (shims, installs) = mab[0];
        assert_eq!(shims, &vec![0]);
        assert_eq!(installs, &vec![1]);
    }

    #[test]
    fn mise_activate_both_does_not_fire_when_only_shims_present() {
        let e = entries(&["/home/u/.local/share/mise/shims", "/usr/bin"]);
        let (sources, relations) = mise_sources_and_relations();
        let diags = analyze(&e, &sources, &relations, Os::Linux, fs_yes, env_none);
        assert!(
            diags
                .iter()
                .filter_map(match_mise_activate_both)
                .next()
                .is_none()
        );
    }

    #[test]
    fn mise_activate_both_does_not_fire_when_only_installs_present() {
        let e = entries(&[
            "/home/u/.local/share/mise/installs/python/3.14/bin",
            "/usr/bin",
        ]);
        let (sources, relations) = mise_sources_and_relations();
        let diags = analyze(&e, &sources, &relations, Os::Linux, fs_yes, env_none);
        assert!(
            diags
                .iter()
                .filter_map(match_mise_activate_both)
                .next()
                .is_none()
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
        let (sources, relations) = mise_sources_and_relations();
        let diags = analyze(&e, &sources, &relations, Os::Linux, fs_yes, env_none);
        let (shims, installs) = diags
            .iter()
            .filter_map(match_mise_activate_both)
            .next()
            .expect("mise_activate_both must fire");
        assert_eq!(shims, &vec![0]);
        assert_eq!(installs, &vec![1, 2]);
    }

    #[test]
    fn conflict_with_fragment_needle_source() {
        // Fragment needles like `Microsoft/WindowsApps` are an
        // intentional built-in shape (see source_match::find): a
        // source can target a path *fragment* rather than a full
        // anchored prefix. The relation walker must treat such a
        // source the same as any other when assembling
        // `ConflictsWhenBothInPath` groups, otherwise a hostile
        // PATH that intersperses the Microsoft Store stub with a
        // peer source would slip past the doctor.
        let e = entries(&[
            "C:/Users/u/AppData/Local/Microsoft/WindowsApps",
            "C:/peer/dir",
            "C:/Windows/System32",
        ]);
        let mut sources = BTreeMap::new();
        sources.insert(
            "windows_apps".into(),
            SourceDef {
                windows: Some("Microsoft/WindowsApps".into()),
                ..Default::default()
            },
        );
        sources.insert(
            "peer".into(),
            SourceDef {
                windows: Some("C:/peer/dir".into()),
                ..Default::default()
            },
        );
        let relations = vec![Relation::ConflictsWhenBothInPath {
            sources: vec!["windows_apps".into(), "peer".into()],
            diagnostic: "store_vs_peer".into(),
        }];
        let diags = analyze(&e, &sources, &relations, Os::Windows, fs_yes, env_none);
        let groups = diags
            .iter()
            .find_map(|d| match &d.kind {
                Kind::Conflict { diagnostic, groups } if diagnostic == "store_vs_peer" => {
                    Some(groups.clone())
                }
                _ => None,
            })
            .expect("store_vs_peer must fire for fragment-needle source");
        assert_eq!(groups, vec![vec![0], vec![1]]);
    }

    #[test]
    fn user_defined_three_way_conflict_fires() {
        // Verifies the relation-driven generality: a user-supplied
        // ConflictsWhenBothInPath with three sources detects all
        // three-way overlaps, not just mise.
        let e = entries(&["/foo/a", "/foo/b", "/foo/c", "/usr/bin"]);
        let mut sources = BTreeMap::new();
        sources.insert("a".into(), unix_source("/foo/a"));
        sources.insert("b".into(), unix_source("/foo/b"));
        sources.insert("c".into(), unix_source("/foo/c"));
        let relations = vec![Relation::ConflictsWhenBothInPath {
            sources: vec!["a".into(), "b".into(), "c".into()],
            diagnostic: "abc_overlap".into(),
        }];
        let diags = analyze(&e, &sources, &relations, Os::Linux, fs_yes, env_none);
        let groups = diags
            .iter()
            .find_map(|d| match &d.kind {
                Kind::Conflict { diagnostic, groups } if diagnostic == "abc_overlap" => {
                    Some(groups.clone())
                }
                _ => None,
            })
            .expect("abc_overlap must fire");
        assert_eq!(groups, vec![vec![0], vec![1], vec![2]]);
    }

    // ---- per-source missing required (0.0.18+) -------------------

    fn cat_local(entries: &[(&str, SourceDef)]) -> BTreeMap<String, SourceDef> {
        entries
            .iter()
            .map(|(n, d)| ((*n).to_string(), d.clone()))
            .collect()
    }

    #[test]
    fn per_source_missing_required_fires_when_declared_dir_does_not_exist() {
        // [source.cargo] unix = "/totally/missing/dir" → fs_no
        // forces fs_exists to return false, the new detector fires.
        let sources = cat_local(&[("cargo", unix_source("/totally/missing/dir"))]);
        let diags = analyze(&[], &sources, &[], Os::Linux, fs_no, env_none);
        let hit = diags
            .iter()
            .find(|d| matches!(d.kind, Kind::PerSourceMissingRequired { .. }))
            .expect("PerSourceMissingRequired must fire");
        match &hit.kind {
            Kind::PerSourceMissingRequired { source } => assert_eq!(source, "cargo"),
            other => panic!("expected PerSourceMissingRequired, got {other:?}"),
        }
        assert_eq!(hit.severity, Severity::Warn);
    }

    #[test]
    fn per_source_missing_required_does_not_fire_when_path_exists() {
        // fs_yes claims every path exists → no hit.
        let sources = cat_local(&[("cargo", unix_source("/home/u/.cargo/bin"))]);
        let diags = analyze(&[], &sources, &[], Os::Linux, fs_yes, env_none);
        assert!(
            diags
                .iter()
                .all(|d| !matches!(d.kind, Kind::PerSourceMissingRequired { .. }))
        );
    }

    #[test]
    fn per_source_missing_required_skips_sources_without_path_for_current_os() {
        // Source defined only for windows → on Linux it has no
        // applicable path and must be silently skipped.
        let sources = cat_local(&[(
            "winget",
            SourceDef {
                windows: Some("C:/Users/u/AppData/Local/Microsoft/WinGet/Links".into()),
                ..Default::default()
            },
        )]);
        let diags = analyze(&[], &sources, &[], Os::Linux, fs_no, env_none);
        assert!(
            diags
                .iter()
                .all(|d| !matches!(d.kind, Kind::PerSourceMissingRequired { .. }))
        );
    }

    #[test]
    fn per_source_missing_required_expands_env_via_injected_lookup() {
        // $HOME is provided via env_map; resolve to /tmp/no_such_path
        // and force fs_exists=false → fire.
        let sources = cat_local(&[("cargo", unix_source("$HOME/.cargo/bin"))]);
        let env = env_map(&[("HOME", "/tmp/no_such_path")]);
        let diags = analyze(&[], &sources, &[], Os::Linux, fs_no, env);
        assert!(diags.iter().any(
            |d| matches!(&d.kind, Kind::PerSourceMissingRequired { source } if source == "cargo")
        ));
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
        let names: Vec<&str> = kept.iter().map(|d| kind_name(&d.kind)).collect();
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
        assert!(validate_filter_names(&f, &[]).is_ok());
    }

    #[test]
    fn validate_filter_names_rejects_typo() {
        let f = Filter {
            include: vec!["duplicat".into()],
            exclude: vec![],
        };
        let err = validate_filter_names(&f, &[]).unwrap_err();
        assert!(err.contains("duplicat"));
        assert!(err.contains("duplicate"), "valid list must be listed");
    }

    #[test]
    fn validate_checks_exclude_too() {
        let f = Filter {
            include: vec![],
            exclude: vec!["nope".into()],
        };
        assert!(validate_filter_names(&f, &[]).is_err());
    }

    #[test]
    fn validate_filter_names_accepts_user_defined_diagnostic() {
        // 0.0.13: user-declared `[[relation]] kind =
        // "conflicts_when_both_in_path" diagnostic = "foo_overlap"`
        // surfaces "foo_overlap" as a valid filter name.
        let f = Filter {
            include: vec!["foo_overlap".into()],
            exclude: vec![],
        };
        let extra = vec!["foo_overlap".to_string()];
        assert!(validate_filter_names(&f, &extra).is_ok());
    }

    #[test]
    fn user_diagnostic_names_collects_only_conflict_kinds() {
        let relations = vec![
            Relation::AliasOf {
                parent: "p".into(),
                children: vec!["c".into()],
            },
            Relation::ConflictsWhenBothInPath {
                sources: vec!["a".into(), "b".into()],
                diagnostic: "ab_overlap".into(),
            },
            Relation::DependsOn {
                source: "x".into(),
                target: "y".into(),
            },
        ];
        let names = user_diagnostic_names(&relations);
        assert_eq!(names, vec!["ab_overlap".to_string()]);
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
