use std::process::ExitCode;

mod dep;
mod version;

use crate::Run;

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    /// Perform dep-related actions including parsing, intersection, and sorting
    Dep(dep::Command),
    /// Perform version-related actions including parsing, intersection, and sorting
    Version(version::Command),
}

impl Run for Subcommand {
    fn run(self) -> anyhow::Result<ExitCode> {
        use Subcommand::*;
        match self {
            Dep(cmd) => cmd.run(),
            Version(cmd) => cmd.run(),
        }
    }
}
