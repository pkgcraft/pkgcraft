use std::hash::Hash;
use std::str::FromStr;

use clap::Args;
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcruft::check::Check;
use pkgcruft::report::{ReportKind, ReportLevel};
use pkgcruft::Error;

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

/// Tri-state value support for command-line arguments.
///
/// This supports arguments of the form: `set`, `+add`, and `-remove` that relate to their
/// matching variants.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
enum ReportAlias {
    Check(Check),
    Level(ReportLevel),
    Report(ReportKind),
}

impl FromStr for ReportAlias {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(val) = s.strip_prefix('@') {
            val.parse().map(Self::Check)
        } else if let Some(val) = s.strip_prefix('%') {
            val.parse()
                .map(Self::Level)
                .map_err(|_| Error::InvalidValue(format!("invalid level: {val}")))
        } else {
            s.parse()
                .map(Self::Report)
                .map_err(|_| Error::InvalidValue(format!("invalid report: {s}")))
        }
    }
}

impl ReportAlias {
    fn expand(
        self,
        defaults: &IndexSet<ReportKind>,
    ) -> Box<dyn Iterator<Item = ReportKind> + '_> {
        match self {
            Self::Check(check) => Box::new(check.reports.iter().copied()),
            Self::Level(level) => {
                Box::new(defaults.iter().filter(move |r| r.level() == level).copied())
            }
            Self::Report(kind) => Box::new([kind].into_iter()),
        }
    }
}

#[derive(Debug, Args)]
#[clap(next_help_heading = Some("Report selection"))]
pub(crate) struct Reports {
    /// Restrict by tri-state report aliases
    #[arg(short, long, value_name = "ALIAS[,...]", value_delimiter = ',')]
    reports: Vec<TriState<ReportAlias>>,
}

impl Reports {
    pub(crate) fn collapse(
        &self,
        defaults: IndexSet<ReportKind>,
    ) -> pkgcruft::Result<(IndexSet<Check>, IndexSet<ReportKind>)> {
        // sort by variant
        let selected: Vec<_> = self.reports.iter().copied().sorted().collect();

        // don't use defaults if neutral options exist
        let mut reports = if let Some(TriState::Set(_)) = selected.first() {
            Default::default()
        } else {
            defaults.clone()
        };

        for x in selected {
            match x {
                TriState::Set(val) => reports.extend(val.expand(&defaults)),
                TriState::Add(val) => reports.extend(val.expand(&defaults)),
                TriState::Remove(val) => {
                    for x in val.expand(&defaults) {
                        reports.swap_remove(&x);
                    }
                }
            };
        }

        if reports.is_empty() {
            Err(Error::InvalidValue("no reports selected".to_string()))
        } else {
            reports.sort();
            let mut checks: IndexSet<_> =
                reports.iter().flat_map(Check::iter_report).collect();
            checks.sort();
            Ok((checks, reports))
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use pkgcraft::test::*;

    use pkgcruft::check::CheckKind;

    use super::*;

    #[derive(Debug, Parser)]
    struct Command {
        #[clap(flatten)]
        reports: Reports,
    }

    #[test]
    fn parse() {
        let data = test_data();

        // default checks for gentoo repo
        let repo = data.ebuild_repo("gentoo").unwrap();
        let defaults = Check::iter_default(repo)
            .flat_map(|x| x.reports)
            .copied()
            .collect();
        let cmd = Command::try_parse_from(["cmd"]).unwrap();
        let (checks, _) = cmd.reports.collapse(defaults).unwrap();
        // repo specific checks enabled when scanning the matching repo
        assert!(checks.contains(&CheckKind::Header));

        // default checks
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let defaults: IndexSet<_> = Check::iter_default(repo)
            .flat_map(|x| x.reports)
            .copied()
            .collect();
        let cmd = Command::try_parse_from(["cmd"]).unwrap();
        let (checks, _) = cmd.reports.collapse(defaults.clone()).unwrap();
        assert!(checks.contains(&CheckKind::Dependency));
        // optional checks aren't run by default when scanning
        assert!(!checks.contains(&CheckKind::UnstableOnly));
        // repo specific checks aren't run by default when scanning non-matching repo
        assert!(!checks.contains(&CheckKind::Header));

        // non-default reports aren't enabled when their matching level is targeted
        let report = ReportKind::HeaderInvalid;
        assert_eq!(report.level(), ReportLevel::Error);
        let cmd = Command::try_parse_from(["cmd", "-r", "%error"]).unwrap();
        let (_, reports) = cmd.reports.collapse(defaults.clone()).unwrap();
        assert!(!reports.contains(&report));
        assert!(!reports.is_empty());

        // enable optional checks in addition to default checks
        let cmd = Command::try_parse_from(["cmd", "-r", "+@UnstableOnly,+@Header"]).unwrap();
        let (checks, _) = cmd.reports.collapse(defaults.clone()).unwrap();
        assert!(checks.contains(&CheckKind::UnstableOnly));
        assert!(checks.contains(&CheckKind::Header));
        assert!(checks.len() > 2);

        // disable checks
        let cmd = Command::try_parse_from(["cmd", "-r=-@Dependency"]).unwrap();
        let (checks, _) = cmd.reports.collapse(defaults.clone()).unwrap();
        assert!(!checks.contains(&CheckKind::Dependency));
        assert!(checks.len() > 1);

        // disable option overrides enable option
        let cmd = Command::try_parse_from(["cmd", "-r=-@Dependency,+@Dependency"]).unwrap();
        let (checks, _) = cmd.reports.collapse(defaults.clone()).unwrap();
        assert!(!checks.contains(&CheckKind::Dependency));
        assert!(checks.len() > 1);

        // error when args cancel out
        let cmd = Command::try_parse_from(["cmd", "-r=-@Dependency,@Dependency"]).unwrap();
        assert!(cmd.reports.collapse(defaults.clone()).is_err());

        // invalid check aliases in args
        for arg in ["-r=@unknown", "-r=-@unknown", "-r=+@unknown"] {
            let r = Command::try_parse_from(["cmd", arg]);
            assert_err_re!(r, "invalid check: unknown");
        }
    }
}
