//! End-to-end orchestration: read config, build catalog, evaluate, print.

use std::path::{Path, PathBuf};

use anyhow::Result;

use pathlint::catalog;
use pathlint::catalog_view::{self, ListStyle};
use crate::cli::{
    CatalogCommand, CatalogListArgs, CatalogRelationsArgs, CheckArgs, Cli, Command, DoctorArgs,
    InitArgs, SortArgs, TraceArgs,
};
use pathlint::config::Config;
use pathlint::doctor::{self, Diagnostic, Filter, Severity};
use pathlint::format;
use pathlint::init::{self, InitOptions, InitOutcome};
use pathlint::lint;
use pathlint::os_detect::Os;
use pathlint::path_source::{self, Target};
use pathlint::report;
use pathlint::resolve;
use pathlint::source_match::{self, SourceWarningReason};
use pathlint::trace::{self, TraceOutcome};

/// Read PATH for the chosen `--target`, surface any warning, and
/// split it into entries. The three caller sites (`check`,
/// `doctor`, `where`) all run the same sequence; centralising it
/// keeps the warning prefix consistent.
fn read_path_entries(global: &crate::cli::GlobalOpts) -> Vec<String> {
    let target: Target = global.target.into();
    let path_read = path_source::read_path(target);
    if let Some(w) = &path_read.warning {
        eprintln!("pathlint: warning: {w}");
    }
    resolve::split_path(&path_read.value)
}

/// Validate the merged catalog and turn warnings into a config
/// error (exit 2) so a hostile or accidentally-broad source needle
/// like `[source.evil] unix = "/"` cannot silently mark every PATH
/// entry as belonging to that source.
///
/// Pure: takes the already-merged catalog and the OS; emits its
/// effect through stderr lines and the returned `Result`. Called
/// from every subcommand that consumes the merged catalog.
fn enforce_source_validation(
    sources: &std::collections::BTreeMap<String, pathlint::config::SourceDef>,
    os: Os,
) -> Result<()> {
    let warnings = source_match::validate_sources(sources, os);
    if warnings.is_empty() {
        return Ok(());
    }
    for w in &warnings {
        eprintln!(
            "pathlint: error: source `{name}` rejected — {reason} ({needle:?})",
            name = w.name,
            needle = w.needle,
            reason = match w.reason {
                SourceWarningReason::RootPath => "path expands to filesystem root",
                SourceWarningReason::NeedleTooShort => "expanded path is too short to match safely",
            },
        );
    }
    anyhow::bail!("{} unsafe source definition(s); aborting", warnings.len());
}

/// Enforce that the merged relation graph is acyclic before any
/// relation consumer (sort / doctor / trace / catalog relations)
/// reads it. A cycle in `served_by_via` / `depends_on` /
/// `prefer_order_over` is a configuration error (exit 2). Without
/// this gate, `sort`'s `apply_prefer_order_over` would silently
/// stop after its bubble-pass guard expires, leaving the user
/// with a partial reorder and no diagnostic. `catalog relations`
/// already performed this check; 0.0.14 extends it to every
/// relation consumer for symmetry.
///
/// Pure: takes the merged relation slice; emits its effect through
/// stderr and the returned Result.
fn enforce_relation_acyclic(relations: &[pathlint::config::Relation]) -> Result<()> {
    if let Err(msg) = catalog::check_acyclic(relations) {
        eprintln!("pathlint: {msg}");
        anyhow::bail!("relation graph has a cycle; aborting");
    }
    Ok(())
}

