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
fn help_top_level_snapshot() {
    // Step 5 of 0.0.16: pin a handful of representative lines
    // from `pathlint --help` so a clap upgrade or accidental
    // copy-paste change in src/cli.rs is caught at CI time.
    // Stops short of a full insta snapshot to avoid the dev-dep
    // pull; contains() over individual representative strings
    // gives the same drift signal with less ceremony.
    let out = Command::new(BIN)
        .arg("--help")
        .output()
        .expect("failed to run pathlint");
    let stdout = String::from_utf8_lossy(&out.stdout);

    let representative = [
        // Crate description (set by clap derive on Cli).
        "Lint PATH against [[expect]] rules",
        // Usage line (clap-generated).
        "Usage:",
        // Subcommand list headings.
        "Commands:",
        // Global options heading.
        "Options:",
        // The 0.0.15 visible_alias surfacing (`[aliases: where]`
        // / `[aliases: --rules]`) — separately gated by
        // help_surfaces_*_alias_*. Asserting the literal
        // `[aliases:` token here pins clap's alias-rendering
        // contract too.
        "[aliases:",
        // Concrete option / flag presence.
        "--target",
        "--config",
        "--no-glyphs",
        // -V/--version is a load-bearing CLI surface.
        "-V, --version",
    ];
    for needle in representative {
        assert!(
            stdout.contains(needle),
            "--help output missing representative line `{needle}`:\n{stdout}"
        );
    }
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
