//! `pathlint init` end-to-end tests.

use std::fs;
use std::path::Path;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_pathlint");

fn run_init(cwd: &Path, args: &[&str]) -> (i32, String, String) {
    let mut cmd = Command::new(BIN);
    cmd.arg("init")
        .args(args)
        .current_dir(cwd)
        // Force a deterministic config search root so this test's cwd
        // never touches the developer's real $HOME-based defaults.
        .env_remove("XDG_CONFIG_HOME");
    let out = cmd.output().expect("failed to run pathlint binary");
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    (code, stdout, stderr)
}

#[test]
fn init_writes_starter_toml_in_cwd() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("pathlint.toml");
    assert!(!target.exists(), "precondition");

    let (code, stdout, _) = run_init(tmp.path(), &[]);
    assert_eq!(code, 0, "stdout: {stdout}");
    assert!(target.is_file(), "starter file must be created");
    assert!(stdout.contains("wrote"), "stdout: {stdout}");

    let body = fs::read_to_string(&target).unwrap();
    assert!(body.contains("[[expect]]"));
    assert!(body.contains("cargo"));
}

#[test]
fn init_refuses_to_overwrite_existing_file() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("pathlint.toml");
    fs::write(&target, "stale = true\n").unwrap();

    let (code, _stdout, stderr) = run_init(tmp.path(), &[]);
    assert_eq!(code, 1);
    assert!(stderr.contains("already exists"), "stderr: {stderr}");

    let body = fs::read_to_string(&target).unwrap();
    assert!(body.contains("stale = true"), "must not have overwritten");
}

#[test]
fn init_force_overwrites() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("pathlint.toml");
    fs::write(&target, "stale = true\n").unwrap();

    let (code, _stdout, _) = run_init(tmp.path(), &["--force"]);
    assert_eq!(code, 0);

    let body = fs::read_to_string(&target).unwrap();
    assert!(!body.contains("stale = true"));
    assert!(body.contains("[[expect]]"));
}

#[test]
fn init_emit_defaults_includes_full_catalog() {
    let tmp = tempfile::tempdir().unwrap();
    let (code, _stdout, _) = run_init(tmp.path(), &["--emit-defaults"]);
    assert_eq!(code, 0);

    let body = fs::read_to_string(tmp.path().join("pathlint.toml")).unwrap();
    // Sample sources from the embedded catalog.
    assert!(body.contains("[source.cargo]"));
    assert!(body.contains("[source.winget]"));
    assert!(body.contains("[source.brew_arm]"));
    assert!(body.contains("[source.apt]"));
}

#[test]
fn check_consumes_starter_without_error() {
    // The starter file pathlint init writes must itself parse
    // cleanly. We verify that by running `pathlint --rules <starter>`
    // against an empty PATH; the only failures expected come from
    // commands not being found, but config-error / parse-error
    // (exit 2) must not happen.
    let tmp = tempfile::tempdir().unwrap();
    let (code, _stdout, _) = run_init(tmp.path(), &[]);
    assert_eq!(code, 0);

    let starter = tmp.path().join("pathlint.toml");
    let out = Command::new(BIN)
        .arg("--rules")
        .arg(&starter)
        .env("PATH", "")
        .env_remove("XDG_CONFIG_HOME")
        .output()
        .expect("failed to run pathlint check");
    let status = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_ne!(status, 2, "starter caused a config error: {stdout}");
}
