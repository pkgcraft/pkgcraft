use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use colored::Colorize;
use pkgcraft::cli::target_ebuild_repo;
use pkgcraft::config::Config;
use pkgcruft::report::ReportKind;
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
        let reports = match (self.reports.is_empty(), self.repo.as_deref()) {
            (true, None) => ReportKind::iter().collect(),
            (false, None) => self.reports.replay().unwrap_or_default(),
            (selected, Some(repo)) => {
                let mut config = Config::new("pkgcraft", "");
                let repo = target_ebuild_repo(&mut config, repo)?;
                let defaults = ReportKind::defaults(&repo);
                if selected {
                    defaults
                } else {
                    let (enabled, _) = self.reports.collapse(defaults)?;
                    enabled
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
