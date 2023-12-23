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

    /// Custom cache path
    #[arg(short, long)]
    path: Option<String>,

    /// Disable progress bar
    #[arg(short, long)]
    no_progress: bool,

    /// Allow output to stderr and stdout (suppressed by default)
    #[arg(short, long)]
    output: bool,

    // positionals
    /// Target repositories
    #[arg(value_name = "REPO", default_value = ".")]
    repos: Vec<String>,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // run metadata regeneration displaying a progress bar if stdout is a terminal
        let progress = stdout().is_terminal() && !self.no_progress && !self.output;

        for repo in target_ebuild_repos(config, &self.repos)? {
            repo.metadata_regen()
                .jobs(self.jobs.unwrap_or_default())
                .force(self.force)
                .progress(progress)
                .suppress(!self.output)
                .cache_path(self.path.as_deref().unwrap_or_default())
                .run()?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
