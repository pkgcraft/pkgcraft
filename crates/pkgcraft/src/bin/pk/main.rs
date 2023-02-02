use std::process::ExitCode;

use clap::{Parser, Subcommand};

mod atom;

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
}

fn main() -> anyhow::Result<ExitCode> {
    let args = Cli::parse();
    match args.command {
        Command::Atom(s) => atom::main(s),
    }
}
