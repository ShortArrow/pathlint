//! TOML schema for `pathlint.toml`.
//!
//! See `docs/PRD.md` §8.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;

/// Top-level `pathlint.toml` document.
#[derive(Debug, Default, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Catalog version embedded in the running binary. Set only by
    /// the embedded `embedded_catalog.toml`; user `pathlint.toml`
    /// files leave this `None` and use `require_catalog` instead.
    #[serde(default)]
    pub catalog_version: Option<u32>,

    /// Minimum embedded catalog version this `pathlint.toml`
    /// requires. If set, pathlint refuses to run when the binary's
    /// `catalog_version` is lower (config error, exit 2). Leave
    /// unset to opt out of the check.
    #[serde(default)]
    pub require_catalog: Option<u32>,

    #[serde(default, rename = "expect")]
    pub expectations: Vec<Expectation>,

    #[serde(default)]
    pub source: BTreeMap<String, SourceDef>,
}

/// A single `[[expect]]` entry.
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Expectation {
    pub command: String,

    #[serde(default)]
    pub prefer: Vec<String>,

    #[serde(default)]
    pub avoid: Vec<String>,

    #[serde(default)]
    pub os: Option<Vec<String>>,

    #[serde(default)]
    pub optional: bool,

    /// R2 — shape check on the resolved path. When set, pathlint
    /// verifies the resolved file matches the expected kind in
    /// addition to the source check. See PRD §7.6.
    #[serde(default)]
    pub kind: Option<Kind>,
}

/// Shape vocabulary for `[[expect]] kind = ...`. Only `executable`
/// today; deliberately kept minimal so we can grow it on real
/// demand instead of OS-specific permutations of `script` /
/// `binary` / `dll` / `wrapper`.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// The resolved path must be an executable file: not a
    /// directory, not a broken symlink, and on Unix have at least
    /// one `+x` mode bit set.
    Executable,
}

/// A `[source.<name>]` definition. Each per-OS field is an optional
/// substring (post env-var expansion / slash normalization).
#[derive(Debug, Default, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct SourceDef {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub windows: Option<String>,
    #[serde(default)]
    pub macos: Option<String>,
    #[serde(default)]
    pub linux: Option<String>,
    #[serde(default)]
    pub termux: Option<String>,
    /// Convenience: applied to macos / linux / termux when those are
    /// not separately set.
    #[serde(default)]
    pub unix: Option<String>,
    /// R4 — shell command template that uninstalls a binary served
    /// by this source. The substring `{bin}` is substituted with
    /// the resolved binary's stem (filename without extension).
    /// Used by `pathlint where`. Leave unset for sources where
    /// uninstall is not a meaningful single command (e.g. shim
    /// layers, system_*).
    #[serde(default)]
    pub uninstall_command: Option<String>,
}

impl SourceDef {
    /// Effective per-OS path string for the given OS, applying the
    /// `unix` fallback. Returns `None` when no path is defined for the
    /// requested OS.
    pub fn path_for(&self, os: crate::os_detect::Os) -> Option<&str> {
        use crate::os_detect::Os;
        let direct = match os {
            Os::Windows => self.windows.as_deref(),
            Os::Macos => self.macos.as_deref(),
            Os::Linux => self.linux.as_deref(),
            Os::Termux => self.termux.as_deref(),
        };
        let fallback = match os {
            Os::Macos | Os::Linux | Os::Termux => self.unix.as_deref(),
            Os::Windows => None,
        };
        direct.or(fallback)
    }

    /// Field-by-field merge with `override_with` taking precedence.
    pub fn merge(&self, override_with: &SourceDef) -> SourceDef {
        SourceDef {
            description: override_with
                .description
                .clone()
                .or_else(|| self.description.clone()),
            windows: override_with
                .windows
                .clone()
                .or_else(|| self.windows.clone()),
            macos: override_with.macos.clone().or_else(|| self.macos.clone()),
            linux: override_with.linux.clone().or_else(|| self.linux.clone()),
            termux: override_with.termux.clone().or_else(|| self.termux.clone()),
            unix: override_with.unix.clone().or_else(|| self.unix.clone()),
            uninstall_command: override_with
                .uninstall_command
                .clone()
                .or_else(|| self.uninstall_command.clone()),
        }
    }
}

