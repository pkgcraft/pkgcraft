use std::process::ExitCode;

use clap::Args;
use pkgcraft::config::Config;

#[derive(Args)]
#[clap(next_help_heading = "Add options")]
pub(crate) struct Command {
    /// Only create the config file
    #[arg(long, short, requires = "name")]
    file: bool,

    /// Repository name
    #[arg(long, short)]
    name: Option<String>,

    /// Repository priority
    #[arg(long, short)]
    priority: Option<i32>,

    /// Repository URI
    uri: String,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // make sure system config is loaded if custom config wasn't specified
        config.load()?;

        // create a RepoConfig
        let mut repo_config = config.repos().add_uri(&self.uri)?;

        if let Some(value) = self.name.as_deref() {
            repo_config.name(value);
        }

        if let Some(value) = self.priority {
            repo_config.priority(value);
        }

        // serialize RepoConfig to file while optionally syncing
        repo_config.add_to_config(!self.file)?;

        Ok(ExitCode::SUCCESS)
    }
}
