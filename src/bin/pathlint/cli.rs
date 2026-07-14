//! Command-line interface definitions.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use pathlint::path_source::Target;

#[derive(Debug, Parser)]
#[command(name = "pathlint", version, about = "Lint PATH against [[expect]] rules", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    #[command(flatten)]
    pub global: GlobalOpts,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Lint PATH against expectations (default).
    Check(CheckArgs),

    /// Write a starter `pathlint.toml` in the current directory.
    Init(InitArgs),

    /// Inspect the source catalog.
    Catalog {
        #[command(subcommand)]
        action: CatalogCommand,
    },

    /// Lint the PATH itself (duplicates, missing dirs, env-var
    /// shortening candidates, Windows 8.3 short names, malformed
    /// entries) plus semantic validation of `pathlint.toml` against
    /// the catalog. Independent of `[[expect]]` rules. New in 0.0.34;
    /// inherits the detector kinds previously emitted by `doctor`.
    Lint(LintArgs),

    /// Check that pathlint itself is functional in this environment:
    /// the running binary's PATH placement, `pathlint.toml` discovery
    /// and parse, and `env_lookup` operational. Does NOT inspect PATH
    /// for anomalies — that moved to `lint` in 0.0.34.
    Doctor(DoctorArgs),

    /// Show where a command resolves from, which sources it matches,
    /// and the most plausible uninstall command. Renamed from
    /// `pathlint where` in 0.0.14; the legacy alias was removed in
    /// 0.0.22 after a deprecation-warning runway in 0.0.20–0.0.21.
    Trace(TraceArgs),

    /// Propose a PATH order that satisfies every applicable
    /// `[[expect]]` rule. Read-only by design — pathlint never
    /// rewrites PATH, just prints the diff (default) or JSON.
    Sort(SortArgs),
}

#[derive(Debug, clap::Args)]
pub struct SortArgs {
    /// Print the proposal without touching PATH. This is the only
    /// mode `sort` ships today; the flag is opt-in so callers
    /// signal awareness that pathlint never mutates PATH and so
    /// that adding `--apply` later (post-1.0) is a non-breaking
    /// change. As of 0.0.14, `pathlint sort` without `--dry-run`
    /// exits 2 with an explanation; a future `--apply` would
    /// override this.
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,

    /// Emit the proposal as a JSON object (`SortPlan`) instead of
    /// the default before / after diff. Schema is stable through
    /// 0.0.x; notes carry a `kind` discriminator so consumers can
    /// pattern-match on them.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, clap::Args, Default)]
pub struct CheckArgs {
    /// Expand each NG outcome into a multi-line breakdown — resolved
    /// path, matched sources, prefer / avoid lists, the underlying
    /// diagnosis, and a follow-up hint. Use this when the one-line
    /// detail is not enough to figure out why a rule failed.
    #[arg(long, conflicts_with = "json")]
    pub explain: bool,

    /// Emit one JSON array describing every expectation: status,
    /// resolved path, matched sources, prefer / avoid, and a
    /// `diagnosis` object on failures. Schema is stable through
    /// 0.0.x; the diagnosis uses a `kind` discriminator so consumers
    /// can match on it. Mutually exclusive with --explain.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, clap::Args)]
pub struct TraceArgs {
    /// The command to look up on PATH.
    pub command: String,

    /// Emit machine-readable JSON instead of the default human
    /// output. The schema is described in the README; provenance
    /// and uninstall objects use a `kind` discriminator so consumers
    /// can match on it.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, clap::Args)]
pub struct DoctorArgs {
    /// Emit selfcheck diagnostics as a JSON array. Each diagnostic
    /// carries `index` / `entry` (sentinels for selfcheck: index =
    /// `2^64 - 1`, entry = ""), `severity` (`error` / `warn` /
    /// `info` — info is new in 0.0.34), and `kind` (4-variant enum:
    /// `binary_not_in_path`, `config_parse_error`, `config_not_found`,
    /// `env_lookup_failed`). Schema: `schemas/doctor.schema.json`
    /// (shared with `pathlint lint --json` — the schema lists all 16
    /// variants, doctor only emits the 4 selfcheck ones). Replaced
    /// the 0.0.33 PATH-anomaly output; the old behaviour is now
    /// `pathlint lint --json`.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, clap::Args)]
pub struct LintArgs {
    /// Only show diagnostics whose kind matches one of the listed
    /// values. Mutually exclusive with `--exclude`. Accepts a comma
    /// or repeated flag form: `--include duplicate,missing` or
    /// `--include duplicate --include missing`. Inherited from the
    /// 0.0.33 doctor surface.
    #[arg(long, value_delimiter = ',', conflicts_with = "exclude")]
    pub include: Vec<String>,

    /// Suppress diagnostics whose kind matches one of the listed
    /// values. Affects exit code too: an excluded `Malformed` no
    /// longer escalates to exit 1.
    #[arg(long, value_delimiter = ',')]
    pub exclude: Vec<String>,

    /// Emit the (already-filtered) diagnostics as a JSON array —
    /// machine-readable counterpart of the human view. Each element
    /// has `index`, `entry`, `severity`, `kind`, plus any per-kind
    /// payload fields (`suggestion`, `canonical`, `first_index`,
    /// `reason`, or `diagnostic` + `groups` for the `conflict` kind).
    /// Schema: `schemas/doctor.schema.json` (shared with
    /// `pathlint doctor --json` since 0.0.34 — the schema lists
    /// all 16 variants, lint only emits the 12 PATH-anomaly
    /// ones). The include / exclude filters still apply;
    /// `--quiet` is ignored in JSON mode (the output is intended
    /// to be complete).
    #[arg(long)]
    pub json: bool,

