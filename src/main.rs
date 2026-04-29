use std::process::ExitCode;

use clap::Parser;
use pathlint::cli::Cli;
use pathlint::run;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run::execute(cli) {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            eprintln!("pathlint: {err:#}");
            ExitCode::from(2)
        }
    }
}
