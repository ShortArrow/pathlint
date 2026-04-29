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
    Check,

    /// Write a starter `pathlint.toml` in the current directory.
    Init(InitArgs),

    /// Inspect the source catalog.
    Catalog {
        #[command(subcommand)]
        action: CatalogCommand,
    },
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
