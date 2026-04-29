//! End-to-end CLI tests. Each test builds an isolated PATH directory
//! with a stub executable and a TOML manifest, then invokes the real
//! `pathlint` binary and asserts on stdout / exit code.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_pathlint");

fn run(rules: &Path, path_value: &str) -> (i32, String, String) {
    let out = Command::new(BIN)
        .arg("--rules")
        .arg(rules)
        .arg("--no-glyphs")
        .env("PATH", path_value)
        .env_remove("XDG_CONFIG_HOME")
        .output()
        .expect("failed to run pathlint binary");
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    (code, stdout, stderr)
}

/// Place an executable stub named `command` inside `dir`. On Windows
/// the stub is a `.cmd` file (PATHEXT picks it up); on Unix it is a
/// shell script with the executable bit set.
fn stub(dir: &Path, command: &str) -> PathBuf {
    fs::create_dir_all(dir).unwrap();
    if cfg!(windows) {
        let p = dir.join(format!("{command}.cmd"));
        fs::write(&p, "@echo stub\r\n").unwrap();
        p
    } else {
        let p = dir.join(command);
        fs::write(&p, "#!/bin/sh\necho stub\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = fs::metadata(&p).unwrap().permissions();
            perm.set_mode(0o755);
            fs::set_permissions(&p, perm).unwrap();
        }
        p
    }
}

fn write_rules(dir: &Path, body: &str) -> PathBuf {
    let p = dir.join("pathlint.toml");
    fs::write(&p, body).unwrap();
    p
}

fn join_path(entries: &[&Path]) -> String {
    let sep = if cfg!(windows) { ";" } else { ":" };
    entries
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(sep)
}

fn os_tag() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

fn key_for_current_os() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

#[test]
fn check_reports_ok_when_command_resolves_under_preferred_source() {
    let tmp = tempfile::tempdir().unwrap();
    let preferred = tmp.path().join("preferred");
    stub(&preferred, "tooly");

    let key = key_for_current_os();
    let body = format!(
        r#"
[[expect]]
command = "tooly"
prefer  = ["preferred_src"]

[source.preferred_src]
{key} = "{path}"
"#,
        path = preferred.display().to_string().replace('\\', "/"),
    );
    let rules = write_rules(tmp.path(), &body);

    let (code, stdout, _) = run(&rules, &join_path(&[&preferred]));
    assert_eq!(code, 0, "stdout was: {stdout}");
    assert!(stdout.contains("OK"), "stdout was: {stdout}");
    assert!(stdout.contains("tooly"), "stdout was: {stdout}");
}

#[test]
fn check_reports_ng_when_resolved_from_avoided_source() {
    let tmp = tempfile::tempdir().unwrap();
    let avoid_dir = tmp.path().join("avoid");
    stub(&avoid_dir, "tooly");

    let key = key_for_current_os();
    let body = format!(
        r#"
[[expect]]
command = "tooly"
prefer  = ["good"]
avoid   = ["bad"]

[source.good]
{key} = "{good}"

[source.bad]
{key} = "{bad}"
"#,
        good = "/this/path/does/not/exist",
        bad = avoid_dir.display().to_string().replace('\\', "/"),
    );
    let rules = write_rules(tmp.path(), &body);

    let (code, stdout, _) = run(&rules, &join_path(&[&avoid_dir]));
    assert_eq!(code, 1, "stdout was: {stdout}");
    assert!(stdout.contains("NG"), "stdout was: {stdout}");
    assert!(stdout.contains("tooly"), "stdout was: {stdout}");
}

#[test]
fn check_reports_not_found_unless_optional() {
    let tmp = tempfile::tempdir().unwrap();
    let empty_dir = tmp.path().join("empty");
    fs::create_dir_all(&empty_dir).unwrap();

    let body = r#"
[[expect]]
command = "definitely_no_such_tool_xyz"
"#;
    let rules = write_rules(tmp.path(), body);

    let (code, stdout, _) = run(&rules, &join_path(&[&empty_dir]));
    assert_eq!(code, 1, "stdout was: {stdout}");
    assert!(stdout.contains("not found on PATH"), "stdout was: {stdout}");
}

#[test]
fn optional_missing_command_is_skipped() {
    let tmp = tempfile::tempdir().unwrap();
    let empty_dir = tmp.path().join("empty");
    fs::create_dir_all(&empty_dir).unwrap();

    let body = r#"
[[expect]]
command = "definitely_no_such_tool_xyz"
optional = true
"#;
    let rules = write_rules(tmp.path(), body);

    let (code, stdout, _) = run(&rules, &join_path(&[&empty_dir]));
    assert_eq!(code, 0, "stdout was: {stdout}");
    assert!(stdout.contains("skip"), "stdout was: {stdout}");
}

#[test]
fn os_filter_excludes_other_os() {
    let tmp = tempfile::tempdir().unwrap();
    let some_dir = tmp.path().join("d");
    fs::create_dir_all(&some_dir).unwrap();

    let other = if os_tag() == "windows" { "linux" } else { "windows" };
    let body = format!(
        r#"
[[expect]]
command = "definitely_no_such_tool_xyz"
os      = ["{other}"]
"#,
    );
    let rules = write_rules(tmp.path(), &body);

    // Without --verbose the n/a line is hidden, so exit must still be 0.
    let (code, stdout, _) = run(&rules, &join_path(&[&some_dir]));
    assert_eq!(code, 0, "stdout was: {stdout}");
}

#[test]
fn config_error_yields_exit_2() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("d");
    stub(&dir, "tooly");

    let body = r#"
[[expect]]
command = "tooly"
prefer  = ["nonexistent_source"]
"#;
    let rules = write_rules(tmp.path(), body);

    let (code, stdout, _) = run(&rules, &join_path(&[&dir]));
    assert_eq!(code, 2, "stdout was: {stdout}");
    assert!(
        stdout.contains("undefined source name"),
        "stdout was: {stdout}"
    );
}

#[test]
fn missing_rules_path_is_reported_with_exit_2() {
    let tmp = tempfile::tempdir().unwrap();
    let nope = tmp.path().join("does_not_exist.toml");
    let (code, _stdout, stderr) = run(&nope, "");
    assert_eq!(code, 2);
    assert!(stderr.contains("--rules"), "stderr was: {stderr}");
}
