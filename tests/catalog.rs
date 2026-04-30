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
