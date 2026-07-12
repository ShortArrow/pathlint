//! `pathlint.toml` discovery e2e tests: the cwd → git-root walk
//! and the `--scope` layer selector (0.0.41+).
//!
//! Discovery rules under test:
//! - cwd hit wins (pre-0.0.41 behaviour, unchanged);
//! - when cwd has no config, parents are searched up to and
//!   including the directory that contains `.git` (a directory or
//!   a worktree marker file) — never past it, and not at all when
//!   no `.git` exists anywhere above;
//! - `--scope=local` stops after the repo-local layers,
//!   `--scope=global` skips straight to the XDG layer, and the
//!   default `--scope=auto` chains local then global.
//!
//! Every test pins `XDG_CONFIG_HOME` to a throwaway directory so
//! the host machine's real user-global config can never leak in.

use std::fs;
use std::path::Path;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_pathlint");

/// Run `pathlint -v [extra args] check` in `cwd` with the XDG
/// layer redirected to `xdg_home`. Returns (exit code, stderr) —
/// `-v` prints the resolved rules location on stderr. Global
/// flags go before the subcommand, matching the CLI's grammar.
fn check_verbose_in(cwd: &Path, xdg_home: &Path, extra: &[&str]) -> (i32, String) {
    let out = Command::new(BIN)
        .arg("-v")
        .args(extra)
        .arg("check")
        .current_dir(cwd)
        .env("XDG_CONFIG_HOME", xdg_home)
        .output()
        .expect("failed to run pathlint binary");
    let code = out.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    (code, stderr)
}

fn rules_line(stderr: &str) -> &str {
    stderr
        .lines()
        .find(|l| l.contains("rules ="))
        .unwrap_or_else(|| panic!("no `rules =` line in stderr: {stderr}"))
}

/// Root layout: <tmp>/pathlint.toml + <tmp>/sub/sub, optionally
/// with a `.git` entry at <tmp>. Returns the nested cwd.
fn repo_layout(root: &Path, git: Option<&str>) -> std::path::PathBuf {
    fs::write(root.join("pathlint.toml"), "# root config\n").unwrap();
    if let Some(kind) = git {
        match kind {
            "dir" => fs::create_dir(root.join(".git")).unwrap(),
            "file" => fs::write(root.join(".git"), "gitdir: /elsewhere/worktrees/x\n").unwrap(),
            other => panic!("unknown git kind {other}"),
        }
    }
    let nested = root.join("sub").join("sub");
    fs::create_dir_all(&nested).unwrap();
    nested
}

#[test]
fn walk_finds_repo_root_config_from_nested_subdir() {
    let tmp = tempfile::tempdir().unwrap();
    let xdg = tempfile::tempdir().unwrap();
    let nested = repo_layout(tmp.path(), Some("dir"));
    let (code, stderr) = check_verbose_in(&nested, xdg.path(), &[]);
    let line = rules_line(&stderr);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        line.contains("pathlint.toml") && !line.contains("<none"),
        "expected walked-to root config, got: {line}"
    );
}

#[test]
fn no_git_boundary_means_no_walk() {
    let tmp = tempfile::tempdir().unwrap();
    let xdg = tempfile::tempdir().unwrap();
    let nested = repo_layout(tmp.path(), None);
    let (code, stderr) = check_verbose_in(&nested, xdg.path(), &[]);
    let line = rules_line(&stderr);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        line.contains("<none"),
        "without .git the parent config must NOT be found, got: {line}"
    );
}

#[test]
fn git_worktree_marker_file_also_bounds_the_walk() {
    let tmp = tempfile::tempdir().unwrap();
    let xdg = tempfile::tempdir().unwrap();
    let nested = repo_layout(tmp.path(), Some("file"));
    let (code, stderr) = check_verbose_in(&nested, xdg.path(), &[]);
    let line = rules_line(&stderr);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        line.contains("pathlint.toml") && !line.contains("<none"),
        "a .git worktree marker file must enable the walk, got: {line}"
    );
}

#[test]
fn walk_does_not_climb_past_the_git_boundary() {
    // Config sits ABOVE the repo root: <tmp>/pathlint.toml,
    // <tmp>/repo/.git, cwd = <tmp>/repo/sub. The walk must stop at
    // <tmp>/repo and never see <tmp>/pathlint.toml.
    let tmp = tempfile::tempdir().unwrap();
    let xdg = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("pathlint.toml"), "# outside repo\n").unwrap();
    let repo = tmp.path().join("repo");
    let nested = repo.join("sub");
    fs::create_dir_all(&nested).unwrap();
    fs::create_dir(repo.join(".git")).unwrap();
    let (code, stderr) = check_verbose_in(&nested, xdg.path(), &[]);
    let line = rules_line(&stderr);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        line.contains("<none"),
        "config above the .git boundary must NOT be picked up, got: {line}"
    );
}

fn write_xdg_config(xdg_home: &Path) {
    let dir = xdg_home.join("pathlint");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("pathlint.toml"), "# xdg config\n").unwrap();
}

#[test]
fn scope_local_never_falls_through_to_xdg() {
    let tmp = tempfile::tempdir().unwrap();
    let xdg = tempfile::tempdir().unwrap();
    write_xdg_config(xdg.path());
    // No repo-local config, no .git.
    let (code, stderr) = check_verbose_in(tmp.path(), xdg.path(), &["--scope=local"]);
    let line = rules_line(&stderr);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        line.contains("<none"),
        "--scope=local must not read the XDG layer, got: {line}"
    );
}

#[test]
fn scope_global_ignores_repo_local_config() {
    let tmp = tempfile::tempdir().unwrap();
    let xdg = tempfile::tempdir().unwrap();
    write_xdg_config(xdg.path());
    fs::write(tmp.path().join("pathlint.toml"), "# local config\n").unwrap();
    let (code, stderr) = check_verbose_in(tmp.path(), xdg.path(), &["--scope=global"]);
    let line = rules_line(&stderr);
    assert_eq!(code, 0, "stderr: {stderr}");
    let xdg_marker = xdg.path().join("pathlint").display().to_string();
    assert!(
        line.contains(&xdg_marker),
        "--scope=global must resolve the XDG config ({xdg_marker}), got: {line}"
    );
}

#[test]
fn explicit_config_beats_scope() {
    let tmp = tempfile::tempdir().unwrap();
    let xdg = tempfile::tempdir().unwrap();
    write_xdg_config(xdg.path());
    let explicit = tmp.path().join("explicit.toml");
    fs::write(&explicit, "# explicit\n").unwrap();
    let explicit_arg = format!("--config={}", explicit.display());
    let (code, stderr) =
        check_verbose_in(tmp.path(), xdg.path(), &[&explicit_arg, "--scope=global"]);
    let line = rules_line(&stderr);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        line.contains("explicit.toml"),
        "--config must win over --scope, got: {line}"
    );
}

#[test]
fn init_scope_global_writes_into_xdg_layer() {
    let tmp = tempfile::tempdir().unwrap();
    let xdg = tempfile::tempdir().unwrap();
    let out = Command::new(BIN)
        .args(["--scope=global", "init"])
        .current_dir(tmp.path())
        .env("XDG_CONFIG_HOME", xdg.path())
        .output()
        .expect("failed to run pathlint binary");
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(code, 0, "stdout: {stdout}");
    let written = xdg.path().join("pathlint").join("pathlint.toml");
    assert!(
        written.is_file(),
        "init --scope=global must create {}, stdout: {stdout}",
        written.display()
    );
    assert!(
        !tmp.path().join("pathlint.toml").exists(),
        "init --scope=global must not write into the cwd"
    );
}
