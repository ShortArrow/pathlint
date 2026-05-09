//! 0.0.15 Step D: contract test for canonical CLI naming.
//!
//! 0.0.14 renamed `pathlint where` → `pathlint trace` and
//! `--rules` → `--config`. The legacy aliases lived as clap
//! `visible_alias` through 0.0.21 (with a deprecation warning
//! from 0.0.20) and were removed in 0.0.22. These tests pin the
//! canonical form *and* the post-removal behaviour so future
//! drift (re-introducing the alias by accident, regressing the
//! canonical diagnostics) is caught at CI time.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_pathlint");

#[test]
fn invalid_config_path_error_uses_canonical_flag_name() {
    // `pathlint --config <missing>` must mention --config in its
    // error, not the legacy --rules. Since 0.0.22 there is no
    // alias to type, but the diagnostic itself should still speak
    // the canonical name.
    let out = Command::new(BIN)
        .args([
            "--config",
            "/definitely/does/not/exist/pathlint.toml",
            "check",
        ])
        .output()
        .expect("failed to run pathlint");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--config"),
        "stderr must mention --config: {stderr}"
    );
    assert!(
        !stderr.contains("--rules"),
        "stale --rules name leaked into error: {stderr}"
    );
}

#[test]
fn rules_alias_no_longer_accepted() {
    // 0.0.22 BREAKING: --rules was removed. clap should reject it
    // as an unknown argument (exit 2 in clap's error path) and
    // mention the unexpected token in the error so the user knows
    // they typed something the binary no longer recognises.
    let out = Command::new(BIN)
        .args([
            "--rules",
            "/definitely/does/not/exist/pathlint.toml",
            "check",
        ])
        .output()
        .expect("failed to run pathlint");
    let code = out.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        code, 2,
        "removed alias must surface as a clap parse error (exit 2); stderr: {stderr}"
    );
    assert!(
        stderr.contains("--rules") || stderr.contains("rules"),
        "clap's error should name the unknown argument: {stderr}"
    );
}

#[test]
fn where_alias_no_longer_accepted() {
    // 0.0.22 BREAKING: the `where` subcommand alias was removed.
    // clap should reject the unknown subcommand with an error.
    let out = Command::new(BIN)
        .args(["where", "ls"])
        .output()
        .expect("failed to run pathlint");
    let code = out.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        code, 2,
        "removed alias must surface as a clap parse error (exit 2); stderr: {stderr}"
    );
    assert!(
        stderr.contains("where"),
        "clap's error should name the unknown subcommand: {stderr}"
    );
}

#[test]
fn canonical_names_emit_no_deprecation_warning() {
    // 0.0.22 keeps this test as a regression guard: even after the
    // alias removal, canonical names (`trace`, `--config`) must
    // never themselves surface a "deprecated" string. If somebody
    // accidentally re-adds a deprecation warning on the canonical
    // path, this fires.
    let out = Command::new(BIN)
        .args(["trace", "definitely_no_such_command_xyz"])
        .output()
        .expect("failed to run pathlint");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("deprecated"),
        "canonical 'trace' must not surface deprecation: {stderr}"
    );

    let out = Command::new(BIN)
        .args([
            "--config",
            "/definitely/does/not/exist/pathlint.toml",
            "check",
        ])
        .output()
        .expect("failed to run pathlint");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("deprecated"),
        "canonical '--config' must not surface deprecation: {stderr}"
    );
}
