use std::io::stdout;
use std::process::ExitCode;

use anyhow::anyhow;
use clap::Args;
use indicatif::ProgressBar;
use is_terminal::IsTerminal;
use pkgcraft::repo::{PkgRepository, Repo};

#[derive(Debug, Args)]
pub struct Command {
    /// Parallel jobs to run
    #[arg(short, long)]
    jobs: Option<usize>,

    /// Force regeneration to occur
    #[arg(short, long)]
    force: bool,
}

impl Command {
    pub(super) fn run(&self, repo: Repo) -> anyhow::Result<ExitCode> {
        // collapse repo into ebuild repo
        let repo = repo
            .as_ebuild()
            .ok_or_else(|| anyhow!("non-ebuild repo: {repo}"))?;

        let jobs = self.jobs.unwrap_or_else(num_cpus::get);

        // use progress bar to show completion progress when outputting to a terminal
        let cb: Option<Box<dyn Fn()>> = if stdout().is_terminal() {
            let repo_size = repo.len().try_into().expect("repo size too large");
            let pb = ProgressBar::new(repo_size);
            Some(Box::new(move || pb.inc(1)))
        } else {
            None
        };

        // run metadata regeneration
        let errors = repo.pkg_metadata_regen(jobs, self.force, cb)?;

        if errors > 0 {
            Ok(ExitCode::FAILURE)
        } else {
            Ok(ExitCode::SUCCESS)
        }
    }
}
