use std::process::ExitCode;

use clap::Args;

mod checks;
mod reports;

#[derive(Debug, Args)]
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
    /// Show checks
    Checks(checks::Subcommand),
    /// Show reports
    Reports(reports::Subcommand),
}

impl Subcommand {
    fn run(&self) -> anyhow::Result<ExitCode> {
        match self {
            Self::Checks(cmd) => cmd.run(),
            Self::Reports(cmd) => cmd.run(),
        }
    }
}
