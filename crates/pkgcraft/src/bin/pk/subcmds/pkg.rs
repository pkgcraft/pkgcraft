use std::process::ExitCode;

use pkgcraft::config::Config;

mod pretend;

#[derive(Debug, clap::Args)]
pub struct Command {
    #[command(subcommand)]
    command: Subcommand,
}

impl Command {
    pub(super) fn run(&self, config: &Config) -> anyhow::Result<ExitCode> {
        self.command.run(config)
    }
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    /// Run the pkg_pretend phase
    Pretend(pretend::Command),
}

impl Subcommand {
    fn run(&self, config: &Config) -> anyhow::Result<ExitCode> {
        use Subcommand::*;
        match self {
            Pretend(cmd) => cmd.run(config),
        }
    }
}
