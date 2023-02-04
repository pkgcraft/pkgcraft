use std::process::ExitCode;

use clap::{Args, Subcommand};

use crate::Run;

mod compare;
mod format;
mod intersect;
mod parse;
mod sort;

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub(super) struct Atom {
    #[command(subcommand)]
    command: Command,
}

impl Run for Atom {
    fn run(&self) -> anyhow::Result<ExitCode> {
        self.command.run()
    }
}

#[derive(Debug, Subcommand)]
pub(super) enum Command {
    /// Compare two atoms
    Compare(compare::Compare),
    /// Parse an atom and print formatted output
    Format(format::Format),
    /// Determine if two atoms intersect
    Intersect(intersect::Intersect),
    /// Parse an atom
    Parse(parse::Parse),
    /// Sort atoms
    Sort(sort::Sort),
}

impl Run for Command {
    fn run(&self) -> anyhow::Result<ExitCode> {
        use Command::*;
        match self {
            Compare(args) => args.run(),
            Format(args) => args.run(),
            Intersect(args) => args.run(),
            Parse(args) => args.run(),
            Sort(args) => args.run(),
        }
    }
}
