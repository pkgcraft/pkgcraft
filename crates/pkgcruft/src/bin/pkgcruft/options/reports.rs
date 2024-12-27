use std::hash::Hash;
use std::str::FromStr;

use clap::Args;
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcruft::report::{ReportAlias, ReportKind};
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
    ) -> pkgcruft::Result<(IndexSet<ReportKind>, IndexSet<ReportKind>)> {
        // sort by variant
        let reports: Vec<_> = self.reports.iter().copied().sorted().collect();

        // don't use defaults if neutral options exist
        let mut enabled = if let Some(TriState::Set(_)) = reports.first() {
            Default::default()
        } else {
            defaults.clone()
        };

        let mut selected = IndexSet::new();
        for x in reports {
            match x {
                TriState::Set(val) => {
                    enabled.extend(val.expand(&defaults));
                    if matches!(val, ReportAlias::Check(_) | ReportAlias::Report(_)) {
                        selected.extend(val.expand(&defaults));
                    }
                }
                TriState::Add(val) => {
                    enabled.extend(val.expand(&defaults));
                    if matches!(val, ReportAlias::Check(_) | ReportAlias::Report(_)) {
                        selected.extend(val.expand(&defaults));
                    }
                }
                TriState::Remove(val) => {
                    for x in val.expand(&defaults) {
                        enabled.swap_remove(&x);
                    }
                }
            };
        }

        if enabled.is_empty() {
            Err(Error::InvalidValue("no reports enabled".to_string()))
        } else {
            Ok((enabled, selected))
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use pkgcraft::test::*;

    use pkgcruft::check::{Check, CheckKind};
    use pkgcruft::report::ReportLevel;

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
        let (enabled, _) = cmd.reports.collapse(defaults).unwrap();
        let checks: IndexSet<_> = enabled.iter().flat_map(Check::iter_report).collect();
        // repo specific checks enabled when scanning the matching repo
        assert!(checks.contains(&CheckKind::Header));

        // default checks
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let defaults: IndexSet<_> = Check::iter_default(repo)
            .flat_map(|x| x.reports)
            .copied()
            .collect();
        let cmd = Command::try_parse_from(["cmd"]).unwrap();
        let (enabled, _) = cmd.reports.collapse(defaults.clone()).unwrap();
        let checks: IndexSet<_> = enabled.iter().flat_map(Check::iter_report).collect();
        assert!(checks.contains(&CheckKind::Dependency));
        // optional checks aren't run by default when scanning
        assert!(!checks.contains(&CheckKind::UnstableOnly));
        // repo specific checks aren't run by default when scanning non-matching repo
        assert!(!checks.contains(&CheckKind::Header));

        // non-default reports aren't enabled when their matching level is targeted
        let report = ReportKind::HeaderInvalid;
        assert_eq!(report.level(), ReportLevel::Error);
        let cmd = Command::try_parse_from(["cmd", "-r", "%error"]).unwrap();
        let (enabled, _) = cmd.reports.collapse(defaults.clone()).unwrap();
        assert!(!enabled.contains(&report));
        assert!(!enabled.is_empty());

        // enable optional checks in addition to default checks
        let cmd = Command::try_parse_from(["cmd", "-r", "+@UnstableOnly,+@Header"]).unwrap();
        let (enabled, _) = cmd.reports.collapse(defaults.clone()).unwrap();
        let checks: IndexSet<_> = enabled.iter().flat_map(Check::iter_report).collect();
        assert!(checks.contains(&CheckKind::UnstableOnly));
        assert!(checks.contains(&CheckKind::Header));
        assert!(checks.len() > 2);

        // disable checks
        let cmd = Command::try_parse_from(["cmd", "-r=-@Dependency"]).unwrap();
        let (enabled, _) = cmd.reports.collapse(defaults.clone()).unwrap();
        let checks: IndexSet<_> = enabled.iter().flat_map(Check::iter_report).collect();
        assert!(!checks.contains(&CheckKind::Dependency));
        assert!(checks.len() > 1);

        // disable option overrides enable option
        let cmd = Command::try_parse_from(["cmd", "-r=-@Dependency,+@Dependency"]).unwrap();
        let (enabled, _) = cmd.reports.collapse(defaults.clone()).unwrap();
        let checks: IndexSet<_> = enabled.iter().flat_map(Check::iter_report).collect();
        assert!(!checks.contains(&CheckKind::Dependency));
        assert!(checks.len() > 1);

        // error when args cancel out
        let cmd = Command::try_parse_from(["cmd", "-r=-@Dependency,@Dependency"]).unwrap();
        let r = cmd.reports.collapse(defaults.clone());
        assert_err_re!(r, "no reports enabled");

        // invalid check aliases in args
        for arg in ["-r=@unknown", "-r=-@unknown", "-r=+@unknown"] {
            let r = Command::try_parse_from(["cmd", arg]);
            assert_err_re!(r, "invalid check: unknown");
        }
    }

    #[test]
    fn tri_state() {
        // empty
        let mut enabled = IndexSet::<ReportKind>::new();
        let selected = IndexSet::new();
        TriState::enabled(&mut enabled, &selected);
        assert_ordered_eq!(&enabled, &[]);

        // no selections
        let mut enabled: IndexSet<_> = [ReportKind::EapiBanned].into_iter().collect();
        let selected = IndexSet::new();
        TriState::enabled(&mut enabled, &selected);
        assert_ordered_eq!(&enabled, &[ReportKind::EapiBanned]);

        // override defaults
        let mut enabled: IndexSet<_> = [ReportKind::EapiBanned].into_iter().collect();
        let selected: IndexSet<TriState<ReportKind>> = ["HeaderInvalid"]
            .iter()
            .map(|s| s.parse())
            .try_collect()
            .unwrap();
        TriState::enabled(&mut enabled, &selected);
        assert_ordered_eq!(&enabled, &[ReportKind::HeaderInvalid]);

        // negated selection
        let mut enabled: IndexSet<_> = [ReportKind::EapiBanned].into_iter().collect();
        let selected: IndexSet<_> = ["HeaderInvalid", "-HeaderInvalid"]
            .iter()
            .map(|s| s.parse())
            .try_collect()
            .unwrap();
        TriState::enabled(&mut enabled, &selected);
        assert_ordered_eq!(&enabled, &[]);

        // add to defaults
        let mut enabled: IndexSet<_> = [ReportKind::EapiBanned].into_iter().collect();
        let selected: IndexSet<_> = ["+HeaderInvalid"]
            .iter()
            .map(|s| s.parse())
            .try_collect()
            .unwrap();
        TriState::enabled(&mut enabled, &selected);
        assert_ordered_eq!(&enabled, &[ReportKind::EapiBanned, ReportKind::HeaderInvalid]);
    }
}
