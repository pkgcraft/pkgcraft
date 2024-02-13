use std::process::ExitCode;

use pkgcraft::config::Config;

mod eapis;
mod leaf;
mod metadata;

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
    /// Output EAPI usage rates
    Eapis(eapis::Command),
    /// Output leaf packages
    Leaf(leaf::Command),
    /// Manipulate repo metadata
    Metadata(metadata::Command),
}

impl Subcommand {
    fn run(self, config: &mut Config) -> anyhow::Result<ExitCode> {
        use Subcommand::*;
        match self {
            Eapis(cmd) => cmd.run(config),
            Leaf(cmd) => cmd.run(config),
            Metadata(cmd) => cmd.run(config),
        }
    }
}
