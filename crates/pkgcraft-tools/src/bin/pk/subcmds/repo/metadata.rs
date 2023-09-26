use std::io::{stdout, IsTerminal};
use std::process::ExitCode;

use clap::Args;
use pkgcraft::config::Config;

use crate::args::target_ebuild_repos;

#[derive(Debug, Args)]
pub struct Command {
    /// Parallel jobs to run
    #[arg(short, long)]
    jobs: Option<usize>,

    /// Force regeneration to occur
    #[arg(short, long)]
    force: bool,

    /// Disable progress bar
    #[arg(short, long)]
    no_progress: bool,

    // positionals
    /// Target repositories
    #[arg(value_name = "REPO", default_value = ".")]
    repos: Vec<String>,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // run metadata regeneration displaying a progress bar if stdout is a terminal
        let progress = stdout().is_terminal() && !self.no_progress;
        for repo in target_ebuild_repos(config, &self.repos)? {
            repo.pkg_metadata_regen(self.jobs, self.force, progress)?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
