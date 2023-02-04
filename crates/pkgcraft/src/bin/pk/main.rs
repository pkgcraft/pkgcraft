use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};

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
    Atom(Atom),
    /// Perform version-related actions including parsing, intersection, and sorting
    Version(Version),
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
struct Atom {
    #[command(subcommand)]
    command: atom::Command,
}

impl Run for Atom {
    fn run(&self) -> anyhow::Result<ExitCode> {
        self.command.run()
    }
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
struct Version {
    #[command(subcommand)]
    command: version::Command,
}

impl Run for Version {
    fn run(&self) -> anyhow::Result<ExitCode> {
        self.command.run()
    }
}

trait Run {
    fn run(&self) -> anyhow::Result<ExitCode>;
}

fn main() -> anyhow::Result<ExitCode> {
    let args = Cli::parse();
    use Command::*;
    match &args.command {
        Atom(cmd) => cmd.run(),
        Version(cmd) => cmd.run(),
    }
}
