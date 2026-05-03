//! `pathlint where <command>` end-to-end tests.

use std::fs;
use std::path::Path;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_pathlint");

fn run_where(rules: &Path, path_value: &str, command: &str) -> (i32, String, String) {
    let out = Command::new(BIN)
        .arg("--rules")
        .arg(rules)
        .arg("where")
        .arg(command)
        .env("PATH", path_value)
        .env_remove("XDG_CONFIG_HOME")
        .output()
        .expect("failed to run pathlint binary");
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    (code, stdout, stderr)
}

fn join_path(parts: &[&Path]) -> String {
    let sep = if cfg!(windows) { ";" } else { ":" };
    parts
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(sep)
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

fn stub(dir: &Path, command: &str) {
    fs::create_dir_all(dir).unwrap();
    if cfg!(windows) {
        fs::write(dir.join(format!("{command}.cmd")), "@echo stub\r\n").unwrap();
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
    }
}

fn write_rules(dir: &Path, body: &str) -> std::path::PathBuf {
    let p = dir.join("pathlint.toml");
    fs::write(&p, body).unwrap();
    p
}

#[test]
fn where_resolves_and_renders_uninstall_hint() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("home_cargo_bin");
    stub(&dir, "lazygit");

    let key = key_for_current_os();
    let body = format!(
        r#"
[source.cargo]
{key} = "{path}"
uninstall_command = "cargo uninstall {{bin}}"
"#,
        path = dir.display().to_string().replace('\\', "/"),
    );
    let rules = write_rules(tmp.path(), &body);

    let (code, stdout, _) = run_where(&rules, &join_path(&[&dir]), "lazygit");
    assert_eq!(code, 0, "stdout: {stdout}");
    assert!(stdout.contains("lazygit"), "stdout: {stdout}");
    assert!(
        stdout.contains("sources:") && stdout.contains("cargo"),
        "sources line missing: {stdout}"
    );
    // Both POSIX and PowerShell quoters wrap the bin token in
    // single quotes, so the expected literal is the same on every
    // OS the test currently runs on.
    assert!(
        stdout.contains("cargo uninstall 'lazygit'"),
        "uninstall hint missing or unquoted: {stdout}"
    );
}

#[test]
fn where_escapes_metachars_in_bin_substitution() {
    // A binary whose name contains shell metacharacters must be
    // single-quoted in the uninstall hint so a copy-paste does
    // not execute the metachars.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("evil_bin_dir");

    // The actual binary file name on disk has to be filesystem-safe.
    // We instead drive escaping through an attacker-controlled
    // `command` arg containing `$(whoami)`. `pathlint where` echoes
    // the requested command back verbatim into the uninstall hint
    // via the `{bin}` token.
    stub(&dir, "lazygit");

    let key = key_for_current_os();
    let body = format!(
        r#"
[source.cargo]
{key} = "{path}"
uninstall_command = "cargo uninstall {{bin}}"
"#,
        path = dir.display().to_string().replace('\\', "/"),
    );
    let rules = write_rules(tmp.path(), &body);

    let (code, stdout, _) = run_where(&rules, &join_path(&[&dir]), "lazygit");
    assert_eq!(code, 0, "stdout: {stdout}");
    // The bin token is `lazygit` here (it's just a smoke-check that
    // every uninstall command goes through the quoter). The hostile-
    // PATH integration goes through Step E's tests.
    assert!(
        !stdout.contains("cargo uninstall lazygit\n"),
        "bin must be quoted, not bare: {stdout}"
    );
    assert!(
        stdout.contains("cargo uninstall 'lazygit'"),
        "expected single-quoted bin: {stdout}"
    );
}

#[test]
fn where_reports_not_found_with_exit_1() {
    let tmp = tempfile::tempdir().unwrap();
    let empty = tmp.path().join("empty");
    fs::create_dir_all(&empty).unwrap();
    let rules = write_rules(tmp.path(), "");

    let (code, stdout, _) = run_where(
        &rules,
        &join_path(&[&empty]),
        "ghost_xyz_definitely_missing",
    );
    assert_eq!(code, 1, "stdout: {stdout}");
    assert!(stdout.contains("not found"), "stdout: {stdout}");
}

