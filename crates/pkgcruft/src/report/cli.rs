use std::str::FromStr;

use indexmap::IndexSet;
use pkgcraft::cli::TriState;

use crate::Error;

use super::{ReportKind, ReportSet};

/// Wrapper for targeted report sets.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
pub struct Reports(TriState<ReportSet>);

impl Reports {
    /// Collapse reports into default and selected sets.
    pub fn collapse<'a, I>(
        targets: I,
        defaults: &IndexSet<ReportKind>,
        supported: &IndexSet<ReportKind>,
    ) -> crate::Result<(IndexSet<ReportKind>, IndexSet<ReportKind>)>
    where
        I: IntoIterator<Item = &'a Self>,
    {
        // sort sets by variant
        let mut targets: IndexSet<_> = targets.into_iter().copied().map(|x| x.0).collect();
        targets.sort_unstable();

        // don't use defaults if neutral options exist
        let mut enabled = if let Some(TriState::Set(_)) = targets.first() {
            Default::default()
        } else {
            defaults.clone()
        };

        // Expand report sets, only adding explicitly selected check and report variants
        // to the selection set. Set membership determines if an enabled check is skipped
        // with a warning or errors out if it is unable to be run.
        let mut selected = IndexSet::new();
        for target in targets {
            match target {
                TriState::Set(set) | TriState::Add(set) => {
                    for r in set.expand(defaults, supported) {
                        enabled.insert(r);
                        // track explicitly selected or supported variants
                        if set.selected() || supported.contains(&r) {
                            selected.insert(r);
                        }
                    }
                }
                TriState::Remove(set) => {
                    for r in set.expand(defaults, supported) {
                        enabled.swap_remove(&r);
                    }
                }
            };
        }

        if enabled.is_empty() {
            Err(Error::InvalidValue("no reports enabled".to_string()))
        } else {
            enabled.sort_unstable();
            selected.sort_unstable();
            Ok((enabled, selected))
        }
    }
}

impl<T: Into<ReportSet>> From<T> for Reports {
    fn from(value: T) -> Self {
        Self(TriState::Set(value.into()))
    }
}

impl FromStr for Reports {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse().map(Self)
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::restrict::Scope;
    use pkgcraft::test::test_data;

    use crate::check::{Check, CheckKind};
    use crate::report::{ReportKind, ReportLevel};

    use super::*;

    #[test]
    fn report_target() {
        let data = test_data();

        // default checks for gentoo repo
        let repo = data.ebuild_repo("gentoo").unwrap();
        let defaults = ReportKind::defaults(repo);
        let supported = ReportKind::supported(repo, Scope::Repo);
        let (enabled, selected) = Reports::collapse([], &defaults, &supported).unwrap();
        assert!(selected.is_empty());
        let checks: IndexSet<_> = Check::iter_report(&enabled).collect();
        // repo specific checks enabled when scanning the matching repo
        assert!(checks.contains(&CheckKind::Header));

        // default checks
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let defaults = ReportKind::defaults(repo);
        let supported = ReportKind::supported(repo, Scope::Repo);
        let (enabled, selected) = Reports::collapse([], &defaults, &supported).unwrap();
        assert!(selected.is_empty());
        let checks: IndexSet<_> = Check::iter_report(&enabled).collect();
        assert!(checks.contains(&CheckKind::Dependency));
        // optional checks aren't run by default when scanning
        assert!(!checks.contains(&CheckKind::UnstableOnly));
        // repo specific checks aren't run by default when scanning non-matching repo
        assert!(!checks.contains(&CheckKind::Header));

        // non-default reports aren't enabled when their matching level is targeted
        let report = ReportKind::HeaderInvalid;
        assert_eq!(report.level(), ReportLevel::Error);
        let target = ReportLevel::Error.into();
        let (enabled, selected) = Reports::collapse([&target], &defaults, &supported).unwrap();
        assert!(!enabled.contains(&report));
        assert!(!enabled.is_empty());
        assert!(selected.is_subset(&enabled));
        assert!(!selected.is_empty());
    }
}
