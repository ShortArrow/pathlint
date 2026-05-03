//! Security-focused end-to-end tests. Each scenario sets up a
//! hostile or malformed `pathlint.toml` and asserts pathlint
//! refuses with exit 2 (config error) instead of silently doing
//! the wrong thing.

use std::fs;
use std::path::Path;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_pathlint");

fn write_rules(dir: &Path, body: &str) -> std::path::PathBuf {
    let p = dir.join("pathlint.toml");
    fs::write(&p, body).unwrap();
    p
}

fn run(subcommand: &str, rules: &Path, path_value: &str) -> (i32, String, String) {
    let out = Command::new(BIN)
        .arg("--rules")
        .arg(rules)
        .arg(subcommand)
        .env("PATH", path_value)
        .env_remove("XDG_CONFIG_HOME")
        .output()
        .expect("failed to run pathlint binary");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn check_rejects_user_source_pointing_at_root() {
    let tmp = tempfile::tempdir().unwrap();
    let key = if cfg!(windows) { "windows" } else { "unix" };
    let body = format!(
        r#"
[source.evil]
{key} = "/"
"#
    );
    let rules = write_rules(tmp.path(), &body);

    let (code, _stdout, stderr) = run("check", &rules, "/usr/bin");
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(
        stderr.contains("evil") && stderr.contains("rejected"),
        "stderr should name the rejected source: {stderr}"
    );
}

#[test]
fn doctor_rejects_user_source_pointing_at_root() {
    let tmp = tempfile::tempdir().unwrap();
    let key = if cfg!(windows) { "windows" } else { "unix" };
    let body = format!(
        r#"
[source.evil]
{key} = "/"
"#
    );
    let rules = write_rules(tmp.path(), &body);

    let (code, _stdout, stderr) = run("doctor", &rules, "/usr/bin");
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(stderr.contains("rejected"), "stderr: {stderr}");
}

#[test]
fn where_rejects_user_source_pointing_at_root() {
    let tmp = tempfile::tempdir().unwrap();
    let key = if cfg!(windows) { "windows" } else { "unix" };
    let body = format!(
        r#"
[source.evil]
{key} = "/"
"#
    );
    let rules = write_rules(tmp.path(), &body);

    let mut cmd = Command::new(BIN);
    cmd.arg("--rules")
        .arg(&rules)
        .arg("where")
        .arg("ls")
        .env("PATH", "/usr/bin")
        .env_remove("XDG_CONFIG_HOME");
    let out = cmd.output().unwrap();
    assert_eq!(out.status.code().unwrap_or(-1), 2);
}

#[test]
fn check_rejects_too_short_needle() {
    let tmp = tempfile::tempdir().unwrap();
    let key = if cfg!(windows) { "windows" } else { "unix" };
    let body = format!(
        r#"
[source.tiny]
{key} = "ab"
"#
    );
    let rules = write_rules(tmp.path(), &body);

    let (code, _stdout, stderr) = run("check", &rules, "/usr/bin");
    assert_eq!(code, 2, "stderr: {stderr}");
}

#[test]
fn check_accepts_normal_user_sources() {
    // Sanity: legitimate user overrides must still pass.
    let tmp = tempfile::tempdir().unwrap();
    let key = if cfg!(windows) { "windows" } else { "unix" };
    let body = format!(
        r#"
[source.dotfiles]
{key} = "/home/user/dotfiles/bin"
"#
    );
    let rules = write_rules(tmp.path(), &body);

    let (code, stdout, stderr) = run("check", &rules, "/usr/bin");
    assert!(
        code == 0 || code == 1,
        "expected lint pass or expectation fail, got code={code}; stdout: {stdout} stderr: {stderr}"
    );
}
