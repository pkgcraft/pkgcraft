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
pub(super) struct Version {
    #[command(subcommand)]
    command: Command,
}

impl Run for Version {
    fn run(self) -> anyhow::Result<ExitCode> {
        self.command.run()
    }
}

#[derive(Debug, Subcommand)]
pub(super) enum Command {
    /// Compare two versions
    Compare(compare::Compare),
    /// Parse a version and print formatted output
    Format(format::Format),
    /// Determine if two versions intersect
    Intersect(intersect::Intersect),
    /// Parse a version
    Parse(parse::Parse),
    /// Sort versions
    Sort(sort::Sort),
}

impl Run for Command {
    fn run(self) -> anyhow::Result<ExitCode> {
        use Command::*;
        match self {
            Compare(cmd) => cmd.run(),
            Format(cmd) => cmd.run(),
            Intersect(cmd) => cmd.run(),
            Parse(cmd) => cmd.run(),
            Sort(cmd) => cmd.run(),
        }
    }
}
