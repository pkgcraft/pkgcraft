use std::process::ExitCode;

use clap::Args;
use pkgcraft::config::Config;

#[derive(Args)]
#[clap(next_help_heading = "Remove options")]
pub(crate) struct Command {
    /// Repository name
    #[arg(required = true, value_name = "REPO", help_heading = "Arguments")]
    repos: Vec<String>,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // make sure system config is loaded if custom config wasn't specified
        config.load()?;

        // remove specified repos
        config.repos_mut()?.remove(&self.repos)?;

        Ok(ExitCode::SUCCESS)
    }
}
