use std::io::{stdout, IsTerminal};
use std::process::ExitCode;

use clap::Args;
use pkgcraft::config::Config;
use pkgcraft::repo::ebuild::cache::CacheFormat::Md5Dict;
use pkgcraft::repo::Repository;

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
        let cache = if let Some(path) = self.path.as_ref() {
            Md5Dict.custom(path)
        } else {
            Md5Dict.repo(repo.path())
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
