use std::process::ExitCode;

use anyhow::anyhow;
use clap::Args;
use pkgcraft::repo::Repo;

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

        let mut failed = false;
        for error in repo.metadata_regen(jobs, self.force)? {
            failed = true;
            eprintln!("{error}");
        }

        if failed {
            Ok(ExitCode::FAILURE)
        } else {
            Ok(ExitCode::SUCCESS)
        }
    }
}
