use std::hash::Hash;
use std::str::FromStr;

use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::Args;
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcruft::check::Check;
use pkgcruft::report::{ReportKind, ReportLevel};
use pkgcruft::scope::Scope;
use pkgcruft::source::SourceKind;
use pkgcruft::Error;
use strum::{IntoEnumIterator, VariantNames};

/// Tri-state value support for command-line arguments.
///
/// This supports arguments of the form: `set`, `+add`, and `-remove` that relate to their
/// matching variants.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
enum TriState<T> {
    Set(T),
    Add(T),
    Remove(T),
}

impl<T: Ord + Copy + Hash> TriState<T> {
    /// Modify the given, enabled set given an iterator of TriState values.
    fn enabled<'a, I>(enabled: &mut IndexSet<T>, selected: I)
    where
        I: IntoIterator<Item = &'a TriState<T>>,
        T: 'a,
    {
        // sort by variant
        let selected: Vec<_> = selected.into_iter().copied().sorted().collect();

        // don't use default if neutral options exist
        if let Some(TriState::Set(_)) = selected.first() {
            std::mem::take(enabled);
        }

        for x in selected {
            match x {
                TriState::Set(val) => enabled.insert(val),
                TriState::Add(val) => enabled.insert(val),
                TriState::Remove(val) => enabled.swap_remove(&val),
            };
        }

        enabled.sort();
    }
}

impl<T: FromStr> FromStr for TriState<T> {
    type Err = <T as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(val) = s.strip_prefix('+') {
            val.parse().map(Self::Add)
        } else if let Some(val) = s.strip_prefix('-') {
            val.parse().map(Self::Remove)
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
    checks: Vec<TriState<Check>>,

    /// Restrict by level
    #[arg(short, long, value_name = "LEVEL[,...]", value_delimiter = ',')]
    levels: Vec<TriState<ReportLevel>>,

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
            TriState::enabled(&mut checks, &self.checks);
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
            let mut levels: IndexSet<_> = ReportLevel::iter().collect();
            TriState::enabled(&mut levels, &self.levels);
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

    use pkgcruft::check::CheckKind;

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

        // default checks for gentoo repo
        let repo = data.ebuild_repo("gentoo").unwrap();
        let cmd = Command::try_parse_from(["cmd"]).unwrap();
        let (checks, _) = cmd.checks.collapse(Some(repo)).unwrap();
        // repo specific checks enabled when scanning the matching repo
        assert!(checks.contains(&CheckKind::Header));

        // verify UnstableOnly is an optional check
        let repo = data.ebuild_repo("qa-primary").unwrap();
        assert!(Check::iter().any(|x| x == CheckKind::UnstableOnly.into()));
        assert!(!Check::iter_default(None).any(|x| x == CheckKind::UnstableOnly.into()));
        assert!(!Check::iter_default(Some(repo)).any(|x| x == CheckKind::UnstableOnly.into()));

        // default checks
        let cmd = Command::try_parse_from(["cmd"]).unwrap();
        let (checks, _) = cmd.checks.collapse(Some(repo)).unwrap();
        assert!(checks.contains(&CheckKind::Dependency));
        // optional checks aren't run by default when scanning
        assert!(!checks.contains(&CheckKind::UnstableOnly));
        // repo specific checks aren't run by default when scanning non-matching repo
        assert!(!checks.contains(&CheckKind::Header));

        // non-default reports aren't enabled when their matching level is targeted
        let report = ReportKind::HeaderInvalid;
        let cmd = Command::try_parse_from(["cmd", "-l", report.level().as_ref()]).unwrap();
        let (_, reports) = cmd.checks.collapse(Some(repo)).unwrap();
        assert!(!reports.contains(&report));
        assert!(!reports.is_empty());

        // non-default checks aren't enabled when their matching scope is targeted
        let check: Check = CheckKind::Header.into();
        let cmd = Command::try_parse_from(["cmd", "-s", check.scope.as_ref()]).unwrap();
        let (checks, _) = cmd.checks.collapse(Some(repo)).unwrap();
        assert!(!checks.contains(&CheckKind::Header));
        assert!(!checks.is_empty());

        // non-default checks aren't enabled when their matching source is targeted
        let check: Check = CheckKind::Header.into();
        let cmd = Command::try_parse_from(["cmd", "-S", check.source.as_ref()]).unwrap();
        let (checks, _) = cmd.checks.collapse(Some(repo)).unwrap();
        assert!(!checks.contains(&CheckKind::Header));
        assert!(!checks.is_empty());

        // enable optional checks in addition to default checks
        let cmd = Command::try_parse_from(["cmd", "-c", "+UnstableOnly,+Header"]).unwrap();
        let (checks, _) = cmd.checks.collapse(Some(repo)).unwrap();
        assert!(checks.contains(&CheckKind::UnstableOnly));
        assert!(checks.contains(&CheckKind::Header));
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
