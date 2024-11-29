use std::str::FromStr;

use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::Args;
use colored::{Color, Colorize};
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::repo::ebuild::EbuildRepo;
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
        &self,
        target_repo: Option<&EbuildRepo>,
    ) -> pkgcruft::Result<(IndexSet<Check>, IndexSet<ReportKind>)> {
        let mut defaults = true;
        let mut checks: IndexSet<_> = Check::iter_default(target_repo).collect();
        let default_reports: IndexSet<_> = checks.iter().flat_map(|x| x.reports).collect();

        // determine enabled check set
        if !self.checks.is_empty() {
            // sort checks by variant
            let selected_checks: Vec<_> = self.checks.iter().copied().sorted().collect();

            // don't use default checks if neutral options exist
            if let Some(TriStateCheck::Set(_)) = selected_checks.first() {
                checks = Default::default();
            }

            for x in selected_checks {
                match x {
                    TriStateCheck::Set(val) => checks.insert(val),
                    TriStateCheck::Add(val) => checks.insert(val),
                    TriStateCheck::Remove(val) => checks.swap_remove(&val),
                };
            }
        }

        // determine enabled report set
        let mut reports: IndexSet<_> = if !self.reports.is_empty() {
            defaults = false;
            self.reports.iter().copied().collect()
        } else if !self.checks.is_empty() {
            // enable reports related to enabled checks
            defaults = false;
            checks.iter().flat_map(|x| x.reports).copied().collect()
        } else {
            Default::default()
        };

        // enable reports related to levels
        if !self.levels.is_empty() {
            let levels: IndexSet<_> = self.levels.iter().collect();
            reports.extend(
                default_reports
                    .iter()
                    .filter(|r| levels.contains(&r.level()))
                    .copied(),
            );
            defaults = false;
        }

        // enable reports related to check scope
        if !self.scopes.is_empty() {
            let scopes: IndexSet<_> = self.scopes.iter().collect();
            reports.extend(
                Check::iter()
                    .filter(|c| scopes.contains(&c.scope))
                    .flat_map(|c| c.reports)
                    .filter(|r| default_reports.contains(r)),
            );
            defaults = false;
        }

        // enable reports related to sources
        if !self.sources.is_empty() {
            reports.extend(
                self.sources
                    .iter()
                    .flat_map(|s| Check::iter_source(s).flat_map(|x| x.reports))
                    .filter(|r| default_reports.contains(r)),
            );
            defaults = false;
        }

        // default to all reports skipping those from optional checks when scanning
        if defaults {
            if target_repo.is_some() {
                reports.extend(Check::iter_default(target_repo).flat_map(|x| x.reports));
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
            checks.sort();
            reports.sort();
            Ok((checks, reports))
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use pkgcraft::test::{assert_err_re, assert_ordered_eq, test_data};

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
        let (checks, reports) = cmd.checks.collapse(None).unwrap();
        assert_ordered_eq!(checks, [CheckKind::Dependency.into()]);
        assert_ordered_eq!(reports, [ReportKind::DependencyInvalid]);

        // reports are populated by checks when unspecified
        let cmd = Command::try_parse_from(["cmd", "-c", "Dependency"]).unwrap();
        let (checks, reports) = cmd.checks.collapse(None).unwrap();
        assert_ordered_eq!(checks, [CheckKind::Dependency.into()]);
        assert!(!reports.is_empty());

        // only enable checks related to specified reports
        let cmd = Command::try_parse_from(["cmd", "-r", "DependencyDeprecated"]).unwrap();
        let (checks, reports) = cmd.checks.collapse(None).unwrap();
        assert_ordered_eq!(checks, [CheckKind::Dependency.into()]);
        assert!(!reports.is_empty());

        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();

        // verify UnstableOnly is an optional check
        assert!(Check::iter().any(|x| x == CheckKind::UnstableOnly.into()));
        assert!(!Check::iter_default(None).any(|x| x == CheckKind::UnstableOnly.into()));
        assert!(!Check::iter_default(Some(repo)).any(|x| x == CheckKind::UnstableOnly.into()));

        // default checks
        let cmd = Command::try_parse_from(["cmd"]).unwrap();
        let (checks, _) = cmd.checks.collapse(Some(repo)).unwrap();
        assert!(checks.contains(&CheckKind::Dependency));
        // optional checks aren't run by default when scanning
        assert!(!checks.contains(&CheckKind::UnstableOnly));

        // enable optional checks in addition to default checks
        let cmd = Command::try_parse_from(["cmd", "-c", "+UnstableOnly"]).unwrap();
        let (checks, _) = cmd.checks.collapse(Some(repo)).unwrap();
        assert!(checks.contains(&CheckKind::UnstableOnly));
        assert!(checks.len() > 1);

        // disable checks
        let cmd = Command::try_parse_from(["cmd", "-c=-Dependency"]).unwrap();
        let (checks, _) = cmd.checks.collapse(Some(repo)).unwrap();
        assert!(!checks.contains(&CheckKind::Dependency));
        assert!(checks.len() > 1);

        // disable option overrides enable option
        let cmd = Command::try_parse_from(["cmd", "-c=-Dependency,+Dependency"]).unwrap();
        let (checks, _) = cmd.checks.collapse(Some(repo)).unwrap();
        assert!(!checks.contains(&CheckKind::Dependency));
        assert!(checks.len() > 1);

        // error when args cancel out
        let cmd = Command::try_parse_from(["cmd", "-c=-Dependency,Dependency"]).unwrap();
        assert!(cmd.checks.collapse(Some(repo)).is_err());

        // invalid check names in args
        for arg in ["-c=unknown", "-c=-unknown", "-c=+unknown"] {
            let r = Command::try_parse_from(["cmd", arg]);
            assert_err_re!(r, "unknown check: unknown");
        }
    }
}
