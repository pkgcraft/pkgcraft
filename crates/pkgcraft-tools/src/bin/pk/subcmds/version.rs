use std::process::ExitCode;

mod compare;
mod intersect;
mod parse;
mod set;
mod sort;

#[derive(Debug, clap::Args)]
#[command(args_conflicts_with_subcommands = true)]
pub(crate) struct Command {
    #[command(subcommand)]
    command: Subcommand,
}

impl Command {
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        self.command.run()
    }
}

#[derive(Debug, clap::Subcommand)]
enum Subcommand {
    /// Compare two versions
    Compare(Box<compare::Command>),
    /// Determine if two versions intersect
    Intersect(Box<intersect::Command>),
    /// Parse a version and optionally print formatted output
    Parse(Box<parse::Command>),
    /// Collapse versions into a set
    Set(Box<set::Command>),
    /// Sort versions
    Sort(Box<sort::Command>),
}

impl Subcommand {
    fn run(&self) -> anyhow::Result<ExitCode> {
        match self {
            Self::Compare(cmd) => cmd.run(),
            Self::Intersect(cmd) => cmd.run(),
            Self::Parse(cmd) => cmd.run(),
            Self::Set(cmd) => cmd.run(),
            Self::Sort(cmd) => cmd.run(),
        }
    }
}
