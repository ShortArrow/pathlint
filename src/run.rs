//! End-to-end orchestration: read config, build catalog, evaluate, print.

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::catalog;
use crate::catalog_view::{self, ListStyle};
use crate::cli::{CatalogCommand, CatalogListArgs, Cli, Command, InitArgs};
use crate::config::Config;
use crate::init::{self, InitOptions, InitOutcome};
use crate::lint;
use crate::os_detect::Os;
use crate::path_source::{self, Target};
use crate::report;
use crate::resolve;

/// Returns a process exit code: 0 = clean, 1 = expectation failure,
/// 2 = config / I/O error (returned as `Err` from `main`).
pub fn execute(cli: Cli) -> Result<u8> {
    match cli.command {
        Some(Command::Init(args)) => return execute_init(&args),
        Some(Command::Catalog {
            action: CatalogCommand::List(args),
        }) => return execute_catalog_list(&args, cli.global.rules.as_deref()),
        Some(Command::Check) | None => {}
    }
    let rules_path = locate_rules(cli.global.rules.as_deref())?;
    let cfg = match rules_path.as_ref() {
        Some(p) => Config::from_path(p)?,
        None => Config::default(),
    };

    let catalog = catalog::merge_with_user(&cfg.source);
    let os = Os::current();
    let target: Target = cli.global.target.into();
    let path_read = path_source::read_path(target);

    if let Some(w) = &path_read.warning {
        eprintln!("pathlint: warning: {w}");
    }

    let path_entries = resolve::split_path(&path_read.value);

    if cli.global.verbose {
        if let Some(p) = &rules_path {
            eprintln!("pathlint: rules = {}", p.display());
        } else {
            eprintln!("pathlint: rules = <none — running with empty config>");
        }
        eprintln!("pathlint: PATH entries ({}):", path_entries.len());
        for entry in &path_entries {
            eprintln!("  {entry}");
        }
    }

    let outcomes = lint::evaluate(&cfg.expectations, &catalog, os, |cmd| {
        resolve::resolve(cmd, &path_entries)
    });

    let style = report::Style {
        no_glyphs: cli.global.no_glyphs,
        verbose: cli.global.verbose,
        quiet: cli.global.quiet,
    };
    print!("{}", report::render(&outcomes, style));

    if report::has_config_error(&outcomes) {
        return Ok(2);
    }
    if outcomes.iter().any(|o| report::is_failure(&o.status)) {
        Ok(1)
    } else {
        Ok(0)
    }
}

fn execute_catalog_list(args: &CatalogListArgs, explicit_rules: Option<&Path>) -> Result<u8> {
    let cfg = match locate_rules(explicit_rules)? {
        Some(p) => Config::from_path(&p)?,
        None => Config::default(),
    };
    let merged = catalog::merge_with_user(&cfg.source);
    let style = ListStyle {
        all_os: args.all,
        names_only: args.names_only,
    };
    print!("{}", catalog_view::render(&merged, Os::current(), style));
    Ok(0)
}

fn execute_init(args: &InitArgs) -> Result<u8> {
    let cwd = std::env::current_dir()?;
    let opts = InitOptions {
        emit_defaults: args.emit_defaults,
        force: args.force,
    };
    match init::run(&cwd, &opts, Os::current())? {
        InitOutcome::Wrote(p) => {
            println!("pathlint: wrote {}", p.display());
            Ok(0)
        }
        InitOutcome::AlreadyExists(p) => {
            eprintln!(
                "pathlint: {} already exists; pass --force to overwrite",
                p.display()
            );
            Ok(1)
        }
    }
}

fn locate_rules(explicit: Option<&Path>) -> Result<Option<PathBuf>> {
    if let Some(p) = explicit {
        if !p.is_file() {
            anyhow::bail!("--rules path not found: {}", p.display());
        }
        return Ok(Some(p.to_path_buf()));
    }
    let local = PathBuf::from("pathlint.toml");
    if local.is_file() {
        return Ok(Some(local));
    }
    if let Some(xdg) = xdg_config_path() {
        if xdg.is_file() {
            return Ok(Some(xdg));
        }
    }
    Ok(None)
}

fn xdg_config_path() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(xdg).join("pathlint").join("pathlint.toml"));
    }
    if let Ok(home) = std::env::var("HOME") {
        return Some(
            PathBuf::from(home)
                .join(".config")
                .join("pathlint")
                .join("pathlint.toml"),
        );
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        return Some(
            PathBuf::from(profile)
                .join(".config")
                .join("pathlint")
                .join("pathlint.toml"),
        );
    }
    None
}
