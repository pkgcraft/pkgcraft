use std::process::ExitCode;

use pkgcraft::config::Config;

mod add;
mod eapi;
mod eclass;
mod leaf;
mod license;
mod metadata;
mod mirror;
mod remove;
mod revdeps;
mod sync;

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
#[allow(clippy::large_enum_variant)]
enum Subcommand {
    /// Add repository to config
    Add(add::Command),
    /// Output EAPI statistics
    Eapi(eapi::Command),
    /// Output eclass statistics
    Eclass(eclass::Command),
    /// Output leaf packages
    Leaf(leaf::Command),
    /// Output license statistics
    License(license::Command),
    /// Manipulate repo metadata
    Metadata(metadata::Command),
    /// Output mirror statistics
    Mirror(mirror::Command),
    /// Remove repositories
    Remove(remove::Command),
    /// Output revdeps cache
    Revdeps(revdeps::Command),
    /// Sync repos
    Sync(sync::Command),
}

impl Subcommand {
    fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        match self {
            Self::Add(cmd) => cmd.run(config),
            Self::Eapi(cmd) => cmd.run(config),
            Self::Eclass(cmd) => cmd.run(config),
            Self::Leaf(cmd) => cmd.run(config),
            Self::License(cmd) => cmd.run(config),
            Self::Metadata(cmd) => cmd.run(config),
            Self::Mirror(cmd) => cmd.run(config),
            Self::Remove(cmd) => cmd.run(config),
            Self::Revdeps(cmd) => cmd.run(config),
            Self::Sync(cmd) => cmd.run(config),
        }
    }
}
