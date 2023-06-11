use std::io::stdout;
use std::path::Path;
use std::process::ExitCode;

use clap::Args;
use indicatif::ProgressBar;
use is_terminal::IsTerminal;
use itertools::Itertools;
use pkgcraft::config::Config;
use pkgcraft::repo::RepoFormat;
use scallop::pool::ProgressCallback;

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

        // use progress bar to show completion progress when outputting to a terminal
        let progress_cb: Option<ProgressCallback> = if stdout().is_terminal() {
            let pb = ProgressBar::new(0);
            let pb_len = pb.clone();
            let cb_inc = move |val| pb.inc(val);
            let cb_set = move |val| pb_len.set_length(val);
            Some(ProgressCallback::new(cb_inc, cb_set))
        } else {
            None
        };

        // determine target repos
        let mut invalid = vec![];
        let mut repos = vec![];
        for repo in &self.repos {
            let repo = if Path::new(repo).exists() {
                RepoFormat::Ebuild.load_from_path(repo, 0, repo, true)
            } else if let Some(r) = config.repos.get(repo) {
                Ok(r.clone())
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
        let mut failed = false;
        for repo in &repos {
            let errors = repo.pkg_metadata_regen(jobs, self.force, progress_cb.as_ref())?;
            if errors > 0 {
                failed = true;
            }
        }

        if failed {
            Ok(ExitCode::FAILURE)
        } else {
            Ok(ExitCode::SUCCESS)
        }
    }
}
