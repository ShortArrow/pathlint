//! `pathlint sort --dry-run` end-to-end tests.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_pathlint");

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
fn sort_already_satisfied_path_says_no_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let cargo_dir = tmp.path().join("cargo");
    let other_dir = tmp.path().join("other");
    fs::create_dir_all(&cargo_dir).unwrap();
    fs::create_dir_all(&other_dir).unwrap();

    let key = key_for_current_os();
    let body = format!(
        r#"
[[expect]]
command = "rg"
prefer  = ["my_cargo"]

[source.my_cargo]
{key} = "{cargo}"
"#,
        cargo = cargo_dir.display().to_string().replace('\\', "/"),
    );
    let rules = write_rules(tmp.path(), &body);

    let (code, stdout, _) = run_with_args(
        &rules,
        // cargo first, other second — already satisfies prefer
        &join_path(&[&cargo_dir, &other_dir]),
        &["sort"],
    );
    assert_eq!(code, 0, "stdout: {stdout}");
    assert!(
        stdout.contains("already in a satisfying order"),
        "stdout: {stdout}"
    );
}

#[test]
fn sort_proposes_swap_when_preferred_entry_is_behind() {
    let tmp = tempfile::tempdir().unwrap();
    let cargo_dir = tmp.path().join("cargo");
    let other_dir = tmp.path().join("other");
    fs::create_dir_all(&cargo_dir).unwrap();
    fs::create_dir_all(&other_dir).unwrap();

    let key = key_for_current_os();
    let body = format!(
        r#"
[[expect]]
command = "rg"
prefer  = ["my_cargo"]

[source.my_cargo]
{key} = "{cargo}"
"#,
        cargo = cargo_dir.display().to_string().replace('\\', "/"),
    );
    let rules = write_rules(tmp.path(), &body);

    // other before cargo — sort should propose moving cargo first.
    let (code, stdout, _) = run_with_args(&rules, &join_path(&[&other_dir, &cargo_dir]), &["sort"]);
    assert_eq!(code, 0, "sort never reports failure; stdout: {stdout}");
    assert!(stdout.contains("--dry-run"), "stdout: {stdout}");
    assert!(stdout.contains("moved:"), "stdout: {stdout}");
    assert!(
        stdout.contains("preferred source for `rg`"),
        "stdout: {stdout}"
    );
}

#[test]
fn sort_json_emits_sort_plan_with_moves() {
    let tmp = tempfile::tempdir().unwrap();
    let cargo_dir = tmp.path().join("cargo");
    let other_dir = tmp.path().join("other");
    fs::create_dir_all(&cargo_dir).unwrap();
    fs::create_dir_all(&other_dir).unwrap();

    let key = key_for_current_os();
    let body = format!(
        r#"
[[expect]]
command = "rg"
prefer  = ["my_cargo"]

[source.my_cargo]
{key} = "{cargo}"
"#,
        cargo = cargo_dir.display().to_string().replace('\\', "/"),
    );
    let rules = write_rules(tmp.path(), &body);

    let (code, stdout, _) = run_with_args(
        &rules,
        &join_path(&[&other_dir, &cargo_dir]),
        &["sort", "--json"],
    );
    assert_eq!(code, 0, "stdout: {stdout}");
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).expect(&stdout);
    assert!(v["original"].is_array());
    assert!(v["sorted"].is_array());
    assert_eq!(v["sorted"][0], v["original"][1]); // cargo (was at index 1) is now at index 0
    let moves = v["moves"].as_array().unwrap();
    assert!(!moves.is_empty(), "moves: {moves:?}");
    assert!(moves.iter().any(|m| m["from"] == 1 && m["to"] == 0));
}

#[test]
fn sort_unsatisfiable_prefer_emits_note_in_json() {
    // No PATH entry matches `my_cargo`; sort cannot fix this by
    // reordering. The JSON must surface the note with the kind
    // discriminator so CI tools can branch on it.
    let tmp = tempfile::tempdir().unwrap();
    let plain_dir = tmp.path().join("plain");
    fs::create_dir_all(&plain_dir).unwrap();

    let key = key_for_current_os();
    let body = format!(
        r#"
[[expect]]
command = "rg"
prefer  = ["my_cargo"]

[source.my_cargo]
{key} = "/this/path/does/not/exist"
"#,
    );
    let rules = write_rules(tmp.path(), &body);

    let (code, stdout, _) = run_with_args(&rules, &join_path(&[&plain_dir]), &["sort", "--json"]);
    assert_eq!(code, 0, "stdout: {stdout}");
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).expect(&stdout);
    let notes = v["notes"].as_array().unwrap();
    assert_eq!(notes.len(), 1, "notes: {notes:?}");
    assert_eq!(notes[0]["kind"], "unsatisfiable_prefer");
    assert_eq!(notes[0]["command"], "rg");
}
