use std::io::{stdout, IsTerminal};
use std::process::ExitCode;

use clap::Args;
use pkgcraft::config::Config;
use pkgcraft::repo::ebuild::cache::{Cache, CacheFormat};

use crate::args::target_ebuild_repo;

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

    /// Custom cache format
    #[arg(long)]
    format: Option<CacheFormat>,

    // positionals
    /// Target repository
    #[arg(default_value = ".")]
    repo: String,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // run metadata regeneration displaying a progress bar if stdout is a terminal
        let progress = stdout().is_terminal() && !self.no_progress && !self.output;
        let repo = target_ebuild_repo(config, &self.repo)?;
        let format = self.format.unwrap_or(repo.cache().format());

        let cache = if let Some(path) = self.path.as_ref() {
            format.from_path(path)
        } else {
            format.from_repo(&repo)
        };

        cache
            .regen()
            .jobs(self.jobs.unwrap_or_default())
            .force(self.force)
            .progress(progress)
            .suppress(!self.output)
            .run(&repo)?;

        Ok(ExitCode::SUCCESS)
    }
}