impl Config {
    pub fn parse_toml(toml_text: &str) -> Result<Self, ConfigError> {
        toml::from_str(toml_text).map_err(|e| ConfigError::Parse(e.to_string()))
    }

    pub fn from_path(path: &Path) -> Result<Self, ConfigError> {
        let text = fs::read_to_string(path)
            .map_err(|e| ConfigError::Read(path.display().to_string(), e.to_string()))?;
        Self::parse_toml(&text)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read {0}: {1}")]
    Read(String, String),
    #[error("failed to parse pathlint.toml: {0}")]
    Parse(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os_detect::Os;

    #[test]
    fn parses_minimal_expect_block() {
        let cfg: Config = Config::parse_toml(
            r#"
[[expect]]
command = "runex"
prefer = ["cargo"]
avoid = ["winget"]
"#,
        )
        .unwrap();
        assert_eq!(cfg.expectations.len(), 1);
        let e = &cfg.expectations[0];
        assert_eq!(e.command, "runex");
        assert_eq!(e.prefer, vec!["cargo"]);
        assert_eq!(e.avoid, vec!["winget"]);
        assert!(e.os.is_none());
        assert!(!e.optional);
    }

    #[test]
    fn parses_source_with_unix_fallback() {
        let cfg: Config = Config::parse_toml(
            r#"
[source.cargo]
windows = "C:/Users/x/.cargo/bin"
unix = "/home/x/.cargo/bin"
"#,
        )
        .unwrap();
        let cargo = cfg.source.get("cargo").unwrap();
        assert_eq!(cargo.path_for(Os::Windows), Some("C:/Users/x/.cargo/bin"));
        assert_eq!(cargo.path_for(Os::Linux), Some("/home/x/.cargo/bin"));
        assert_eq!(cargo.path_for(Os::Macos), Some("/home/x/.cargo/bin"));
    }

    #[test]
    fn merge_prefers_override_fields() {
        let base = SourceDef {
            windows: Some("C:/old".into()),
            unix: Some("/usr/old".into()),
            ..Default::default()
        };
        let user = SourceDef {
            windows: Some("D:/new".into()),
            ..Default::default()
        };
        let merged = base.merge(&user);
        assert_eq!(merged.windows.as_deref(), Some("D:/new"));
        assert_eq!(merged.unix.as_deref(), Some("/usr/old"));
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let err = Config::parse_toml(
            r#"
[[expect]]
command = "x"
unknown_field = true
"#,
        )
        .unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("unknown_field"), "got: {msg}");
    }

    #[test]
    fn catalog_version_and_require_catalog_are_parsed() {
        let cfg = Config::parse_toml(
            r#"
catalog_version = 7
require_catalog = 5
"#,
        )
        .unwrap();
        assert_eq!(cfg.catalog_version, Some(7));
        assert_eq!(cfg.require_catalog, Some(5));
    }

    #[test]
    fn require_catalog_is_optional() {
        let cfg = Config::parse_toml("").unwrap();
        assert_eq!(cfg.catalog_version, None);
        assert_eq!(cfg.require_catalog, None);
    }

    #[test]
    fn kind_executable_parses() {
        let cfg = Config::parse_toml(
            r#"
[[expect]]
command = "x"
kind    = "executable"
"#,
        )
        .unwrap();
        assert_eq!(cfg.expectations[0].kind, Some(Kind::Executable));
    }

    #[test]
    fn kind_unknown_value_is_a_parse_error() {
        let err = Config::parse_toml(
            r#"
[[expect]]
command = "x"
kind    = "binary"
"#,
        )
        .unwrap_err();
        assert!(format!("{err}").contains("kind"), "{err}");
    }

    #[test]
    fn kind_is_optional() {
        let cfg = Config::parse_toml(
            r#"
[[expect]]
command = "x"
"#,
        )
        .unwrap();
        assert_eq!(cfg.expectations[0].kind, None);
    }
}
