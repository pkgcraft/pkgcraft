use std::process::ExitCode;

use anyhow::anyhow;
use pkgcraft::config::Config;
use pkgcraft::repo::Repo;

use crate::Run;

mod metadata;

#[derive(Debug, clap::Args)]
pub struct Command {
    #[command(subcommand)]
    command: Subcommand,

    /// Target repository
    #[arg(short, long, required = true)]
    repo: String,
}

impl Run for Command {
    fn run(self, config: &Config) -> anyhow::Result<ExitCode> {
        // determine target repo
        let repo = match config.repos.get(&self.repo) {
            Some(r) => Ok(r.clone()),
            None => Repo::from_path(&self.repo, 0, &self.repo, true),
        };

        let repo = repo.map_err(|_| anyhow!("unknown repo: {}", self.repo))?;

        self.command.run(repo)
    }
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    /// Generate repo metadata
    Metadata(metadata::Command),
}

impl Subcommand {
    fn run(&self, repo: Repo) -> anyhow::Result<ExitCode> {
        use Subcommand::*;
        match self {
            Metadata(cmd) => cmd.run(repo),
        }
    }
}
