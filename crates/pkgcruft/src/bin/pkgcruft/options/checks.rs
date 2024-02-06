use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::Args;
use pkgcruft::check::{Check, CheckKind, CHECKS};
use pkgcruft::report::ReportKind;
use pkgcruft::source::SourceKind;
use strum::VariantNames;

#[derive(Debug, Args)]
#[clap(next_help_heading = Some("Check selection"))]
pub(crate) struct Checks {
    /// Specific checks to run
    #[arg(short, long)]
    checks: Vec<CheckKind>,

    /// Limit to specific report variants
    #[arg(short, long)]
    reports: Vec<ReportKind>,

    /// Limit to specific source variants
    #[arg(
        short = 'S',
        long,
        hide_possible_values = true,
        value_parser = PossibleValuesParser::new(SourceKind::VARIANTS)
            .map(|s| s.parse::<SourceKind>().unwrap()),
    )]
    sources: Vec<SourceKind>,
}

impl Checks {
    pub(crate) fn collapse(self) -> (Vec<Check>, Vec<ReportKind>) {
        let mut checks: Vec<_> = self.checks.iter().map(Check::from).collect();

        // add checks related to report options
        for report in &self.reports {
            for check in &*CHECKS {
                if check.reports().contains(report) {
                    checks.push(*check);
                }
            }
        }

        // add checks related to source options
        for source in &self.sources {
            for check in &*CHECKS {
                if check.source() == *source {
                    checks.push(*check);
                }
            }
        }

        (checks, self.reports)
    }
}
