use clap::Args;

use pkgcruft::check::{CheckKind, CHECKS};
use pkgcruft::report::ReportKind;

#[derive(Debug, Args)]
#[clap(next_help_heading = Some("Check selection"))]
pub(crate) struct Options {
    /// Specific checks to run
    #[arg(short, long)]
    checks: Vec<CheckKind>,

    /// Limit to specific keywords
    #[arg(short, long)]
    keywords: Vec<ReportKind>,
}

impl Options {
    pub(crate) fn collapse(&self) -> (Vec<CheckKind>, Vec<ReportKind>) {
        let mut checks = self.checks.clone();
        let mut reports = self.keywords.clone();

        // add checks related to report options
        for keyword in &self.keywords {
            for check in &*CHECKS {
                if check.reports().contains(keyword) {
                    checks.push(check.kind());
                }
            }
        }

        // add reports related to check options
        for check in self.checks.iter().filter_map(|c| CHECKS.get(c)) {
            reports.extend(check.reports().iter().copied());
        }

        (checks, reports)
    }
}
