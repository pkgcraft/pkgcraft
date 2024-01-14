use std::process::ExitCode;

use clap::Args;
use indexmap::IndexMap;
use pkgcraft::cli::target_restriction;
use pkgcraft::config::Config;
use pkgcraft::repo::RepoFormat;
use pkgcruft::scanner::Scanner;

use crate::args::StdinOrArgs;
use crate::options;

#[derive(Debug, Args)]
#[clap(next_help_heading = "Scan options")]
pub struct Command {
    /// Parallel jobs to run
    #[arg(short, long)]
    jobs: Option<usize>,

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
        let targets: Result<Vec<_>, _> = self
            .targets
            .stdin_or_args()
            .split_whitespace()
            .map(|s| target_restriction(config, Some(RepoFormat::Ebuild), &s))
            .collect();
        let targets = targets?;

        // TODO: Implement custom types for ordered maps of ordered collections so FromIterator
        // works directly instead of instead of having to first collect to a vector.
        let mut collapsed_targets = IndexMap::<_, Vec<_>>::new();
        for (set, restrict) in targets {
            collapsed_targets.entry(set).or_default().push(restrict);
        }

        // create report scanner
        let scanner = Scanner::new()
            .jobs(self.jobs.unwrap_or_default())
            .checks(&checks)
            .reports(&reports);

        // run scanner for all targets
        for (repo_set, restricts) in &collapsed_targets {
            for repo in repo_set.repos() {
                for report in scanner.run(repo, restricts)? {
                    reporter.report(&report)?;
                }
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
