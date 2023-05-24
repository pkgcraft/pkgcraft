use std::process::ExitCode;

use pkgcraft::config::Config;

use crate::Run;

mod metadata;

#[derive(Debug, clap::Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct Command {
    #[command(subcommand)]
    command: Subcommand,
}

impl Run for Command {
    fn run(self, config: &Config) -> anyhow::Result<ExitCode> {
        self.command.run(config)
    }
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    /// Generate repo metadata
    Metadata(metadata::Command),
}

impl Run for Subcommand {
    fn run(self, config: &Config) -> anyhow::Result<ExitCode> {
        use Subcommand::*;
        match self {
            Metadata(cmd) => cmd.run(config),
        }
    }
}
