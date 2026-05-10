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

use crate::expand;
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
        Target::Process => read_process(),
        Target::User => read_registry(target),
        Target::Machine => read_registry(target),
    }
}

#[cfg(not(windows))]
fn read_process() -> PathRead {
    PathRead {
        entries: split_into_entries(&std::env::var("PATH").unwrap_or_default()),
        warning: None,
    }
}

/// Windows: read the process PATH, then overlay HKCU / HKLM raw
/// forms onto the entries whose `expanded` matches a registry
/// entry. The OS expands `REG_EXPAND_SZ` before handing PATH to a
/// child process, so `getenv` gives detectors a literal even when
/// the user wrote `%LocalAppData%\...` in `regedit`. The reconciler
/// restores user intent without changing what `--target` means.
///
/// Registry read failures (key missing, decode error, unsupported
/// type) downgrade to a `warning` and skip the overlay; the process
/// observed PATH is still returned so doctor / sort / where stay
/// usable.
#[cfg(windows)]
fn read_process() -> PathRead {
    let process = split_into_entries(&std::env::var("PATH").unwrap_or_default());
    let user_reg = read_registry(Target::User);
    let machine_reg = read_registry(Target::Machine);
    let entries =
        reconcile_process_with_registry(&process, &user_reg.entries, &machine_reg.entries);

    // Bubble up registry warnings under a clear prefix so the user
    // can tell process-target ran but the overlay was incomplete.
    let mut warnings: Vec<String> = Vec::new();
    if let Some(w) = user_reg.warning {
        warnings.push(format!("user-registry overlay: {w}"));
    }
    if let Some(w) = machine_reg.warning {
        warnings.push(format!("machine-registry overlay: {w}"));
    }
    let warning = if warnings.is_empty() {
        None
    } else {
        Some(warnings.join("; "))
    };

    PathRead { entries, warning }
}

/// Pure overlay: for each `process` entry, find the first registry
/// entry whose `expand::normalize`d expanded matches, prefer HKCU
/// over HKLM, and attach its `raw` as `provenance_raw` only when
/// it differs from the process raw (REG_SZ entries don't need
/// overlays). When no match is found, leave `provenance_raw` as
/// `None` — codex's safety rule: false-negative is preferable to
/// false suppression.
///
/// Pure: no I/O, no allocation beyond the cloned `Vec<PathEntry>`.
/// Lives outside `cfg(windows)` so it can be unit-tested on every
/// platform; only the `read_process` Windows branch calls it. The
/// `allow(dead_code)` is needed for non-Windows lib builds where
/// the call site is gated out — the function is still exercised by
/// the cross-platform `overlay_tests` module.
#[allow(dead_code)]
pub(crate) fn reconcile_process_with_registry(
    process: &[PathEntry],
    user_reg: &[PathEntry],
    machine_reg: &[PathEntry],
) -> Vec<PathEntry> {
    process
        .iter()
        .map(|p| {
            let candidate =
                find_expanded_match(p, user_reg).or_else(|| find_expanded_match(p, machine_reg));
            match candidate {
                Some(reg_raw) if reg_raw != p.raw => p.clone().with_provenance(reg_raw),
                _ => p.clone(),
            }
        })
        .collect()
}

