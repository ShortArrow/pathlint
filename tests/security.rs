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

#[cfg(unix)]
#[test]
fn check_accepts_relative_symlink_to_real_file() {
    // 0.0.13: 1-hop symlink whose target is a *relative* path
    // (e.g. `real.toml`, not `/abs/real.toml`) must be resolved
    // relative to the link's parent directory, not relative to
    // the process cwd. Earlier code stat-ed the relative target
    // directly and either spuriously rejected or spuriously
    // accepted depending on what cwd happened to be.
    use std::os::unix::fs::symlink;

    let tmp = tempfile::tempdir().unwrap();
    let real = tmp.path().join("real.toml");
    fs::write(&real, "").unwrap();
    let link = tmp.path().join("link.toml");
    symlink("real.toml", &link).unwrap(); // relative target

    let (code, stdout, stderr) = run("check", &link, "/usr/bin");
    assert_eq!(
        code, 0,
        "relative-target single symlink must be accepted; stdout: {stdout} stderr: {stderr}"
    );
}

#[test]
fn sort_requires_dry_run_flag() {
    // 0.0.14: --dry-run is opt-in. `pathlint sort` without it
    // exits 2 with an explanation that --apply is reserved for
    // post-1.0.
    let tmp = tempfile::tempdir().unwrap();
    let rules = write_rules(tmp.path(), "");

    let mut cmd = Command::new(BIN);
    cmd.arg("--rules")
        .arg(&rules)
        .arg("sort")
        .env("PATH", "/usr/bin")
        .env_remove("XDG_CONFIG_HOME");
    let out = cmd.output().expect("failed to run pathlint binary");
    assert_eq!(out.status.code().unwrap_or(-1), 2);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--dry-run"),
        "stderr should explain the missing flag: {stderr}"
    );
}

#[test]
fn check_rejects_user_catalog_version() {
    // 0.0.14: catalog_version is reserved for the embedded
    // catalog. A user pathlint.toml that declares it is a
    // configuration error (exit 2). Use require_catalog = N to
    // pin a minimum embedded catalog version instead.
    let tmp = tempfile::tempdir().unwrap();
    let body = "catalog_version = 5\n";
    let rules = write_rules(tmp.path(), body);

    let (code, _stdout, stderr) = run("check", &rules, "/usr/bin");
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(
        stderr.contains("catalog_version") || stderr.contains("reserved"),
        "stderr should explain the rejection: {stderr}"
    );
}

#[test]
fn user_config_rejects_catalog_version_via_deny_unknown_fields() {
    // 0.0.15 Step B: catalog_version is structurally absent from
    // UserConfig. The reject path is now serde's
    // deny_unknown_fields, not a post-parse ConfigError variant.
    // The error message must still mention the field so 0.0.14
    // users keep recognising the error.
    let body = "catalog_version = 5\n";
    let err = pathlint::config::Config::parse_toml(body)
        .expect_err("user TOML must reject catalog_version");
    let msg = err.to_string();
    assert!(
        msg.contains("catalog_version"),
        "error must mention the field: {msg}"
    );
}

#[test]
fn sort_rejects_user_relation_cycle() {
    // 0.0.14: every relation consumer (sort/doctor/trace, plus
    // the pre-existing catalog relations) must surface a cycle as
    // exit 2. Before 0.0.14, sort silently bubble-passed through
    // a cycle until its N^2 guard expired and emitted a partial
    // reorder.
    let tmp = tempfile::tempdir().unwrap();
    let body = r#"
[[relation]]
kind = "prefer_order_over"
earlier = "a"
later = "b"

[[relation]]
kind = "prefer_order_over"
earlier = "b"
later = "a"
"#;
    let rules = write_rules(tmp.path(), body);

    let mut cmd = Command::new(BIN);
    cmd.arg("--rules")
        .arg(&rules)
        .arg("sort")
        .arg("--dry-run")
        .env("PATH", "/usr/bin")
        .env_remove("XDG_CONFIG_HOME");
    let out = cmd.output().expect("failed to run pathlint binary");
    assert_eq!(out.status.code().unwrap_or(-1), 2);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("cycle"),
        "stderr should mention the cycle: {stderr}"
    );
}

#[test]
fn doctor_rejects_user_relation_cycle() {
    let tmp = tempfile::tempdir().unwrap();
    let body = r#"
[[relation]]
kind = "depends_on"
source = "a"
target = "b"

[[relation]]
kind = "depends_on"
source = "b"
target = "a"
"#;
    let rules = write_rules(tmp.path(), body);

    let (code, _stdout, stderr) = run("doctor", &rules, "/usr/bin");
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(stderr.contains("cycle"), "stderr: {stderr}");
}

#[test]
fn where_rejects_user_relation_cycle() {
    let tmp = tempfile::tempdir().unwrap();
    let body = r#"
[[relation]]
kind = "served_by_via"
host = "a"
guest_pattern = "x-*"
guest_provider = "b"

[[relation]]
kind = "served_by_via"
host = "b"
guest_pattern = "y-*"
guest_provider = "a"
"#;
    let rules = write_rules(tmp.path(), body);

    let mut cmd = Command::new(BIN);
    cmd.arg("--rules")
        .arg(&rules)
        .arg("where")
        .arg("ls")
        .env("PATH", "/usr/bin")
        .env_remove("XDG_CONFIG_HOME");
    let out = cmd.output().unwrap();
    assert_eq!(out.status.code().unwrap_or(-1), 2);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("cycle"), "stderr: {stderr}");
}

#[cfg(unix)]
#[test]
fn check_rejects_relative_symlink_chain() {
    // 0.0.13: a relative-target symlink chain (link2 -> link1 ->
    // real.toml, both relative) must still be rejected by the
    // multi-hop policy.
    use std::os::unix::fs::symlink;

    let tmp = tempfile::tempdir().unwrap();
    let real = tmp.path().join("real.toml");
    fs::write(&real, "").unwrap();
    let link1 = tmp.path().join("link1.toml");
    symlink("real.toml", &link1).unwrap();
    let link2 = tmp.path().join("link2.toml");
    symlink("link1.toml", &link2).unwrap();

    let (code, _stdout, stderr) = run("check", &link2, "/usr/bin");
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(
        stderr.contains("not a regular file") || stderr.contains("symlink"),
        "stderr should mention the symlink policy: {stderr}"
    );
}
