use clap::Args;
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::cli::TriState;
use pkgcruft::report::{ReportKind, ReportSet};
use pkgcruft::Error;
use strum::IntoEnumIterator;

#[derive(Debug, Args)]
#[clap(next_help_heading = Some("Report options"))]
pub(crate) struct Reports {
    /// Restrict by report set
    #[arg(
        short,
        long,
        value_name = "SET[,...]",
        value_delimiter = ',',
        allow_hyphen_values = true
    )]
    reports: Vec<TriState<ReportSet>>,
}

impl Reports {
    pub(crate) fn collapse(
        &self,
        defaults: IndexSet<ReportKind>,
        supported: IndexSet<ReportKind>,
    ) -> pkgcruft::Result<(IndexSet<ReportKind>, IndexSet<ReportKind>)> {
        // sort by variant
        let reports: Vec<_> = self.reports.iter().copied().sorted().collect();

        // don't use defaults if neutral options exist
        let mut enabled = if let Some(TriState::Set(_)) = reports.first() {
            Default::default()
        } else {
            defaults.clone()
        };

        // Expand report sets, only adding explicitly selected check and report variants
        // to the selection set. Set membership determines if an enabled check is skipped
        // with a warning or errors out if it is unable to be run.
        let mut selected = IndexSet::new();
        for x in reports {
            match x {
                TriState::Set(set) | TriState::Add(set) => {
                    for r in set.expand(&defaults, &supported) {
                        enabled.insert(r);
                        // track explicitly selected or supported variants
                        if set.selected()
                            || (supported.contains(&r)
                                && (r != ReportKind::IgnoreUnused || set == ReportSet::All))
                        {
                            selected.insert(r);
                        }
                    }
                }
                TriState::Remove(set) => {
                    for r in set.expand(&defaults, &supported) {
                        enabled.swap_remove(&r);
                    }
                }
            };
        }

        if enabled.is_empty() {
            Err(Error::InvalidValue("no reports enabled".to_string()))
        } else {
            enabled.sort();
            selected.sort();
            Ok((enabled, selected))
        }
    }

    /// Return true if no reports are selected.
    pub(crate) fn is_empty(&self) -> bool {
        self.reports.is_empty()
    }

    /// Return the set of report variants enabled for replaying.
    pub(crate) fn replay(&self) -> pkgcruft::Result<IndexSet<ReportKind>> {
        let defaults: IndexSet<_> = ReportKind::iter().collect();
        let (enabled, _) = self.collapse(defaults.clone(), defaults)?;
        Ok(enabled)
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use pkgcraft::restrict::Scope;
    use pkgcraft::test::*;

    use pkgcruft::check::Check;
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
        let defaults = ReportKind::defaults(repo);
        let supported = ReportKind::supported(repo, Scope::Repo);
        let cmd = Command::try_parse_from(["cmd"]).unwrap();
        let (enabled, _) = cmd.reports.collapse(defaults, supported).unwrap();
        let checks: IndexSet<_> = Check::iter_report(&enabled).collect();
        // repo specific checks enabled when scanning the matching repo
        assert!(checks.contains(&Check::Header));

        // default checks
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let defaults = ReportKind::defaults(repo);
        let supported = ReportKind::supported(repo, Scope::Repo);
        let cmd = Command::try_parse_from(["cmd"]).unwrap();
        let (enabled, _) = cmd
            .reports
            .collapse(defaults.clone(), supported.clone())
            .unwrap();
        let checks: IndexSet<_> = Check::iter_report(&enabled).collect();
        assert!(checks.contains(&Check::Dependency));
        // optional checks aren't run by default when scanning
        assert!(!checks.contains(&Check::UnstableOnly));
        // repo specific checks aren't run by default when scanning non-matching repo
        assert!(!checks.contains(&Check::Header));

        // non-default reports aren't enabled when their matching level is targeted
        let report = ReportKind::HeaderInvalid;
        assert_eq!(report.level(), ReportLevel::Error);
        let cmd = Command::try_parse_from(["cmd", "-r", "@error"]).unwrap();
        let (enabled, _) = cmd
            .reports
            .collapse(defaults.clone(), supported.clone())
            .unwrap();
        assert!(!enabled.contains(&report));
        assert!(!enabled.is_empty());

        // enable optional checks in addition to default checks
        let cmd = Command::try_parse_from(["cmd", "-r", "+@UnstableOnly,+@Header"]).unwrap();
        let (enabled, _) = cmd
            .reports
            .collapse(defaults.clone(), supported.clone())
            .unwrap();
        let checks: IndexSet<_> = Check::iter_report(&enabled).collect();
        assert!(checks.contains(&Check::UnstableOnly));
        assert!(checks.contains(&Check::Header));
        assert!(checks.len() > 2);

        // disable checks
        let cmd = Command::try_parse_from(["cmd", "-r", "-@Dependency"]).unwrap();
        let (enabled, _) = cmd
            .reports
            .collapse(defaults.clone(), supported.clone())
            .unwrap();
        let checks: IndexSet<_> = Check::iter_report(&enabled).collect();
        assert!(!checks.contains(&Check::Dependency));
        assert!(checks.len() > 1);

        // disable option overrides enable option
        let cmd = Command::try_parse_from(["cmd", "-r", "-@Dependency,+@Dependency"]).unwrap();
        let (enabled, _) = cmd
            .reports
            .collapse(defaults.clone(), supported.clone())
            .unwrap();
        let checks: IndexSet<_> = Check::iter_report(&enabled).collect();
        assert!(!checks.contains(&Check::Dependency));
        assert!(checks.len() > 1);

        // error when args cancel out
        let cmd = Command::try_parse_from(["cmd", "-r", "-@Dependency,@Dependency"]).unwrap();
        let r = cmd.reports.collapse(defaults.clone(), supported.clone());
        assert_err_re!(r, "no reports enabled");

        // invalid sets
        for value in ["@unknown", "-@unknown", "+@unknown"] {
            let r = Command::try_parse_from(["cmd", "-r", value]);
            assert_err_re!(r, "invalid report set: unknown");
        }
    }
}
