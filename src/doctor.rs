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
//!
//! # Examples
//!
//! ```
//! use pathlint::{Attribution, doctor};
//! use pathlint::config::Config;
//! use pathlint::os_detect::Os;
//! use pathlint::path_entry::PathEntry;
//!
//! let null_env = |_: &str| -> Option<String> { None };
//! let entries = vec![
//!     Attribution::new(PathEntry::from_raw("/usr/bin", null_env)),
//!     Attribution::new(PathEntry::from_raw("/usr/bin", null_env)), // duplicate
//! ];
//! let cfg = Config::default();
//! let sources = pathlint::catalog::merge_with_user(&cfg.source);
//! let relations = pathlint::catalog::merge_with_user_relations(&cfg.relations);
//! let deps = doctor::AnalyzeDeps {
//!     common: pathlint::CommonDeps { env_lookup: Box::new(|_v: &str| -> Option<String> { None }) },
//!     fs_exists: Box::new(|_: &str| true),
//!     fs_list_dir: Box::new(|_: &str| Vec::new()),
//!     is_writable_dir: Box::new(|_: &str| false),
//! };
//! let diags = doctor::analyze(&entries, &sources, &relations, Os::Linux, deps);
//! assert!(diags.iter().any(|d| matches!(d.kind, doctor::Kind::Duplicate { .. })));
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::path::Path;

use crate::Attribution;
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

/// Dependency carrier for [`analyze`]. Groups the four closures
/// that the detector pipeline injects (filesystem existence check,
/// env oracle via the embedded [`CommonDeps`], directory listing,
/// writable-directory check) so adding a new detector becomes an
/// additive field on this struct rather than a `pub fn analyze`
/// signature change.
///
/// Production wiring uses [`AnalyzeDeps::production`]. Embedders
/// and tests construct the struct directly with deterministic
/// closures.
///
/// `Fn(&str) -> bool` predicate, type-erased through a trait
/// object. Shared shape for [`AnalyzeDeps::fs_exists`] and
/// [`AnalyzeDeps::is_writable_dir`]. 0.0.27+.
pub type FsBoolFn<'a> = Box<dyn Fn(&str) -> bool + 'a>;

/// `Fn(&str) -> Vec<String>` directory lister. Shape for
/// [`AnalyzeDeps::fs_list_dir`]. 0.0.27+.
pub type FsListDirFn<'a> = Box<dyn Fn(&str) -> Vec<String> + 'a>;

/// 0.0.27+.
pub struct AnalyzeDeps<'a> {
    /// Shared env oracle. See [`crate::CommonDeps`].
    pub common: crate::CommonDeps<'a>,
    /// Existence check for the `Missing` detector. Production
    /// wiring uses [`fs_exists_real`]; tests pass deterministic
    /// stubs via `Box::new(|_| true)` etc.
    pub fs_exists: FsBoolFn<'a>,
    /// Directory listing for the `DuplicateButShadowed` detector.
    /// Production wiring uses [`fs_list_dir_real`].
    pub fs_list_dir: FsListDirFn<'a>,
    /// Writable-by-others probe for the `WriteablePathDir`
    /// detector. Production wiring uses [`is_writable_dir_real`].
    pub is_writable_dir: FsBoolFn<'a>,
}

impl AnalyzeDeps<'static> {
    /// Production wiring: each closure is the corresponding
    /// `_real` helper at the top of this module.
    pub fn production() -> Self {
        AnalyzeDeps {
            common: crate::CommonDeps::production(),
            fs_exists: Box::new(fs_exists_real),
            fs_list_dir: Box::new(fs_list_dir_real),
            is_writable_dir: Box::new(is_writable_dir_real),
        }
    }
}

/// Convenience: production wiring of `analyze` that uses the real
/// filesystem and process env. `sources` and `relations` come from
/// the merged catalog; relation-driven conflict diagnostics
/// (`Relation::ConflictsWhenBothInPath`) fire from this set.
pub fn analyze_real(
    entries: &[Attribution],
    sources: &BTreeMap<String, SourceDef>,
    relations: &[Relation],
    os: Os,
) -> Vec<Diagnostic> {
    analyze(entries, sources, relations, os, AnalyzeDeps::production())
}

/// Production wrapper for the `is_writable_dir` closure on
/// `analyze`. Returns `true` when the directory at `path` is
/// writable by users other than the owner — Unix checks the
/// others-write bit (`mode & 0o002`); Windows queries the DACL
/// for the "Everyone" SID's effective `FILE_GENERIC_WRITE` /
/// `FILE_APPEND_DATA`. Permission errors, missing dirs, files
/// that aren't directories, and any winapi failure all return
/// `false` (skip — the dedicated `Missing` detector covers
/// nonexistence). Used by the 0.0.21 `WriteablePathDir` detector.
#[cfg(unix)]
pub fn is_writable_dir_real(path: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.is_dir() && m.permissions().mode() & 0o002 != 0)
        .unwrap_or(false)
}

