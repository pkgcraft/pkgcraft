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
    pub(super) fn run(self, config: &mut Config) -> anyhow::Result<ExitCode> {
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
        use Subcommand::*;
        match self {
            Clean(cmd) => cmd.run(config),
            Regen(cmd) => cmd.run(config),
            Remove(cmd) => cmd.run(config),
        }
    }
}
