use clap::Args;

use pkgcruft::check::{CheckKind, CHECKS};
use pkgcruft::report::ReportKind;

#[derive(Debug, Args)]
#[clap(next_help_heading = Some("Check selection"))]
pub(crate) struct Options {
    /// Specific checks to run
    #[arg(short, long)]
    pub(crate) checks: Vec<CheckKind>,

    /// Limit to specific keywords
    #[arg(short, long)]
    pub(crate) keywords: Vec<ReportKind>,
}

impl Options {
    pub(crate) fn checks(&self) -> Vec<CheckKind> {
        let mut checks = self.checks.clone();

        // add checks related to report options
        for keyword in &self.keywords {
            for check in &*CHECKS {
                if check.reports().contains(keyword) {
                    checks.push(check.kind());
                }
            }
        }

        checks
    }
}
