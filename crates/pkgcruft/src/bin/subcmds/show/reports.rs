use std::io::Write;
use std::process::ExitCode;

use clap::Args;
use pkgcraft::cli::Targets;
use pkgcraft::config::Config;
use pkgcraft::restrict::Scope;
use pkgcruft::report::{ReportKind, Reports};
use strum::IntoEnumIterator;

use crate::options;

#[derive(Debug, Args)]
#[clap(next_help_heading = "Report options")]
pub(super) struct Subcommand {
    /// Target repo
    #[arg(long, num_args = 0..=1, default_missing_value = ".")]
    repo: Option<String>,

    #[clap(flatten)]
    reports: options::reports::ReportOptions,
}

impl Subcommand {
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        let reports = match (!self.reports.is_empty(), self.repo.as_deref()) {
            (false, None) => ReportKind::iter().collect(),
            (true, None) => self.reports.replay().unwrap_or_default(),
            (selected, Some(repo)) => {
                let mut config = Config::new("pkgcraft", "");
                let repo = Targets::new(&mut config)?
                    .repo_targets([repo])?
                    .ebuild_repo()?;
                let defaults = ReportKind::defaults(&repo);
                let supported = ReportKind::supported(&repo, Scope::Repo);
                if selected {
                    let (enabled, _) =
                        Reports::collapse(&self.reports, &defaults, &supported)?;
                    enabled
                } else {
                    defaults
                }
            }
        };

        let mut stdout = anstream::stdout().lock();
        for report in reports {
            writeln!(stdout, "{}", report.colorize())?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
