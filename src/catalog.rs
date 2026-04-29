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
}
