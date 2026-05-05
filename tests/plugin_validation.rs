//! `cargo test` で `plugins/*.toml` が main crate の plugin shape
//! (`SourceDef` / `Relation`) と整合するか確認する。
//!
//! `build.rs` にも shape 検証ロジックがあるが、build script の
//! `#[cfg(test)] mod tests` は cargo test の対象外（rustc が build
//! script として走らせるだけ）。本ファイルは runtime 側の二重
//! チェックとして CI が `cargo test` のたびに走る。
//!
//! 二重で見るのは意図的: build.rs は build-time に「壊れた plugin
//! を main crate に取り込むのを止める」、こちらは runtime
//! 型と plugin TOML が乖離していないかを継続的に保証する。
//!
//! 0.0.15: parse via `PluginFileShape` (source + relation) instead
//! of `Config`; `catalog_version` belongs to `plugins/_index.toml`
//! only and is asserted absent in every individual plugin file.

use std::fs;
use std::path::Path;

#[test]
fn every_plugin_toml_parses_as_plugin_file_shape() {
    // PluginFileShape has #[serde(deny_unknown_fields)], so any
    // field a plugin TOML carries that the runtime does not know
    // would fail this test — including a typo in a relation kind
    // name or a stray catalog_version (which belongs to _index.toml).
    let plugins_dir = Path::new("plugins");
    let entries = fs::read_dir(plugins_dir).expect("plugins/ must exist");
    let mut checked = 0;
    for entry in entries {
        let entry = entry.unwrap();
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if !name.ends_with(".toml") || name == "_index.toml" {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap();
        let _: pathlint::catalog::PluginFileShape = toml::from_str(&text)
            .unwrap_or_else(|e| panic!("plugin {name} fails to parse as PluginFileShape: {e}"));
        checked += 1;
    }
    assert!(
        checked >= 5,
        "fewer than 5 plugin files were exercised; cwd is probably wrong"
    );
}

#[test]
fn no_plugin_toml_contains_catalog_version_key() {
    // catalog_version belongs to plugins/_index.toml only; if a
    // plugin file accidentally declares it, the embed-time
    // concatenation in build.rs would produce a TOML with two
    // catalog_version keys (parse error). Catch the violation
    // pre-build at cargo test time.
    let plugins_dir = Path::new("plugins");
    for entry in fs::read_dir(plugins_dir).expect("plugins/ must exist") {
        let entry = entry.unwrap();
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if !name.ends_with(".toml") || name == "_index.toml" {
            continue;
        }
        let body = fs::read_to_string(&path).unwrap();
        assert!(
            !body.contains("catalog_version"),
            "plugin {name} declares catalog_version; that key belongs to _index.toml only"
        );
    }
}

#[test]
fn index_toml_lists_existing_plugin_files() {
    // _index.toml has plugins = ["cargo", "go", ...]; each entry
    // must map to a sibling TOML file. build.rs already enforces
    // this via panic, but a cargo test catches the violation
    // pre-build (e.g. when a contributor adds a plugin to
    // _index.toml but forgets to commit the TOML file).
    let index_text = fs::read_to_string("plugins/_index.toml").unwrap();
    let parsed: toml::Value = toml::from_str(&index_text).unwrap();
    let plugins = parsed
        .get("plugins")
        .and_then(|v| v.as_array())
        .expect("_index.toml must declare a `plugins` array");
    for name in plugins {
        let name = name.as_str().expect("plugin entries are strings");
        let path = format!("plugins/{name}.toml");
        assert!(
            Path::new(&path).is_file(),
            "_index.toml lists `{name}` but {path} does not exist"
        );
    }
}
