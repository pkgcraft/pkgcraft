use std::collections::HashSet;
use std::str::FromStr;

use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::Args;
use colored::{Color, Colorize};
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcruft::check::{Check, CheckKind};
use pkgcruft::report::{ReportKind, ReportLevel};
use pkgcruft::scope::Scope;
use pkgcruft::source::SourceKind;
use pkgcruft::Error;
use strum::{IntoEnumIterator, VariantNames};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum TriStateCheck {
    Set(Check),
    Add(Check),
    Remove(Check),
}

impl FromStr for TriStateCheck {
    type Err = Error;

    fn from_str(s: &str) -> pkgcruft::Result<Self> {
        let err = |err: Error| -> Error {
            let possible = CheckKind::iter()
                .map(|x| x.as_ref().color(Color::Green))
                .join(", ");
            let message = indoc::formatdoc! {"
                {err}
                    [possible values: {possible}]"};
            Error::InvalidValue(message)
        };

        if let Some(val) = s.strip_prefix('+') {
            val.parse().map(Self::Add).map_err(err)
        } else if let Some(val) = s.strip_prefix('-') {
            val.parse().map(Self::Remove).map_err(err)
        } else {
            s.parse().map(Self::Set).map_err(err)
        }
    }
}

#[derive(Debug, Args)]
#[clap(next_help_heading = Some("Report selection"))]
pub(crate) struct Checks {
    /// Restrict by check
    #[arg(short, long, value_name = "CHECK[,...]", value_delimiter = ',')]
    checks: Vec<TriStateCheck>,

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
    pub(crate) fn collapse(
        mut self,
        scan: bool,
    ) -> pkgcruft::Result<(IndexSet<Check>, IndexSet<ReportKind>)> {
        // determine enabled check set
        let mut checks: IndexSet<_> = Check::iter_default().collect();
        if !self.checks.is_empty() {
            // sort checks by variant
            self.checks.sort();

            // don't use default checks if neutral options exist
            if let Some(TriStateCheck::Set(_)) = self.checks.first() {
                checks = Default::default();
            }

            for x in &self.checks {
                match x {
                    TriStateCheck::Set(val) => checks.insert(*val),
                    TriStateCheck::Add(val) => checks.insert(*val),
                    TriStateCheck::Remove(val) => checks.swap_remove(val),
                };
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

        if checks.is_empty() {
            Err(Error::InvalidValue("no checks selected".to_string()))
        } else {
            Ok((checks, reports))
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use pkgcraft::test::{assert_err_re, assert_ordered_eq};

    use super::*;

    #[derive(Debug, Parser)]
    struct Command {
        #[clap(flatten)]
        checks: Checks,
    }

    #[test]
    fn parse() {
        // verify checks and reports options don't affect each other when both are specified
        let cmd = Command::try_parse_from(["cmd", "-c", "Dependency", "-r", "DependencyInvalid"])
            .unwrap();
        let (checks, reports) = cmd.checks.collapse(false).unwrap();
        assert_ordered_eq!(checks.iter().map(|x| x.as_ref()), ["Dependency"]);
        assert_ordered_eq!(reports.iter().map(|x| x.as_ref()), ["DependencyInvalid"]);

        // reports are populated by checks when unspecified
        let cmd = Command::try_parse_from(["cmd", "-c", "Dependency"]).unwrap();
        let (checks, reports) = cmd.checks.collapse(false).unwrap();
        assert_ordered_eq!(checks.iter().map(|x| x.as_ref()), ["Dependency"]);
        assert!(!reports.is_empty());

        // only enable checks related to specified reports
        let cmd = Command::try_parse_from(["cmd", "-r", "DependencyDeprecated"]).unwrap();
        let (checks, reports) = cmd.checks.collapse(false).unwrap();
        assert_ordered_eq!(checks.iter().map(|x| x.as_ref()), ["Dependency"]);
        assert!(!reports.is_empty());

        // verify UnstableOnly is an optional check
        assert!(Check::iter().any(|x| x.as_ref() == "UnstableOnly"));
        assert!(!Check::iter_default().any(|x| x.as_ref() == "UnstableOnly"));

        // default checks
        let cmd = Command::try_parse_from(["cmd"]).unwrap();
        let (checks, _) = cmd.checks.collapse(true).unwrap();
        assert!(checks.iter().any(|x| x.as_ref() == "Dependency"));
        // optional checks aren't run by default when scanning
        assert!(!checks.iter().any(|x| x.as_ref() == "UnstableOnly"));

        // enable optional checks in addition to default checks
        let cmd = Command::try_parse_from(["cmd", "-c", "+UnstableOnly"]).unwrap();
        let (checks, _) = cmd.checks.collapse(true).unwrap();
        assert!(checks.iter().any(|x| x.as_ref() == "UnstableOnly"));
        assert!(checks.len() > 1);

        // disable checks
        let cmd = Command::try_parse_from(["cmd", "-c=-Dependency"]).unwrap();
        let (checks, _) = cmd.checks.collapse(true).unwrap();
        assert!(!checks.iter().any(|x| x.as_ref() == "Dependency"));
        assert!(checks.len() > 1);

        // disable option overrides enable option
        let cmd = Command::try_parse_from(["cmd", "-c=-Dependency,+Dependency"]).unwrap();
        let (checks, _) = cmd.checks.collapse(true).unwrap();
        assert!(!checks.iter().any(|x| x.as_ref() == "Dependency"));
        assert!(checks.len() > 1);

        // error when args cancel out
        let cmd = Command::try_parse_from(["cmd", "-c=-Dependency,Dependency"]).unwrap();
        assert!(cmd.checks.collapse(true).is_err());

        // invalid check names in args
        for arg in ["-c=unknown", "-c=-unknown", "-c=+unknown"] {
            let r = Command::try_parse_from(["cmd", arg]);
            assert_err_re!(r, "unknown check: unknown");
        }
    }
}
