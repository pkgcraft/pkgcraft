use std::collections::HashSet;

use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::Args;
use indexmap::IndexSet;
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
    pub(crate) fn collapse(self) -> (IndexSet<Check>, IndexSet<ReportKind>) {
        // determine enabled report set
        let mut default_reports = true;
        let mut reports: IndexSet<_> = if !self.reports.is_empty() {
            self.reports.into_iter().collect()
        } else {
            Default::default()
        };

        // enable reports related to levels
        if !self.levels.is_empty() {
            let levels: HashSet<_> = self.levels.into_iter().collect();
            reports.extend(REPORTS.iter().filter(|r| levels.contains(&r.level())));
            default_reports = false;
        }

        // enable reports related to sources
        if !self.sources.is_empty() {
            reports.extend(self.sources.iter().flat_map(|s| {
                SOURCE_CHECKS
                    .get(s)
                    .unwrap_or_else(|| panic!("no checks for source variant: {s}"))
                    .into_iter()
                    .flat_map(|x| x.reports())
            }));
            default_reports = false;
        }

        // enable reports related to checks
        if !self.checks.is_empty() {
            reports.extend(self.checks.iter().flat_map(|x| {
                CHECKS
                    .get(x)
                    .unwrap_or_else(|| panic!("no check: {x}"))
                    .reports()
            }));
            default_reports = false;
        }

        // default to enabling all report variants
        if default_reports {
            reports.clone_from(&REPORTS);
        }

        // determine enabled check set
        let checks = if !self.checks.is_empty() {
            self.checks.iter().map(Into::into).collect()
        } else {
            reports
                .iter()
                .flat_map(|r| {
                    REPORT_CHECKS
                        .get(r)
                        .unwrap_or_else(|| panic!("no checks for report variant: {r}"))
                })
                .copied()
                .collect()
        };

        (checks, reports)
    }
}
