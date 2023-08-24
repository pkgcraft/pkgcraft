use std::io::{stdout, IsTerminal};
use std::process::ExitCode;

use clap::Args;
use pkgcraft::config::Config;

use crate::args::{bounded_jobs, target_ebuild_repos};

#[derive(Debug, Args)]
pub struct Command {
    /// Parallel jobs to run
    #[arg(short, long)]
    jobs: Option<usize>,

    /// Force regeneration to occur
    #[arg(short, long)]
    force: bool,

    // positionals
    /// Target repositories
    #[arg(value_name = "REPO", required = true)]
    repos: Vec<String>,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // force bounds on jobs
        let jobs = bounded_jobs(self.jobs)?;
        let progress = stdout().is_terminal();
        let mut status = ExitCode::SUCCESS;

        // run metadata regeneration displaying a progress bar if stdout is a terminal
        for repo in target_ebuild_repos(config, &self.repos)? {
            let errors = repo.pkg_metadata_regen(jobs, self.force, progress)?;
            if errors > 0 {
                status = ExitCode::FAILURE;
            }
        }

        Ok(status)
    }
}
