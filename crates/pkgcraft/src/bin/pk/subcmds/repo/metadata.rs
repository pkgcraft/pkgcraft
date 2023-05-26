use std::process::ExitCode;

use anyhow::anyhow;
use clap::Args;
use pkgcraft::pkg::SourceablePackage;
use pkgcraft::repo::Repo;
use scallop::pool::Pool;

#[derive(Debug, Args)]
pub struct Command {
    /// Parallel jobs to run
    #[arg(short, long)]
    jobs: Option<usize>,

    /// Force regeneration to occur
    #[arg(short, long)]
    force: bool,

    /// Regenerate metadata without serializing to disk
    #[arg(short, long)]
    pretend: bool,
}

impl Command {
    pub(super) fn run(&self, repo: Repo) -> anyhow::Result<ExitCode> {
        // collapse repo into ebuild repo
        let repo = repo
            .as_ebuild()
            .ok_or_else(|| anyhow!("non-ebuild repo: {repo}"))?;

        let jobs = self.jobs.unwrap_or_else(num_cpus::get);
        let mut pool = Pool::new(jobs)?;

        // generate metadata for the selected pkgs
        for pkg in repo.iter_raw() {
            pool.spawn(move || pkg.metadata(self.force, self.pretend))?;
        }

        pool.join()?;

        Ok(ExitCode::SUCCESS)
    }
}
