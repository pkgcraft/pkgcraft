use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::Args;
use pkgcruft::check::{CheckKind, CHECKS};
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
    pub(crate) fn collapse(mut self) -> (Vec<CheckKind>, Vec<ReportKind>) {
        // add checks related to report options
        for report in &self.reports {
            for check in &*CHECKS {
                if check.reports().contains(report) {
                    self.checks.push(check.kind());
                }
            }
        }

        // add checks related to source options
        for source in &self.sources {
            for check in &*CHECKS {
                if check.source() == *source {
                    self.checks.push(check.kind());
                }
            }
        }

        (self.checks, self.reports)
    }
}