/// Returns a process exit code: 0 = clean, 1 = expectation failure,
/// 2 = config / I/O error (returned as `Err` from `main`).
pub fn execute(cli: Cli) -> Result<u8> {
    let check_args = match cli.command {
        Some(Command::Init(args)) => return execute_init(&args),
        Some(Command::Catalog {
            action: CatalogCommand::List(args),
        }) => return execute_catalog_list(&args, cli.global.config.as_deref()),
        Some(Command::Catalog {
            action: CatalogCommand::Relations(args),
        }) => return execute_catalog_relations(&args, cli.global.config.as_deref()),
        Some(Command::Doctor(args)) => return execute_doctor(&args, &cli.global),
        Some(Command::Trace(args)) => return execute_trace(&args, &cli.global),
        Some(Command::Sort(args)) => return execute_sort(&args, &cli.global),
        Some(Command::Check(args)) => args,
        None => CheckArgs::default(),
    };
    let rules_path = locate_rules(cli.global.config.as_deref())?;
    let cfg = match rules_path.as_ref() {
        Some(p) => Config::from_path(p)?,
        None => Config::default(),
    };

    if let Err(msg) = catalog::version_check(cfg.require_catalog, catalog::embedded_version()) {
        eprintln!("pathlint: {msg}");
        return Ok(2);
    }

    let catalog = catalog::merge_with_user(&cfg.source);
    let os = Os::current();
    enforce_source_validation(&catalog, os)?;
    let path_entries = read_path_entries(&cli.global);

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

    let outcomes = lint::evaluate(
        &cfg.expectations,
        &catalog,
        os,
        |cmd| resolve::resolve(cmd, &path_entries),
        lint::check_shape_filesystem,
    );

    if check_args.json {
        let json = format::check_json(&outcomes)?;
        println!("{json}");
    } else {
        use std::io::IsTerminal;
        let style = report::Style {
            no_glyphs: cli.global.no_glyphs,
            verbose: cli.global.verbose,
            quiet: cli.global.quiet,
            explain: check_args.explain,
            color: cli.global.color.resolve(std::io::stdout().is_terminal()),
        };
        print!("{}", report::render(&outcomes, style));
    }

    Ok(lint::exit_code(&outcomes))
}

