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
    pub(super) fn run(self) -> anyhow::Result<ExitCode> {
        self.command.run()
    }
}

#[derive(Debug, clap::Subcommand)]
enum Subcommand {
    /// Compare two deps
    Compare(Box<compare::Command>),
    /// Determine if a dep intersects another value
    Intersect(Box<intersect::Command>),
    /// Parse a dep and optionally print formatted output
    Parse(Box<parse::Command>),
    /// Collapse deps into a set
    Set(Box<set::Command>),
    /// Sort deps
    Sort(Box<sort::Command>),
}

impl Subcommand {
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
