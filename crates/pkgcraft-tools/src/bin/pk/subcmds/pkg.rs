use std::process::ExitCode;

use pkgcraft::config::Config;

mod env;
mod metadata;
mod pretend;
mod revdeps;
mod showkw;
mod source;

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
    /// Output global environment
    Env(env::Command),
    /// Generate package metadata
    Metadata(metadata::Command),
    /// Run the pkg_pretend phase
    Pretend(pretend::Command),
    /// Output reverse dependencies
    Revdeps(revdeps::Command),
    /// Output package keywords
    Showkw(showkw::Command),
    /// Source ebuilds and dump elapsed time
    Source(source::Command),
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
