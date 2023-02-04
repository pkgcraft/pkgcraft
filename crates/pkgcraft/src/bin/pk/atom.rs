use std::process::ExitCode;

use clap::Subcommand;

use crate::Run;

mod compare;
mod format;
mod intersect;
mod parse;
mod sort;

#[derive(Debug, Subcommand)]
pub(super) enum Command {
    /// Compare two atoms
    Compare(compare::Args),
    /// Parse an atom and print formatted output
    Format(format::Args),
    /// Determine if two atoms intersect
    Intersect(intersect::Args),
    /// Parse an atom
    Parse(parse::Args),
    /// Sort atoms
    Sort(sort::Args),
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
