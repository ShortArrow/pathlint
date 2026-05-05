//! `pathlint` binary entry point.
//!
//! 0.0.17 moved this file (along with `cli.rs` and `run.rs`)
//! out of the library crate root into `src/bin/pathlint/` so
//! the binary owns its CLI plumbing instead of having to leak
//! `cli` / `run` modules through the lib via
//! `#[doc(hidden)] pub mod`. The library surface is now strictly
//! the nine supported modules — see `pathlint::*` rustdocs.

use std::process::ExitCode;

use clap::Parser;

mod cli;
mod run;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();
    match run::execute(cli) {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            eprintln!("pathlint: {err:#}");
            ExitCode::from(2)
        }
    }
}
