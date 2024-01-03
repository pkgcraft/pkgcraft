use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use pkgcraft::config::{Config, Repos};
use pkgcraft::utils::bounded_jobs;
use pkgcruft::pipeline::Pipeline;

use crate::args::{target_restriction, StdinOrArgs};
use crate::options::{arches, checks, profiles};

#[derive(Debug, Args)]
pub struct Command {
    // positionals
    /// Target packages or paths
    #[arg(value_name = "TARGET", default_value = ".")]
    targets: Vec<String>,

    /// Parallel jobs to run
    #[arg(short, long)]
    jobs: Option<usize>,

    /// Target repository
    #[arg(short, long)]
    repo: Option<String>,

    /// Specific checks to run
    #[clap(flatten)]
    check_opts: checks::Options,

    #[clap(flatten)]
    arch_opts: arches::Options,

    #[clap(flatten)]
    profile_opts: profiles::Options,
}

impl Command {
    pub(super) fn run(self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // determine target repo set
        let repos = if let Some(target) = self.repo.as_ref() {
            config.add_target_repo(target)?.into()
        } else {
            config.repos.set(Repos::Ebuild)
        };

        // determine target restrictions
        let targets: Result<Vec<_>, _> = self
            .targets
            .stdin_or_args()
            .split_whitespace()
            .map(|s| target_restriction(config, &repos, &s, true))
            .collect();
        let targets = targets?;

        let mut stdout = io::stdout().lock();
        let jobs = bounded_jobs(self.jobs.unwrap_or_default());

        for (repo_set, restrict) in targets {
            for repo in repo_set.repos() {
                let pipeline = Pipeline::new(jobs, &self.check_opts.checks, repo, &restrict);
                for result in &pipeline {
                    writeln!(stdout, "{result}")?;
                }
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
