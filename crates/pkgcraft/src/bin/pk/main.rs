use std::process::ExitCode;

use clap::{Parser, Subcommand};

mod atom;
mod version;

#[derive(Parser, Debug)]
#[command(version, long_about = None, disable_help_subcommand = true)]
/// pkgcraft command-line tool
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Perform actions on atoms including parsing, intersection, and sorting
    Atom(atom::AtomSubCmd),
    /// Perform actions on versions including parsing, intersection, and sorting
    Version(version::VersionSubCmd),
}

fn main() -> anyhow::Result<ExitCode> {
    let args = Cli::parse();
    match args.command {
        Command::Atom(s) => atom::main(s),
        Command::Version(s) => version::main(s),
    }
}
