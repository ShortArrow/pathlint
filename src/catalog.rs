//! Built-in source catalog and merge with user-defined sources.

use std::collections::BTreeMap;

use crate::config::{Config, SourceDef};

const EMBEDDED: &str = include_str!("embedded_catalog.toml");

/// Parse the embedded catalog (panics on failure — would be a build bug).
pub fn builtin() -> BTreeMap<String, SourceDef> {
    let cfg: Config = Config::parse_toml(EMBEDDED).expect("embedded_catalog.toml must parse");
    cfg.source
}

/// Version of the catalog embedded in this binary. Bumped whenever
/// an existing source's path or semantics changes — see
/// `embedded_catalog.toml` for the policy. Defaults to `0` if the
/// embedded file forgets to declare one (which would be a build bug
/// caught at code review).
pub fn embedded_version() -> u32 {
    let cfg: Config = Config::parse_toml(EMBEDDED).expect("embedded_catalog.toml must parse");
    cfg.catalog_version.unwrap_or(0)
}

/// Pure compatibility check: does this binary's embedded catalog
/// satisfy the user's `require_catalog` directive? `Ok(())` means
/// either no requirement was set or the embedded version meets or
/// exceeds it. `Err` carries a one-line user-facing message naming
/// both versions and the recommended fix.
///
/// Pure: no I/O, no globals — both versions are passed in. Unit-
/// testable without touching the embedded catalog or stderr.
pub fn version_check(require_catalog: Option<u32>, embedded: u32) -> Result<(), String> {
    let Some(required) = require_catalog else {
        return Ok(());
    };
    if embedded >= required {
        return Ok(());
    }
    Err(format!(
        "rules require catalog_version >= {required}, but this binary embeds version {embedded}. \
         Upgrade pathlint or lower require_catalog."
    ))
}

/// Merge user-defined sources on top of the built-in catalog. User
/// entries with the same name override field-by-field; new names are
/// added.
pub fn merge_with_user(user: &BTreeMap<String, SourceDef>) -> BTreeMap<String, SourceDef> {
    let mut out = builtin();
    for (name, user_def) in user {
        let merged = match out.get(name) {
            Some(existing) => existing.merge(user_def),
            None => user_def.clone(),
        };
        out.insert(name.clone(), merged);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os_detect::Os;

    #[test]
    fn embedded_catalog_parses() {
        let cat = builtin();
        assert!(cat.contains_key("cargo"));
        assert!(cat.contains_key("winget"));
        assert!(cat.contains_key("brew_arm"));
        assert!(cat.contains_key("pkg"));
    }

    #[test]
    fn cargo_has_per_os_paths() {
        let cat = builtin();
        let cargo = &cat["cargo"];
        assert!(cargo.path_for(Os::Windows).is_some());
        assert!(cargo.path_for(Os::Linux).is_some());
        assert!(cargo.path_for(Os::Macos).is_some());
        assert!(cargo.path_for(Os::Termux).is_some()); // unix fallback
    }

    #[test]
    fn user_override_replaces_only_specified_field() {
        let mut user = BTreeMap::new();
        user.insert(
            "mise".to_string(),
            SourceDef {
                windows: Some("D:/tools/mise".into()),
                ..Default::default()
            },
        );
        let merged = merge_with_user(&user);
        let mise = &merged["mise"];
        assert_eq!(mise.path_for(Os::Windows), Some("D:/tools/mise"));
        // unix fallback from the built-in survives.
        assert!(mise.path_for(Os::Linux).is_some());
    }

    #[test]
    fn user_can_add_new_source() {
        let mut user = BTreeMap::new();
        user.insert(
            "my_dotfiles_bin".to_string(),
            SourceDef {
                unix: Some("$HOME/dotfiles/bin".into()),
                ..Default::default()
            },
        );
        let merged = merge_with_user(&user);
        assert!(merged.contains_key("my_dotfiles_bin"));
    }

    #[test]
    fn linux_only_source_is_none_on_windows() {
        let cat = builtin();
        let apt = &cat["apt"];
        assert!(apt.path_for(Os::Linux).is_some());
        assert!(apt.path_for(Os::Windows).is_none());
        assert!(apt.path_for(Os::Macos).is_none());
        assert!(apt.path_for(Os::Termux).is_none());
    }

    #[test]
    fn user_override_can_replace_all_known_fields() {
        let mut user = BTreeMap::new();
        user.insert(
            "cargo".to_string(),
            SourceDef {
                description: Some("user-overridden".into()),
                windows: Some("X:/cargo".into()),
                unix: Some("/x/cargo".into()),
                ..Default::default()
            },
        );
        let merged = merge_with_user(&user);
        let cargo = &merged["cargo"];
        assert_eq!(cargo.description.as_deref(), Some("user-overridden"));
        assert_eq!(cargo.path_for(Os::Windows), Some("X:/cargo"));
        assert_eq!(cargo.path_for(Os::Linux), Some("/x/cargo"));
    }

    #[test]
    fn embedded_version_is_at_least_one() {
        // Bumping catalog_version is a deliberate act; default of 0
        // would mean somebody removed the declaration. Guard the
        // floor at 1 — the version we shipped in 0.0.3.
        assert!(embedded_version() >= 1);
    }

    #[test]
    fn version_check_passes_when_no_requirement_set() {
        assert!(version_check(None, 0).is_ok());
        assert!(version_check(None, 9999).is_ok());
    }

    #[test]
    fn version_check_passes_when_embedded_meets_required() {
        assert!(version_check(Some(2), 2).is_ok());
        assert!(version_check(Some(2), 3).is_ok());
    }

    #[test]
    fn version_check_fails_when_embedded_below_required() {
        let err = version_check(Some(7), 3).unwrap_err();
        assert!(err.contains("7"), "error must name required: {err}");
        assert!(err.contains("3"), "error must name embedded: {err}");
        assert!(
            err.contains("Upgrade") || err.contains("require_catalog"),
            "error must hint at fix: {err}"
        );
    }

    #[test]
    fn mise_layered_sources_are_present() {
        let cat = builtin();
        assert!(cat.contains_key("mise"));
        assert!(cat.contains_key("mise_shims"));
        assert!(cat.contains_key("mise_installs"));
    }

    #[test]
    fn mise_shims_path_is_a_subdirectory_of_mise() {
        let cat = builtin();
        let mise = cat["mise"].path_for(Os::Linux).unwrap();
        let shims = cat["mise_shims"].path_for(Os::Linux).unwrap();
        let installs = cat["mise_installs"].path_for(Os::Linux).unwrap();
        // mise_shims and mise_installs must each live inside the
        // mise root, so any binary path that matches a subordinate
        // source automatically also matches the parent `mise`.
        assert!(shims.starts_with(mise), "{shims} not under {mise}");
        assert!(installs.starts_with(mise), "{installs} not under {mise}");
    }
}
