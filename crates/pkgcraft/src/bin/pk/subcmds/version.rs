use std::process::ExitCode;

use pkgcraft::dep::Version;

use crate::Run;

mod compare;
mod intersect;
mod parse;
mod set;
mod sort;

#[derive(Debug, clap::Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct VersionCmd {
    #[command(subcommand)]
    command: Subcommand,
}

impl Run for VersionCmd {
    fn run(self) -> anyhow::Result<ExitCode> {
        self.command.run()
    }
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
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

impl Run for Subcommand {
    fn run(self) -> anyhow::Result<ExitCode> {
        use Subcommand::*;
        match self {
            Compare(cmd) => cmd.run(),
            Intersect(cmd) => cmd.run(),
            Parse(cmd) => cmd.run(),
            Set(cmd) => cmd.run(),
            Sort(cmd) => cmd.run(),
        }
    }
}

// Parse regular version with op-ed fallback.
fn ver_new(s: &str) -> pkgcraft::Result<Version> {
    Version::new(s).or_else(|_| Version::new_with_op(s))
}
