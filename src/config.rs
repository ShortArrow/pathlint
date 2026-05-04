//! TOML schema for `pathlint.toml`.
//!
//! See `docs/PRD.md` §8.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;

/// Top-level `pathlint.toml` document.
#[derive(Debug, Default, Deserialize, Clone, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(title = "pathlint.toml")]
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

    /// Declarative relations between sources. The built-in catalog
    /// declares them in `plugins/<name>.toml`; users can also write
    /// their own in `pathlint.toml` to express alias / conflict /
    /// served-by-via / depends-on relationships between custom
    /// sources. See PRD §9.
    #[serde(default, rename = "relation")]
    pub relations: Vec<Relation>,
}

/// A relation between sources, declared as `[[relation]]` in plugin
/// or user TOML. The `kind` discriminator decides which payload
/// fields are required; serde rejects unknown kinds.
#[derive(Debug, Deserialize, serde::Serialize, Clone, PartialEq, Eq, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Relation {
    /// One source is a catch-all alias for one or more more-specific
    /// children. Matching the parent does not exclude matching the
    /// children — both can fire on the same path. Used today for the
    /// `mise` parent over `mise_shims` and `mise_installs`.
    AliasOf {
        parent: String,
        children: Vec<String>,
    },

    /// Two or more sources should not be active in PATH at the same
    /// time; `pathlint doctor` raises `diagnostic` (the snake_case
    /// `Kind` name) when more than one of them appears in PATH.
    /// Used today for `mise_activate_both`.
    ConflictsWhenBothInPath {
        sources: Vec<String>,
        diagnostic: String,
    },

    /// `host` serves binaries that originally came from
    /// `guest_provider` via paths matching `guest_pattern`. Used by
    /// `pathlint where` to attribute provenance through wrapper
    /// installers (e.g. mise installing a cargo binary).
    ///
    /// `installer_token` (0.0.10+) is the human-facing installer
    /// name that uninstall hints quote — it can differ from the
    /// `guest_provider` source name. For example,
    /// `guest_provider = "pip_user"` but `installer_token = "pipx"`
    /// because `mise uninstall pipx:black` is what the user runs.
    /// `None` falls back to `guest_provider`.
    ServedByVia {
        host: String,
        guest_pattern: String,
        guest_provider: String,
        #[serde(default)]
        installer_token: Option<String>,
    },

    /// `target` is a hard prerequisite of the source declaring this
    /// relation (the implicit subject is the plugin file's source).
    /// Surfaced by `pathlint where` so users know that uninstalling
    /// a wrapper does not remove the underlying tool.
    DependsOn { source: String, target: String },

    /// `earlier` should come before `later` in PATH for the user's
    /// preferred resolution order. Consumed by `pathlint sort` to
    /// break ties within the same preferred/neutral/avoided bucket.
    /// Forms a directed edge (`earlier` -> `later`) for cycle
    /// detection.
    PreferOrderOver { earlier: String, later: String },
}

/// A single `[[expect]]` entry.
#[derive(Debug, Default, Deserialize, Clone, schemars::JsonSchema)]
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

    /// How a failure on this rule should affect the run's exit
    /// code. `error` (default, 0.0.x behaviour) escalates an NG to
    /// exit 1 — appropriate for "this absolutely must come from
    /// cargo" rules. `warn` keeps the diagnostic visible but lets
    /// the run pass (exit 0) — appropriate for nudges and
    /// preferences in CI where a single rogue path should not block
    /// the build. The shape (NG variant, resolved path, etc.) is
    /// unchanged; only the exit-code consequence differs.
    #[serde(default)]
    pub severity: Severity,
}

/// Per-rule severity for `[[expect]]`. Defaults to `Error` so 0.0.x
/// rules behave exactly as before.
#[derive(
    Debug, Deserialize, serde::Serialize, Clone, Copy, PartialEq, Eq, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// NG escalates to exit 1. Default.
    #[default]
    Error,
    /// NG is reported but does not change the exit code.
    Warn,
}

/// Shape vocabulary for `[[expect]] kind = ...`. Only `executable`
/// today; deliberately kept minimal so we can grow it on real
/// demand instead of OS-specific permutations of `script` /
/// `binary` / `dll` / `wrapper`.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// The resolved path must be an executable file: not a
    /// directory, not a broken symlink, and on Unix have at least
    /// one `+x` mode bit set.
    Executable,
}

/// A `[source.<name>]` definition. Each per-OS field is an optional
/// substring (post env-var expansion / slash normalization).
#[derive(Debug, Default, Deserialize, Clone, schemars::JsonSchema)]
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
        let text = load_rules_text(path)?;
        Self::parse_toml(&text)
    }
}

/// Maximum size in bytes for a rules TOML. Anything larger is
/// rejected before any disk read so a hostile or accidental huge
/// file cannot OOM pathlint at startup. 16 MiB is several orders
/// of magnitude above any real-world `pathlint.toml`; bumping it
/// is a config decision, not a security one.
pub const RULES_MAX_BYTES: u64 = 16 * 1024 * 1024;

