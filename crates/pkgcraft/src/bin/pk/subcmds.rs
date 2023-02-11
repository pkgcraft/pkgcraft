use std::process::ExitCode;

mod atom;
mod version;

use crate::Run;

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    /// Perform atom-related actions including parsing, intersection, and sorting
    Atom(atom::Atom),
    /// Perform version-related actions including parsing, intersection, and sorting
    Version(version::Version),
}

impl Run for Subcommand {
    fn run(self) -> anyhow::Result<ExitCode> {
        use Subcommand::*;
        match self {
            Atom(cmd) => cmd.run(),
            Version(cmd) => cmd.run(),
        }
    }
}
