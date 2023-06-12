use std::process::ExitCode;

use pkgcraft::config::Config;

mod metadata;

#[derive(Debug, clap::Args)]
pub struct Command {
    #[command(subcommand)]
    command: Subcommand,
}

impl Command {
    pub(super) fn run(self, config: &Config) -> anyhow::Result<ExitCode> {
        self.command.run(config)
    }
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    /// Generate repo metadata
    Metadata(metadata::Command),
}

impl Subcommand {
    fn run(&self, config: &Config) -> anyhow::Result<ExitCode> {
        use Subcommand::*;
        match self {
            Metadata(cmd) => cmd.run(config),
        }
    }
}
