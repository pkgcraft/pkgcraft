use std::io::stdout;
use std::process::ExitCode;

use anyhow::anyhow;
use clap::Args;
use indicatif::ProgressBar;
use is_terminal::IsTerminal;
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
    /// Target repository
    #[arg(required = true)]
    repo: String,
}

impl Command {
    pub(super) fn run(&self, config: &Config) -> anyhow::Result<ExitCode> {
        // determine target repo
        let repo = match config.repos.get(&self.repo) {
            Some(r) => r.clone(),
            None => RepoFormat::Ebuild.load_from_path(&self.repo, 0, &self.repo, true)?,
        };

        let repo = repo
            .as_ebuild()
            .ok_or_else(|| anyhow!("non-ebuild repo: {repo}"))?;

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

        // run metadata regeneration
        let errors = repo.pkg_metadata_regen(jobs, self.force, progress_cb)?;

        if errors > 0 {
            Ok(ExitCode::FAILURE)
        } else {
            Ok(ExitCode::SUCCESS)
        }
    }
}
