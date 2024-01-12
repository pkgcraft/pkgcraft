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
    filter: Vec<ReportKind>,
}

impl Checks {
    pub(crate) fn collapse(&self) -> (Vec<CheckKind>, Vec<ReportKind>) {
        let mut checks = self.checks.clone();
        let mut filter = self.filter.clone();

        // add checks related to report options
        for report in &self.filter {
            for check in &*CHECKS {
                if check.reports().contains(report) {
                    checks.push(check.kind());
                }
            }
        }

        // add reports related to check options
        for check in self.checks.iter().filter_map(|c| CHECKS.get(c)) {
            filter.extend(check.reports().iter().copied());
        }

        (checks, filter)
    }
}
