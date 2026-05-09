//! Acquire the PATH for a chosen `--target` and split it into
//! [`PathEntry`] values. The boundary that captures the raw / expanded
//! duality of every entry: this module is the only place that turns
//! a string into a `PathEntry`, so detectors and resolvers downstream
//! never have to ask "is this already expanded?" at runtime.
//!
//! * `process` — `getenv("PATH")` on every OS.
//! * `user` — `HKCU\Environment\Path` on Windows; warn and fall back
//!   to `process` on Unix.
//! * `machine` — `HKLM\System\CurrentControlSet\Control\Session Manager\Environment\Path`
//!   on Windows; warn and fall back on Unix.
//!
//! On Windows, registry values may be `REG_EXPAND_SZ` (containing
//! `%LocalAppData%`-style references the OS expands at use) or
//! `REG_SZ` (literal). The default `winreg::RegKey::get_value::<String, _>`
//! call silently expands `REG_EXPAND_SZ` via
//! `ExpandEnvironmentStringsW`, which would feed downstream detectors
//! a string the user never typed. We instead read the raw bytes via
//! `get_raw_value`, decode them ourselves, and let `PathEntry::from_raw`
//! run `expand::expand_env` once — so every platform follows the same
//! "raw at the source, expanded at the boundary" rule.

use crate::path_entry::PathEntry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Process,
    User,
    Machine,
}

#[derive(Debug)]
pub struct PathRead {
    pub entries: Vec<PathEntry>,
    pub warning: Option<String>,
}

pub fn read_path(target: Target) -> PathRead {
    match target {
        Target::Process => PathRead {
            entries: split_into_entries(&std::env::var("PATH").unwrap_or_default()),
            warning: None,
        },
        Target::User => read_registry(target),
        Target::Machine => read_registry(target),
    }
}

/// Split a raw PATH string on the platform's separator and lift each
/// entry into a [`PathEntry`]. Empty entries are dropped — they are
/// the result of `::` / `;;` artefacts in the source, not genuine
/// PATH directories.
///
/// `path_source` is the infrastructure boundary, so this is one of
/// the two places in the lib that reads `std::env::var` (the other
/// is `resolve::split_path`). Every other caller of
/// `PathEntry::from_raw` injects a deterministic closure.
pub(crate) fn split_into_entries(s: &str) -> Vec<PathEntry> {
    let sep = if cfg!(windows) { ';' } else { ':' };
    s.split(sep)
        .filter(|x| !x.is_empty())
        .map(|raw| PathEntry::from_raw(raw, |v| std::env::var(v).ok()))
        .collect()
}

#[cfg(windows)]
fn read_registry(target: Target) -> PathRead {
    use winreg::RegKey;
    use winreg::enums::*;

    let (root, subkey) = match target {
        Target::User => (RegKey::predef(HKEY_CURRENT_USER), "Environment"),
        Target::Machine => (
            RegKey::predef(HKEY_LOCAL_MACHINE),
            r"System\CurrentControlSet\Control\Session Manager\Environment",
        ),
        Target::Process => unreachable!(),
    };

    let key = match root.open_subkey(subkey) {
        Ok(k) => k,
        Err(e) => {
            return PathRead {
                entries: Vec::new(),
                warning: Some(format!("could not open registry key: {e}")),
            };
        }
    };

    // get_raw_value returns the bytes + REG_SZ / REG_EXPAND_SZ tag.
    // We decode UTF-16 LE ourselves and intentionally do *not* call
    // ExpandEnvironmentStrings — expand happens later in
    // PathEntry::from_raw via the same expand_env we use on Linux /
    // macOS, so behaviour is platform-uniform and the raw form is
    // preserved for detectors that care (Shortenable).
    let raw_value = match key.get_raw_value("Path") {
        Ok(v) => v,
        Err(e) => {
            return PathRead {
                entries: Vec::new(),
                warning: Some(format!("could not read Path value: {e}")),
            };
        }
    };

    match decode_reg_string(&raw_value) {
        Ok(raw_string) => PathRead {
            entries: split_into_entries(&raw_string),
            warning: None,
        },
        Err(reason) => PathRead {
            entries: Vec::new(),
            warning: Some(format!("registry Path is not a valid string ({reason})")),
        },
    }
}