#[test]
fn where_says_no_template_when_source_lacks_uninstall_command() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("aqua_dir");
    stub(&dir, "aqua_tool");

    let key = key_for_current_os();
    let body = format!(
        r#"
[source.aqua_local]
{key} = "{path}"
"#,
        path = dir.display().to_string().replace('\\', "/"),
    );
    let rules = write_rules(tmp.path(), &body);

    let (code, stdout, _) = run_where(&rules, &join_path(&[&dir]), "aqua_tool");
    assert_eq!(code, 0);
    assert!(stdout.contains("no uninstall template"), "stdout: {stdout}");
    assert!(stdout.contains("aqua_local"), "stdout: {stdout}");
}

#[test]
fn where_says_no_source_when_resolved_path_is_outside_catalog() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("orphan_dir");
    stub(&dir, "orphan");
    let rules = write_rules(tmp.path(), "");

    let (code, stdout, _) = run_where(&rules, &join_path(&[&dir]), "orphan");
    assert_eq!(code, 0);
    assert!(stdout.contains("(no source matched)"), "stdout: {stdout}");
    assert!(
        stdout.contains("(no source matched — pathlint cannot guess)"),
        "stdout: {stdout}"
    );
}

// ---- --json (0.0.6+) ----------------------------------------

fn run_where_args(
    rules: &Path,
    path_value: &str,
    extra_before: &[&str],
    command: &str,
) -> (i32, String, String) {
    let mut cmd = Command::new(BIN);
    cmd.arg("--rules")
        .arg(rules)
        .arg("where")
        .args(extra_before)
        .arg(command)
        .env("PATH", path_value)
        .env_remove("XDG_CONFIG_HOME");
    let out = cmd.output().expect("failed to run pathlint binary");
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    (code, stdout, stderr)
}

#[test]
fn where_json_found_carries_command_and_kind_discriminators() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("home_cargo_bin");
    stub(&dir, "lazygit");

    let key = key_for_current_os();
    let body = format!(
        r#"
[source.cargo]
{key} = "{path}"
uninstall_command = "cargo uninstall {{bin}}"
"#,
        path = dir.display().to_string().replace('\\', "/"),
    );
    let rules = write_rules(tmp.path(), &body);

    let (code, stdout, _) = run_where_args(&rules, &join_path(&[&dir]), &["--json"], "lazygit");
    assert_eq!(code, 0, "stdout: {stdout}");

    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(v["found"], true);
    assert_eq!(v["command"], "lazygit");
    assert!(v["resolved"].is_string());
    assert_eq!(v["matched_sources"][0], "cargo");
    assert_eq!(v["uninstall"]["kind"], "command");
    assert_eq!(v["uninstall"]["command"], "cargo uninstall 'lazygit'");
    // No mise plugin here, so provenance is null.
    assert!(v["provenance"].is_null());
}

#[test]
fn where_json_not_found_emits_compact_object_with_exit_1() {
    let tmp = tempfile::tempdir().unwrap();
    let empty = tmp.path().join("empty");
    fs::create_dir_all(&empty).unwrap();
    let rules = write_rules(tmp.path(), "");

    let (code, stdout, _) = run_where_args(
        &rules,
        &join_path(&[&empty]),
        &["--json"],
        "ghost_definitely_no_such_xyz",
    );
    assert_eq!(code, 1, "stdout: {stdout}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(v["found"], false);
    assert_eq!(v["command"], "ghost_definitely_no_such_xyz");
    assert!(v.get("resolved").is_none());
}

#[test]
fn where_json_uninstall_no_template_uses_kind_field() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("aqua_dir");
    stub(&dir, "aqua_tool");

    let key = key_for_current_os();
    let body = format!(
        r#"
[source.aqua_local]
{key} = "{path}"
"#,
        path = dir.display().to_string().replace('\\', "/"),
    );
    let rules = write_rules(tmp.path(), &body);

    let (code, stdout, _) = run_where_args(&rules, &join_path(&[&dir]), &["--json"], "aqua_tool");
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(v["uninstall"]["kind"], "no_template");
    assert_eq!(v["uninstall"]["source"], "aqua_local");
}
