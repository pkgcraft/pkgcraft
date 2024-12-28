use std::process::ExitCode;

use pkgcraft::config::Config;

mod eapis;
mod leaf;
mod metadata;

#[derive(clap::Args)]
pub(crate) struct Command {
    #[command(subcommand)]
    command: Subcommand,
}

impl Command {
    pub(super) fn run(&self, mut config: Config) -> anyhow::Result<ExitCode> {
        self.command.run(&mut config)
    }
}

#[derive(clap::Subcommand)]
enum Subcommand {
    /// Output EAPI usage rates
    Eapis(Box<eapis::Command>),
    /// Output leaf packages
    Leaf(Box<leaf::Command>),
    /// Manipulate repo metadata
    Metadata(Box<metadata::Command>),
}

impl Subcommand {
    fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        match self {
            Self::Eapis(cmd) => cmd.run(config),
            Self::Leaf(cmd) => cmd.run(config),
            Self::Metadata(cmd) => cmd.run(config),
        }
    }
}
