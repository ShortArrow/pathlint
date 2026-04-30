//! `pathlint init` — write a starter `pathlint.toml` in the current
//! directory.
//!
//! See PRD §7.3.

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::os_detect::Os;

const EMBEDDED_CATALOG: &str = include_str!("embedded_catalog.toml");

#[derive(Debug)]
pub struct InitOptions {
    pub emit_defaults: bool,
    pub force: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum InitOutcome {
    Wrote(PathBuf),
    AlreadyExists(PathBuf),
}

pub fn run(dir: &Path, opts: &InitOptions, os: Os) -> Result<InitOutcome> {
    let target = dir.join("pathlint.toml");
    if target.exists() && !opts.force {
        return Ok(InitOutcome::AlreadyExists(target));
    }
    let body = render_starter(os, opts.emit_defaults);
    std::fs::write(&target, body)?;
    Ok(InitOutcome::Wrote(target))
}

/// Build the starter file body for the given OS, optionally with the
/// full embedded catalog appended.
pub fn render_starter(os: Os, emit_defaults: bool) -> String {
    let mut buf = String::new();
    buf.push_str(HEADER);
    buf.push('\n');
    buf.push_str(starter_expects_for(os));

    if emit_defaults {
        buf.push_str("\n\n");
        buf.push_str(DEFAULTS_HEADER);
        buf.push('\n');
        buf.push_str(EMBEDDED_CATALOG);
    }

    buf
}

const HEADER: &str = "\
# pathlint configuration. See https://github.com/ShortArrow/pathlint
# for the full schema.
#
# Each [[expect]] declares a command and which installer(s) it should
# come from. Run `pathlint` to evaluate every expectation against the
# current PATH.
";

const DEFAULTS_HEADER: &str = "\
# ---- Built-in source catalog (emitted by --emit-defaults) ----
#
# Override any per-OS path here, or remove what you do not need.
# Anything you delete simply falls back to the embedded default.
";

fn starter_expects_for(os: Os) -> &'static str {
    match os {
        Os::Windows => WINDOWS_STARTER,
        Os::Macos => MACOS_STARTER,
        Os::Linux => LINUX_STARTER,
        Os::Termux => TERMUX_STARTER,
    }
}

const WINDOWS_STARTER: &str = r#"# ---- Cross-OS examples ----

[[expect]]
command = "cargo"
prefer  = ["cargo", "scoop", "winget"]

# ---- Windows-specific examples ----
#
# `mise_shims` is the recommended way to consume mise — it matches
# binaries served via `mise/shims/`. `mise_installs` matches the
# per-runtime install dirs. The catch-all `mise` source covers
# either layer for backwards compatibility.

[[expect]]
command = "python"
prefer  = ["mise_shims", "scoop"]
avoid   = ["WindowsApps", "choco"]
os      = ["windows"]

[[expect]]
command = "git"
optional = true
prefer  = ["winget", "scoop"]
"#;

const MACOS_STARTER: &str = r#"# ---- Cross-OS examples ----

[[expect]]
command = "cargo"
prefer  = ["cargo"]

# ---- macOS-specific examples ----

[[expect]]
command = "python"
prefer  = ["mise_shims", "brew_arm", "brew_intel"]
os      = ["macos"]

[[expect]]
command = "gcc"
prefer  = ["brew_arm", "brew_intel"]
avoid   = ["system_macos"]
os      = ["macos"]
"#;

const LINUX_STARTER: &str = r#"# ---- Cross-OS examples ----

[[expect]]
command = "cargo"
prefer  = ["cargo"]

# ---- Linux-specific examples ----
#
# On Arch / openSUSE TW / Solus, /usr/sbin is a symlink to /usr/bin
# and `which` reports /usr/sbin/<cmd>. The built-in apt / pacman /
# dnf sources cover /usr/bin only, so `prefer = ["pacman"]` would
# miss on those distros. Either reference both `pacman` and
# `usr_sbin` here, or add `[source.usr_sbin] linux = "/usr/sbin"`
# in your own [source.*] section below.

[[expect]]
command = "python"
prefer  = ["mise_shims", "asdf", "apt", "pacman"]
os      = ["linux"]

[[expect]]
command = "node"
prefer  = ["mise_shims", "volta"]
avoid   = ["snap"]
os      = ["linux"]

[source.usr_sbin]
linux = "/usr/sbin"
"#;

const TERMUX_STARTER: &str = r#"# ---- Cross-OS examples ----

[[expect]]
command = "cargo"
prefer  = ["cargo"]

# ---- Termux-specific examples ----

[[expect]]
command = "python"
prefer  = ["pkg"]
os      = ["termux"]

[[expect]]
command = "git"
prefer  = ["pkg"]
os      = ["termux"]
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn starter_for_each_os_parses_as_valid_toml() {
        for os in [Os::Windows, Os::Macos, Os::Linux, Os::Termux] {
            let body = render_starter(os, false);
            Config::parse_toml(&body)
                .unwrap_or_else(|e| panic!("starter for {os:?} did not parse: {e}"));
        }
    }

    #[test]
    fn starter_with_emit_defaults_parses_and_includes_catalog() {
        let body = render_starter(Os::Linux, true);
        let cfg = Config::parse_toml(&body).expect("emit-defaults must parse");
        assert!(cfg.source.contains_key("cargo"));
        assert!(cfg.source.contains_key("apt"));
    }

    #[test]
    fn starter_includes_at_least_one_os_specific_expectation() {
        let body = render_starter(Os::Windows, false);
        let cfg = Config::parse_toml(&body).unwrap();
        assert!(
            cfg.expectations.iter().any(|e| e
                .os
                .as_ref()
                .is_some_and(|tags| tags.iter().any(|t| t.eq_ignore_ascii_case("windows")))),
            "starter must reference its own OS"
        );
    }

    #[test]
    fn run_creates_file_and_refuses_to_overwrite() {
        let tmp = tempfile::tempdir().unwrap();
        let opts = InitOptions {
            emit_defaults: false,
            force: false,
        };

        let first = run(tmp.path(), &opts, Os::Linux).unwrap();
        assert!(matches!(first, InitOutcome::Wrote(_)));
        let target = tmp.path().join("pathlint.toml");
        assert!(target.is_file());

        let second = run(tmp.path(), &opts, Os::Linux).unwrap();
        assert!(matches!(second, InitOutcome::AlreadyExists(_)));
    }

    #[test]
    fn run_with_force_overwrites_existing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("pathlint.toml");
        std::fs::write(&target, "stale = true\n").unwrap();

        let opts = InitOptions {
            emit_defaults: false,
            force: true,
        };
        let outcome = run(tmp.path(), &opts, Os::Linux).unwrap();
        assert!(matches!(outcome, InitOutcome::Wrote(_)));

        let written = std::fs::read_to_string(&target).unwrap();
        assert!(!written.contains("stale = true"));
        assert!(written.contains("[[expect]]"));
    }
}
