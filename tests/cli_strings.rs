//! 0.0.15 Step D: contract test for canonical CLI naming.
//!
//! 0.0.14 renamed `pathlint where` → `pathlint trace` and
//! `--rules` → `--config`, but several user-facing strings still
//! reference the old names. These tests pin the canonical form
//! so future drift is caught at CI time.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_pathlint");

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
