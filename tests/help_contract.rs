//! 0.0.15 Step I: contract test for `--version` and `--help`
//! output. Pins the user-facing surface so a typo or accidental
//! drop in subcommand registration is caught at CI time.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_pathlint");

#[test]
fn version_output_matches_cargo_pkg_version() {
    let out = Command::new(BIN)
        .arg("--version")
        .output()
        .expect("failed to run pathlint");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains(env!("CARGO_PKG_VERSION")),
        "version output `{}` must contain {}",
        stdout.trim(),
        env!("CARGO_PKG_VERSION")
    );
    assert!(
        stdout.starts_with("pathlint "),
        "version output must start with `pathlint `: {}",
        stdout.trim()
    );
}

#[test]
fn help_lists_every_canonical_subcommand() {
    let out = Command::new(BIN)
        .arg("--help")
        .output()
        .expect("failed to run pathlint");
    let stdout = String::from_utf8_lossy(&out.stdout);
    for sc in ["check", "init", "catalog", "doctor", "trace", "sort"] {
        assert!(
            stdout.contains(sc),
            "subcommand `{sc}` missing from --help: {stdout}"
        );
    }
}

#[test]
fn help_surfaces_where_alias_for_trace() {
    // 0.0.15: visible_alias on Command::Trace publishes `where`
    // in the help output. The alias is wired in src/cli.rs and
    // documented as scheduled for removal in a future release;
    // until then it must remain discoverable.
    let out = Command::new(BIN)
        .arg("--help")
        .output()
        .expect("failed to run pathlint");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("where"),
        "trace alias `where` must surface in --help: {stdout}"
    );
}

#[test]
fn help_surfaces_rules_alias_for_config() {
    // 0.0.16: parallel discoverability gate for the --config /
    // --rules alias. visible_alias on GlobalOpts.config publishes
    // `rules` in the help output. Without this test, the alias
    // could be silently dropped from --help (e.g. by a clap
    // upgrade that changed visible_alias rendering) and only
    // detected at the next break window.
    let out = Command::new(BIN)
        .arg("--help")
        .output()
        .expect("failed to run pathlint");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("rules"),
        "--config alias `--rules` must surface in --help: {stdout}"
    );
}
