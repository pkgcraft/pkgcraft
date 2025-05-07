use std::process::ExitCode;

use clap::Args;
use pkgcraft::config::Config;

#[derive(Args)]
#[clap(next_help_heading = "Add options")]
pub(crate) struct Command {
    /// Repository name
    name: String,

    /// Target repositories
    /// Repository URL
    url: String,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // make sure system config is loaded if custom config wasn't specified
        config.load()?;

        // add custom repo to the config
        config.repos_mut()?.add_uri(&self.name, 0, &self.url)?;

        Ok(ExitCode::SUCCESS)
    }
}
