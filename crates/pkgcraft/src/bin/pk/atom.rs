use std::process::ExitCode;

use clap::{Args, Subcommand};

use crate::Run;

mod compare;
mod intersect;
mod parse;
mod set;
mod sort;

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub(super) struct Atom {
    #[command(subcommand)]
    command: Command,
}

impl Run for Atom {
    fn run(self) -> anyhow::Result<ExitCode> {
        self.command.run()
    }
}

#[derive(Debug, Subcommand)]
pub(super) enum Command {
    /// Compare two atoms
    Compare(compare::Compare),
    /// Determine if two atoms intersect
    Intersect(intersect::Intersect),
    /// Parse an atom and optionally print formatted output
    Parse(parse::Parse),
    /// Collapse input into a set of atoms
    Set(set::Set),
    /// Sort atoms
    Sort(sort::Sort),
}

impl Run for Command {
    fn run(self) -> anyhow::Result<ExitCode> {
        use Command::*;
        match self {
            Compare(cmd) => cmd.run(),
            Intersect(cmd) => cmd.run(),
            Parse(cmd) => cmd.run(),
            Set(cmd) => cmd.run(),
            Sort(cmd) => cmd.run(),
        }
    }
}
