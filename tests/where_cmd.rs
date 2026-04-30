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
    assert!(
        stdout.contains("cargo uninstall lazygit"),
        "uninstall hint missing: {stdout}"
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
