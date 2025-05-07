use std::process::ExitCode;

use clap::Args;
use pkgcraft::config::Config;

#[derive(Args)]
#[clap(next_help_heading = "Remove options")]
pub(crate) struct Command {
    /// Repository names
    repos: Vec<String>,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // make sure system config is loaded if custom config wasn't specified
        config.load()?;

        // remove specified repos
        config.repos().remove(&self.repos)?;

        Ok(ExitCode::SUCCESS)
    }
}
