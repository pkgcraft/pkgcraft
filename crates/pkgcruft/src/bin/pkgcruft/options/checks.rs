use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::Args;
use pkgcruft::check::{Check, CheckKind, SOURCE_CHECKS};
use pkgcruft::report::{ReportKind, REPORT_CHECKS};
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
        checks.extend(self.reports.iter().flat_map(|r| {
            REPORT_CHECKS
                .get(r)
                .expect("no checks for report variant: {r}")
        }));

        // add checks related to source options
        checks.extend(self.sources.iter().flat_map(|s| {
            SOURCE_CHECKS
                .get(s)
                .expect("no checks for source variant: {s}")
        }));

        (checks, self.reports)
    }
}