fn execute_doctor(args: &DoctorArgs, global: &crate::cli::GlobalOpts) -> Result<u8> {
    let filter = Filter {
        include: args.include.clone(),
        exclude: args.exclude.clone(),
    };

    // doctor lints PATH itself but still consumes the merged
    // catalog (e.g. `mise_activate_both` uses source paths). A
    // hostile rules override could weaponise the catalog if we
    // didn't enforce safe needles before continuing.
    let rules_path = locate_rules(global.config.as_deref())?;
    let cfg = match rules_path.as_ref() {
        Some(p) => Config::from_path(p)?,
        None => Config::default(),
    };
    let merged = catalog::merge_with_user(&cfg.source);
    enforce_source_validation(&merged, Os::current())?;
    let relations = catalog::merge_with_user_relations(&cfg.relations);
    enforce_relation_acyclic(&relations)?;

    // Validate the filter against built-in kind names plus any
    // user-declared conflict diagnostics from the merged relation
    // set. Validation runs after relations are loaded so that
    // `--include foo_overlap` survives a fast-fail check when the
    // user declared `[[relation]] diagnostic = "foo_overlap"`.
    let user_diags = doctor::user_diagnostic_names(&relations);
    if let Err(msg) = doctor::validate_filter_names(&filter, &user_diags) {
        anyhow::bail!(msg);
    }

    let entries = read_path_entries(global);
    let diags = doctor::analyze_real(&entries, &merged, &relations, Os::current());
    let kept = filter.apply(&diags);

    if args.json {
        // JSON view ignores --quiet on purpose: the consumer is a
        // tool, not a human, and intermediate filtering would
        // surprise pipelines that expect "filter == include/exclude".
        let json = format::doctor_json(&kept)?;
        println!("{json}");
    } else {
        let printable: Vec<&Diagnostic> = if global.quiet {
            kept.iter()
                .copied()
                .filter(|d| d.severity == Severity::Error)
                .collect()
        } else {
            kept.clone()
        };
        for d in &printable {
            println!("{}", format::doctor_line(d, &entries));
        }
    }

    Ok(if doctor::has_error(&kept) { 1 } else { 0 })
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

fn execute_catalog_relations(
    args: &CatalogRelationsArgs,
    explicit_rules: Option<&Path>,
) -> Result<u8> {
    let cfg = match locate_rules(explicit_rules)? {
        Some(p) => Config::from_path(&p)?,
        None => Config::default(),
    };
    let relations = catalog::merge_with_user_relations(&cfg.relations);

    // DAG check: catch a circular `served_by_via` / `depends_on`
    // / `prefer_order_over` before showing the user a list they
    // cannot reason about. The same gate is applied by every
    // other relation consumer (doctor / trace / sort) since 0.0.14.
    enforce_relation_acyclic(&relations)?;

    if args.json {
        let json = format::relations_json(&relations)?;
        println!("{json}");
    } else {
        println!("{}", format::relations_human(&relations));
    }
    Ok(0)
}

fn execute_trace(args: &TraceArgs, global: &crate::cli::GlobalOpts) -> Result<u8> {
    // R4 reads the same merged catalog `check` does so user
    // overrides apply; the rules file's `[[expect]]` block is
    // ignored — `where` is per-command, not rule-driven.
    let rules_path = locate_rules(global.config.as_deref())?;
    let cfg = match rules_path.as_ref() {
        Some(p) => Config::from_path(p)?,
        None => Config::default(),
    };
    let merged = catalog::merge_with_user(&cfg.source);
    enforce_source_validation(&merged, Os::current())?;
    let relations = catalog::merge_with_user_relations(&cfg.relations);
    enforce_relation_acyclic(&relations)?;
    let path_entries = read_path_entries(global);

    let outcome = trace::locate(&args.command, &merged, &relations, Os::current(), |cmd| {
        resolve::resolve(cmd, &path_entries)
    });

    if args.json {
        let json = format::where_json(&args.command, &outcome)?;
        println!("{json}");
        return Ok(match outcome {
            TraceOutcome::NotFound => 1,
            TraceOutcome::Found(_) => 0,
        });
    }

    match outcome {
        TraceOutcome::NotFound => {
            println!("{}", format::where_not_found(&args.command));
            Ok(1)
        }
        TraceOutcome::Found(found) => {
            println!("{}", format::where_human(&found));
            Ok(0)
        }
    }
}

fn execute_sort(args: &SortArgs, global: &crate::cli::GlobalOpts) -> Result<u8> {
    // 0.0.14: --dry-run is opt-in. Running `pathlint sort` without
    // a mode flag is a configuration error so callers always
    // declare intent (and so that adding `--apply` post-1.0 is
    // non-breaking — the new flag will simply switch the path).
    if !args.dry_run {
        anyhow::bail!(
            "pathlint sort requires --dry-run (the only mode currently shipped). \
             A future --apply mode is reserved for post-1.0; pass --dry-run \
             explicitly to acknowledge that pathlint never mutates PATH today."
        );
    }
    // sort reads the same merged catalog and rules as `check`, so
    // its proposal aligns with the rules the user is already
    // running against. The rules-file `[[expect]]` block is the
    // input — `prefer` rules drive the reordering.
    let rules_path = locate_rules(global.config.as_deref())?;
    let cfg = match rules_path.as_ref() {
        Some(p) => pathlint::config::Config::from_path(p)?,
        None => pathlint::config::Config::default(),
    };
    let catalog = catalog::merge_with_user(&cfg.source);
    enforce_source_validation(&catalog, Os::current())?;
    let relations = catalog::merge_with_user_relations(&cfg.relations);
    enforce_relation_acyclic(&relations)?;
    let path_entries = read_path_entries(global);

    let plan = pathlint::sort::sort_path(
        &path_entries,
        &cfg.expectations,
        &catalog,
        &relations,
        Os::current(),
    );

    if args.json {
        let json = format::sort_json(&plan)?;
        println!("{json}");
    } else {
        println!("{}", format::sort_human(&plan));
    }

    // sort is a *suggestion* command — it never reports failure,
    // even when the plan would change the order. Use `pathlint
    // check` for go/no-go.
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
            anyhow::bail!("--config path not found: {}", p.display());
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
