//! Command-line interface definitions.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::path_source::Target;

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
    /// entries). Independent of `[[expect]]` rules.
    Doctor(DoctorArgs),

    /// Show where a command resolves from, which sources it matches,
    /// and the most plausible uninstall command.
    Where(WhereArgs),
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
pub struct WhereArgs {
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
    /// Only show diagnostics whose kind matches one of the listed
    /// values. Mutually exclusive with `--exclude`. Accepts a comma
    /// or repeated flag form: `--include duplicate,missing` or
    /// `--include duplicate --include missing`.
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
    /// `reason`, `shim_indices` / `install_indices`). Schema is
    /// stable through 0.0.x, parallels `check --json`. The
    /// include / exclude filters still apply; `--quiet` is ignored
    /// in JSON mode (the output is intended to be complete).
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Subcommand)]
pub enum CatalogCommand {
    /// List every known source — built-in plus any defined in
    /// `pathlint.toml` — with its description and per-OS path.
    List(CatalogListArgs),
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
    /// are Windows-only.
    #[arg(long, value_enum, default_value_t = TargetArg::Process)]
    pub target: TargetArg,

    /// Path to pathlint.toml. Default search: ./pathlint.toml then
    /// $XDG_CONFIG_HOME/pathlint/pathlint.toml.
    #[arg(long)]
    pub rules: Option<PathBuf>,

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
