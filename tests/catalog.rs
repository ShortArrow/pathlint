//! `pathlint catalog list` end-to-end tests.

use std::fs;
use std::path::Path;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_pathlint");

fn run_catalog_list(cwd: &Path, args: &[&str]) -> (i32, String, String) {
    run_with_global(cwd, &[], args)
}

fn run_with_global(cwd: &Path, global: &[&str], list_args: &[&str]) -> (i32, String, String) {
    let mut cmd = Command::new(BIN);
    cmd.args(global)
        .arg("catalog")
        .arg("list")
        .args(list_args)
        .current_dir(cwd)
        .env_remove("XDG_CONFIG_HOME");
    let out = cmd.output().expect("failed to run pathlint binary");
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    (code, stdout, stderr)
}

#[test]
fn catalog_list_default_includes_built_in_sources() {
    let tmp = tempfile::tempdir().unwrap();
    let (code, stdout, _) = run_catalog_list(tmp.path(), &[]);
    assert_eq!(code, 0);
    for name in ["cargo", "mise", "winget", "brew_arm", "apt", "pkg"] {
        assert!(stdout.contains(name), "missing {name} in: {stdout}");
    }
}

#[test]
fn catalog_list_names_only_emits_one_name_per_line() {
    let tmp = tempfile::tempdir().unwrap();
    let (code, stdout, _) = run_catalog_list(tmp.path(), &["--names-only"]);
    assert_eq!(code, 0);
    let names: Vec<&str> = stdout.lines().collect();
    assert!(names.contains(&"cargo"), "names: {names:?}");
    assert!(names.contains(&"winget"), "names: {names:?}");
    for line in &names {
        assert!(
            !line.contains(' '),
            "names-only line must have no spaces: {line:?}"
        );
    }
}

#[test]
fn catalog_list_all_shows_every_per_os_field() {
    let tmp = tempfile::tempdir().unwrap();
    let (code, stdout, _) = run_catalog_list(tmp.path(), &["--all"]);
    assert_eq!(code, 0);
    // brew_arm only has macos, apt only has linux, pkg only termux.
    assert!(stdout.contains("macos"));
    assert!(stdout.contains("linux"));
    assert!(stdout.contains("termux"));
    assert!(stdout.contains("windows"));
}

#[test]
fn catalog_list_picks_up_user_overrides_via_rules() {
    let tmp = tempfile::tempdir().unwrap();
    let rules = tmp.path().join("pathlint.toml");
    fs::write(
        &rules,
        r#"
[source.my_dotfiles_bin]
unix = "$HOME/dotfiles/bin"
"#,
    )
    .unwrap();

    let (code, stdout, _) = run_with_global(
        tmp.path(),
        &["--rules", rules.to_str().unwrap()],
        &["--names-only"],
    );
    assert_eq!(code, 0);
    assert!(stdout.lines().any(|l| l == "my_dotfiles_bin"));
    // Built-ins are still present.
    assert!(stdout.lines().any(|l| l == "cargo"));
}

#[test]
fn catalog_list_default_includes_catalog_version() {
    // The version line should appear at the top of default output
    // (so users can spot which catalog vintage they're matching
    // against), but NOT in --names-only.
    let tmp = tempfile::tempdir().unwrap();
    let (code, stdout, _) = run_catalog_list(tmp.path(), &[]);
    assert_eq!(code, 0);
    let first = stdout.lines().next().unwrap_or("");
    assert!(
        first.starts_with("# catalog_version = "),
        "first line should announce the catalog version: {first}"
    );

    let (_, names_only, _) = run_catalog_list(tmp.path(), &["--names-only"]);
    assert!(
        !names_only.contains("catalog_version"),
        "--names-only must stay machine-readable: {names_only}"
    );
}

// ---- catalog relations (0.0.9+) ----------------------------------

fn run_catalog_relations(cwd: &Path, global: &[&str], args: &[&str]) -> (i32, String, String) {
    let mut cmd = Command::new(BIN);
    cmd.args(global)
        .arg("catalog")
        .arg("relations")
        .args(args)
        .current_dir(cwd)
        .env_remove("XDG_CONFIG_HOME");
    let out = cmd.output().expect("failed to run pathlint binary");
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    (code, stdout, stderr)
}

#[test]
fn catalog_relations_default_includes_builtin_mise_relations() {
    let tmp = tempfile::tempdir().unwrap();
    let (code, stdout, _) = run_catalog_relations(tmp.path(), &[], &[]);
    assert_eq!(code, 0);
    // Built-in mise plugin declares all four kinds we care about.
    assert!(stdout.contains("alias_of"), "stdout: {stdout}");
    assert!(
        stdout.contains("conflicts_when_both_in_path"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("served_by_via"), "stdout: {stdout}");
    assert!(stdout.contains("`mise`"));
    assert!(stdout.contains("mise_activate_both"));
}

#[test]
fn catalog_relations_json_emits_array_with_kind_discriminator() {
    let tmp = tempfile::tempdir().unwrap();
    let (code, stdout, _) = run_catalog_relations(tmp.path(), &[], &["--json"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).expect(&stdout);
    let arr = v.as_array().expect("must be an array");
    assert!(!arr.is_empty(), "built-in relations must not be empty");
    // Every element carries `kind`.
    for r in arr {
        assert!(r["kind"].is_string(), "missing kind discriminator: {r}");
    }
    // Built-in catalog declares the alias_of for mise.
    assert!(
        arr.iter()
            .any(|r| r["kind"] == "alias_of" && r["parent"] == "mise"),
        "built-in alias_of mise missing"
    );
}

#[test]
fn catalog_relations_appends_user_relations_at_the_end() {
    let tmp = tempfile::tempdir().unwrap();
    let rules = tmp.path().join("pathlint.toml");
    fs::write(
        &rules,
        r#"
[[relation]]
kind = "depends_on"
source = "paru"
target = "pacman"
"#,
    )
    .unwrap();

    let (code, stdout, _) = run_catalog_relations(
        tmp.path(),
        &["--rules", rules.to_str().unwrap()],
        &["--json"],
    );
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).expect(&stdout);
    let arr = v.as_array().unwrap();
    let last = arr.last().unwrap();
    assert_eq!(last["kind"], "depends_on");
    assert_eq!(last["source"], "paru");
    assert_eq!(last["target"], "pacman");
}

#[test]
fn catalog_relations_rejects_user_cycle_with_exit_2() {
    // A two-node cycle through depends_on must be caught at startup
    // and reported as a config error (exit 2).
    let tmp = tempfile::tempdir().unwrap();
    let rules = tmp.path().join("pathlint.toml");
    fs::write(
        &rules,
        r#"
[[relation]]
kind = "depends_on"
source = "a"
target = "b"

[[relation]]
kind = "depends_on"
source = "b"
target = "a"
"#,
    )
    .unwrap();

    let (code, _stdout, stderr) =
        run_catalog_relations(tmp.path(), &["--rules", rules.to_str().unwrap()], &[]);
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(stderr.contains("cycle"), "stderr: {stderr}");
}

#[test]
fn catalog_list_rejects_unknown_subcommand() {
    let tmp = tempfile::tempdir().unwrap();
    let mut cmd = Command::new(BIN);
    cmd.arg("catalog")
        .arg("nope")
        .current_dir(tmp.path())
        .env_remove("XDG_CONFIG_HOME");
    let out = cmd.output().unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unrecognized") || stderr.contains("not found"),
        "stderr: {stderr}"
    );
}
