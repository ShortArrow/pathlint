//! `pathlint lint --sarif` e2e tests (0.0.42+).
//!
//! Pins the SARIF 2.1.0 output contract:
//! - top-level `version` / `$schema` / `runs[0].tool.driver`;
//! - every `results[].ruleId` resolves into `tool.driver.rules[]`,
//!   and every rule carries the four fields GitHub Code Scanning
//!   requires (`id`, `shortDescription.text`,
//!   `fullDescription.text`, `help.text`);
//! - every result carries a physical location with
//!   `artifactLocation.uri` and `region.startLine` (GitHub's
//!   ingestion minimum), anchored at the discovered config file
//!   (falling back to the literal `pathlint.toml` when discovery
//!   finds nothing);
//! - `--sarif` and `--json` are mutually exclusive.
//!
//! Severity → level mapping (`error` for `Severity::Error`) is
//! unit-tested in `src/format.rs` because a malformed PATH entry
//! (embedded NUL) cannot be injected through a process env var.

use std::fs;
use std::path::Path;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_pathlint");

/// PATH with one duplicate pair and one dangling directory —
/// guarantees at least `duplicate` and `missing` findings on
/// every OS.
fn messy_path(tmp: &Path) -> String {
    let real = tmp.join("bin");
    fs::create_dir_all(&real).unwrap();
    let gone = tmp.join("gone");
    let sep = if cfg!(windows) { ";" } else { ":" };
    format!(
        "{real}{sep}{real}{sep}{gone}",
        real = real.display(),
        gone = gone.display()
    )
}

fn run_lint_sarif(cwd: &Path, path: &str, extra: &[&str]) -> (i32, String, String) {
    let out = Command::new(BIN)
        .args(extra)
        .args(["lint", "--sarif"])
        .current_dir(cwd)
        .env("PATH", path)
        .env("XDG_CONFIG_HOME", cwd.join("no-xdg-here"))
        .output()
        .expect("failed to run pathlint binary");
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    (code, stdout, stderr)
}

fn parse_sarif(stdout: &str) -> serde_json::Value {
    serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("--sarif output is not valid JSON ({e}): {stdout}"))
}

#[test]
fn sarif_envelope_has_version_schema_and_driver() {
    let tmp = tempfile::tempdir().unwrap();
    let path = messy_path(tmp.path());
    let (code, stdout, stderr) = run_lint_sarif(tmp.path(), &path, &[]);
    assert_eq!(code, 0, "lint --sarif must not fail on warnings: {stderr}");
    let v = parse_sarif(&stdout);
    assert_eq!(v["version"], "2.1.0", "{stdout}");
    assert!(
        v["$schema"].as_str().unwrap_or("").contains("sarif"),
        "$schema must reference the official SARIF schema: {stdout}"
    );
    let driver = &v["runs"][0]["tool"]["driver"];
    assert_eq!(driver["name"], "pathlint", "{stdout}");
    assert_eq!(driver["version"], env!("CARGO_PKG_VERSION"), "{stdout}");
}

