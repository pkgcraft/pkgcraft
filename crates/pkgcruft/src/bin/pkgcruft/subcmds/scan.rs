use std::process::ExitCode;

use clap::Args;
use pkgcraft::cli::target_restriction;
use pkgcraft::config::Config;
use pkgcraft::repo::RepoFormat;
use pkgcruft::reporter::Reporter;
use pkgcruft::scanner::Scanner;

use crate::args::StdinOrArgs;
use crate::options;

#[derive(Debug, Args)]
#[clap(next_help_heading = "Scan options")]
pub struct Command {
    /// Parallel jobs to run
    #[arg(short, long)]
    jobs: Option<usize>,

    /// Reporter to use
    #[arg(short, long, default_value = "fancy")]
    reporter: Reporter,

    #[clap(flatten)]
    checks: options::checks::Checks,

    #[clap(flatten)]
    arches: options::arches::Arches,

    #[clap(flatten)]
    profiles: options::profiles::Profiles,

    // positionals
    /// Target packages or paths
    #[arg(default_value = ".", help_heading = "Arguments")]
    targets: Vec<String>,
}

impl Command {
    pub(super) fn run(mut self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // determine checks to run and report filter
        let (checks, filter) = self.checks.collapse();

        // determine target restrictions
        let targets: Result<Vec<_>, _> = self
            .targets
            .stdin_or_args()
            .split_whitespace()
            .map(|s| target_restriction(config, Some(RepoFormat::Ebuild), &s))
            .collect();
        let targets = targets?;

        // create report scanner
        let scanner = Scanner::new()
            .jobs(self.jobs.unwrap_or_default())
            .checks(&checks)
            .filter(&filter);

        // run scanner for all targets
        for (repo_set, restrict) in targets {
            for repo in repo_set.repos() {
                for report in scanner.run(repo, &restrict)? {
                    self.reporter.report(&report)?;
                }
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
