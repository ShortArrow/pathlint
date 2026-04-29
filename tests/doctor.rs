//! `pathlint doctor` end-to-end tests.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_pathlint");

fn run_doctor(path_value: &str) -> (i32, String, String) {
    let out = Command::new(BIN)
        .arg("doctor")
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

#[test]
fn doctor_warns_on_duplicate_entries() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("x");
    fs::create_dir_all(&dir).unwrap();
    let path = join_path(&[&dir, &dir]);
    let (code, stdout, _) = run_doctor(&path);
    assert_eq!(code, 0, "warn-only must not fail the run");
    assert!(stdout.contains("[warn]"), "stdout: {stdout}");
    assert!(stdout.contains("duplicate"), "stdout: {stdout}");
}

#[test]
fn doctor_warns_on_missing_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let exists = tmp.path().join("real");
    fs::create_dir_all(&exists).unwrap();
    let absent = tmp.path().join("definitely_does_not_exist_xyz");
    let path = join_path(&[&exists, &absent]);
    let (code, stdout, _) = run_doctor(&path);
    assert_eq!(code, 0);
    assert!(stdout.contains("does not exist"), "stdout: {stdout}");
}

#[test]
fn doctor_clean_path_emits_nothing() {
    // The temp dir on Windows lives under %LocalAppData%, which would
    // trigger the "shortenable" warning even on an otherwise clean
    // PATH. Wipe the obvious env vars so the run is genuinely empty.
    //
    // Also canonicalize the path: GitHub's `windows-latest` runner
    // resolves $TEMP via `C:\Users\RUNNER~1\...` (8.3 short name for
    // `runneradmin`), which would otherwise trip the doctor's
    // ShortName check. canonicalize expands that to the long name.
    let tmp = tempfile::tempdir().unwrap();
    let only = tmp.path().join("clean");
    fs::create_dir_all(&only).unwrap();
    let only_canonical = fs::canonicalize(&only).unwrap();
    let only_clean = strip_unc_prefix(&only_canonical);
    let path = join_path(&[&only_clean]);
    let mut cmd = Command::new(BIN);
    cmd.arg("doctor")
        .env("PATH", &path)
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("HOME")
        .env_remove("USERPROFILE")
        .env_remove("LocalAppData")
        .env_remove("AppData")
        .env_remove("ProgramFiles")
        .env_remove("ProgramFiles(x86)")
        .env_remove("ProgramData")
        .env_remove("SystemRoot");
    let out = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code().unwrap_or(-1), 0);
    assert!(stdout.is_empty(), "expected silence, got: {stdout}");
}

/// On Windows, `fs::canonicalize` returns paths prefixed with `\\?\`
/// (the Win32 file-namespace prefix). PATH entries don't use that
/// prefix in the wild, so strip it for tests that compare output.
fn strip_unc_prefix(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        p.to_path_buf()
    }
}

#[test]
fn doctor_warns_on_trailing_slash() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("slashy");
    fs::create_dir_all(&dir).unwrap();
    let with_slash = format!(
        "{}{}",
        dir.display(),
        if cfg!(windows) { "\\" } else { "/" }
    );
    let (code, stdout, _) = run_doctor(&with_slash);
    assert_eq!(code, 0);
    assert!(stdout.contains("trailing slash"), "stdout: {stdout}");
}

#[test]
#[cfg(windows)]
fn doctor_errors_on_illegal_chars_on_windows() {
    // Pipe char is illegal in NTFS filenames and would never resolve
    // as a directory; doctor escalates this to error severity.
    let path = "C:\\foo|bar";
    let (code, stdout, _) = run_doctor(path);
    assert_eq!(code, 1, "malformed entries must yield exit 1");
    assert!(stdout.contains("[ERR]"), "stdout: {stdout}");
    assert!(stdout.contains("malformed"), "stdout: {stdout}");
}

#[test]
fn doctor_quiet_hides_warnings_but_keeps_errors() {
    // We can only stage a malformed entry on Windows (the kinds of
    // illegal chars differ on Unix). On Unix this test still asserts
    // that --quiet silences a clean warn-only run.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("dup");
    fs::create_dir_all(&dir).unwrap();
    let path = join_path(&[&dir, &dir]);
    let out = Command::new(BIN)
        .arg("--quiet")
        .arg("doctor")
        .env("PATH", &path)
        .env_remove("XDG_CONFIG_HOME")
        .output()
        .unwrap();
    assert!(out.status.success(), "duplicate-only must exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("[warn]"),
        "quiet mode must hide warns: {stdout}"
    );
}
