use clap::Args;

use pkgcruft::check::CheckKind;
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
