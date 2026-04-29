//! Runtime OS classification used by `[[expect]] os = [...]` filters and
//! `[source.<name>]` per-OS keys.

use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Os {
    Windows,
    Macos,
    Linux,
    Termux,
}

impl Os {
    pub fn current() -> Self {
        if cfg!(windows) {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::Macos
        } else if is_termux() {
            Os::Termux
        } else {
            Os::Linux
        }
    }

    /// Returns true if the OS tag string from a TOML file matches this OS.
    /// Tags: "windows", "macos", "linux", "termux", "unix".
    pub fn matches_tag(self, tag: &str) -> bool {
        let t = tag.to_ascii_lowercase();
        matches!(
            (self, t.as_str()),
            (Os::Windows, "windows")
                | (Os::Macos, "macos")
                | (Os::Linux, "linux")
                | (Os::Termux, "termux")
                | (Os::Macos | Os::Linux | Os::Termux, "unix")
        )
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Os::Windows => "windows",
            Os::Macos => "macos",
            Os::Linux => "linux",
            Os::Termux => "termux",
        }
    }
}

fn is_termux() -> bool {
    env::var("PREFIX")
        .map(|p| p.contains("/data/data/com.termux/files"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_matches_windows_and_not_unix() {
        assert!(Os::Windows.matches_tag("windows"));
        assert!(!Os::Windows.matches_tag("unix"));
        assert!(!Os::Windows.matches_tag("linux"));
    }

    #[test]
    fn macos_matches_unix() {
        assert!(Os::Macos.matches_tag("macos"));
        assert!(Os::Macos.matches_tag("unix"));
        assert!(!Os::Macos.matches_tag("linux"));
    }

    #[test]
    fn termux_matches_unix_but_not_linux() {
        assert!(Os::Termux.matches_tag("termux"));
        assert!(Os::Termux.matches_tag("unix"));
        assert!(!Os::Termux.matches_tag("linux"));
    }

    #[test]
    fn tag_match_is_case_insensitive() {
        assert!(Os::Linux.matches_tag("LINUX"));
        assert!(Os::Linux.matches_tag("Unix"));
    }
}