#[cfg(not(windows))]
fn read_registry(target: Target) -> PathRead {
    let label = match target {
        Target::User => "user",
        Target::Machine => "machine",
        Target::Process => unreachable!(),
    };
    PathRead {
        entries: split_into_entries(&std::env::var("PATH").unwrap_or_default()),
        warning: Some(format!(
            "--target {label} is Windows-only; falling back to process PATH"
        )),
    }
}

/// Decode a `REG_SZ` / `REG_EXPAND_SZ` registry payload as UTF-16 LE,
/// trimming the trailing NUL terminator. Lossy on invalid surrogate
/// pairs (replacement char for the bad code unit) — registry strings
/// are usually well-formed, but we never panic on a hostile value.
/// Other registry types (`REG_MULTI_SZ`, `REG_BINARY`, `REG_DWORD`,
/// …) return `Err` so the caller can warn and fall back to an empty
/// PATH instead of silently feeding garbage diagnostics.
///
/// Pure: takes a `RegValue`, returns the decoded `String`. Does not
/// touch the registry, the filesystem, or the process environment.
#[cfg(windows)]
pub(crate) fn decode_reg_string(v: &winreg::RegValue) -> Result<String, &'static str> {
    use winreg::enums::RegType;
    match v.vtype {
        RegType::REG_SZ | RegType::REG_EXPAND_SZ => {
            // Bytes are UTF-16 LE; pair them up. An odd byte count
            // means a malformed payload — be defensive and reject.
            if v.bytes.len() % 2 != 0 {
                return Err("UTF-16 byte stream has odd length");
            }
            let units: Vec<u16> = v
                .bytes
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            // Trailing NUL terminator(s) — registry strings store at
            // least one, sometimes more from sloppy writers.
            let trimmed: &[u16] = match units.iter().position(|&u| u == 0) {
                Some(idx) => &units[..idx],
                None => &units[..],
            };
            Ok(String::from_utf16_lossy(trimmed))
        }
        _ => Err("unexpected registry value type"),
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use winreg::RegValue;
    use winreg::enums::RegType;

    /// Build a `RegValue` that mimics what `RegQueryValueEx` would
    /// return for the given UTF-16 string and registry type. Adds a
    /// trailing NUL so the decoder's NUL-trim path is exercised.
    fn reg_value(s: &str, vtype: RegType) -> RegValue {
        let mut units: Vec<u16> = s.encode_utf16().collect();
        units.push(0);
        let bytes: Vec<u8> = units.iter().flat_map(|u| u.to_le_bytes()).collect();
        RegValue { bytes, vtype }
    }

    #[test]
    fn decode_reg_string_keeps_percent_var_for_reg_expand_sz() {
        // REG_EXPAND_SZ values from a real registry contain
        // `%LocalAppData%`-style placeholders. The decoder must
        // return them verbatim — expand happens later, in
        // PathEntry::from_raw.
        let v = reg_value(
            r"%LocalAppData%\Microsoft\WindowsApps",
            RegType::REG_EXPAND_SZ,
        );
        let decoded = decode_reg_string(&v).expect("REG_EXPAND_SZ decode");
        assert_eq!(decoded, r"%LocalAppData%\Microsoft\WindowsApps");
    }

    #[test]
    fn decode_reg_string_handles_reg_sz_literal() {
        let v = reg_value(r"C:\Program Files\PowerShell\7", RegType::REG_SZ);
        let decoded = decode_reg_string(&v).expect("REG_SZ decode");
        assert_eq!(decoded, r"C:\Program Files\PowerShell\7");
    }

    #[test]
    fn decode_reg_string_rejects_unsupported_reg_type() {
        // A REG_DWORD payload would naively decode to garbage as
        // UTF-16; the explicit type guard rejects it instead.
        let v = RegValue {
            bytes: vec![0, 0, 0, 0],
            vtype: RegType::REG_DWORD,
        };
        let err = decode_reg_string(&v).unwrap_err();
        assert!(err.contains("unexpected"), "err was: {err}");
    }

    #[test]
    fn decode_reg_string_rejects_odd_byte_length() {
        // Defensive: a malformed payload with odd length cannot be a
        // valid UTF-16 LE string. Reject rather than panic on chunks.
        let v = RegValue {
            bytes: vec![b'A', b'B', b'C'],
            vtype: RegType::REG_SZ,
        };
        let err = decode_reg_string(&v).unwrap_err();
        assert!(err.contains("odd length"), "err was: {err}");
    }
}
