//! Built-in source catalog and merge with user-defined sources.

use std::collections::BTreeMap;

use crate::config::{Config, SourceDef};

const EMBEDDED: &str = include_str!("embedded_catalog.toml");

/// Parse the embedded catalog (panics on failure — would be a build bug).
pub fn builtin() -> BTreeMap<String, SourceDef> {
    let cfg: Config = Config::parse_toml(EMBEDDED).expect("embedded_catalog.toml must parse");
    cfg.source
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
