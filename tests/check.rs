//! End-to-end CLI tests. Each test builds an isolated PATH directory
//! with a stub executable and a TOML manifest, then invokes the real
//! `pathlint` binary and asserts on stdout / exit code.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_pathlint");

fn run(rules: &Path, path_value: &str) -> (i32, String, String) {
    run_with_args(rules, path_value, &[])
}

fn run_with_args(rules: &Path, path_value: &str, extra: &[&str]) -> (i32, String, String) {
    let mut cmd = Command::new(BIN);
    cmd.arg("--rules")
        .arg(rules)
        .arg("--no-glyphs")
        .env("PATH", path_value)
        .env_remove("XDG_CONFIG_HOME");
    for a in extra {
        cmd.arg(a);
    }
    let out = cmd.output().expect("failed to run pathlint binary");
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

    let other = if os_tag() == "windows" {
        "linux"
    } else {
        "windows"
    };
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
fn lazygit_resolves_from_any_of_multiple_preferred_sources() {
    // `lazygit` ships via cargo, winget, and mise. The user does not
    // care which one wins, only that one of them does. prefer is a set
    // of acceptable sources; matching any one is OK.
    let tmp = tempfile::tempdir().unwrap();
    let cargo_dir = tmp.path().join("cargo_bin");
    let winget_dir = tmp.path().join("winget_links");
    let mise_dir = tmp.path().join("mise_shims");
    // Only winget actually contains the binary on this run.
    stub(&winget_dir, "lazygit");

    let key = key_for_current_os();
    let body = format!(
        r#"
[[expect]]
command = "lazygit"
prefer  = ["my_cargo", "my_winget", "my_mise"]

[source.my_cargo]
{key} = "{cargo}"

[source.my_winget]
{key} = "{winget}"

[source.my_mise]
{key} = "{mise}"
"#,
        cargo = cargo_dir.display().to_string().replace('\\', "/"),
        winget = winget_dir.display().to_string().replace('\\', "/"),
        mise = mise_dir.display().to_string().replace('\\', "/"),
    );
    let rules = write_rules(tmp.path(), &body);

    // Only winget_dir is on PATH; cargo_dir / mise_dir are empty
    // directories. lazygit must resolve from winget and the run is OK.
    fs::create_dir_all(&cargo_dir).unwrap();
    fs::create_dir_all(&mise_dir).unwrap();
    let (code, stdout, _) = run(&rules, &join_path(&[&cargo_dir, &winget_dir, &mise_dir]));
    assert_eq!(code, 0, "stdout was: {stdout}");
    assert!(stdout.contains("OK"), "stdout was: {stdout}");
    assert!(
        stdout.contains("my_winget"),
        "matched source should be reported: {stdout}"
    );
}

#[test]
fn mixed_outcomes_yield_exit_1_and_print_each_line() {
    // OK + NG + skip in one TOML — exit 1 because at least one NG.
    let tmp = tempfile::tempdir().unwrap();
    let good_dir = tmp.path().join("good");
    let bad_dir = tmp.path().join("bad");
    stub(&good_dir, "alpha");
    stub(&bad_dir, "beta");

    let key = key_for_current_os();
    let body = format!(
        r#"
[[expect]]
command = "alpha"
prefer  = ["good"]

[[expect]]
command = "beta"
prefer  = ["good"]
avoid   = ["bad"]

[[expect]]
command = "missing_optional_xyz"
optional = true

[source.good]
{key} = "{good}"

[source.bad]
{key} = "{bad}"
"#,
        good = good_dir.display().to_string().replace('\\', "/"),
        bad = bad_dir.display().to_string().replace('\\', "/"),
    );
    let rules = write_rules(tmp.path(), &body);

    let (code, stdout, _) = run(&rules, &join_path(&[&good_dir, &bad_dir]));
    assert_eq!(code, 1, "stdout was: {stdout}");
    assert!(
        stdout.contains("OK") && stdout.contains("alpha"),
        "alpha OK missing: {stdout}"
    );
    assert!(
        stdout.contains("NG") && stdout.contains("beta"),
        "beta NG missing: {stdout}"
    );
    assert!(
        stdout.contains("skip") && stdout.contains("missing_optional_xyz"),
        "skip missing: {stdout}"
    );
}

#[test]
fn quiet_mode_hides_ok_and_skip_lines() {
    let tmp = tempfile::tempdir().unwrap();
    let good_dir = tmp.path().join("good");
    let bad_dir = tmp.path().join("bad");
    stub(&good_dir, "alpha");
    stub(&bad_dir, "beta");

    let key = key_for_current_os();
    let body = format!(
        r#"
[[expect]]
command = "alpha"
prefer  = ["good"]

[[expect]]
command = "beta"
prefer  = ["good"]
avoid   = ["bad"]

[source.good]
{key} = "{good}"

[source.bad]
{key} = "{bad}"
"#,
        good = good_dir.display().to_string().replace('\\', "/"),
        bad = bad_dir.display().to_string().replace('\\', "/"),
    );
    let rules = write_rules(tmp.path(), &body);

    let (code, stdout, _) = run_with_args(&rules, &join_path(&[&good_dir, &bad_dir]), &["--quiet"]);
    assert_eq!(code, 1, "stdout was: {stdout}");
    assert!(!stdout.contains("alpha"), "OK line leaked: {stdout}");
    assert!(stdout.contains("beta"), "NG line missing: {stdout}");
}

#[test]
fn verbose_shows_not_applicable_lines() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("d");
    stub(&dir, "alpha");

    let other = if cfg!(windows) { "linux" } else { "windows" };
    let key = key_for_current_os();
    let body = format!(
        r#"
[[expect]]
command = "alpha"
prefer  = ["good"]

[[expect]]
command = "alpha"
prefer  = ["good"]
os      = ["{other}"]

[source.good]
{key} = "{path}"
"#,
        path = dir.display().to_string().replace('\\', "/"),
    );
    let rules = write_rules(tmp.path(), &body);

    let (default_code, default_stdout, _) = run(&rules, &join_path(&[&dir]));
    assert_eq!(default_code, 0);
    assert!(
        !default_stdout.contains("n/a"),
        "n/a hidden by default: {default_stdout}"
    );

    let (verbose_code, verbose_stdout, _) =
        run_with_args(&rules, &join_path(&[&dir]), &["--verbose"]);
    assert_eq!(verbose_code, 0);
    assert!(
        verbose_stdout.contains("n/a"),
        "verbose should show n/a: {verbose_stdout}"
    );
}

#[test]
fn os_branching_applies_only_current_os_rules() {
    // Same command name with two os-tagged expectations; only the one
    // matching the current OS is evaluated. PRD §8 python example.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("bin");
    stub(&dir, "python");

    let me = os_tag();
    let other = if me == "windows" { "linux" } else { "windows" };
    let key = key_for_current_os();
    let body = format!(
        r#"
[[expect]]
command = "python"
prefer  = ["mine"]
os      = ["{me}"]

[[expect]]
command = "python"
# This rule references an undefined source — it would ConfigError if
# evaluated. The os filter must keep it from being evaluated at all.
prefer  = ["never_defined"]
os      = ["{other}"]

[source.mine]
{key} = "{path}"
"#,
        path = dir.display().to_string().replace('\\', "/"),
    );
    let rules = write_rules(tmp.path(), &body);

    let (code, stdout, _) = run(&rules, &join_path(&[&dir]));
    assert_eq!(code, 0, "stdout was: {stdout}");
    assert!(
        !stdout.contains("ERR"),
        "other-os rule must not produce a config error: {stdout}"
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