    /// Emit the (already-filtered) diagnostics as a SARIF 2.1.0
    /// log for GitHub Code Scanning and other static-analysis
    /// aggregators. Rule ids are the same snake_case kind names
    /// the --json output uses; severity maps to SARIF level
    /// (error / warning / note). Each result anchors to the
    /// discovered pathlint.toml and carries the PATH entry in its
    /// message and logicalLocations. Mutually exclusive with
    /// `--json`. New in 0.0.42.
    #[arg(long, conflicts_with = "json")]
    pub sarif: bool,
}

#[derive(Debug, Subcommand)]
pub enum CatalogCommand {
    /// List every known source — built-in plus any defined in
    /// `pathlint.toml` — with its description and per-OS path.
    List(CatalogListArgs),

    /// List every declared `[[relation]]` between sources, both
    /// built-in (from `plugins/*.toml`) and user-defined (from
    /// `pathlint.toml`). Useful for understanding why a doctor
    /// diagnostic fires or how `pathlint trace` infers provenance.
    Relations(CatalogRelationsArgs),
}

#[derive(Debug, clap::Args)]
pub struct CatalogListArgs {
    /// Show every per-OS path of each source, not just the one for
    /// the running OS.
    #[arg(long)]
    pub all: bool,

    /// Print only source names, one per line.
    #[arg(long)]
    pub names_only: bool,
}

#[derive(Debug, clap::Args)]
pub struct CatalogRelationsArgs {
    /// Emit the relations as a JSON array, with each element
    /// carrying its `kind` discriminator. Schema is stable through
    /// 0.0.x.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, clap::Args)]
pub struct InitArgs {
    /// Also embed the entire built-in source catalog so users can
    /// edit per-OS paths field by field. Off by default to keep the
    /// starter file short.
    #[arg(long)]
    pub emit_defaults: bool,

    /// Overwrite an existing `pathlint.toml` if one is already present.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, clap::Args)]
pub struct GlobalOpts {
    /// PATH source: process (default) / user / machine. user / machine
    /// are Windows-only. On Windows, the default `process` target
    /// additionally overlays HKCU + HKLM raw forms on each entry whose
    /// expanded path matches the registry, so `%LocalAppData%\...`
    /// authored entries display in their raw form and Shortenable
    /// does not mis-suggest re-shortening them (see PRD §10.1).
    #[arg(long, value_enum, default_value_t = TargetArg::Process)]
    pub target: TargetArg,

    /// Path to pathlint.toml. Default search: ./pathlint.toml, then
    /// parent directories up to the enclosing `.git`, then
    /// $XDG_CONFIG_HOME/pathlint/pathlint.toml. Renamed from
    /// `--rules` in 0.0.14; the legacy `--rules` alias was removed
    /// in 0.0.22 after a deprecation-warning runway in 0.0.20–0.0.21.
    #[arg(long = "config")]
    pub config: Option<PathBuf>,

    /// Which configuration layers to search for pathlint.toml.
    /// `auto` (default) searches the current directory, then parent
    /// directories up to the enclosing `.git`, then the user-global
    /// `$XDG_CONFIG_HOME/pathlint/` location. `local` stops after
    /// the repo-local layers and never falls through to the
    /// user-global file; `global` reads only the user-global
    /// location. An explicit `--config <path>` always wins over
    /// this flag. `init --scope=global` writes the starter file
    /// into the user-global location instead of the current
    /// directory. New in 0.0.41.
    #[arg(long, value_enum, default_value_t = ScopeArg::Auto)]
    pub scope: ScopeArg,

    /// Print every expectation incl. n/a, plus the resolved PATH.
    #[arg(short, long)]
    pub verbose: bool,

    /// Only print failures.
    #[arg(short, long)]
    pub quiet: bool,

    /// Color output.
    #[arg(long, value_enum, default_value_t = ColorArg::Auto)]
    pub color: ColorArg,

    /// ASCII-only output.
    #[arg(long)]
    pub no_glyphs: bool,
}

/// Which configuration layers `pathlint.toml` discovery may read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ScopeArg {
    /// Repo-local layers first (cwd, then parents up to the
    /// enclosing `.git`), then the user-global XDG location.
    Auto,
    /// Repo-local layers only; never fall through to the
    /// user-global file.
    Local,
    /// User-global XDG location only; ignore repo-local files.
    Global,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum TargetArg {
    Process,
    User,
    Machine,
}

impl From<TargetArg> for Target {
    fn from(t: TargetArg) -> Self {
        match t {
            TargetArg::Process => Target::Process,
            TargetArg::User => Target::User,
            TargetArg::Machine => Target::Machine,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ColorArg {
    Auto,
    Always,
    Never,
}

impl ColorArg {
    /// Resolve `auto` against the terminal-detection signal a caller
    /// already obtained (typically `std::io::stdout().is_terminal()`).
    /// `always` and `never` ignore the signal.
    ///
    /// 0.0.17 promoted this from a parsed-but-ignored CLI flag to an
    /// effective contract; the `bool` return is what
    /// `report::Style::color` consumes.
    pub fn resolve(self, is_tty: bool) -> bool {
        match self {
            ColorArg::Always => true,
            ColorArg::Never => false,
            ColorArg::Auto => is_tty,
        }
    }
}