#[test]
fn every_result_rule_id_resolves_into_rules() {
    let tmp = tempfile::tempdir().unwrap();
    let path = messy_path(tmp.path());
    let (_, stdout, _) = run_lint_sarif(tmp.path(), &path, &[]);
    let v = parse_sarif(&stdout);
    let rules: Vec<&str> = v["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .expect("rules must be an array")
        .iter()
        .map(|r| r["id"].as_str().expect("rule.id must be a string"))
        .collect();
    let results = v["runs"][0]["results"].as_array().expect("results array");
    assert!(
        !results.is_empty(),
        "messy PATH must produce findings: {stdout}"
    );
    for r in results {
        let id = r["ruleId"].as_str().expect("result.ruleId");
        assert!(
            rules.contains(&id),
            "ruleId {id} missing from rules[]: {stdout}"
        );
    }
    // The messy PATH fires at least these two stable rule ids.
    for expected in ["duplicate", "missing"] {
        assert!(
            rules.contains(&expected),
            "expected rule id {expected} in rules[]: {stdout}"
        );
    }
}

#[test]
fn every_rule_carries_the_github_required_fields() {
    let tmp = tempfile::tempdir().unwrap();
    let path = messy_path(tmp.path());
    let (_, stdout, _) = run_lint_sarif(tmp.path(), &path, &[]);
    let v = parse_sarif(&stdout);
    for rule in v["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .expect("rules array")
    {
        for pointer in [
            "/id",
            "/shortDescription/text",
            "/fullDescription/text",
            "/help/text",
        ] {
            assert!(
                rule.pointer(pointer)
                    .and_then(|f| f.as_str())
                    .is_some_and(|s| !s.is_empty()),
                "rule missing required field {pointer}: {rule}"
            );
        }
    }
}

#[test]
fn every_result_has_a_physical_location_with_start_line() {
    let tmp = tempfile::tempdir().unwrap();
    let path = messy_path(tmp.path());
    let (_, stdout, _) = run_lint_sarif(tmp.path(), &path, &[]);
    let v = parse_sarif(&stdout);
    for r in v["runs"][0]["results"].as_array().expect("results array") {
        let uri = r
            .pointer("/locations/0/physicalLocation/artifactLocation/uri")
            .and_then(|u| u.as_str());
        assert!(
            uri.is_some_and(|u| !u.is_empty()),
            "result missing artifactLocation.uri: {r}"
        );
        let start_line = r
            .pointer("/locations/0/physicalLocation/region/startLine")
            .and_then(|l| l.as_u64());
        assert_eq!(start_line, Some(1), "region.startLine must be 1: {r}");
    }
}

#[test]
fn results_carry_the_path_entry_as_logical_location() {
    let tmp = tempfile::tempdir().unwrap();
    let path = messy_path(tmp.path());
    let (_, stdout, _) = run_lint_sarif(tmp.path(), &path, &[]);
    let v = parse_sarif(&stdout);
    let results = v["runs"][0]["results"].as_array().expect("results array");
    let any_entry = results.iter().any(|r| {
        r.pointer("/locations/0/logicalLocations/0/name")
            .and_then(|n| n.as_str())
            .is_some_and(|n| n.contains("gone") || n.contains("bin"))
    });
    assert!(
        any_entry,
        "at least one result must carry its PATH entry in logicalLocations: {stdout}"
    );
}

#[test]
fn location_uri_is_the_discovered_config_or_the_fallback() {
    // Without any config: literal fallback.
    let tmp = tempfile::tempdir().unwrap();
    let path = messy_path(tmp.path());
    let (_, stdout, _) = run_lint_sarif(tmp.path(), &path, &[]);
    let v = parse_sarif(&stdout);
    let uri = v
        .pointer("/runs/0/results/0/locations/0/physicalLocation/artifactLocation/uri")
        .and_then(|u| u.as_str())
        .expect("uri");
    assert_eq!(uri, "pathlint.toml", "fallback uri: {stdout}");

    // With an explicit --config: that file's path (forward slashes).
    let cfg = tmp.path().join("explicit.toml");
    fs::write(&cfg, "").unwrap();
    let cfg_arg = format!("--config={}", cfg.display());
    let (_, stdout, _) = run_lint_sarif(tmp.path(), &path, &[&cfg_arg]);
    let v = parse_sarif(&stdout);
    let uri = v
        .pointer("/runs/0/results/0/locations/0/physicalLocation/artifactLocation/uri")
        .and_then(|u| u.as_str())
        .expect("uri");
    assert!(
        uri.ends_with("explicit.toml") && !uri.contains('\\'),
        "explicit config must anchor the uri with forward slashes, got: {uri}"
    );
}

#[test]
fn sarif_and_json_are_mutually_exclusive() {
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(BIN)
        .args(["lint", "--sarif", "--json"])
        .current_dir(tmp.path())
        .output()
        .expect("failed to run pathlint binary");
    assert_ne!(
        out.status.code(),
        Some(0),
        "--sarif --json must be rejected"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--json") || stderr.contains("--sarif"),
        "clap conflict message expected, got: {stderr}"
    );
}
