use std::collections::HashSet;

use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::Args;
use pkgcruft::check::{Check, CheckKind, CHECKS, SOURCE_CHECKS};
use pkgcruft::report::{ReportKind, ReportLevel, REPORTS, REPORT_CHECKS};
use pkgcruft::source::SourceKind;
use strum::VariantNames;

#[derive(Debug, Args)]
#[clap(next_help_heading = Some("Check selection"))]
pub(crate) struct Checks {
    /// Specific checks to run
    #[arg(
        short,
        long,
        value_name = "CHECK[,...]",
        value_delimiter = ',',
        hide_possible_values = true,
        value_parser = PossibleValuesParser::new(CHECKS.iter().map(|r| r.as_ref()))
            .map(|s| s.parse::<CheckKind>().unwrap()),
    )]
    checks: Vec<CheckKind>,

    /// Limit to specific report levels
    #[arg(
        short,
        long,
        value_name = "LEVEL[,...]",
        value_delimiter = ',',
        hide_possible_values = true,
        value_parser = PossibleValuesParser::new(ReportLevel::VARIANTS)
            .map(|s| s.parse::<ReportLevel>().unwrap()),
    )]
    levels: Vec<ReportLevel>,

    /// Limit to specific report variants
    #[arg(
        short,
        long,
        value_name = "REPORT[,...]",
        value_delimiter = ',',
        hide_possible_values = true,
        value_parser = PossibleValuesParser::new(REPORTS.iter().map(|r| r.as_ref()))
            .map(|s| s.parse::<ReportKind>().unwrap()),
    )]
    reports: Vec<ReportKind>,

    /// Limit to specific source variants
    #[arg(
        short = 'S',
        long,
        value_name = "SOURCE[,...]",
        value_delimiter = ',',
        hide_possible_values = true,
        value_parser = PossibleValuesParser::new(SourceKind::VARIANTS)
            .map(|s| s.parse::<SourceKind>().unwrap()),
    )]
    sources: Vec<SourceKind>,
}

impl Checks {
    pub(crate) fn collapse(self) -> (Vec<Check>, Vec<ReportKind>) {
        let mut reports = self.reports;

        // enable reports related to level options
        if !self.levels.is_empty() {
            let levels: HashSet<_> = self.levels.into_iter().collect();
            reports.extend(REPORTS.iter().filter(|r| levels.contains(&r.level())));
        }

        // enable reports related to source options
        if !self.sources.is_empty() {
            reports.extend(self.sources.iter().flat_map(|s| {
                SOURCE_CHECKS
                    .get(s)
                    .unwrap_or_else(|| panic!("no checks for source variant: {s}"))
                    .into_iter()
                    .flat_map(|x| x.reports())
            }));
        }

        let mut checks: Vec<_> = self.checks.iter().map(Check::from).collect();

        // add checks related to enabled report variants
        checks.extend(reports.iter().flat_map(|r| {
            REPORT_CHECKS
                .get(r)
                .unwrap_or_else(|| panic!("no checks for report variant: {r}"))
        }));

        (checks, reports)
    }
}
