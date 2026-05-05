//! 0.0.17 Step 5: contract test for global CLI options
//! `--color` and `--no-glyphs`. Pre-0.0.17 the `--color` flag
//! was parsed but silently ignored; codex review flagged that
//! as a CLI surface defect.

use std::fs;
use std::path::Path;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_pathlint");

fn write_rules(dir: &Path, body: &str) -> std::path::PathBuf {
    let p = dir.join("pathlint.toml");
    fs::write(&p, body).unwrap();
    p
}

fn run_check(rules: &Path, color: &str) -> (i32, String, String) {
    let out = Command::new(BIN)
        .args(["--color", color, "--config"])
        .arg(rules)
        .arg("check")
        .env("PATH", "")
        .env_remove("XDG_CONFIG_HOME")
        .output()
        .expect("failed to run pathlint");
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    (code, stdout, stderr)
}

#[test]
fn color_never_emits_no_ansi_escape() {
    // 0.0.17: `--color never` must produce ANSI-free output even
    // when the human renderer would otherwise colourise tags.
    // Pre-0.0.17 this flag was parsed and silently ignored, so
    // the test had nothing to gate.
    let tmp = tempfile::tempdir().unwrap();
    let rules = write_rules(tmp.path(), "");
    let (_, stdout, _) = run_check(&rules, "never");
    assert!(
        !stdout.contains('\x1b'),
        "ANSI escape leaked under --color never:\n{stdout}"
    );
}

#[test]
fn color_always_emits_ansi_escape_when_output_has_tags() {
    // `--color always` forces colourisation regardless of TTY
    // detection. With at least one [[expect]] outcome the
    // human renderer must emit a status tag wrapped in ANSI
    // escapes.
    let tmp = tempfile::tempdir().unwrap();
    // A single expect rule that resolves to "not found" (PATH
    // is empty). The status tag goes through colourize_tag and
    // should pick up a red colour code under --color always.
    let rules = write_rules(
        tmp.path(),
        r#"
[[expect]]
command = "pathlint_definitely_no_such_xyz"
"#,
    );
    let (_, stdout, _) = run_check(&rules, "always");
    assert!(
        stdout.contains('\x1b'),
        "--color always must emit ANSI escapes:\n{stdout}"
    );
}