/// Read a rules-file TOML from disk with two preflight guards:
///
/// 1. **Symlink policy**: a symlink at `path` is followed at most
///    one hop. If the target is itself a symlink, the read is
///    rejected. This keeps pathlint compatible with dotfiles
///    layouts (where `~/.config/pathlint/pathlint.toml` is often
///    a symlink into a managed checkout) while blocking
///    multi-hop chains that an attacker can use to mask the real
///    target.
/// 2. **Size cap**: `RULES_MAX_BYTES` (16 MiB). Anything beyond
///    is rejected before any byte is buffered. Without this a
///    `--rules /dev/zero` would happily consume all RAM.
///
/// 0.0.13 hardens this against a TOCTOU race that 0.0.11–0.0.12
/// inherited: `File::open(path)` is called once, and every
/// subsequent check (file kind, size cap, byte read) goes through
/// the same kernel handle. A racing rename/unlink between the
/// stat and the open cannot affect a check that is performed on
/// the open handle itself. The multi-hop symlink test is still
/// done via `fs::read_link` because that is a one-shot string
/// operation that does not race with the open.
///
/// Returns the full file contents as `String`. Errors carry
/// `ConfigError` variants suitable for surfacing as exit code 2.
fn load_rules_text(path: &Path) -> Result<String, ConfigError> {
    use std::io::Read;

    let display = path.display().to_string();

    // Step 1: classify the path itself. We need to know whether it
    // is a symlink so we can apply the multi-hop check below; the
    // rest of the validation runs on the opened handle.
    let lst = fs::symlink_metadata(path)
        .map_err(|e| ConfigError::Read(display.clone(), e.to_string()))?;
    let is_symlink = lst.file_type().is_symlink();

    // Step 2: when path is a symlink, ensure it points directly at
    // a regular file (not at another symlink). 0.0.12 had a bug
    // here: it called `fs::symlink_metadata(fs::read_link(path))`
    // which interprets a relative target relative to the process
    // cwd, not relative to `path.parent()`. Fix that by joining the
    // target against the link's parent dir before re-statting.
    if is_symlink {
        let target = fs::read_link(path)
            .map_err(|e| ConfigError::Read(display.clone(), format!("read_link failed: {e}")))?;
        let resolved_target = if target.is_absolute() {
            target
        } else {
            path.parent().map(|p| p.join(&target)).unwrap_or(target)
        };
        let target_lst = fs::symlink_metadata(&resolved_target).map_err(|e| {
            ConfigError::Read(display.clone(), format!("symlink target stat failed: {e}"))
        })?;
        if target_lst.file_type().is_symlink() {
            return Err(ConfigError::RulesNotRegularFile(display));
        }
    }

    // Step 3: open exactly once. `File::open` follows the (single
    // allowed) symlink and gives us a handle that subsequent
    // checks bind to. Even if an attacker swaps the path/symlink
    // target between symlink_metadata and open, the swap would be
    // visible to the OS at open time and to every check after.
    let file =
        std::fs::File::open(path).map_err(|e| ConfigError::Read(display.clone(), e.to_string()))?;

    // Step 4: validate the *opened handle*. metadata() on File
    // queries the kernel handle, so the `is_file` and `len`
    // checks below cannot be defeated by a path-level race.
    let md = file
        .metadata()
        .map_err(|e| ConfigError::Read(display.clone(), e.to_string()))?;
    if !md.is_file() {
        return Err(ConfigError::RulesNotRegularFile(display));
    }
    if md.len() > RULES_MAX_BYTES {
        return Err(ConfigError::RulesTooLarge(display, md.len()));
    }

    // Step 5: read from the same handle, capped at MAX+1 so a file
    // that grew between metadata() and read() is still rejected.
    let mut buf = String::new();
    file.take(RULES_MAX_BYTES + 1)
        .read_to_string(&mut buf)
        .map_err(|e| ConfigError::Read(display.clone(), e.to_string()))?;
    if buf.len() as u64 > RULES_MAX_BYTES {
        return Err(ConfigError::RulesTooLarge(display, buf.len() as u64));
    }
    Ok(buf)
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read {0}: {1}")]
    Read(String, String),
    #[error("failed to parse pathlint.toml: {0}")]
    Parse(String),
    #[error(
        "{0} is not a regular file (a single-hop symlink to a regular file is the only indirection allowed)"
    )]
    RulesNotRegularFile(String),
    #[error("{0} is too large ({1} bytes); rules files are capped at 16 MiB")]
    RulesTooLarge(String, u64),
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

    #[test]
    fn severity_defaults_to_error() {
        // Unspecified severity must keep 0.0.x behaviour: NG => exit 1.
        let cfg = Config::parse_toml(
            r#"
[[expect]]
command = "x"
"#,
        )
        .unwrap();
        assert_eq!(cfg.expectations[0].severity, Severity::Error);
    }

    #[test]
    fn severity_warn_parses() {
        let cfg = Config::parse_toml(
            r#"
[[expect]]
command = "x"
severity = "warn"
"#,
        )
        .unwrap();
        assert_eq!(cfg.expectations[0].severity, Severity::Warn);
    }

    #[test]
    fn severity_error_parses_explicitly() {
        let cfg = Config::parse_toml(
            r#"
[[expect]]
command = "x"
severity = "error"
"#,
        )
        .unwrap();
        assert_eq!(cfg.expectations[0].severity, Severity::Error);
    }

    #[test]
    fn severity_unknown_value_is_a_parse_error() {
        let err = Config::parse_toml(
            r#"
[[expect]]
command = "x"
severity = "info"
"#,
        )
        .unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("severity") || msg.contains("info"), "{msg}");
    }

    #[test]
    fn relations_default_to_empty() {
        let cfg = Config::parse_toml("").unwrap();
        assert!(cfg.relations.is_empty());
    }

    #[test]
    fn relation_alias_of_parses() {
        let cfg = Config::parse_toml(
            r#"
[[relation]]
kind = "alias_of"
parent = "mise"
children = ["mise_shims", "mise_installs"]
"#,
        )
        .unwrap();
        assert_eq!(cfg.relations.len(), 1);
        match &cfg.relations[0] {
            Relation::AliasOf { parent, children } => {
                assert_eq!(parent, "mise");
                assert_eq!(
                    children,
                    &vec!["mise_shims".to_string(), "mise_installs".to_string()]
                );
            }
            other => panic!("expected AliasOf, got {other:?}"),
        }
    }

    #[test]
    fn relation_conflicts_when_both_in_path_parses() {
        let cfg = Config::parse_toml(
            r#"
[[relation]]
kind = "conflicts_when_both_in_path"
sources = ["mise_shims", "mise_installs"]
diagnostic = "mise_activate_both"
"#,
        )
        .unwrap();
        match &cfg.relations[0] {
            Relation::ConflictsWhenBothInPath {
                sources,
                diagnostic,
            } => {
                assert_eq!(sources.len(), 2);
                assert_eq!(diagnostic, "mise_activate_both");
            }
            other => panic!("expected ConflictsWhenBothInPath, got {other:?}"),
        }
    }

    #[test]
    fn relation_served_by_via_parses() {
        let cfg = Config::parse_toml(
            r#"
[[relation]]
kind = "served_by_via"
host = "mise_installs"
guest_pattern = "cargo-*"
guest_provider = "cargo"
"#,
        )
        .unwrap();
        match &cfg.relations[0] {
            Relation::ServedByVia {
                host,
                guest_pattern,
                guest_provider,
                installer_token,
            } => {
                assert_eq!(host, "mise_installs");
                assert_eq!(guest_pattern, "cargo-*");
                assert_eq!(guest_provider, "cargo");
                assert!(installer_token.is_none());
            }
            other => panic!("expected ServedByVia, got {other:?}"),
        }
    }

    #[test]
    fn relation_served_by_via_parses_installer_token() {
        let cfg = Config::parse_toml(
            r#"
[[relation]]
kind = "served_by_via"
host = "mise_installs"
guest_pattern = "pipx-*"
guest_provider = "pip_user"
installer_token = "pipx"
"#,
        )
        .unwrap();
        match &cfg.relations[0] {
            Relation::ServedByVia {
                installer_token, ..
            } => {
                assert_eq!(installer_token.as_deref(), Some("pipx"));
            }
            other => panic!("expected ServedByVia, got {other:?}"),
        }
    }

    #[test]
    fn relation_prefer_order_over_parses() {
        let cfg = Config::parse_toml(
            r#"
[[relation]]
kind = "prefer_order_over"
earlier = "cargo"
later = "system_linux"
"#,
        )
        .unwrap();
        match &cfg.relations[0] {
            Relation::PreferOrderOver { earlier, later } => {
                assert_eq!(earlier, "cargo");
                assert_eq!(later, "system_linux");
            }
            other => panic!("expected PreferOrderOver, got {other:?}"),
        }
    }

    #[test]
    fn relation_depends_on_parses() {
        let cfg = Config::parse_toml(
            r#"
[[relation]]
kind = "depends_on"
source = "paru"
target = "pacman"
"#,
        )
        .unwrap();
        match &cfg.relations[0] {
            Relation::DependsOn { source, target } => {
                assert_eq!(source, "paru");
                assert_eq!(target, "pacman");
            }
            other => panic!("expected DependsOn, got {other:?}"),
        }
    }

    #[test]
    fn relation_unknown_kind_is_a_parse_error() {
        let err = Config::parse_toml(
            r#"
[[relation]]
kind = "this_does_not_exist"
"#,
        )
        .unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("kind") || msg.contains("this_does_not_exist"),
            "{msg}"
        );
    }

    #[test]
    fn relation_missing_required_field_is_a_parse_error() {
        // alias_of requires `children`. Missing it must be rejected.
        let err = Config::parse_toml(
            r#"
[[relation]]
kind = "alias_of"
parent = "mise"
"#,
        )
        .unwrap_err();
        assert!(format!("{err}").contains("children"), "{err}");
    }
}
