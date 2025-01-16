use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use colored::Colorize;
use pkgcraft::cli::target_ebuild_repo;
use pkgcraft::config::Config;
use pkgcraft::restrict::Scope;
use pkgcruft::report::{ReportKind, ReportTarget};
use strum::IntoEnumIterator;

use crate::options;

#[derive(Debug, Args)]
#[clap(next_help_heading = "Report options")]
pub(super) struct Subcommand {
    /// Target repo
    #[arg(long, num_args = 0..=1, default_missing_value = ".")]
    repo: Option<String>,

    #[clap(flatten)]
    reports: options::reports::Reports,
}

impl Subcommand {
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        let reports = match (!self.reports.is_empty(), self.repo.as_deref()) {
            (false, None) => ReportKind::iter().collect(),
            (true, None) => self.reports.replay().unwrap_or_default(),
            (selected, Some(repo)) => {
                let mut config = Config::new("pkgcraft", "");
                let repo = target_ebuild_repo(&mut config, repo)?;
                let defaults = ReportKind::defaults(&repo);
                let supported = ReportKind::supported(&repo, Scope::Repo);
                if selected {
                    let (enabled, _) =
                        ReportTarget::collapse(&self.reports, &defaults, &supported)?;
                    enabled
                } else {
                    defaults
                }
            }
        };

        let mut stdout = io::stdout().lock();
        for report in reports {
            writeln!(stdout, "{}", report.as_ref().color(report.level()))?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
