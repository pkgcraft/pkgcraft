use std::process::ExitCode;

use pkgcraft::config::Config;

mod env;
mod metadata;
mod pretend;
mod revdeps;
mod showkw;
mod source;

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
    /// Output ebuild environment
    Env(Box<env::Command>),
    /// Manipulate package metadata
    Metadata(Box<metadata::Command>),
    /// Run the pkg_pretend phase
    Pretend(Box<pretend::Command>),
    /// Output reverse dependencies
    Revdeps(Box<revdeps::Command>),
    /// Output package keywords
    Showkw(Box<showkw::Command>),
    /// Benchmark ebuild sourcing
    Source(Box<source::Command>),
}

impl Subcommand {
    fn run(self, config: &mut Config) -> anyhow::Result<ExitCode> {
        use Subcommand::*;
        match self {
            Env(cmd) => cmd.run(config),
            Metadata(cmd) => cmd.run(config),
            Pretend(cmd) => cmd.run(config),
            Revdeps(cmd) => cmd.run(config),
            Showkw(cmd) => cmd.run(config),
            Source(cmd) => cmd.run(config),
        }
    }
}
