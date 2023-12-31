use std::process::ExitCode;

use pkgcraft::config::Config;

mod prune;
mod regen;
mod remove;

#[derive(Debug, clap::Args)]
pub struct Command {
    #[command(subcommand)]
    command: Subcommand,
}

impl Command {
    pub(super) fn run(self, config: &mut Config) -> anyhow::Result<ExitCode> {
        self.command.run(config)
    }
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    /// Prune outdated entries
    Prune(prune::Command),
    /// Regenerate metadata cache
    Regen(regen::Command),
    /// Remove metadata cache
    Remove(remove::Command),
}

impl Subcommand {
    fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        use Subcommand::*;
        match self {
            Regen(cmd) => cmd.run(config),
            Prune(cmd) => cmd.run(config),
            Remove(cmd) => cmd.run(config),
        }
    }
}
