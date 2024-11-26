use std::process::ExitCode;

use pkgcraft::config::Config;

mod clean;
mod regen;
mod remove;

#[derive(Debug, clap::Args)]
pub(crate) struct Command {
    #[command(subcommand)]
    command: Subcommand,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        self.command.run(config)
    }
}

#[derive(Debug, clap::Subcommand)]
enum Subcommand {
    /// Clean metadata cache
    Clean(clean::Command),
    /// Regenerate metadata cache
    Regen(regen::Command),
    /// Remove metadata cache
    Remove(remove::Command),
}

impl Subcommand {
    fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        match self {
            Self::Clean(cmd) => cmd.run(config),
            Self::Regen(cmd) => cmd.run(config),
            Self::Remove(cmd) => cmd.run(config),
        }
    }
}