/// Windows variant of [`is_writable_dir_real`]. Reads the DACL
/// via `GetNamedSecurityInfoW`, asks `GetEffectiveRightsFromAclW`
/// for the rights granted to the well-known "Everyone" SID
/// (`S-1-1-0`), and returns `true` iff `FILE_GENERIC_WRITE` or
/// `FILE_APPEND_DATA` is set. Any winapi failure returns `false`.
#[cfg(windows)]
pub fn is_writable_dir_real(path: &str) -> bool {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{ERROR_SUCCESS, LocalFree};
    use windows_sys::Win32::Security::Authorization::{
        GetEffectiveRightsFromAclW, GetNamedSecurityInfoW, SE_FILE_OBJECT, TRUSTEE_IS_SID,
        TRUSTEE_IS_WELL_KNOWN_GROUP, TRUSTEE_W,
    };
    use windows_sys::Win32::Security::{
        ACL, AllocateAndInitializeSid, DACL_SECURITY_INFORMATION, FreeSid, PSECURITY_DESCRIPTOR,
        SID_IDENTIFIER_AUTHORITY,
    };
    use windows_sys::Win32::Storage::FileSystem::{FILE_APPEND_DATA, FILE_GENERIC_WRITE};
    use windows_sys::Win32::System::SystemServices::{
        DOMAIN_ALIAS_RID_USERS, SECURITY_AUTHENTICATED_USER_RID, SECURITY_BUILTIN_DOMAIN_RID,
        SECURITY_WORLD_RID,
    };

    // Confirm the path is a directory before consulting ACLs;
    // metadata() also captures the "doesn't exist" case which
    // belongs to the Missing detector, not this one.
    match std::fs::metadata(path) {
        Ok(m) if m.is_dir() => {}
        _ => return false,
    }

    // SIDs to test against the directory's effective rights. A real
    // dotfiles host on Windows almost never grants Everyone write,
    // but routinely grants BUILTIN\Users (and Authenticated Users
    // by inheritance) write — so checking Everyone alone is the
    // canonical false-negative trap. Probe all three; short-circuit
    // on the first SID that gets effective write.
    //
    // Layout: (auth_value, sub_authority_count, sub_authorities[8]).
    // Only the first `count` sub-authorities are read by
    // AllocateAndInitializeSid; the trailing zeros are filler so we
    // can keep one fixed signature for the loop.
    const SIDS_TO_CHECK: &[([u8; 6], u8, [u32; 8])] = &[
        // Everyone (S-1-1-0)
        (
            [0, 0, 0, 0, 0, 1],
            1,
            [SECURITY_WORLD_RID as u32, 0, 0, 0, 0, 0, 0, 0],
        ),
        // Authenticated Users (S-1-5-11)
        (
            [0, 0, 0, 0, 0, 5],
            1,
            [SECURITY_AUTHENTICATED_USER_RID as u32, 0, 0, 0, 0, 0, 0, 0],
        ),
        // BUILTIN\Users (S-1-5-32-545)
        (
            [0, 0, 0, 0, 0, 5],
            2,
            [
                SECURITY_BUILTIN_DOMAIN_RID as u32,
                DOMAIN_ALIAS_RID_USERS as u32,
                0,
                0,
                0,
                0,
                0,
                0,
            ],
        ),
    ];

    let wide: Vec<u16> = OsStr::new(path).encode_wide().chain([0]).collect();

    let mut dacl: *mut ACL = std::ptr::null_mut();
    let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    let result = unsafe {
        GetNamedSecurityInfoW(
            wide.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut dacl,
            std::ptr::null_mut(),
            &mut sd,
        )
    };
    if result != ERROR_SUCCESS || dacl.is_null() {
        if !sd.is_null() {
            unsafe { LocalFree(sd as _) };
        }
        return false;
    }

    let mut writable = false;
    for (auth_value, count, sub_auths) in SIDS_TO_CHECK {
        let auth = SID_IDENTIFIER_AUTHORITY { Value: *auth_value };
        let mut sid: *mut core::ffi::c_void = std::ptr::null_mut();
        let alloc_ok = unsafe {
            AllocateAndInitializeSid(
                &auth,
                *count,
                sub_auths[0],
                sub_auths[1],
                sub_auths[2],
                sub_auths[3],
                sub_auths[4],
                sub_auths[5],
                sub_auths[6],
                sub_auths[7],
                &mut sid,
            )
        };
        if alloc_ok == 0 || sid.is_null() {
            continue;
        }

        let mut trustee: TRUSTEE_W = unsafe { std::mem::zeroed() };
        trustee.TrusteeForm = TRUSTEE_IS_SID;
        trustee.TrusteeType = TRUSTEE_IS_WELL_KNOWN_GROUP;
        trustee.ptstrName = sid as *mut _;

        let mut rights: u32 = 0;
        let eff_ok = unsafe { GetEffectiveRightsFromAclW(dacl, &trustee, &mut rights) };

        unsafe { FreeSid(sid) };

        if eff_ok == ERROR_SUCCESS && rights & (FILE_GENERIC_WRITE | FILE_APPEND_DATA) != 0 {
            writable = true;
            break;
        }
    }

    unsafe { LocalFree(sd as _) };
    writable
}

/// Real-world `fs_list_dir` for `analyze`: enumerates basenames of
/// regular executable files in `path`. Unix requires `chmod +x`
/// (any of the user/group/other exec bits). Windows accepts any
/// regular file — extension-based filtering happens in the
/// detector via PATHEXT. Permission-denied / non-existent dirs
/// return an empty `Vec`.
pub fn fs_list_dir_real(path: &str) -> Vec<String> {
    let Ok(read) = std::fs::read_dir(path) else {
        return Vec::new();
    };
    read.filter_map(|entry| {
        let entry = entry.ok()?;
        let meta = entry.metadata().ok()?;
        if !meta.is_file() {
            return None;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if meta.permissions().mode() & 0o111 == 0 {
                return None;
            }
        }
        entry.file_name().into_string().ok()
    })
    .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Informational; does not affect exit code. 0.0.34+ for the
    /// `config_not_found` selfcheck (running pathlint without a
    /// config is legitimate).
    Info,
    Warn,
    Error,
}

