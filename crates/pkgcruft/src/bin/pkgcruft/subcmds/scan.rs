use std::process::ExitCode;

use clap::Args;
use pkgcraft::cli::target_restriction;
use pkgcraft::config::Config;
use pkgcraft::repo::RepoFormat;
use pkgcraft::utils::bounded_jobs;
use pkgcruft::pipeline::Pipeline;
use pkgcruft::reporter::Reporter;

use crate::args::StdinOrArgs;
use crate::options;

#[derive(Debug, Args)]
#[clap(next_help_heading = "Scan options")]
pub struct Command {
    /// Parallel jobs to run
    #[arg(short, long)]
    jobs: Option<usize>,

    /// Reporter to use
    #[arg(short, default_value = "fancy")]
    reporter: Reporter,

    #[clap(flatten)]
    options: options::Options,

    // positionals
    /// Target packages or paths
    #[arg(default_value = ".", help_heading = "Arguments")]
    targets: Vec<String>,
}

impl Command {
    pub(super) fn run(mut self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // determine checks and reports
        let (checks, reports) = self.options.checks.collapse();

        // determine target restrictions
        let targets: Result<Vec<_>, _> = self
            .targets
            .stdin_or_args()
            .split_whitespace()
            .map(|s| target_restriction(config, Some(RepoFormat::Ebuild), &s))
            .collect();
        let targets = targets?;

        let jobs = bounded_jobs(self.jobs.unwrap_or_default());

        for (repo_set, restrict) in targets {
            for repo in repo_set.repos() {
                let pipeline = Pipeline::new(jobs, &checks, &reports, repo, &restrict)?;
                for result in &pipeline {
                    self.reporter.report(&result)?;
                }
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
