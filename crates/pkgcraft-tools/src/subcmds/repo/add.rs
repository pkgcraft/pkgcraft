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

    /// Repository URIs
    #[arg(required = true, value_name = "URI", help_heading = "Arguments")]
    uris: Vec<String>,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // make sure system config is loaded if custom config wasn't specified
        config.load()?;

        for uri in &self.uris {
            // create a RepoConfig
            let mut config_builder = config.repos().add_uri(uri)?;

            if let Some(value) = self.name.as_deref() {
                config_builder.name(value);
            }

            if let Some(value) = self.priority {
                config_builder.priority(value);
            }

            // serialize RepoConfig to file while optionally syncing
            config_builder.add_to_config(!self.file)?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
