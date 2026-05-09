//! Resolve a command name against a PATH string, mirroring shell
//! lookup semantics:
//!
//! * Windows: try the bare name then each `PATHEXT` extension; the
//!   match is whichever file exists first across PATH entries (left
//!   wins).
//! * Unix: file must be regular and executable (mode bit), no
//!   extension probing.

use std::path::{Path, PathBuf};

use crate::path_entry::PathEntry;

/// Split a `PATH` string on the platform's separator. Each entry is
/// lifted into a [`PathEntry`] so the caller never juggles raw vs
/// expanded forms manually.
pub fn split_path(path_value: &str) -> Vec<PathEntry> {
    let sep = if cfg!(windows) { ';' } else { ':' };
    path_value
        .split(sep)
        .filter(|s| !s.is_empty())
        .map(PathEntry::from_raw)
        .collect()
}

/// Resolve `command` against the given PATH entries. Returns the
/// first matching full path, or `None`.
///
/// Reads `entry.expanded` because the filesystem cannot evaluate
/// `%LocalAppData%` etc. — `PathEntry::from_raw` (computed once at
/// the `path_source` boundary) already ran `expand_env`.
///
/// 0.0.16: dropped the `Resolution { full_path }` wrapper —
/// `PathBuf` carries the same information and lets the public
/// surface (`lint::evaluate`, `trace::locate`) accept resolver
/// closures expressed in standard-library types alone.
/// 0.0.23: lifted to `&[PathEntry]` so callers no longer pass raw
/// strings whose expansion state is implicit.
pub fn resolve(command: &str, path_entries: &[PathEntry]) -> Option<PathBuf> {
    let exts = pathext_list();
    for entry in path_entries {
        let dir = Path::new(&entry.expanded);
        if !dir.is_dir() {
            continue;
        }
        if let Some(found) = probe(dir, command, &exts) {
            return Some(found);
        }
    }
    None
}

fn probe(dir: &Path, command: &str, exts: &[String]) -> Option<PathBuf> {
    let already_has_ext = command.contains('.');

    if cfg!(windows) {
        if already_has_ext {
            let candidate = dir.join(command);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        for ext in exts {
            let mut name = command.to_string();
            if !ext.is_empty() {
                name.push_str(ext);
            }
            let candidate = dir.join(&name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    } else {
        let candidate = dir.join(command);
        if is_executable_file(&candidate) {
            Some(candidate)
        } else {
            None
        }
    }
}

#[cfg(windows)]
fn pathext_list() -> Vec<String> {
    crate::expand::pathext_raw(|v| std::env::var(v).ok())
}

#[cfg(not(windows))]
fn pathext_list() -> Vec<String> {
    Vec::new()
}

#[cfg(unix)]
fn is_executable_file(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::metadata(p) {
        Ok(md) => md.is_file() && (md.permissions().mode() & 0o111) != 0,
        Err(_) => false,
    }
}

#[cfg(not(any(unix, windows)))]
fn is_executable_file(p: &Path) -> bool {
    p.is_file()
}

#[cfg(windows)]
#[allow(dead_code)]
fn is_executable_file(p: &Path) -> bool {
    p.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_path_handles_empty_entries() {
        let sep = if cfg!(windows) { ';' } else { ':' };
        let s = format!("a{sep}{sep}b");
        let parts = split_path(&s);
        let raw: Vec<&str> = parts.iter().map(|e| e.raw.as_str()).collect();
        assert_eq!(raw, vec!["a", "b"]);
    }

    #[test]
    fn missing_command_returns_none() {
        let dir = std::env::temp_dir();
        let entry = PathEntry::from_raw(dir.to_string_lossy().into_owned());
        let result = resolve("pathlint_definitely_no_such_command_xyz", &[entry]);
        assert!(result.is_none());
    }
}
