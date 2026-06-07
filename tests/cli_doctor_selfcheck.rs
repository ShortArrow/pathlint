//! `pathlint doctor` (selfcheck) end-to-end tests (ADR-0028, 0.0.34).
//!
//! 0.0.34 redefines doctor: it answers "is pathlint itself functional
//! in this environment?" — binary self-locate, pathlint.toml discovery
//! and parse, env_lookup operational. The PATH-anomaly detection that
//! 0.0.33-and-earlier doctor did now lives in `pathlint lint`
//! (see [`cli_lint`]).

use std::fs;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_pathlint");

fn run_doctor_in(cwd: &std::path::Path) -> (i32, String, String) {
    let out = Command::new(BIN)
        .arg("doctor")
        .current_dir(cwd)
        .env_remove("XDG_CONFIG_HOME")
        .output()
        .expect("failed to run pathlint binary");
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    (code, stdout, stderr)
}

#[test]
fn doctor_does_not_report_path_anomalies() {
    // 0.0.34 doctor must NOT surface duplicate / shortenable / shadow
    // / etc. Those moved to lint. If a duplicate-rich PATH still
    // produces those diagnostics from doctor, the move is incomplete.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("x");
    fs::create_dir_all(&dir).unwrap();
    let sep = if cfg!(windows) { ";" } else { ":" };
    let path = format!("{0}{1}{0}", dir.display(), sep);
    let out = Command::new(BIN)
        .arg("doctor")
        .env("PATH", &path)
        .env_remove("XDG_CONFIG_HOME")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("duplicate"),
        "doctor must not surface duplicate (moved to lint): {stdout}"
    );
    assert!(
        !stdout.contains("shortenable"),
        "doctor must not surface shortenable (moved to lint): {stdout}"
    );
    assert!(
        !stdout.contains("shadows"),
        "doctor must not surface shadow (moved to lint): {stdout}"
    );
}

#[test]
fn doctor_reports_config_not_found_when_no_toml() {
    // In a tempdir with no pathlint.toml anywhere up the tree, doctor
    // should report config_not_found at info severity (legitimate
    // case: user runs pathlint without a config).
    let tmp = tempfile::tempdir().unwrap();
    let (code, stdout, stderr) = run_doctor_in(tmp.path());
    let context = format!("code={code} stdout={stdout:?} stderr={stderr:?}");
    // Info-severity findings must not fail the run.
    assert_eq!(code, 0, "{context}");
    assert!(
        stdout.contains("config_not_found") || stdout.contains("pathlint.toml"),
        "{context}"
    );
}

#[test]
fn doctor_parses_a_valid_toml_without_error() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("pathlint.toml");
    fs::write(&cfg, "# valid empty config\n").unwrap();
    let (code, stdout, stderr) = run_doctor_in(tmp.path());
    let context = format!("code={code} stdout={stdout:?} stderr={stderr:?}");
    assert_eq!(code, 0, "{context}");
    assert!(
        !stdout.contains("config_parse_error"),
        "valid toml must not produce parse error: {context}"
    );
}

#[test]
fn doctor_reports_config_parse_error_on_broken_toml() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("pathlint.toml");
    fs::write(&cfg, "[broken\nnot = valid toml\n").unwrap();
    let (code, stdout, stderr) = run_doctor_in(tmp.path());
    let context = format!("code={code} stdout={stdout:?} stderr={stderr:?}");
    // Parse error is severity=error → non-zero exit.
    assert_ne!(code, 0, "broken toml must produce non-zero exit; {context}");
    assert!(
        stdout.contains("failed to parse") || stderr.contains("parse"),
        "{context}"
    );
}

#[test]
fn doctor_json_emits_top_level_array() {
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(BIN)
        .args(["doctor", "--json"])
        .current_dir(tmp.path())
        .env_remove("XDG_CONFIG_HOME")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let v: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|_| panic!("doctor --json not valid JSON: {stdout}"));
    // 0.0.34 doctor JSON shape mirrors 0.0.33's: top-level array.
    // Kind enum is the selfcheck subset (ADR-0028).
    assert!(v.is_array(), "expected array, got: {stdout}");
}

#[test]
fn doctor_emits_selfcheck_kinds_only() {
    // The kind enum doctor emits must be a small selfcheck set:
    // { binary_not_in_path, config_not_found, config_parse_error,
    //   env_lookup_failed }. No PATH-anomaly kinds (ADR-0028).
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(BIN)
        .args(["doctor", "--json"])
        .current_dir(tmp.path())
        .env_remove("XDG_CONFIG_HOME")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).expect(&stdout);
    let lint_kinds = [
        "duplicate",
        "duplicate_but_shadowed",
        "shortenable",
        "missing",
        "trailing_slash",
        "writeable_path_dir",
        "relative_path_entry",
        "conflict",
        "case_variant",
        "short_name",
        "malformed",
        "per_source_missing_required",
    ];
    if let Some(arr) = v.as_array() {
        for diag in arr {
            let kind = diag.get("kind").and_then(|k| k.as_str()).unwrap_or("");
            assert!(
                !lint_kinds.contains(&kind),
                "doctor emitted lint kind {kind}: {stdout}"
            );
        }
    }
}

#[test]
fn doctor_emits_binary_not_in_path_when_running_dir_not_on_path() {
    // Deterministic green-path opposite: when PATH doesn't contain
    // the running binary's directory, doctor must surface
    // binary_not_in_path. The test harness's BIN lives under
    // target/debug, so a PATH of just `/tmp` (or C:\) on Windows
    // will reliably miss it.
    let tmp = tempfile::tempdir().unwrap();
    let other = tmp.path().join("other");
    fs::create_dir_all(&other).unwrap();
    let out = Command::new(BIN)
        .arg("doctor")
        .current_dir(tmp.path())
        .env("PATH", other.display().to_string())
        .env_remove("XDG_CONFIG_HOME")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("not on PATH"),
        "expected binary_not_in_path: {stdout}"
    );
}

#[test]
fn doctor_binary_self_locate_green_path() {
    // Deterministic green-path: when PATH contains the running
    // binary's directory, doctor must NOT emit binary_not_in_path.
    let tmp = tempfile::tempdir().unwrap();
    let bin_dir = std::path::Path::new(BIN).parent().unwrap();
    let path_value = bin_dir.display().to_string();
    let out = Command::new(BIN)
        .arg("doctor")
        .current_dir(tmp.path())
        .env("PATH", &path_value)
        .env_remove("XDG_CONFIG_HOME")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("not on PATH"),
        "binary should self-locate: {stdout}"
    );
}

#[test]
#[cfg(not(windows))]
fn doctor_emits_env_lookup_failed_when_home_unset() {
    // Unix: HOME unset triggers env_lookup_failed (warn — HOME
    // only affects config search, not pathlint's core).
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(BIN)
        .arg("doctor")
        .current_dir(tmp.path())
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("HOME")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("env_lookup failed for HOME"),
        "expected env_lookup_failed for HOME: {stdout}"
    );
}

#[test]
#[cfg(windows)]
fn doctor_emits_env_lookup_failed_when_userprofile_unset() {
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(BIN)
        .arg("doctor")
        .current_dir(tmp.path())
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("USERPROFILE")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("env_lookup failed for USERPROFILE"),
        "expected env_lookup_failed for USERPROFILE: {stdout}"
    );
}
