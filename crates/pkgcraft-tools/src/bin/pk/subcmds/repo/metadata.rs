use std::io::stdout;
use std::process::ExitCode;

use anyhow::anyhow;
use clap::Args;
use indicatif::ProgressBar;
use is_terminal::IsTerminal;
use pkgcraft::repo::Repo;

use crate::args::bounded_jobs;

#[derive(Debug, Args)]
pub struct Command {
    /// Parallel jobs to run
    #[arg(short, long)]
    jobs: Option<usize>,

    /// Force regeneration to occur
    #[arg(short, long)]
    force: bool,
}

type Callbacks = Option<(Box<dyn Fn()>, Box<dyn Fn(u64)>)>;

impl Command {
    pub(super) fn run(&self, repo: Repo) -> anyhow::Result<ExitCode> {
        // collapse repo into ebuild repo
        let repo = repo
            .as_ebuild()
            .ok_or_else(|| anyhow!("non-ebuild repo: {repo}"))?;

        // force bounds on jobs
        let jobs = bounded_jobs(self.jobs);

        // use progress bar to show completion progress when outputting to a terminal
        let cbs: Callbacks = if stdout().is_terminal() {
            let pb = ProgressBar::new(0);
            let pb_len = pb.clone();
            Some((Box::new(move || pb.inc(1)), Box::new(move |len| pb_len.set_length(len))))
        } else {
            None
        };

        // run metadata regeneration
        let errors = repo.pkg_metadata_regen(jobs, self.force, cbs)?;

        if errors > 0 {
            Ok(ExitCode::FAILURE)
        } else {
            Ok(ExitCode::SUCCESS)
        }
    }
}