#[allow(dead_code)]
fn find_expanded_match(p: &PathEntry, reg: &[PathEntry]) -> Option<String> {
    let key = expand::normalize(&p.expanded);
    reg.iter()
        .find(|r| expand::normalize(&r.expanded) == key)
        .map(|r| r.raw.clone())
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

#[cfg(test)]
mod overlay_tests {
    //! 0.0.24: provenance overlay is a pure operation over
    //! `Vec<PathEntry>` slices, so the reconciler is tested on every
    //! OS. The Windows-only registry I/O that *feeds* the reconciler
    //! is exercised separately in the `tests` module below (gated on
    //! `cfg(all(test, windows))`).

    use super::*;

    fn raw_entry(raw: &str, expanded: &str) -> PathEntry {
        PathEntry {
            raw: raw.into(),
            expanded: expanded.into(),
            provenance_raw: None,
        }
    }

    #[test]
    fn reconcile_overlays_user_provenance_when_expanded_matches() {
        // process side: OS handed us an expanded literal.
        let process = vec![raw_entry(
            r"C:\Users\me\AppData\Local\Microsoft\WindowsApps",
            r"C:\Users\me\AppData\Local\Microsoft\WindowsApps",
        )];
        // user registry: same expanded path, but raw is `%VAR%`.
        let user_reg = vec![raw_entry(
            r"%LocalAppData%\Microsoft\WindowsApps",
            r"C:\Users\me\AppData\Local\Microsoft\WindowsApps",
        )];
        let machine_reg: Vec<PathEntry> = Vec::new();

        let out = reconcile_process_with_registry(&process, &user_reg, &machine_reg);
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].provenance_raw.as_deref(),
            Some(r"%LocalAppData%\Microsoft\WindowsApps"),
        );
    }

    #[test]
    fn reconcile_prefers_user_over_machine_when_both_match() {
        // codex decision rule: HKCU before HKLM.
        let process = vec![raw_entry(
            r"C:\Users\me\AppData\Local\Microsoft\WindowsApps",
            r"C:\Users\me\AppData\Local\Microsoft\WindowsApps",
        )];
        let user_reg = vec![raw_entry(
            r"%LocalAppData%\Microsoft\WindowsApps",
            r"C:\Users\me\AppData\Local\Microsoft\WindowsApps",
        )];
        let machine_reg = vec![raw_entry(
            r"%MACHINE_VAR%\Microsoft\WindowsApps",
            r"C:\Users\me\AppData\Local\Microsoft\WindowsApps",
        )];

        let out = reconcile_process_with_registry(&process, &user_reg, &machine_reg);
        assert_eq!(
            out[0].provenance_raw.as_deref(),
            Some(r"%LocalAppData%\Microsoft\WindowsApps"),
            "HKCU must win over HKLM when expanded matches both",
        );
    }

    #[test]
    fn reconcile_skips_when_process_expanded_has_no_registry_match() {
        // Process entry that's not in either registry source (e.g.
        // injected by `set PATH=...` at runtime, or registry was
        // mutated since session start). Overlay must not fire.
        let process = vec![raw_entry(
            r"C:\runtime\injected\bin",
            r"C:\runtime\injected\bin",
        )];
        let user_reg = vec![raw_entry(
            r"%LocalAppData%\Microsoft\WindowsApps",
            r"C:\Users\me\AppData\Local\Microsoft\WindowsApps",
        )];
        let machine_reg: Vec<PathEntry> = Vec::new();

        let out = reconcile_process_with_registry(&process, &user_reg, &machine_reg);
        assert_eq!(out.len(), 1);
        assert!(
            out[0].provenance_raw.is_none(),
            "no expanded match in either registry; provenance must stay None",
        );
    }

    #[test]
    fn reconcile_no_overlay_when_raw_already_matches_registry() {
        // REG_SZ case: registry stored a literal, OS did not expand.
        // process raw == registry raw → nothing to overlay.
        let process = vec![raw_entry(
            r"C:\Program Files\PowerShell\7",
            r"C:\Program Files\PowerShell\7",
        )];
        let user_reg = vec![raw_entry(
            r"C:\Program Files\PowerShell\7",
            r"C:\Program Files\PowerShell\7",
        )];
        let machine_reg: Vec<PathEntry> = Vec::new();

        let out = reconcile_process_with_registry(&process, &user_reg, &machine_reg);
        assert!(
            out[0].provenance_raw.is_none(),
            "raw already matches; provenance overlay would be redundant",
        );
    }

    #[test]
    fn reconcile_first_occurrence_wins_within_user_registry() {
        // Pathological registry with two raws that expand to the same
        // path. The reconciler must pick the first occurrence so the
        // result is deterministic across runs.
        let process = vec![raw_entry(
            r"C:\Users\me\AppData\Local\Microsoft\WindowsApps",
            r"C:\Users\me\AppData\Local\Microsoft\WindowsApps",
        )];
        let user_reg = vec![
            raw_entry(
                r"%LocalAppData%\Microsoft\WindowsApps",
                r"C:\Users\me\AppData\Local\Microsoft\WindowsApps",
            ),
            raw_entry(
                r"%USERPROFILE%\AppData\Local\Microsoft\WindowsApps",
                r"C:\Users\me\AppData\Local\Microsoft\WindowsApps",
            ),
        ];
        let machine_reg: Vec<PathEntry> = Vec::new();

        let out = reconcile_process_with_registry(&process, &user_reg, &machine_reg);
        assert_eq!(
            out[0].provenance_raw.as_deref(),
            Some(r"%LocalAppData%\Microsoft\WindowsApps"),
            "first occurrence must win",
        );
    }

    #[test]
    fn reconcile_uses_normalized_expanded_for_match() {
        // Process expanded uses backslashes + mixed case; registry
        // expanded happens to use forward slashes + lowercase. After
        // `expand::normalize` they are the same path, so the overlay
        // should fire.
        let process = vec![raw_entry(
            r"C:\Users\Me\AppData\Local\X",
            r"C:\Users\Me\AppData\Local\X",
        )];
        let user_reg = vec![raw_entry(
            r"%LocalAppData%\X",
            "c:/users/me/appdata/local/x",
        )];
        let machine_reg: Vec<PathEntry> = Vec::new();

        let out = reconcile_process_with_registry(&process, &user_reg, &machine_reg);
        assert_eq!(
            out[0].provenance_raw.as_deref(),
            Some(r"%LocalAppData%\X"),
            "match must use expand::normalize on both sides",
        );
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
