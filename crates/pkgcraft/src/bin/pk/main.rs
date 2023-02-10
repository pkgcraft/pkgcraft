use std::process::ExitCode;

use clap::{Parser, Subcommand};

mod atom;
mod version;

#[derive(Debug, Parser)]
#[command(version, long_about = None, disable_help_subcommand = true)]
/// pkgcraft command-line tool
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Perform atom-related actions including parsing, intersection, and sorting
    Atom(atom::Atom),
    /// Perform version-related actions including parsing, intersection, and sorting
    Version(version::Version),
}

trait Run {
    fn run(self) -> anyhow::Result<ExitCode>;
}

fn main() -> anyhow::Result<ExitCode> {
    let args = Cli::parse();
    use Command::*;
    match args.command {
        Atom(cmd) => cmd.run(),
        Version(cmd) => cmd.run(),
    }
}
