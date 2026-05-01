//! End-to-end orchestration: read config, build catalog, evaluate, print.

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::catalog;
use crate::catalog_view::{self, ListStyle};
use crate::cli::{CatalogCommand, CatalogListArgs, Cli, Command, DoctorArgs, InitArgs, WhereArgs};
use crate::config::Config;
use crate::doctor::{self, Diagnostic, Kind, Severity};
use crate::init::{self, InitOptions, InitOutcome};
use crate::lint;
use crate::os_detect::Os;
use crate::path_source::{self, Target};
use crate::report;
use crate::resolve;
use crate::where_cmd::{self, Provenance, UninstallHint, WhereOutcome};

/// Returns a process exit code: 0 = clean, 1 = expectation failure,
/// 2 = config / I/O error (returned as `Err` from `main`).
pub fn execute(cli: Cli) -> Result<u8> {
    match cli.command {
        Some(Command::Init(args)) => return execute_init(&args),
        Some(Command::Catalog {
            action: CatalogCommand::List(args),
        }) => return execute_catalog_list(&args, cli.global.rules.as_deref()),
        Some(Command::Doctor(args)) => return execute_doctor(&args, &cli.global),
        Some(Command::Where(args)) => return execute_where(&args, &cli.global),
        Some(Command::Check) | None => {}
    }
    let rules_path = locate_rules(cli.global.rules.as_deref())?;
    let cfg = match rules_path.as_ref() {
        Some(p) => Config::from_path(p)?,
        None => Config::default(),
    };

    if let Some(required) = cfg.require_catalog {
        let embedded = catalog::embedded_version();
        if embedded < required {
            eprintln!(
                "pathlint: rules require catalog_version >= {required}, but this binary embeds version {embedded}. Upgrade pathlint or lower require_catalog."
            );
            return Ok(2);
        }
    }

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

fn execute_doctor(args: &DoctorArgs, global: &crate::cli::GlobalOpts) -> Result<u8> {
    let target: Target = global.target.into();
    let path_read = path_source::read_path(target);
    if let Some(w) = &path_read.warning {
        eprintln!("pathlint: warning: {w}");
    }
    let entries = resolve::split_path(&path_read.value);
    let diags = doctor::analyze(&entries, Os::current());

    // Validate filter inputs before running anything else so a typo
    // is caught fast (exit 2 — config error, not a lint failure).
    let known: std::collections::BTreeSet<&'static str> =
        doctor::all_kind_names().iter().copied().collect();
    for name in args.include.iter().chain(args.exclude.iter()) {
        if !known.contains(name.as_str()) {
            anyhow::bail!(
                "unknown doctor kind `{name}`; valid values: {}",
                doctor::all_kind_names().join(", ")
            );
        }
    }

    let kept: Vec<&Diagnostic> = diags
        .iter()
        .filter(|d| {
            let name = doctor::kind_name(&d.kind);
            if !args.include.is_empty() {
                args.include.iter().any(|s| s == name)
            } else if !args.exclude.is_empty() {
                !args.exclude.iter().any(|s| s == name)
            } else {
                true
            }
        })
        .collect();

    let printable: Vec<&Diagnostic> = if global.quiet {
        kept.iter()
            .copied()
            .filter(|d| d.severity == Severity::Error)
            .collect()
    } else {
        kept.clone()
    };

    for d in &printable {
        println!("{}", format_diagnostic(d, &entries));
    }

    // Exit code reflects the *kept* set so excluding a Malformed
    // diagnostic genuinely lets the run pass.
    let has_error = kept.iter().any(|d| d.severity == Severity::Error);
    Ok(if has_error { 1 } else { 0 })
}

fn format_diagnostic(d: &Diagnostic, entries: &[String]) -> String {
    let tag = match d.severity {
        Severity::Error => "[ERR] ",
        Severity::Warn => "[warn]",
    };
    let detail = match &d.kind {
        Kind::Duplicate { first_index } => format!(
            "duplicate of entry #{first} ({first_path})",
            first = first_index,
            first_path = entries.get(*first_index).cloned().unwrap_or_default(),
        ),
        Kind::Missing => "directory does not exist".into(),
        Kind::Shortenable { suggestion } => format!("could be written as {suggestion}"),
        Kind::TrailingSlash => "trailing slash; some shells handle this oddly".into(),
        Kind::CaseVariant { canonical } => {
            format!("case / slash variant of {canonical}; OS treats them as one directory")
        }
        Kind::ShortName => "Windows 8.3 short name in PATH; long-name form is more portable".into(),
        Kind::Malformed { reason } => format!("malformed entry: {reason}"),
        Kind::MiseActivateBoth {
            shim_indices,
            install_indices,
        } => return format_mise_activate_both(d, entries, shim_indices, install_indices),
    };
    format!(
        "{tag} #{idx:>3} {entry}\n      {detail}",
        idx = d.index,
        entry = d.entry
    )
}

fn format_mise_activate_both(
    d: &Diagnostic,
    entries: &[String],
    shim_indices: &[usize],
    install_indices: &[usize],
) -> String {
    let tag = "[warn]";
    let mut buf =
        format!("{tag} mise activate exposes both shim and install layers (PATH order matters)\n");
    buf.push_str("      shims:\n");
    for &i in shim_indices {
        let entry = entries.get(i).cloned().unwrap_or_default();
        buf.push_str(&format!("        #{i:>3} {entry}\n"));
    }
    buf.push_str("      installs:\n");
    for &i in install_indices {
        let entry = entries.get(i).cloned().unwrap_or_default();
        buf.push_str(&format!("        #{i:>3} {entry}\n"));
    }
    // strip the trailing newline so the outer renderer can add its own
    buf.pop();
    let _ = d; // keep parameter for symmetry with other format funcs
    buf
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
    if !args.names_only {
        println!("# catalog_version = {}", catalog::embedded_version());
    }
    print!("{}", catalog_view::render(&merged, Os::current(), style));
    Ok(0)
}

fn execute_where(args: &WhereArgs, global: &crate::cli::GlobalOpts) -> Result<u8> {
    // R4 reads the same merged catalog `check` does so user
    // overrides apply; the rules file's `[[expect]]` block is
    // ignored — `where` is per-command, not rule-driven.
    let rules_path = locate_rules(global.rules.as_deref())?;
    let cfg = match rules_path.as_ref() {
        Some(p) => Config::from_path(p)?,
        None => Config::default(),
    };
    let merged = catalog::merge_with_user(&cfg.source);

    let target: Target = global.target.into();
    let path_read = path_source::read_path(target);
    if let Some(w) = &path_read.warning {
        eprintln!("pathlint: warning: {w}");
    }
    let path_entries = resolve::split_path(&path_read.value);

    let outcome = where_cmd::locate(&args.command, &merged, Os::current(), |cmd| {
        resolve::resolve(cmd, &path_entries)
    });

    if args.json {
        return execute_where_json(&args.command, &outcome);
    }

    match outcome {
        WhereOutcome::NotFound => {
            println!("{} — not found on PATH", args.command);
            Ok(1)
        }
        WhereOutcome::Found(found) => {
            println!("{}", found.command);
            println!("  resolved: {}", found.resolved.display());
            if found.matched_sources.is_empty() {
                println!("  sources:  (no source matched)");
            } else {
                println!("  sources:  {}", found.matched_sources.join(", "));
            }
            if let Some(prov) = &found.provenance {
                match prov {
                    Provenance::MiseInstallerPlugin {
                        installer,
                        plugin_segment,
                    } => {
                        println!("  provenance: {installer} (via mise plugin `{plugin_segment}`)");
                    }
                }
            }
            match found.uninstall {
                UninstallHint::Command { command } => {
                    println!("  hint:     {command}");
                }
                UninstallHint::NoTemplate { source } => {
                    println!("  hint:     (no uninstall template for source `{source}`)");
                }
                UninstallHint::NoSource => {
                    println!("  hint:     (no source matched — pathlint cannot guess)");
                }
            }
            Ok(0)
        }
    }
}

fn execute_where_json(command: &str, outcome: &WhereOutcome) -> Result<u8> {
    // For NotFound we emit `{ "command": "...", "found": false }`
    // so a script can match on a stable shape; Found uses the
    // serde-derived layout on `where_cmd::Found` plus an explicit
    // `"found": true` discriminator.
    #[derive(serde::Serialize)]
    #[serde(untagged)]
    enum Out<'a> {
        NotFound {
            command: &'a str,
            found: bool,
        },
        Found {
            found: bool,
            #[serde(flatten)]
            inner: &'a where_cmd::Found,
        },
    }

    let payload = match outcome {
        WhereOutcome::NotFound => Out::NotFound {
            command,
            found: false,
        },
        WhereOutcome::Found(f) => Out::Found {
            found: true,
            inner: f,
        },
    };
    let json = serde_json::to_string_pretty(&payload)?;
    println!("{json}");
    let exit = match outcome {
        WhereOutcome::NotFound => 1,
        WhereOutcome::Found(_) => 0,
    };
    Ok(exit)
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
