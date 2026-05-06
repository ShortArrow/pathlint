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

/// Helper to drive a non-check subcommand.
fn run_subcommand(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(BIN)
        .args(args)
        .env_remove("XDG_CONFIG_HOME")
        .output()
        .expect("failed to run pathlint");
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    (code, stdout, stderr)
}

fn assert_ascii_only(stdout: &str, where_: &str) {
    for c in stdout.chars() {
        assert!(
            c.is_ascii(),
            "non-ASCII char `{c}` (U+{u:04X}) leaked under --no-glyphs in {where_}: {stdout}",
            u = c as u32
        );
    }
}

#[test]
fn no_glyphs_strips_unicode_in_doctor_output() {
    // 0.0.18: --no-glyphs must apply to doctor's human renderer.
    // Pre-0.0.18 the flag only routed through src/report.rs (check
    // only); doctor/trace/sort emitted `—` and `→` regardless.
    let (_, stdout, _) = run_subcommand(&["--no-glyphs", "doctor"]);
    assert_ascii_only(&stdout, "doctor output");
}

#[test]
fn no_glyphs_strips_unicode_in_trace_output() {
    // trace falls back to `not found on PATH` for an unknown command,
    // and the dash there must be ASCII under --no-glyphs.
    let (_, stdout, _) = run_subcommand(&[
        "--no-glyphs",
        "trace",
        "pathlint_definitely_no_such_command",
    ]);
    assert_ascii_only(&stdout, "trace output");
}

#[test]
fn no_glyphs_strips_unicode_in_sort_output() {
    // sort emits an em-dash inside the `unsatisfiable_prefer` note
    // and the catalog relations stanza. --no-glyphs must swap them.
    let (_, stdout, _) = run_subcommand(&["--no-glyphs", "sort", "--dry-run"]);
    assert_ascii_only(&stdout, "sort output");
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