/// One PATH-hygiene problem reported by `pathlint doctor`.
/// See PRD §7.7 for the full list of diagnostic kinds.
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
    /// Same command basename exists as a real executable in two or
    /// more PATH dirs. The earlier dir wins (`Diagnostic.index`);
    /// later dirs are shadowed (`shadowed_indexes`). Reported
    /// regardless of whether the dirs are named sources — duplicates
    /// are facts. Pairs with the relation-driven `Conflict` detector
    /// (e.g. `mise_activate_both`): that one covers the
    /// named-source-pair angle, this one covers the unnamed
    /// command-name angle. Windows compares case-insensitively after
    /// stripping PATHEXT extensions, so `python.exe` and `python.bat`
    /// count as the same command. 0.0.19+.
    DuplicateButShadowed {
        command: String,
        shadowed_indexes: Vec<usize>,
    },
    /// PATH entry resolves to a relative path (`.`, `./bin`, bare
    /// `bin`, …). The shell resolves these against the current
    /// working directory at command-invocation time, so the binary
    /// that runs depends on where the user happens to be — almost
    /// always a security or portability footgun. Env vars are
    /// expanded before the check, so `$HOME/bin` does not fire when
    /// `HOME` is set; an unresolved `$VAR/bin` stays verbatim and
    /// fires (it is itself a config bug). 0.0.20+.
    RelativePathEntry,
    /// PATH entry resolves to a directory that is writable by users
    /// other than the owner. An attacker with shell access could
    /// drop a malicious binary that the user runs by typing a
    /// common command name. On Unix, "writable by others" means the
    /// others-write bit (`mode & 0o002`) is set. On Windows, the
    /// directory's DACL grants the "Everyone" SID effective
    /// `FILE_GENERIC_WRITE` or `FILE_APPEND_DATA`. Env vars are
    /// expanded first; missing or unreadable directories are
    /// skipped (the `Missing` detector covers them). 0.0.21+.
    WriteablePathDir,
    /// Selfcheck: the running pathlint binary's absolute path was
    /// not found anywhere on PATH. Either invoked by absolute path
    /// or PATH was unset. 0.0.34+.
    BinaryNotInPath {
        running: String,
    },
    /// Selfcheck: a `pathlint.toml` was located but failed to parse.
    /// `reason` carries the parser's error string. 0.0.34+.
    ConfigParseError {
        config_path: String,
        reason: String,
    },
    /// Selfcheck: no `pathlint.toml` found via the standard search
    /// (cwd then `$XDG_CONFIG_HOME`). Info severity because running
    /// without a config is legitimate. 0.0.34+.
    ConfigNotFound,
    /// Selfcheck: a required env var (`PATH`, `PATHEXT` on Windows,
    /// `HOME`/`USERPROFILE`) returned `None`. `entry` field carries
    /// the var name. 0.0.34+.
    EnvLookupFailed {
        var: String,
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
        Kind::DuplicateButShadowed { .. } => "duplicate_but_shadowed",
        Kind::RelativePathEntry => "relative_path_entry",
        Kind::WriteablePathDir => "writeable_path_dir",
        Kind::BinaryNotInPath { .. } => "binary_not_in_path",
        Kind::ConfigParseError { .. } => "config_parse_error",
        Kind::ConfigNotFound => "config_not_found",
        Kind::EnvLookupFailed { .. } => "env_lookup_failed",
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
        "duplicate_but_shadowed",
        "relative_path_entry",
        "writeable_path_dir",
        "binary_not_in_path",
        "config_parse_error",
        "config_not_found",
        "env_lookup_failed",
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
                "unknown lint kind `{name}`; valid values: {}",
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
pub fn analyze(
    entries: &[Attribution],
    sources: &BTreeMap<String, SourceDef>,
    relations: &[Relation],
    os: Os,
    deps: AnalyzeDeps<'_>,
) -> Vec<Diagnostic> {
    let AnalyzeDeps {
        common,
        fs_exists,
        fs_list_dir,
        is_writable_dir,
    } = deps;
    let env_lookup = common.env_lookup;
    let mut out = Vec::new();
    for (i, entry) in entries.iter().enumerate() {
        // User-intent detectors read `effective_raw_for_user_intent`
        // so a Windows process entry whose registry form is `%VAR%`
        // (carried in `provenance_raw` by the path_source reconciler)
        // is treated as the user's authored form, not the
        // OS-expanded literal `getenv` returned.
        let effective_raw = entry.effective_raw_for_user_intent();
        // Malformed reads the raw form: NUL bytes / illegal characters
        // are user-input concerns, not filesystem concerns.
        if let Some(d) = check_malformed(i, effective_raw) {
            out.push(d);
            // If the entry is malformed, skip the other checks for it
            // — they're going to be noisy or wrong.
            continue;
        }
        // Missing reads the expanded form: the filesystem doesn't
        // know what `%LocalAppData%` means.
        if let Some(d) = check_missing(i, entry, &fs_exists) {
            out.push(d);
        }
        // TrailingSlash reads the raw form: the user wrote it.
        if let Some(d) = check_trailing_slash(i, effective_raw) {
            out.push(d);
        }
        if os == Os::Windows {
            // ShortName reads the raw form: 8.3 short-name segments
            // appear in raw paths if anywhere.
            if let Some(d) = check_short_name(i, effective_raw) {
                out.push(d);
            }
        }
        // Shortenable reads the raw form: the whole point is to
        // detect entries the user wrote in *un*-shortened form.
        // Reading the expanded form would chase the registry's
        // auto-expansion of REG_EXPAND_SZ and falsely suggest
        // shortening an entry the user already shortened.
        if let Some(d) = check_shortenable(i, entry, os, &env_lookup) {
            out.push(d);
        }
    }
    // Pair-wise checks need every entry's normalized form. Normalising
    // the expanded view collapses `%LocalAppData%` and the underlying
    // `C:\Users\...` to the same key so duplicate / case-variant
    // detection treats them as the same directory.
    let normalized: Vec<String> = entries
        .iter()
        .map(|e| expand::normalize(&e.observed.expanded))
        .collect();
    add_duplicate_diagnostics(&normalized, entries, &mut out);
    add_case_variant_diagnostics(entries, &mut out);
    add_relation_conflict_diagnostics(
        &normalized,
        entries,
        sources,
        relations,
        os,
        &env_lookup,
        &mut out,
    );
    add_per_source_missing_required_diagnostics(sources, os, &fs_exists, &env_lookup, &mut out);
    add_duplicate_but_shadowed_diagnostics(entries, os, &fs_list_dir, &env_lookup, &mut out);
    add_relative_path_entry_diagnostics(entries, os, &mut out);
    add_writeable_path_dir_diagnostics(entries, &is_writable_dir, &mut out);
    out
}

/// Flag PATH entries whose env-expanded form is still a relative
/// path. Shells resolve relative entries against the cwd at command
/// invocation time, so the binary that runs depends on where the
/// user happens to be — almost always a security or portability
/// footgun. Empty entries are left to `Malformed` to flag (or
/// quietly skip, depending on the previous detectors); this one
/// only fires on a non-empty post-expansion relative path.
///
/// Reads `entry.observed.expanded`, which `PathEntry::from_raw` already
/// computed via the caller-supplied env lookup at the
/// `path_source` boundary. Unresolved `$VAR` references stay
/// verbatim in `expanded`, which means `$VAR/bin` is treated as
/// relative and fires — that itself is a config bug worth
/// surfacing.
///
/// "Absolute" depends on the target OS, not the host running
/// pathlint: `/usr/bin` is absolute on Linux but relative on
/// Windows (no drive letter), and `C:\Users\u\bin` is absolute on
/// Windows but a bare relative name on Unix.
fn add_relative_path_entry_diagnostics(entries: &[Attribution], os: Os, out: &mut Vec<Diagnostic>) {
    for (i, entry) in entries.iter().enumerate() {
        if entry.observed.expanded.is_empty() {
            continue;
        }
        if !is_absolute_for_os(&entry.observed.expanded, os) {
            out.push(Diagnostic {
                index: i,
                entry: entry.effective_raw_for_user_intent().to_string(),
                severity: Severity::Warn,
                kind: Kind::RelativePathEntry,
            });
        }
    }
}

/// Flag PATH entries whose env-expanded form points at a directory
/// writable by users other than the owner. The shell does not care
/// who can write to a PATH dir, so an attacker with shell access can
/// drop a malicious binary that the user runs by typing a common
/// command name. Production wires `is_writable_dir_real` (Unix
/// `mode & 0o002`; Windows DACL "Everyone" effective write) into
/// the closure; tests pass deterministic stubs. Empty entries skip
/// silently — `Malformed` already covers them — and missing dirs
/// fall through to `Missing`. 0.0.21+.
fn add_writeable_path_dir_diagnostics<W>(
    entries: &[Attribution],
    is_writable_dir: &W,
    out: &mut Vec<Diagnostic>,
) where
    W: Fn(&str) -> bool,
{
    for (i, entry) in entries.iter().enumerate() {
        if entry.observed.expanded.is_empty() {
            continue;
        }
        if is_writable_dir(&entry.observed.expanded) {
            out.push(Diagnostic {
                index: i,
                entry: entry.effective_raw_for_user_intent().to_string(),
                severity: Severity::Warn,
                kind: Kind::WriteablePathDir,
            });
        }
    }
}

/// Cross-OS "absolute path?" check. `Path::is_absolute()` would
/// answer for the host (the machine running pathlint), but a PATH
/// entry meant for a different OS needs to be judged by that OS's
/// rules. Windows: `C:\` / `C:/` drive-letter + separator, or
/// `\\server\share` UNC. Unix-family (Linux/macOS/Termux): leading
/// `/`. Everything else is treated as relative.
fn is_absolute_for_os(s: &str, os: Os) -> bool {
    match os {
        Os::Windows => {
            let bytes = s.as_bytes();
            if bytes.len() >= 3
                && bytes[0].is_ascii_alphabetic()
                && bytes[1] == b':'
                && (bytes[2] == b'/' || bytes[2] == b'\\')
            {
                return true;
            }
            s.starts_with("\\\\") || s.starts_with("//")
        }
        Os::Macos | Os::Linux | Os::Termux => s.starts_with('/'),
    }
}

/// Detect commands whose basename appears as a real executable in
/// two or more PATH dirs. The earliest dir wins; all later ones
/// are shadowed. Pairs with the relation-driven `Conflict` detector
/// (`mise_activate_both` etc.) — that handles the named-source-pair
/// angle, this handles the unnamed command-name angle. Reported
/// regardless of named relations: duplicates are facts the user
/// should know about. Suppress per host with
/// `--exclude duplicate_but_shadowed`.
fn add_duplicate_but_shadowed_diagnostics<L, V>(
    entries: &[Attribution],
    os: Os,
    fs_list_dir: &L,
    env_lookup: &V,
    out: &mut Vec<Diagnostic>,
) where
    L: Fn(&str) -> Vec<String>,
    V: Fn(&str) -> Option<String>,
{
    let pathext_lower: Vec<String> = if os == Os::Windows {
        expand::pathext_lower(|v| env_lookup(v))
    } else {
        Vec::new()
    };

    // command basename -> sorted unique PATH indices
    let mut by_command: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    for (i, entry) in entries.iter().enumerate() {
        // The expanded form is what `read_dir` needs; case + slash
        // style stay verbatim there so case-sensitive filesystems
        // (Linux) can still find the directory.
        if entry.observed.expanded.is_empty() {
            continue;
        }
        for file in fs_list_dir(&entry.observed.expanded) {
            let Some(cmd) = normalize_command(&file, os, &pathext_lower) else {
                continue;
            };
            by_command.entry(cmd).or_default().insert(i);
        }
    }
    for (cmd, indexes) in by_command {
        if indexes.len() < 2 {
            continue;
        }
        let sorted: Vec<usize> = indexes.into_iter().collect();
        let winning = sorted[0];
        let shadowed_indexes = sorted[1..].to_vec();
        out.push(Diagnostic {
            index: winning,
            entry: entries[winning].effective_raw_for_user_intent().to_string(),
            severity: Severity::Warn,
            kind: Kind::DuplicateButShadowed {
                command: cmd,
                shadowed_indexes,
            },
        });
    }
}

/// Map a directory entry name to a normalised command name for
/// duplicate-but-shadowed comparison. Windows: lowercase, then
/// strip the trailing PATHEXT extension if present (returns `None`
/// for files whose extension is not in PATHEXT — those are not
/// runnable commands as far as the shell is concerned). Unix: pass
/// through verbatim, since any executable file is a runnable
/// command regardless of name.
fn normalize_command(file: &str, os: Os, pathext_lower: &[String]) -> Option<String> {
    if os == Os::Windows {
        let lower = file.to_ascii_lowercase();
        for ext in pathext_lower {
            if let Some(stripped) = lower.strip_suffix(ext) {
                return Some(stripped.to_string());
            }
        }
        None
    } else {
        Some(file.to_string())
    }
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

/// Fires when the directory the entry points at does not exist.
/// Reads `entry.observed.expanded` because the filesystem cannot evaluate
/// `%LocalAppData%` etc. — that work was done once at the
/// `path_source` boundary by `PathEntry::from_raw`. The
/// user-facing `Diagnostic.entry` uses
/// [`PathEntry::effective_raw_for_user_intent`] so the report
/// matches what the user authored (registry `%VAR%` form on
/// Windows process target, observed raw elsewhere).
fn check_missing<F>(index: usize, entry: &Attribution, fs_exists: &F) -> Option<Diagnostic>
where
    F: Fn(&str) -> bool,
{
    if fs_exists(&entry.observed.expanded) {
        return None;
    }
    Some(Diagnostic {
        index,
        entry: entry.effective_raw_for_user_intent().to_string(),
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

/// Suggest shortening a literal path the user could write more
/// portably with `%LocalAppData%` / `$HOME` / etc. Reads
/// [`PathEntry::effective_raw_for_user_intent`] (not `expanded`)
/// so an entry the user already wrote in `%VAR%`-form is left
/// alone — the whole point of the detector is "you typed a long
/// literal, would `%VAR%` be shorter?", not "expand-then-suggest".
/// A future change that tries to read the expanded form would
/// re-introduce the 0.0.22 regression where Windows registry
/// `REG_EXPAND_SZ` entries (always pre-expanded by
/// `winreg::get_value`) tripped the suggestion despite the user
/// already having shortened them.
///
/// 0.0.24 extends the same rule across the process / registry
/// boundary: when `--target process` runs on Windows, the OS
/// hands us literals via `getenv`, but the `path_source`
/// reconciler can recover the original `%VAR%` form via
/// `provenance_raw`. The accessor returns provenance when
/// present, so this detector skips registry-authored entries
/// even though `raw` itself is a literal.
fn check_shortenable<V>(
    index: usize,
    entry: &Attribution,
    os: Os,
    env_lookup: &V,
) -> Option<Diagnostic>
where
    V: Fn(&str) -> Option<String>,
{
    let effective_raw = entry.effective_raw_for_user_intent();
    // Skip if the user already wrote a variable form. We read the
    // *effective raw* (provenance overlay or observed raw) here on
    // purpose: pre-expanded literals whose registry form is `%VAR%`
    // must be left alone.
    if effective_raw.contains('%') || effective_raw.contains('$') {
        return None;
    }
    // Match on normalized form (lowercased + slash-unified) but reuse
    // the effective raw's tail so the suggestion preserves the user's
    // capitalization and slash style.
    let normalized_entry = expand::normalize(effective_raw);
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
        // change. Cut the same number of bytes off the effective raw.
        let suffix = effective_raw.get(normalized_var.len()..).unwrap_or("");
        let suggestion = match prefer_style {
            VarStyle::Percent => format!("%{var}%{suffix}"),
            VarStyle::Dollar => format!("${var}{suffix}"),
        };
        return Some(Diagnostic {
            index,
            entry: effective_raw.to_string(),
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

fn add_duplicate_diagnostics(
    normalized: &[String],
    entries: &[Attribution],
    out: &mut Vec<Diagnostic>,
) {
    let mut first_seen: BTreeMap<&str, usize> = BTreeMap::new();
    for (i, n) in normalized.iter().enumerate() {
        if n.is_empty() {
            continue;
        }
        if let Some(&first) = first_seen.get(n.as_str()) {
            out.push(Diagnostic {
                index: i,
                entry: entries[i].effective_raw_for_user_intent().to_string(),
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
    entries: &[Attribution],
    sources: &BTreeMap<String, SourceDef>,
    relations: &[Relation],
    os: Os,
    env_lookup: &dyn Fn(&str) -> Option<String>,
    out: &mut Vec<Diagnostic>,
) {
    // 0.0.18: walk conflicts via RelationIndex so this call site
    // does not destructure the Relation enum.
    let index = crate::catalog::RelationIndex::from_slice(relations);
    for (src_names, diagnostic) in index.iter_conflicts() {
        let groups: Vec<Vec<usize>> = src_names
            .iter()
            .map(|name| matched_entries_for_source(name, normalized, sources, os, env_lookup))
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
            entry: entries[anchor].effective_raw_for_user_intent().to_string(),
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
    env_lookup: &dyn Fn(&str) -> Option<String>,
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
            let hit = source_match::find_with(n, &single, os, env_lookup);
            if hit.is_empty() { None } else { Some(i) }
        })
        .collect()
}

fn add_case_variant_diagnostics(entries: &[Attribution], out: &mut Vec<Diagnostic>) {
    // Two PATH entries can have identical normalized form but differ
    // verbatim (case difference, mixed slashes). The plain Duplicate
    // diagnostic already covers exact-string duplicates; this one
    // catches "looks the same to the OS, looks different in the
    // file" cases so the user can decide whether to canonicalize.
    let mut buckets: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, entry) in entries.iter().enumerate() {
        let key = expand::normalize(&entry.observed.expanded);
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
            // Compare on the *observed* raw, not the user-intent
            // accessor: two entries that differ verbatim in `raw`
            // but happen to share a registry overlay are still
            // distinct as the user wrote them.
            if entries[i].observed.raw == entries[first].observed.raw {
                continue;
            }
            out.push(Diagnostic {
                index: i,
                entry: entries[i].effective_raw_for_user_intent().to_string(),
                severity: Severity::Warn,
                kind: Kind::CaseVariant {
                    canonical: entries[first].effective_raw_for_user_intent().to_string(),
                },
            });
        }
    }
}

/// Selfcheck: does pathlint itself work in this environment?
///
/// Three checks (0.0.34+):
/// 1. Binary self-locate — is the running pathlint binary on PATH?
/// 2. `pathlint.toml` discovery + parse — the caller resolves the
///    config location through whichever helper every other subcommand
///    uses (`locate_rules` in the CLI; embedders may pass an explicit
///    path or `None`). `selfcheck` only acts on the located path.
/// 3. `env_lookup` operational — `PATH`, plus `PATHEXT` on Windows
///    and `HOME`/`USERPROFILE` for config search.
///
/// Each diagnostic uses `index = usize::MAX, entry = ""` because
/// selfcheck findings are not bound to a PATH entry. The CLI
/// formatter renders them via `format::selfcheck_line`.
///
/// Arguments:
/// * `located_config` — the path the caller's discovery helper found
///   (or `None` if no config was located).
/// * `explicit_requested` — true if the user passed `--config` at
///   the CLI; used to differentiate "no config sought, none found"
///   (info) from "user asked for a config we couldn't locate"
///   (error from the CLI layer before selfcheck is even called, so
///   this parameter only affects whether the info diagnostic fires).
pub fn selfcheck(located_config: Option<&Path>, explicit_requested: bool) -> Vec<Diagnostic> {
    let mut out = Vec::new();

    // 1. Binary self-locate.
    if let Ok(running) = std::env::current_exe() {
        let running_str = running.display().to_string();
        match std::env::var("PATH") {
            Ok(path_value) => {
                let sep = if cfg!(windows) { ';' } else { ':' };
                let on_path = path_value.split(sep).any(|dir| {
                    if dir.is_empty() {
                        return false;
                    }
                    let candidate = Path::new(dir).join(
                        running
                            .file_name()
                            .map(|f| f.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                    );
                    candidate.exists()
                });
                if !on_path {
                    out.push(Diagnostic {
                        index: usize::MAX,
                        entry: String::new(),
                        severity: Severity::Warn,
                        kind: Kind::BinaryNotInPath {
                            running: running_str,
                        },
                    });
                }
            }
            Err(_) => {
                out.push(Diagnostic {
                    index: usize::MAX,
                    entry: String::new(),
                    severity: Severity::Error,
                    kind: Kind::EnvLookupFailed {
                        var: "PATH".to_string(),
                    },
                });
            }
        }
    }

    // 2. pathlint.toml discovery + parse — the located config (if
    //    any) is what every other subcommand will read; just verify
    //    parse on the same file. config_not_found is info unless the
    //    user explicitly asked for one (in which case the CLI layer
    //    will have already errored out before reaching selfcheck).
    match located_config {
        Some(p) if p.exists() => {
            if let Err(e) = crate::config::Config::from_path(p) {
                out.push(Diagnostic {
                    index: usize::MAX,
                    entry: String::new(),
                    severity: Severity::Error,
                    kind: Kind::ConfigParseError {
                        config_path: p.display().to_string(),
                        reason: e.to_string(),
                    },
                });
            }
        }
        _ => {
            // Suppress the info diagnostic when the user explicitly
            // wanted a config and the CLI surfaced its own error
            // (avoids double-reporting).
            if !explicit_requested {
                out.push(Diagnostic {
                    index: usize::MAX,
                    entry: String::new(),
                    severity: Severity::Info,
                    kind: Kind::ConfigNotFound,
                });
            }
        }
    }

    // 3. env_lookup operational. PATH was already checked under (1).
    #[cfg(windows)]
    {
        if std::env::var("PATHEXT").is_err() {
            out.push(Diagnostic {
                index: usize::MAX,
                entry: String::new(),
                severity: Severity::Error,
                kind: Kind::EnvLookupFailed {
                    var: "PATHEXT".to_string(),
                },
            });
        }
        if std::env::var("USERPROFILE").is_err() {
            out.push(Diagnostic {
                index: usize::MAX,
                entry: String::new(),
                severity: Severity::Warn,
                kind: Kind::EnvLookupFailed {
                    var: "USERPROFILE".to_string(),
                },
            });
        }
    }
    #[cfg(not(windows))]
    {
        if std::env::var("HOME").is_err() {
            out.push(Diagnostic {
                index: usize::MAX,
                entry: String::new(),
                severity: Severity::Warn,
                kind: Kind::EnvLookupFailed {
                    var: "HOME".to_string(),
                },
            });
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommonDeps;
    use crate::path_entry::PathEntry;

    fn entries(strs: &[&str]) -> Vec<Attribution> {
        // Doctor unit tests that don't need env injection use the
        // null lookup so `$VAR` / `%VAR%` in test strings stays
        // verbatim. Tests that DO need a non-empty env build their
        // PathEntry directly with explicit raw + expanded fields,
        // which is more obvious than threading a closure here.
        strs.iter()
            .map(|s| Attribution::new(PathEntry::from_raw(*s, |_| -> Option<String> { None })))
            .collect()
    }

    /// Like `entries`, but each PATH entry is expanded via the live
    /// process environment. Used by the small handful of tests that
    /// stage a `std::env::set_var` and want the entry's `$VAR` to
    /// resolve through the same channel.
    fn entries_with_process_env(strs: &[&str]) -> Vec<Attribution> {
        strs.iter()
            .map(|s| Attribution::new(PathEntry::from_raw(*s, |v| env::var(v).ok())))
            .collect()
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
    fn fs_list_empty(_: &str) -> Vec<String> {
        Vec::new()
    }
    fn fs_list_map<'a>(pairs: &'a [(&'a str, &'a [&'a str])]) -> impl Fn(&str) -> Vec<String> + 'a {
        move |path| {
            pairs
                .iter()
                .find(|(p, _)| *p == path)
                .map(|(_, files)| files.iter().map(|s| (*s).to_string()).collect())
                .unwrap_or_default()
        }
    }
    fn fs_writable_no(_: &str) -> bool {
        false
    }
    #[allow(dead_code)]
    fn fs_writable_yes(_: &str) -> bool {
        true
    }
    fn fs_writable_map<'a>(pairs: &'a [(&'a str, bool)]) -> impl Fn(&str) -> bool + 'a {
        move |path| {
            pairs
                .iter()
                .find(|(p, _)| *p == path)
                .map(|(_, w)| *w)
                .unwrap_or(false)
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
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
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
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_no),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
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
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
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
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
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
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_map(&[("UserProfile", "C:\\Users\\Mixed")])),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
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
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_map(&[("HOME", "/home/u")])),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
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
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
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
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
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
        let diags = analyze(
            &e,
            &sources,
            &relations,
            Os::Linux,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
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
        let diags = analyze(
            &e,
            &sources,
            &relations,
            Os::Linux,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
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
        let diags = analyze(
            &e,
            &sources,
            &relations,
            Os::Linux,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
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
        let diags = analyze(
            &e,
            &sources,
            &relations,
            Os::Linux,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
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
        let diags = analyze(
            &e,
            &sources,
            &relations,
            Os::Windows,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
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
        let diags = analyze(
            &e,
            &sources,
            &relations,
            Os::Linux,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
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
        let diags = analyze(
            &[],
            &sources,
            &[],
            Os::Linux,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_no),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
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
        let diags = analyze(
            &[],
            &sources,
            &[],
            Os::Linux,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
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
        let diags = analyze(
            &[],
            &sources,
            &[],
            Os::Linux,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_no),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
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
        let diags = analyze(
            &[],
            &sources,
            &[],
            Os::Linux,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env),
                },
                fs_exists: Box::new(fs_no),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
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

    #[test]
    fn duplicate_but_shadowed_basic_unix() {
        let e = entries(&["/a", "/b"]);
        let listing: [(&str, &[&str]); 2] = [("/a", &["git"]), ("/b", &["git"])];
        let fs_list = fs_list_map(&listing);
        let diags = analyze(
            &e,
            &empty_sources(),
            &[],
            Os::Linux,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
        let dbs: Vec<&Diagnostic> = diags
            .iter()
            .filter(|d| matches!(d.kind, Kind::DuplicateButShadowed { .. }))
            .collect();
        assert_eq!(dbs.len(), 1, "exactly one shadow diagnostic expected");
        assert_eq!(dbs[0].index, 0, "winning entry is the earliest dir");
        assert_eq!(dbs[0].severity, Severity::Warn);
        match &dbs[0].kind {
            Kind::DuplicateButShadowed {
                command,
                shadowed_indexes,
            } => {
                assert_eq!(command, "git");
                assert_eq!(shadowed_indexes, &vec![1]);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn duplicate_but_shadowed_three_dirs_lists_all_shadowed() {
        let e = entries(&["/a", "/b", "/c"]);
        let listing: [(&str, &[&str]); 3] =
            [("/a", &["node"]), ("/b", &["node"]), ("/c", &["node"])];
        let fs_list = fs_list_map(&listing);
        let diags = analyze(
            &e,
            &empty_sources(),
            &[],
            Os::Linux,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
        let dbs: Vec<&Diagnostic> = diags
            .iter()
            .filter(|d| matches!(d.kind, Kind::DuplicateButShadowed { .. }))
            .collect();
        assert_eq!(dbs.len(), 1);
        assert_eq!(dbs[0].index, 0);
        match &dbs[0].kind {
            Kind::DuplicateButShadowed {
                shadowed_indexes, ..
            } => {
                assert_eq!(shadowed_indexes, &vec![1, 2]);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn duplicate_but_shadowed_case_insensitive_on_windows() {
        let e = entries(&["C:/a", "C:/b"]);
        let listing: [(&str, &[&str]); 2] = [("C:/a", &["Git.exe"]), ("C:/b", &["git.exe"])];
        let fs_list = fs_list_map(&listing);
        let env = env_map(&[("PATHEXT", ".EXE;.BAT;.CMD")]);
        let diags = analyze(
            &e,
            &empty_sources(),
            &[],
            Os::Windows,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
        let dbs: Vec<&Diagnostic> = diags
            .iter()
            .filter(|d| matches!(d.kind, Kind::DuplicateButShadowed { .. }))
            .collect();
        assert_eq!(dbs.len(), 1);
        match &dbs[0].kind {
            Kind::DuplicateButShadowed {
                command,
                shadowed_indexes,
            } => {
                assert_eq!(command, "git", "Git.exe and git.exe collapse to `git`");
                assert_eq!(shadowed_indexes, &vec![1]);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn duplicate_but_shadowed_pathext_strips_extension() {
        let e = entries(&["C:/a", "C:/b"]);
        let listing: [(&str, &[&str]); 2] = [("C:/a", &["python.exe"]), ("C:/b", &["python.bat"])];
        let fs_list = fs_list_map(&listing);
        let env = env_map(&[("PATHEXT", ".EXE;.BAT")]);
        let diags = analyze(
            &e,
            &empty_sources(),
            &[],
            Os::Windows,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
        let dbs: Vec<&Diagnostic> = diags
            .iter()
            .filter(|d| matches!(d.kind, Kind::DuplicateButShadowed { .. }))
            .collect();
        assert_eq!(
            dbs.len(),
            1,
            "python.exe and python.bat both strip to `python` and collide"
        );
        match &dbs[0].kind {
            Kind::DuplicateButShadowed { command, .. } => assert_eq!(command, "python"),
            _ => unreachable!(),
        }
    }

    #[test]
    fn duplicate_but_shadowed_no_fire_when_only_one_dir_has_command() {
        let e = entries(&["/a", "/b"]);
        let listing: [(&str, &[&str]); 2] = [("/a", &["rg"]), ("/b", &["fd"])];
        let fs_list = fs_list_map(&listing);
        let diags = analyze(
            &e,
            &empty_sources(),
            &[],
            Os::Linux,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
        let dbs: Vec<&Diagnostic> = diags
            .iter()
            .filter(|d| matches!(d.kind, Kind::DuplicateButShadowed { .. }))
            .collect();
        assert!(dbs.is_empty(), "no shadow when no command repeats");
    }

    fn relative_kinds(diags: &[Diagnostic]) -> Vec<&Diagnostic> {
        diags
            .iter()
            .filter(|d| matches!(d.kind, Kind::RelativePathEntry))
            .collect()
    }

    #[test]
    fn relative_path_entry_fires_on_dot() {
        let e = entries(&["."]);
        let diags = analyze(
            &e,
            &empty_sources(),
            &[],
            Os::Linux,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
        let hits = relative_kinds(&diags);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].index, 0);
        assert_eq!(hits[0].entry, ".");
        assert_eq!(hits[0].severity, Severity::Warn);
    }

    #[test]
    fn relative_path_entry_fires_on_relative_with_subpath() {
        let e = entries(&["./bin", "bin", "app/bin"]);
        let diags = analyze(
            &e,
            &empty_sources(),
            &[],
            Os::Linux,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
        let hits = relative_kinds(&diags);
        assert_eq!(hits.len(), 3, "all three relative entries fire");
    }

    #[test]
    fn relative_path_entry_no_fire_on_absolute_unix() {
        let e = entries(&["/usr/bin", "/home/u/.cargo/bin"]);
        let diags = analyze(
            &e,
            &empty_sources(),
            &[],
            Os::Linux,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
        assert!(relative_kinds(&diags).is_empty());
    }

    #[test]
    fn relative_path_entry_no_fire_after_env_expansion() {
        // SAFETY: PathEntry::from_raw reads process env when wired
        // with `entries_with_process_env`. Unique var name keeps
        // cross-test interference low.
        unsafe { env::set_var("PATHLINT_TEST_REL_HOME", "/home/u") };
        let e = entries_with_process_env(&["$PATHLINT_TEST_REL_HOME/bin"]);
        let diags = analyze(
            &e,
            &empty_sources(),
            &[],
            Os::Linux,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
        unsafe { env::remove_var("PATHLINT_TEST_REL_HOME") };
        assert!(
            relative_kinds(&diags).is_empty(),
            "env-expanded absolute path must not trigger relative_path_entry",
        );
    }

    #[test]
    fn relative_path_entry_fires_when_env_unresolved() {
        // Use a name that is exceedingly unlikely to be set in any
        // CI runner. expand_env keeps `$NAME` verbatim, so the entry
        // stays as `$PATHLINT_TEST_REL_UNDEFINED_XYZ/bin`, which is
        // a relative path and should fire.
        let e = entries(&["$PATHLINT_TEST_REL_UNDEFINED_XYZ/bin"]);
        let diags = analyze(
            &e,
            &empty_sources(),
            &[],
            Os::Linux,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
        let hits = relative_kinds(&diags);
        assert_eq!(
            hits.len(),
            1,
            "unresolved env var leaves the entry verbatim ($VAR/bin), \
             which is treated as relative for safety",
        );
    }

    fn writable_kinds(diags: &[Diagnostic]) -> Vec<&Diagnostic> {
        diags
            .iter()
            .filter(|d| matches!(d.kind, Kind::WriteablePathDir))
            .collect()
    }

    #[test]
    fn writeable_path_dir_fires_when_writable() {
        let e = entries(&["/usr/bin"]);
        let writable = fs_writable_map(&[("/usr/bin", true)]);
        let diags = analyze(
            &e,
            &empty_sources(),
            &[],
            Os::Linux,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(writable),
            },
        );
        let hits = writable_kinds(&diags);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].index, 0);
        assert_eq!(hits[0].entry, "/usr/bin");
        assert_eq!(hits[0].severity, Severity::Warn);
    }

    #[test]
    fn writeable_path_dir_no_fire_when_readonly() {
        let e = entries(&["/usr/bin", "/usr/local/bin"]);
        let diags = analyze(
            &e,
            &empty_sources(),
            &[],
            Os::Linux,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
        assert!(writable_kinds(&diags).is_empty());
    }

    #[test]
    fn writeable_path_dir_skips_empty_entry() {
        // Empty entry is handled by the malformed / shape stage; the
        // writeable detector must not double-report on it.
        let e = entries(&[""]);
        let writable = fs_writable_map(&[("", true)]);
        let diags = analyze(
            &e,
            &empty_sources(),
            &[],
            Os::Linux,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(writable),
            },
        );
        assert!(writable_kinds(&diags).is_empty());
    }

    #[test]
    fn writeable_path_dir_index_matches_entry_position() {
        // Three entries; only the second is writable. The diagnostic
        // index must point at #1, not #0 or #2.
        let e = entries(&["/usr/bin", "/tmp", "/usr/local/bin"]);
        let writable = fs_writable_map(&[("/tmp", true)]);
        let diags = analyze(
            &e,
            &empty_sources(),
            &[],
            Os::Linux,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(writable),
            },
        );
        let hits = writable_kinds(&diags);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].index, 1);
        assert_eq!(hits[0].entry, "/tmp");
    }

    #[test]
    fn writeable_path_dir_after_env_expansion() {
        // The detector is supposed to expand env vars before asking
        // whether the dir is writable. Use the same set_var + remove_var
        // dance the relative_path_entry test uses.
        unsafe { env::set_var("PATHLINT_TEST_WRITEABLE_DIR", "/srv/writable") };
        let e = entries_with_process_env(&["$PATHLINT_TEST_WRITEABLE_DIR"]);
        let writable = fs_writable_map(&[("/srv/writable", true)]);
        let diags = analyze(
            &e,
            &empty_sources(),
            &[],
            Os::Linux,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(writable),
            },
        );
        unsafe { env::remove_var("PATHLINT_TEST_WRITEABLE_DIR") };
        let hits = writable_kinds(&diags);
        assert_eq!(
            hits.len(),
            1,
            "expanded path is forwarded to is_writable_dir, so the env-var entry fires",
        );
    }

    // ---- 0.0.23: REG_EXPAND_SZ raw/expanded duality regression -----

    fn shortenable_kinds(diags: &[Diagnostic]) -> Vec<&Diagnostic> {
        diags
            .iter()
            .filter(|d| matches!(d.kind, Kind::Shortenable { .. }))
            .collect()
    }

    fn missing_kinds(diags: &[Diagnostic]) -> Vec<&Diagnostic> {
        diags.iter().filter(|d| d.kind == Kind::Missing).collect()
    }

    /// 0.0.23 regression gate: a Windows registry `REG_EXPAND_SZ`
    /// entry that the user already wrote in `%LocalAppData%\...` form
    /// must NOT trip the Shortenable detector. Pre-0.0.23, winreg's
    /// `get_value::<String, _>` auto-expanded REG_EXPAND_SZ via
    /// `ExpandEnvironmentStrings`, feeding `analyze` an already-
    /// expanded path; the detector's `entry.contains('%')` skip was
    /// never reached, and the user got told to "shorten" an entry
    /// they had already shortened. PathEntry preserves the raw form
    /// so this never fires for an entry whose raw side has `%VAR%`.
    #[test]
    fn shortenable_does_not_fire_for_already_shortened_entry() {
        let raw = r"%LocalAppData%\Microsoft\WindowsApps";
        let expanded = r"C:\Users\me\AppData\Local\Microsoft\WindowsApps";
        let pe = Attribution {
            observed: PathEntry {
                raw: raw.to_string(),
                expanded: expanded.to_string(),
            },
            provenance_raw: None,
        };
        let env = env_map(&[("LocalAppData", r"C:\Users\me\AppData\Local")]);
        let diags = analyze(
            &[pe],
            &empty_sources(),
            &[],
            Os::Windows,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
        let hits = shortenable_kinds(&diags);
        assert!(
            hits.is_empty(),
            "raw entry already uses %LocalAppData%; Shortenable must not fire. got: {hits:?}"
        );
    }

    /// Symmetric green case: a Windows entry stored verbatim as a
    /// fully-expanded path (REG_SZ in the registry, or any literal
    /// path on PATH) SHOULD still trip Shortenable when it lines up
    /// with a candidate var. Without this test the regression gate
    /// above could be satisfied by a detector that simply never
    /// fires.
    #[test]
    fn shortenable_still_fires_for_unshortened_entry() {
        let raw = r"C:\Users\me\AppData\Local\Microsoft\WindowsApps";
        let pe = Attribution::new(PathEntry::from_raw(raw, |_| -> Option<String> { None }));
        let env = env_map(&[("LocalAppData", r"C:\Users\me\AppData\Local")]);
        let diags = analyze(
            &[pe],
            &empty_sources(),
            &[],
            Os::Windows,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
        let hits = shortenable_kinds(&diags);
        assert_eq!(
            hits.len(),
            1,
            "literal path under %LocalAppData% should fire Shortenable. got: {hits:?}"
        );
    }

    /// Missing reads the *expanded* side: the filesystem doesn't know
    /// what `%LocalAppData%` means. Pin the contract by passing a
    /// PathEntry whose raw still has `%VAR%` and whose expanded
    /// resolves to a path the fs_exists stub claims doesn't exist.
    #[test]
    fn missing_uses_expanded_form_not_raw() {
        let pe = Attribution {
            observed: PathEntry {
                raw: r"%LocalAppData%\NopeDir".into(),
                expanded: r"C:\Users\me\AppData\Local\NopeDir".into(),
            },
            provenance_raw: None,
        };
        // fs_exists returns true only for the literal `%LocalAppData%`
        // string (which the fs would never see in practice). Expanded
        // path is unknown → Missing should fire.
        let fs_check = |p: &str| p.starts_with('%');
        let diags = analyze(
            &[pe],
            &empty_sources(),
            &[],
            Os::Windows,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_check),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
        let hits = missing_kinds(&diags);
        assert_eq!(
            hits.len(),
            1,
            "Missing must consult expanded; got: {hits:?}"
        );
        // The user-facing diagnostic preserves the raw form.
        assert_eq!(hits[0].entry, r"%LocalAppData%\NopeDir");
    }

    /// Diagnostic.entry must hold the raw text the user typed, not
    /// the post-expansion form. Otherwise the user looks at the
    /// doctor output and can't recognise the entry they put in their
    /// shell rc / registry.
    #[test]
    fn diagnostic_entry_text_is_raw_form() {
        let pe = Attribution {
            observed: PathEntry {
                raw: r"%LocalAppData%\Some\WhereThatDoesNotExist".into(),
                expanded: r"C:\Users\me\AppData\Local\Some\WhereThatDoesNotExist".into(),
            },
            provenance_raw: None,
        };
        let diags = analyze(
            &[pe],
            &empty_sources(),
            &[],
            Os::Windows,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_no), // force Missing
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
        let hits = missing_kinds(&diags);
        assert_eq!(hits.len(), 1);
        assert!(
            hits[0].entry.contains('%'),
            "Diagnostic.entry must keep the raw `%VAR%` form; got: {:?}",
            hits[0].entry
        );
    }

    // -- 0.0.24: provenance overlay (Windows process target) --------

    /// 0.0.24 regression gate: even when the entry's `raw` is the
    /// fully-expanded literal that the OS handed us through
    /// `getenv("PATH")`, an attached provenance (= the original
    /// registry `%LocalAppData%\...` form) must suppress Shortenable.
    /// The reconciler in `path_source` is responsible for setting
    /// that provenance; this test pins the detector contract.
    #[test]
    fn shortenable_skipped_when_provenance_has_percent_var() {
        let pe = Attribution {
            observed: PathEntry {
                raw: r"C:\Users\me\AppData\Local\Microsoft\WindowsApps".into(),
                expanded: r"C:\Users\me\AppData\Local\Microsoft\WindowsApps".into(),
            },
            provenance_raw: Some(r"%LocalAppData%\Microsoft\WindowsApps".into()),
        };
        let env = env_map(&[("LocalAppData", r"C:\Users\me\AppData\Local")]);
        let diags = analyze(
            &[pe],
            &empty_sources(),
            &[],
            Os::Windows,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
        let hits = shortenable_kinds(&diags);
        assert!(
            hits.is_empty(),
            "provenance carries %LocalAppData%; Shortenable must not fire. got: {hits:?}"
        );
    }

    /// 0.0.24: when a diagnostic does fire on a process entry that
    /// has a provenance overlay, the user-facing `entry` text must
    /// be the registry raw form (`%LocalAppData%\...`), not the
    /// OS-expanded literal. Otherwise the user can't connect what
    /// pathlint says with what they see in `regedit`.
    #[test]
    fn diagnostic_entry_text_prefers_provenance_over_raw() {
        // Force `Missing` to fire on the entry so something goes into
        // `Diagnostic.entry`. The entry is missing on disk, but its
        // raw side is the OS-expanded literal and provenance carries
        // the registry `%VAR%` form.
        let pe = Attribution {
            observed: PathEntry {
                raw: r"C:\Users\me\AppData\Local\Vanished".into(),
                expanded: r"C:\Users\me\AppData\Local\Vanished".into(),
            },
            provenance_raw: Some(r"%LocalAppData%\Vanished".into()),
        };
        let diags = analyze(
            &[pe],
            &empty_sources(),
            &[],
            Os::Windows,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env_none),
                },
                fs_exists: Box::new(fs_no),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
        let hits = missing_kinds(&diags);
        assert_eq!(hits.len(), 1);
        assert_eq!(
            hits[0].entry, r"%LocalAppData%\Vanished",
            "Diagnostic.entry must reflect provenance when present; got: {:?}",
            hits[0].entry
        );
    }

    /// 0.0.24 anti-regression: an entry without provenance and whose
    /// raw is the OS-expanded literal SHOULD still trip Shortenable.
    /// Without this, the new "skip when provenance has %VAR%" rule
    /// could be silently widened into "skip whenever raw is a
    /// literal", which is wrong.
    #[test]
    fn shortenable_still_fires_when_provenance_is_none_and_raw_is_literal() {
        let pe = Attribution {
            observed: PathEntry {
                raw: r"C:\Users\me\AppData\Local\Microsoft\WindowsApps".into(),
                expanded: r"C:\Users\me\AppData\Local\Microsoft\WindowsApps".into(),
            },
            provenance_raw: None,
        };
        let env = env_map(&[("LocalAppData", r"C:\Users\me\AppData\Local")]);
        let diags = analyze(
            &[pe],
            &empty_sources(),
            &[],
            Os::Windows,
            AnalyzeDeps {
                common: CommonDeps {
                    env_lookup: Box::new(env),
                },
                fs_exists: Box::new(fs_yes),
                fs_list_dir: Box::new(fs_list_empty),
                is_writable_dir: Box::new(fs_writable_no),
            },
        );
        let hits = shortenable_kinds(&diags);
        assert_eq!(
            hits.len(),
            1,
            "no provenance overlay → literal path should still fire Shortenable. got: {hits:?}"
        );
    }
}
