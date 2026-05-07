//! 0.0.15 Step D: contract test for canonical CLI naming.
//!
//! 0.0.14 renamed `pathlint where` → `pathlint trace` and
//! `--rules` → `--config`, but several user-facing strings still
//! reference the old names. These tests pin the canonical form
//! so future drift is caught at CI time.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_pathlint");

#[test]
fn rules_alias_works_for_backwards_compat() {
    // 0.0.16: most integration tests now drive pathlint with the
    // canonical --config flag. This test is the single dedicated
    // gate that the legacy --rules alias still resolves to the
    // same global option (clap's visible_alias). Without it, all
    // alias coverage would be lost when the rest of the suite
    // moved over.
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
    // Exit 2 means clap accepted --rules as an alias and the
    // subsequent file check fired; the diagnostic itself uses
    // the canonical name --config (see
    // invalid_config_path_error_uses_canonical_flag_name below).
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(
        stderr.contains("--config"),
        "diagnostic must speak the canonical name even when invoked via --rules: {stderr}"
    );
}

#[test]
fn invalid_config_path_error_uses_canonical_flag_name() {
    // `pathlint --config <missing>` must mention --config in its
    // error, not the legacy --rules. The alias is wired via clap
    // so users may type --rules; our own diagnostic should still
    // speak the canonical name.
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
fn rules_alias_emits_deprecation_warning() {
    // 0.0.20: --rules is still accepted as a clap visible_alias of
    // --config, but using it should print a one-line deprecation
    // warning to stderr so users know to migrate before the alias
    // is dropped in a future breaking release.
    let out = Command::new(BIN)
        .args([
            "--rules",
            "/definitely/does/not/exist/pathlint.toml",
            "check",
        ])
        .output()
        .expect("failed to run pathlint");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("'--rules' is deprecated"),
        "alias use must surface deprecation warning: {stderr}"
    );
}

#[test]
fn where_alias_emits_deprecation_warning() {
    // 0.0.20: `pathlint where` is still accepted as a clap visible
    // alias of `pathlint trace`, but using it prints a one-line
    // deprecation warning to stderr.
    let out = Command::new(BIN)
        .args(["where", "definitely_no_such_command_xyz"])
        .output()
        .expect("failed to run pathlint");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("'where' is deprecated"),
        "alias use must surface deprecation warning: {stderr}"
    );
}

#[test]
fn canonical_names_emit_no_deprecation_warning() {
    // pathlint trace + --config (canonical) must not surface the
    // deprecation warning. This pins that the warning fires only
    // on alias use.
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
