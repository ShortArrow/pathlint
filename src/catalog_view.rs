//! Render the merged source catalog for `pathlint catalog list`.

use std::collections::BTreeMap;

use crate::config::SourceDef;
use crate::os_detect::Os;

#[derive(Debug, Clone, Copy)]
pub struct ListStyle {
    /// Show every per-OS path, not only the one for `os`.
    pub all_os: bool,
    /// Print only source names, one per line.
    pub names_only: bool,
}

pub fn render(catalog: &BTreeMap<String, SourceDef>, os: Os, style: ListStyle) -> String {
    if style.names_only {
        return render_names_only(catalog);
    }
    if style.all_os {
        render_all_os(catalog)
    } else {
        render_for_os(catalog, os)
    }
}

fn render_names_only(catalog: &BTreeMap<String, SourceDef>) -> String {
    let mut buf = String::new();
    for name in catalog.keys() {
        buf.push_str(name);
        buf.push('\n');
    }
    buf
}

fn render_for_os(catalog: &BTreeMap<String, SourceDef>, os: Os) -> String {
    let name_width = catalog.keys().map(|n| n.len()).max().unwrap_or(0).max(4);
    let mut buf = String::new();
    for (name, def) in catalog {
        let path = def.path_for(os).unwrap_or("(no path for this OS)");
        let desc = def.description.as_deref().unwrap_or("");
        let desc_part = if desc.is_empty() {
            String::new()
        } else {
            format!("  — {desc}")
        };
        buf.push_str(&format!(
            "{name:<width$}  {path}{desc_part}\n",
            width = name_width,
        ));
    }
    buf
}

fn render_all_os(catalog: &BTreeMap<String, SourceDef>) -> String {
    let mut buf = String::new();
    for (name, def) in catalog {
        buf.push_str(name);
        if let Some(d) = def.description.as_deref() {
            if !d.is_empty() {
                buf.push_str("  — ");
                buf.push_str(d);
            }
        }
        buf.push('\n');
        for (label, val) in [
            ("windows", def.windows.as_deref()),
            ("macos", def.macos.as_deref()),
            ("linux", def.linux.as_deref()),
            ("termux", def.termux.as_deref()),
            ("unix", def.unix.as_deref()),
        ] {
            if let Some(v) = val {
                buf.push_str(&format!("    {label:<8} {v}\n"));
            }
        }
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog;

    #[test]
    fn for_os_lists_known_sources_with_paths() {
        let cat = catalog::builtin();
        let out = render(
            &cat,
            Os::Linux,
            ListStyle {
                all_os: false,
                names_only: false,
            },
        );
        assert!(out.contains("cargo"), "out: {out}");
        assert!(out.contains("apt"), "out: {out}");
        // brew_arm has no Linux path; should fall back to placeholder.
        assert!(out.contains("brew_arm"), "out: {out}");
        assert!(out.contains("(no path for this OS)"), "out: {out}");
    }

    #[test]
    fn names_only_strips_paths_and_descriptions() {
        let cat = catalog::builtin();
        let out = render(
            &cat,
            Os::Linux,
            ListStyle {
                all_os: false,
                names_only: true,
            },
        );
        for line in out.lines() {
            assert!(
                !line.contains(' '),
                "names-only must have no extra columns: {line}"
            );
        }
        assert!(out.lines().any(|l| l == "cargo"));
        assert!(out.lines().any(|l| l == "winget"));
    }

    #[test]
    fn all_os_shows_every_defined_per_os_field() {
        let cat = catalog::builtin();
        let out = render(
            &cat,
            Os::Linux,
            ListStyle {
                all_os: true,
                names_only: false,
            },
        );
        // cargo has both windows and unix paths.
        assert!(out.contains("windows  $UserProfile/.cargo/bin"));
        assert!(out.contains("unix     $HOME/.cargo/bin"));
    }
}
