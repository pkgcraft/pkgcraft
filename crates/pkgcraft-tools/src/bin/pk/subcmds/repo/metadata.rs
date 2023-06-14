use std::io::stdout;
use std::path::Path;
use std::process::ExitCode;

use clap::Args;
use is_terminal::IsTerminal;
use itertools::Itertools;
use pkgcraft::config::Config;
use pkgcraft::repo::RepoFormat;

use crate::args::bounded_jobs;

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
    pub(super) fn run(&self, config: &Config) -> anyhow::Result<ExitCode> {
        // force bounds on jobs
        let jobs = bounded_jobs(self.jobs)?;

        // determine target repos
        let mut invalid = vec![];
        let mut repos = vec![];
        for repo in &self.repos {
            let repo = if let Some(r) = config.repos.get(repo) {
                Ok(r.clone())
            } else if Path::new(repo).exists() {
                RepoFormat::Ebuild.load_from_path(repo, 0, repo, true)
            } else {
                anyhow::bail!("unknown repo: {repo}")
            }?;

            if let Some(r) = repo.as_ebuild() {
                repos.push(r.clone());
            } else {
                invalid.push(repo);
            }
        }

        if !invalid.is_empty() {
            let repos = invalid.iter().map(|s| s.to_string()).join(", ");
            anyhow::bail!("non-ebuild repos: {repos}");
        }

        // run metadata regeneration
        let mut status = ExitCode::SUCCESS;
        for repo in &repos {
            let errors = repo.pkg_metadata_regen(jobs, self.force, stdout().is_terminal())?;
            if errors > 0 {
                status = ExitCode::FAILURE;
            }
        }

        Ok(status)
    }
}
