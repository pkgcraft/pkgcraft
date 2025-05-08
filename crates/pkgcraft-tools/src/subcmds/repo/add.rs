use std::process::ExitCode;

use clap::Args;
use pkgcraft::config::Config;

#[derive(Args)]
#[clap(next_help_heading = "Add options")]
pub(crate) struct Command {
    /// Repository name
    #[arg(long, short)]
    name: Option<String>,

    /// Repository priority
    #[arg(long, short)]
    priority: Option<i32>,

    /// Sync the repository
    #[arg(long, short)]
    sync: bool,

    /// Repository URL
    url: String,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // make sure system config is loaded if custom config wasn't specified
        config.load()?;

        // create a RepoConfig
        let mut repo_config = config.repos().add_uri(&self.url)?;

        if let Some(value) = self.name.as_deref() {
            repo_config.name(value);
        }

        if let Some(value) = self.priority {
            repo_config.priority(value);
        }

        // serialize RepoConfig to file while optionally syncing
        repo_config.add_to_config(self.sync)?;

        Ok(ExitCode::SUCCESS)
    }
}
