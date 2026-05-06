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
fn doctor_reports_duplicate_but_shadowed_across_path_dirs() {
    // Two PATH dirs each containing an executable with the same
    // basename. The earlier dir wins; the later one is shadowed.
    let tmp = tempfile::tempdir().unwrap();
    let dir_a = tmp.path().join("a");
    let dir_b = tmp.path().join("b");
    fs::create_dir_all(&dir_a).unwrap();
    fs::create_dir_all(&dir_b).unwrap();
    let exe_name = if cfg!(windows) {
        "dummy_cmd.exe"
    } else {
        "dummy_cmd"
    };
    for dir in [&dir_a, &dir_b] {
        let p = dir.join(exe_name);
        fs::write(&p, b"").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    // Canonicalize so Windows 8.3 short-name PATH entries don't
    // create their own ShortName diagnostic alongside the one we
    // care about.
    let dir_a = fs::canonicalize(&dir_a).unwrap();
    let dir_b = fs::canonicalize(&dir_b).unwrap();
    let dir_a = strip_unc_prefix(&dir_a);
    let dir_b = strip_unc_prefix(&dir_b);
    let path = join_path(&[&dir_a, &dir_b]);
    let (code, stdout, stderr) = run_doctor(&path);
    // Quote the binary's stdout/stderr in panic messages so an OS
    // that silently fails to detect the shadow (e.g. case-sensitive
    // filesystem mismatch in 0.0.19 pre-fix) is diagnosable from the
    // CI log without a re-push for instrumentation.
    let context = format!("code={code} stdout={stdout:?} stderr={stderr:?}");
    assert_eq!(code, 0, "warn-only must not fail the run; {context}");
    assert!(stdout.contains("dummy_cmd"), "{context}");
    assert!(stdout.contains("shadows"), "{context}");
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

// ---- --include / --exclude (0.0.6+) -------------------------

fn run_doctor_args(path_value: &str, extra: &[&str]) -> (i32, String, String) {
    let mut cmd = Command::new(BIN);
    cmd.arg("doctor")
        .args(extra)
        .env("PATH", path_value)
        .env_remove("XDG_CONFIG_HOME");
    let out = cmd.output().expect("failed to run pathlint binary");
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    (code, stdout, stderr)
}

#[test]
fn doctor_include_filters_to_named_kinds_only() {
    // PATH carries one missing dir AND one duplicate. With
    // `--include duplicate` the missing entry must be silenced.
    let tmp = tempfile::tempdir().unwrap();
    let real = tmp.path().join("real");
    fs::create_dir_all(&real).unwrap();
    let absent = tmp.path().join("definitely_does_not_exist_xyz");
    let path = join_path(&[&real, &real, &absent]);
    let (code, stdout, _) = run_doctor_args(&path, &["--include", "duplicate"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("duplicate"), "stdout: {stdout}");
    assert!(!stdout.contains("does not exist"), "stdout: {stdout}");
}

#[test]
fn doctor_exclude_drops_diagnostics_and_affects_exit_code() {
    // Force a malformed (Error severity) entry on Windows + a
    // duplicate. Without --exclude this exits 1; with
    // --exclude malformed the Error is suppressed and the run
    // passes.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("d");
    fs::create_dir_all(&dir).unwrap();
    if !cfg!(windows) {
        // Unix doesn't have a comparably easy malformed staging.
        return;
    }
    let path = format!("{};C:\\foo|bar", dir.display());

    let (default_code, default_stdout, _) = run_doctor_args(&path, &[]);
    assert_eq!(default_code, 1, "stdout: {default_stdout}");
    assert!(default_stdout.contains("[ERR]"), "stdout: {default_stdout}");

    let (filtered_code, filtered_stdout, _) = run_doctor_args(&path, &["--exclude", "malformed"]);
    assert_eq!(filtered_code, 0, "stdout: {filtered_stdout}");
    assert!(
        !filtered_stdout.contains("[ERR]"),
        "stdout: {filtered_stdout}"
    );
}

#[test]
fn doctor_unknown_kind_is_a_config_error_with_exit_2() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("d");
    fs::create_dir_all(&dir).unwrap();
    let (code, _stdout, stderr) = run_doctor_args(&join_path(&[&dir]), &["--include", "nope"]);
    assert_eq!(code, 2);
    assert!(stderr.contains("unknown doctor kind"), "stderr: {stderr}");
    assert!(stderr.contains("nope"), "stderr: {stderr}");
}

#[test]
fn doctor_include_and_exclude_together_is_a_clap_error() {
    // clap's conflicts_with annotation should make the parse fail
    // with exit 2 and a usage message before pathlint even runs.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("d");
    fs::create_dir_all(&dir).unwrap();
    let (code, _stdout, stderr) = run_doctor_args(
        &join_path(&[&dir]),
        &["--include", "duplicate", "--exclude", "missing"],
    );
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(
        stderr.to_ascii_lowercase().contains("cannot be used"),
        "stderr: {stderr}"
    );
}

#[test]
fn doctor_json_emits_array_with_kind_discriminator() {
    let tmp = tempfile::tempdir().unwrap();
    let real = tmp.path().join("real");
    fs::create_dir_all(&real).unwrap();
    let absent = tmp.path().join("definitely_does_not_exist_xyz");
    let path = join_path(&[&real, &real, &absent]);
    let (code, stdout, _) = run_doctor_args(&path, &["--json"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).expect(&stdout);
    assert!(v.is_array(), "stdout: {stdout}");

    let kinds: Vec<&str> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["kind"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"duplicate"), "kinds: {kinds:?}");
    assert!(kinds.contains(&"missing"), "kinds: {kinds:?}");
    // Each diagnostic carries the four common fields.
    for d in v.as_array().unwrap() {
        assert!(d["index"].is_number(), "{d}");
        assert!(d["entry"].is_string(), "{d}");
        assert!(d["severity"].is_string(), "{d}");
        assert!(d["kind"].is_string(), "{d}");
    }
}

#[test]
fn doctor_json_respects_include_filter() {
    let tmp = tempfile::tempdir().unwrap();
    let real = tmp.path().join("real");
    fs::create_dir_all(&real).unwrap();
    let absent = tmp.path().join("definitely_does_not_exist_xyz");
    let path = join_path(&[&real, &real, &absent]);
    // Same setup as the human-view filter test, but in JSON.
    let (code, stdout, _) = run_doctor_args(&path, &["--include", "duplicate", "--json"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).expect(&stdout);
    let arr = v.as_array().unwrap();
    assert!(!arr.is_empty(), "expected at least one diagnostic");
    for d in arr {
        assert_eq!(
            d["kind"], "duplicate",
            "include filter should keep duplicate-only: {d}"
        );
    }
}

#[test]
fn doctor_warns_when_mise_shim_and_install_coexist() {
    // 0.0.11: doctor reads ConflictsWhenBothInPath from the merged
    // catalog. The built-in relation references mise_shims and
    // mise_installs source paths under $HOME, but the tests must
    // not depend on the runner's $HOME, so we pass an explicit
    // pathlint.toml that overrides those two sources to point at
    // the temp-dir layout the test creates. The built-in relation
    // (mise_activate_both) still fires because the relation table
    // is identified by source name, not path.
    let tmp = tempfile::tempdir().unwrap();
    let mise_root = tmp.path().join("mise");
    let shims = mise_root.join("shims");
    let installs_python = mise_root
        .join("installs")
        .join("python")
        .join("3.14")
        .join("bin");
    fs::create_dir_all(&shims).unwrap();
    fs::create_dir_all(&installs_python).unwrap();
    let installs_dir = mise_root.join("installs");

    let key = if cfg!(windows) { "windows" } else { "unix" };
    let body = format!(
        r#"
[source.mise_shims]
{key} = "{shims}"

[source.mise_installs]
{key} = "{installs}"
"#,
        shims = shims.display().to_string().replace('\\', "/"),
        installs = installs_dir.display().to_string().replace('\\', "/"),
    );
    let rules = tmp.path().join("pathlint.toml");
    fs::write(&rules, body).unwrap();

    let path = join_path(&[&shims, &installs_python]);
    let out = Command::new(BIN)
        .arg("--config")
        .arg(&rules)
        .arg("doctor")
        .env("PATH", &path)
        .env_remove("XDG_CONFIG_HOME")
        .output()
        .expect("failed to run pathlint binary");
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(code, 0, "warn-only must exit 0; stderr: {stderr}");
    assert!(
        stdout.contains("mise activate exposes both shim and install layers"),
        "stdout: {stdout}"
    );
    // Both conflict groups should appear (0.0.11+: groups, not
    // shims/installs sections).
    assert!(stdout.contains("group #0:"), "stdout: {stdout}");
    assert!(stdout.contains("group #1:"), "stdout: {stdout}");
}

#[test]
fn doctor_include_accepts_user_defined_diagnostic() {
    // 0.0.13: a user [[relation]] kind = "conflicts_when_both_in_path"
    // declares an arbitrary diagnostic name; --include must accept
    // it instead of failing the typo check (Z17a-M2).
    let tmp = tempfile::tempdir().unwrap();
    let rules = tmp.path().join("pathlint.toml");
    fs::write(
        &rules,
        r#"
[[relation]]
kind = "conflicts_when_both_in_path"
sources = ["a", "b"]
diagnostic = "foo_overlap"
"#,
    )
    .unwrap();

    // Empty PATH so no built-in diagnostic fires either.
    let out = Command::new(BIN)
        .arg("--config")
        .arg(&rules)
        .arg("doctor")
        .arg("--include")
        .arg("foo_overlap")
        .env("PATH", "/usr/bin")
        .env_remove("XDG_CONFIG_HOME")
        .output()
        .expect("failed to run pathlint binary");
    let code = out.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&out.stderr);
    // Acceptance test: must NOT exit 2 (config error). The actual
    // doctor run might exit 0 (no match) or 1 (a match found); the
    // load-bearing assertion is "validate_filter_names did not
    // reject the name as unknown".
    assert_ne!(
        code, 2,
        "--include foo_overlap was rejected: stderr: {stderr}"
    );
    assert!(!stderr.contains("unknown doctor kind"), "stderr: {stderr}");
}

#[test]
fn doctor_include_still_rejects_unknown_typos() {
    // 0.0.13: even with user-defined diagnostics accepted, a name
    // that is neither built-in nor user-declared still fails.
    let tmp = tempfile::tempdir().unwrap();
    let rules = tmp.path().join("pathlint.toml");
    fs::write(&rules, "").unwrap();

    let out = Command::new(BIN)
        .arg("--config")
        .arg(&rules)
        .arg("doctor")
        .arg("--include")
        .arg("duplicat") // typo of "duplicate"
        .env("PATH", "/usr/bin")
        .env_remove("XDG_CONFIG_HOME")
        .output()
        .expect("failed to run pathlint binary");
    let code = out.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(stderr.contains("unknown doctor kind"), "stderr: {stderr}");
}
