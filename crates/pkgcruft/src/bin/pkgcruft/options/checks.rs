use std::collections::HashSet;
use std::str::FromStr;

use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::Args;
use indexmap::IndexSet;
use pkgcruft::check::Check;
use pkgcruft::report::{ReportKind, ReportLevel};
use pkgcruft::scope::Scope;
use pkgcruft::source::SourceKind;
use strum::{IntoEnumIterator, VariantNames};

#[derive(Debug, Clone, Copy)]
enum TriStateArg<T: FromStr> {
    Add(T),
    Remove(T),
    Set(T),
}

impl<T: FromStr<Err = pkgcruft::Error>> FromStr for TriStateArg<T> {
    type Err = pkgcruft::Error;

    fn from_str(s: &str) -> pkgcruft::Result<Self> {
        if let Some(val) = s.strip_prefix('-') {
            val.parse().map(Self::Remove)
        } else if let Some(val) = s.strip_prefix('+') {
            val.parse().map(Self::Add)
        } else {
            s.parse().map(Self::Set)
        }
    }
}

#[derive(Debug, Args)]
#[clap(next_help_heading = Some("Report selection"))]
pub(crate) struct Checks {
    /// Restrict by check
    #[arg(short, long, value_name = "CHECK[,...]", value_delimiter = ',')]
    checks: Vec<TriStateArg<Check>>,

    /// Restrict by level
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

    /// Restrict by report
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

    /// Restrict by scope
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

    /// Restrict by source
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
    pub(crate) fn collapse(self, scan: bool) -> (IndexSet<Check>, IndexSet<ReportKind>) {
        // determine enabled check set
        let mut checks: IndexSet<_> = Check::iter_default().collect();
        if !self.checks.is_empty() {
            let mut overrides = IndexSet::new();

            for x in &self.checks {
                match x {
                    TriStateArg::Add(val) => checks.insert(*val),
                    TriStateArg::Remove(val) => checks.swap_remove(val),
                    TriStateArg::Set(val) => overrides.insert(*val),
                };
            }

            if !overrides.is_empty() {
                checks = overrides;
            }
        }

        // determine enabled report set
        let mut default_reports = true;
        let mut reports: IndexSet<_> = if !self.reports.is_empty() {
            default_reports = false;
            self.reports.into_iter().collect()
        } else if !self.checks.is_empty() {
            // enable reports related to enabled checks
            default_reports = false;
            checks.iter().flat_map(|x| x.reports).copied().collect()
        } else {
            Default::default()
        };

        // enable reports related to levels
        if !self.levels.is_empty() {
            let levels: HashSet<_> = self.levels.into_iter().collect();
            reports.extend(ReportKind::iter().filter(|r| levels.contains(&r.level())));
            default_reports = false;
        }

        // enable reports related to check scope
        if !self.scopes.is_empty() {
            let scopes: HashSet<_> = self.scopes.into_iter().collect();
            reports.extend(
                Check::iter()
                    .filter(|c| scopes.contains(&c.scope))
                    .flat_map(|c| c.reports),
            );
            default_reports = false;
        }

        // enable reports related to sources
        if !self.sources.is_empty() {
            reports.extend(
                self.sources
                    .iter()
                    .flat_map(|s| Check::iter_source(s).flat_map(|x| x.reports)),
            );
            default_reports = false;
        }

        // default to all reports skipping those from optional checks when scanning
        if default_reports {
            if scan {
                reports.extend(Check::iter_default().flat_map(|x| x.reports));
            } else {
                reports.extend(ReportKind::iter());
            }
        }

        // enable checks for target reports if none are explicitly specified
        if self.checks.is_empty() {
            checks = reports.iter().flat_map(Check::iter_report).collect();
        }

        (checks, reports)
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use pkgcraft::test::assert_ordered_eq;

    use super::*;

    #[derive(Parser)]
    struct Command {
        #[clap(flatten)]
        checks: Checks,
    }

    #[test]
    fn parse() {
        // verify checks and reports options don't affect each other when both are specified
        let cmd = Command::try_parse_from(["cmd", "-c", "Dependency", "-r", "DependencyInvalid"])
            .unwrap();
        let (checks, reports) = cmd.checks.collapse(false);
        assert_ordered_eq!(checks.iter().map(|x| x.as_ref()), ["Dependency"]);
        assert_ordered_eq!(reports.iter().map(|x| x.as_ref()), ["DependencyInvalid"]);

        // reports are populated by checks when unspecified
        let cmd = Command::try_parse_from(["cmd", "-c", "Dependency"]).unwrap();
        let (checks, reports) = cmd.checks.collapse(false);
        assert_ordered_eq!(checks.iter().map(|x| x.as_ref()), ["Dependency"]);
        assert!(!reports.is_empty());

        // only enable checks related to specified reports
        let cmd = Command::try_parse_from(["cmd", "-r", "DependencyDeprecated"]).unwrap();
        let (checks, reports) = cmd.checks.collapse(false);
        assert_ordered_eq!(checks.iter().map(|x| x.as_ref()), ["Dependency"]);
        assert!(!reports.is_empty());

        // verify UnstableOnly is an optional check
        assert!(Check::iter().any(|x| x.as_ref() == "UnstableOnly"));
        assert!(!Check::iter_default().any(|x| x.as_ref() == "UnstableOnly"));

        // default checks
        let cmd = Command::try_parse_from(["cmd"]).unwrap();
        let (checks, _) = cmd.checks.collapse(true);
        assert!(checks.iter().map(|x| x.as_ref()).any(|x| x == "Dependency"));
        // optional checks aren't run by default when scanning
        assert!(!checks.iter().any(|x| x.as_ref() == "UnstableOnly"));

        // disable checks
        let cmd = Command::try_parse_from(["cmd", "-c=-Dependency"]).unwrap();
        let (checks, _) = cmd.checks.collapse(true);
        assert!(!checks.iter().map(|x| x.as_ref()).any(|x| x == "Dependency"));
        assert!(checks.len() > 1);

        // enable optional checks in addition to default checks
        let cmd = Command::try_parse_from(["cmd", "-c", "+UnstableOnly"]).unwrap();
        let (checks, _) = cmd.checks.collapse(true);
        assert!(checks.iter().any(|x| x.as_ref() == "UnstableOnly"));
        assert!(checks.len() > 1);
    }
}
