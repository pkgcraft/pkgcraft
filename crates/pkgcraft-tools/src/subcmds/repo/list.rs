use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use pkgcraft::config::Config;
use pkgcraft::repo::Repository;

#[derive(Args)]
#[clap(next_help_heading = "List options")]
pub(crate) struct Command {
    /// Output full repository info
    #[arg(short, long)]
    full: bool,

    /// Output repository paths
    #[arg(short, long, conflicts_with = "full")]
    path: bool,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // make sure system config is loaded if custom config wasn't specified
        config.load()?;

        let mut stdout = io::stdout().lock();
        for (name, repo) in config.repos() {
            if self.path {
                writeln!(stdout, "{}", repo.path())?;
            } else {
                writeln!(stdout, "{name}")?;
                if self.full {
                    writeln!(stdout, "{}", repo.config())?;
                }
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
