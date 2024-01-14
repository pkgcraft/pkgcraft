use std::process::ExitCode;

use clap::Args;
use pkgcraft::cli::TargetRestrictions;
use pkgcraft::config::Config;
use pkgcruft::scanner::Scanner;

use crate::args::StdinOrArgs;
use crate::options;

#[derive(Debug, Args)]
#[clap(next_help_heading = "Scan options")]
pub struct Command {
    /// Parallel jobs to run
    #[arg(short, long)]
    jobs: Option<usize>,

    /// Target repo
    #[arg(long)]
    repo: Option<String>,

    #[clap(flatten)]
    reporter: options::reporter::ReporterOptions,

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
    pub(super) fn run(self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // determine checks and reports
        let (checks, reports) = self.checks.collapse();

        // determine reporter
        let mut reporter = self.reporter.collapse()?;

        // determine target restrictions
        let targets = TargetRestrictions::new(config)
            .repo(self.repo)?
            .targets(self.targets.stdin_or_args().split_whitespace())?;

        // create report scanner
        let scanner = Scanner::new()
            .jobs(self.jobs.unwrap_or_default())
            .checks(&checks)
            .reports(&reports);

        // run scanner for all targets
        for (repo_set, restricts) in targets {
            for repo in repo_set.repos() {
                for report in scanner.run(repo, &restricts)? {
                    reporter.report(&report)?;
                }
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
