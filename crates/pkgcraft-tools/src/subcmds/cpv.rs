use std::process::ExitCode;

mod compare;
mod intersect;
mod parse;
mod set;
mod sort;

#[derive(clap::Args)]
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

#[derive(clap::Subcommand)]
#[allow(clippy::large_enum_variant)]
enum Subcommand {
    /// Compare two cpvs
    Compare(compare::Command),
    /// Determine if a cpv intersects another value
    Intersect(intersect::Command),
    /// Parse cpv and optionally print formatted output
    Parse(parse::Command),
    /// Collapse cpvs into a set
    Set(set::Command),
    /// Sort cpvs
    Sort(sort::Command),
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
