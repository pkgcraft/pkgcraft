use clap::Args;
use pkgcruft::check::{CheckKind, CHECKS};
use pkgcruft::report::ReportKind;

#[derive(Debug, Args)]
#[clap(next_help_heading = Some("Check selection"))]
pub(crate) struct Checks {
    /// Specific checks to run
    #[arg(short, long)]
    checks: Vec<CheckKind>,

    /// Limit to specific report variants
    #[arg(short, long)]
    reports: Vec<ReportKind>,
}

impl Checks {
    pub(crate) fn collapse(mut self) -> (Vec<CheckKind>, Vec<ReportKind>) {
        // add checks related to report options
        for report in &self.reports {
            for check in &*CHECKS {
                if check.reports().contains(report) {
                    self.checks.push(check.kind());
                }
            }
        }

        (self.checks, self.reports)
    }
}
