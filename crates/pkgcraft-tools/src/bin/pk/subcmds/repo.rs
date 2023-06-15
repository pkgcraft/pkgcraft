use std::process::ExitCode;

use pkgcraft::config::Config;

mod eapi;
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
    /// Output repo EAPI usage rates
    Eapi(eapi::Command),
    /// Generate repo metadata
    Metadata(metadata::Command),
}

impl Subcommand {
    fn run(&self, config: &Config) -> anyhow::Result<ExitCode> {
        use Subcommand::*;
        match self {
            Eapi(cmd) => cmd.run(config),
            Metadata(cmd) => cmd.run(config),
        }
    }
}
