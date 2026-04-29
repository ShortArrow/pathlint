//! Resolve a command name against a PATH string, mirroring shell
//! lookup semantics:
//!
//! * Windows: try the bare name then each `PATHEXT` extension; the
//!   match is whichever file exists first across PATH entries (left
//!   wins).
//! * Unix: file must be regular and executable (mode bit), no
//!   extension probing.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Resolution {
    pub full_path: PathBuf,
}

/// Split a `PATH` string on the platform's separator.
pub fn split_path(path_value: &str) -> Vec<String> {
    let sep = if cfg!(windows) { ';' } else { ':' };
    path_value
        .split(sep)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Resolve `command` against the given PATH entries. Returns the
/// first matching full path, or `None`.
pub fn resolve(command: &str, path_entries: &[String]) -> Option<Resolution> {
    let exts = pathext_list();
    for entry in path_entries {
        let dir = Path::new(entry);
        if !dir.is_dir() {
            continue;
        }
        if let Some(found) = probe(dir, command, &exts) {
            return Some(Resolution { full_path: found });
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
    let raw = std::env::var("PATHEXT").unwrap_or_else(|_| {
        ".COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.JSE;.WSF;.WSH;.MSC".to_string()
    });
    raw.split(';')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
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
        assert_eq!(parts, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn missing_command_returns_none() {
        let dir = std::env::temp_dir();
        let result = resolve(
            "pathlint_definitely_no_such_command_xyz",
            &[dir.to_string_lossy().into_owned()],
        );
        assert!(result.is_none());
    }
}
