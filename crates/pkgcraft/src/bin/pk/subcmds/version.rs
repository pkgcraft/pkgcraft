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
pub struct Version {
    #[command(subcommand)]
    command: Command,
}

impl Run for Version {
    fn run(self) -> anyhow::Result<ExitCode> {
        self.command.run()
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Compare two versions
    Compare(compare::Compare),
    /// Determine if two versions intersect
    Intersect(intersect::Intersect),
    /// Parse a version and optionally print formatted output
    Parse(parse::Parse),
    /// Collapse input into a set of versions
    Set(set::Set),
    /// Sort versions
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
