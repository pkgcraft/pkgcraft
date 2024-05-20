use std::collections::HashSet;

use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::Args;
use indexmap::IndexSet;
use pkgcruft::check::{CheckKind, REPORT_CHECKS, SOURCE_CHECKS};
use pkgcruft::report::{ReportKind, ReportLevel};
use pkgcruft::scope::Scope;
use pkgcruft::source::SourceKind;
use strum::{IntoEnumIterator, VariantNames};

#[derive(Debug, Args)]
#[clap(next_help_heading = Some("Report selection"))]
pub(crate) struct Checks {
    /// Limit to specific checks
    #[arg(
        short,
        long,
        value_name = "CHECK[,...]",
        value_delimiter = ',',
        hide_possible_values = true,
        value_parser = PossibleValuesParser::new(CheckKind::VARIANTS)
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
        value_parser = PossibleValuesParser::new(ReportKind::VARIANTS)
            .map(|s| s.parse::<ReportKind>().unwrap()),
    )]
    reports: Vec<ReportKind>,

    /// Limit to specific scope variants
    #[arg(
        short,
        long,
        value_name = "SCOPE[,...]",
        value_delimiter = ',',
        hide_possible_values = true,
        value_parser = PossibleValuesParser::new(Scope::VARIANTS)
            .map(|s| s.parse::<Scope>().unwrap()),
    )]
    scopes: Vec<Scope>,

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
    pub(crate) fn collapse(self) -> (IndexSet<CheckKind>, IndexSet<ReportKind>) {
        // determine enabled report set
        let mut default_reports = true;
        let mut reports: IndexSet<_> = if !self.reports.is_empty() {
            default_reports = false;
            self.reports.into_iter().collect()
        } else {
            Default::default()
        };

        // enable reports related to levels
        if !self.levels.is_empty() {
            let levels: HashSet<_> = self.levels.into_iter().collect();
            reports.extend(ReportKind::iter().filter(|r| levels.contains(&r.level())));
            default_reports = false;
        }

        // enable reports related to scopes
        if !self.scopes.is_empty() {
            let scopes: HashSet<_> = self.scopes.into_iter().collect();
            reports.extend(ReportKind::iter().filter(|r| scopes.contains(&r.scope())));
            default_reports = false;
        }

        // enable reports related to sources
        if !self.sources.is_empty() {
            reports.extend(self.sources.iter().flat_map(|s| {
                SOURCE_CHECKS
                    .get(s)
                    .unwrap_or_else(|| unreachable!("no checks for source variant: {s}"))
                    .into_iter()
                    .flat_map(|x| &x.check().reports)
            }));
            default_reports = false;
        }

        // enable reports related to checks
        if !self.checks.is_empty() {
            reports.extend(self.checks.iter().flat_map(|x| &x.check().reports));
            default_reports = false;
        }

        // default to enabling all report variants
        if default_reports {
            reports.extend(ReportKind::iter());
        }

        // determine enabled check set
        let checks = if !self.checks.is_empty() {
            self.checks.into_iter().collect()
        } else {
            reports
                .iter()
                .flat_map(|r| {
                    REPORT_CHECKS
                        .get(r)
                        .unwrap_or_else(|| unreachable!("no checks for report variant: {r}"))
                })
                .copied()
                .collect()
        };

        (checks, reports)
    }
}
