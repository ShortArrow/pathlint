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

#[test]
fn check_rejects_oversize_rules_file() {
    // 17 MiB of dummy TOML — over the 16 MiB cap. The file does
    // not need to be valid TOML; the size guard fires before any
    // parse happens.
    let tmp = tempfile::tempdir().unwrap();
    let rules = tmp.path().join("huge.toml");
    let chunk = vec![b'#'; 1024 * 1024];
    let mut buf = Vec::with_capacity(chunk.len() * 17);
    for _ in 0..17 {
        buf.extend_from_slice(&chunk);
    }
    fs::write(&rules, &buf).unwrap();

    let (code, _stdout, stderr) = run("check", &rules, "/usr/bin");
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(
        stderr.contains("too large") || stderr.contains("16 MiB"),
        "stderr should mention the size cap: {stderr}"
    );
}

#[cfg(unix)]
#[test]
fn check_accepts_single_symlink_to_real_file() {
    use std::os::unix::fs::symlink;

    let tmp = tempfile::tempdir().unwrap();
    let real = tmp.path().join("real.toml");
    fs::write(&real, "").unwrap();
    let link = tmp.path().join("link.toml");
    symlink(&real, &link).unwrap();

    let (code, stdout, stderr) = run("check", &link, "/usr/bin");
    // Empty config: lint passes (exit 0). Specifically NOT exit 2.
    assert_eq!(
        code, 0,
        "single symlink to a real file must be accepted; stdout: {stdout} stderr: {stderr}"
    );
}

#[cfg(unix)]
#[test]
fn check_rejects_double_symlinked_rules() {
    use std::os::unix::fs::symlink;

    let tmp = tempfile::tempdir().unwrap();
    let real = tmp.path().join("real.toml");
    fs::write(&real, "").unwrap();
    let link1 = tmp.path().join("link1.toml");
    symlink(&real, &link1).unwrap();
    let link2 = tmp.path().join("link2.toml");
    symlink(&link1, &link2).unwrap();

    let (code, _stdout, stderr) = run("check", &link2, "/usr/bin");
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(
        stderr.contains("not a regular file") || stderr.contains("symlink"),
        "stderr should mention the symlink policy: {stderr}"
    );
}
