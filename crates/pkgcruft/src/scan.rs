use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use indexmap::IndexSet;
use itertools::{Either, Itertools};
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::restrict::{Restrict, Scope};
use pkgcraft::utils::bounded_jobs;
use strum::IntoEnumIterator;
use tracing::{info, warn};

use crate::check::Check;
use crate::error::Error;
use crate::iter::ReportIter;
use crate::report::{ReportAlias, ReportKind};
use crate::source::PkgFilter;

pub struct Scanner {
    pub(crate) jobs: usize,
    default: IndexSet<ReportKind>,
    enabled: Option<IndexSet<Check>>,
    selected: Option<IndexSet<Check>>,
    pub(crate) reports: Arc<IndexSet<ReportKind>>,
    pub(crate) exit: Arc<IndexSet<ReportKind>>,
    pub(crate) filters: IndexSet<PkgFilter>,
    pub(crate) failed: Arc<AtomicBool>,
    pub(crate) repo: EbuildRepo,
}

impl Scanner {
    /// Create a new scanner.
    pub fn new(repo: &EbuildRepo) -> Self {
        Self {
            jobs: bounded_jobs(0),
            default: ReportKind::defaults(repo),
            enabled: Default::default(),
            selected: Default::default(),
            reports: Arc::new(ReportKind::iter().collect()),
            exit: Default::default(),
            filters: Default::default(),
            failed: Default::default(),
            repo: repo.clone(),
        }
    }

    /// Set the number of parallel scanner jobs to run.
    pub fn jobs(mut self, jobs: usize) -> Self {
        self.jobs = bounded_jobs(jobs);
        self
    }

    /// Set the enabled checks.
    pub fn checks<I>(mut self, values: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<Check>,
    {
        self.enabled = Some(values.into_iter().map(Into::into).collect());
        self
    }

    /// Set enabled report variants.
    pub fn reports<I>(mut self, values: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<ReportAlias>,
    {
        self.reports = Arc::new(
            values
                .into_iter()
                .map(Into::into)
                .flat_map(|x| x.expand(&self.default))
                .collect(),
        );
        self
    }

    /// Set the enabled and selected reports.
    pub fn selected(
        mut self,
        enabled: &IndexSet<ReportKind>,
        selected: &IndexSet<ReportKind>,
    ) -> Self {
        self.enabled = Some(Check::iter_report(enabled).collect());
        self.selected = Some(Check::iter_report(selected).collect());
        self
    }

    /// Set report variants that trigger exit code failures.
    pub fn exit<I>(mut self, values: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<ReportAlias>,
    {
        self.exit = Arc::new(
            values
                .into_iter()
                .map(Into::into)
                .flat_map(|x| x.expand(&self.default))
                .collect(),
        );
        self
    }

    /// Set package filters for target filtering.
    pub fn filters<I>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = PkgFilter>,
    {
        self.filters = values.into_iter().collect();
        self
    }

    /// Return true if the scanning process failed, false otherwise.
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Relaxed)
    }

    /// Run the scanner returning an iterator of reports.
    pub fn run<T>(&self, restrict: T) -> crate::Result<ReportIter>
    where
        T: Into<Restrict>,
    {
        let restrict = restrict.into();
        let scope = Scope::from(&restrict);
        info!("repo: {}", self.repo);
        info!("scope: {scope}");
        info!("target: {restrict:?}");

        // determine enabled and selected checks
        let empty = Default::default();
        let (enabled, selected) = match (self.enabled.as_ref(), self.selected.as_ref()) {
            (Some(x), Some(y)) => (Either::Left(x.iter().copied()), y),
            (Some(x), None) | (None, Some(x)) => (Either::Left(x.iter().copied()), x),
            (None, None) => (Either::Right(Check::iter_report(&self.default)), &empty),
        };

        // filter checks -- errors if filtered check is selected
        let mut checks: IndexSet<_> = enabled
            .map(|check| {
                if !self.filters.is_empty() && check.filtered() {
                    Err(Error::CheckInit(check, "requires no filters".to_string()))
                } else if let Some(context) = check.skipped(&self.repo, selected) {
                    Err(Error::CheckInit(check, format!("requires {context} context")))
                } else if let Some(scope) = check.scoped(scope) {
                    Err(Error::CheckInit(check, format!("requires {scope} scope")))
                } else {
                    Ok(check)
                }
            })
            .filter(|result| {
                if let Err(Error::CheckInit(check, msg)) = &result {
                    if !selected.contains(check) {
                        warn!("skipping {check} check: {msg}");
                        return false;
                    }
                }
                true
            })
            .try_collect()?;

        checks.sort();
        Ok(ReportIter::new(scope, checks, self, restrict))
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use pkgcraft::dep::Dep;
    use pkgcraft::repo::Repository;
    use pkgcraft::test::*;
    use tracing_test::traced_test;

    use crate::check::CheckKind;
    use crate::report::ReportLevel;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn targets() {
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let path = repo.path();

        // repo
        let scanner = Scanner::new(repo);
        let expected = glob_reports!("{path}/**/reports.json");
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // category
        let expected = glob_reports!("{path}/Keywords/*/reports.json");
        let restrict = repo.restrict_from_path("Keywords").unwrap();
        let reports = scanner.run(restrict).unwrap();
        assert_unordered_eq!(reports, expected);

        // package
        let expected = glob_reports!("{path}/Dependency/DependencyInvalid/reports.json");
        let restrict = repo
            .restrict_from_path("Dependency/DependencyInvalid")
            .unwrap();
        let reports = scanner.run(restrict).unwrap();
        assert_ordered_eq!(reports, expected);

        // version
        let expected = glob_reports!("{path}/Whitespace/WhitespaceInvalid/reports.json");
        let restrict = repo
            .restrict_from_path("Whitespace/WhitespaceInvalid/WhitespaceInvalid-0.ebuild")
            .unwrap();
        let reports = scanner.run(restrict).unwrap();
        assert_ordered_eq!(reports, expected);

        // non-matching restriction doesn't raise error
        let scanner = Scanner::new(repo);
        let dep = Dep::try_new("nonexistent/pkg").unwrap();
        let reports = scanner.run(&dep).unwrap();
        assert_unordered_eq!(reports, []);
    }

    #[test]
    fn checks() {
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let path = repo.path();

        // specific checks
        let scanner = Scanner::new(repo).checks([CheckKind::Dependency]);
        let expected = glob_reports!("{path}/Dependency/**/reports.json");
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // no checks
        let checks: [Check; 0] = [];
        let scanner = Scanner::new(repo).checks(checks);
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, []);
    }

    #[test]
    fn reports() {
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();

        // no reports
        let kinds: [ReportKind; 0] = [];
        let scanner = Scanner::new(repo).reports(kinds);
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, []);

        // report
        let scanner = Scanner::new(repo).reports([ReportKind::DependencyDeprecated]);
        let reports = scanner.run(repo).unwrap().count();
        assert!(reports > 0);

        // check
        let scanner = Scanner::new(repo).reports([CheckKind::Dependency]);
        let reports = scanner.run(repo).unwrap().count();
        assert!(reports > 0);

        // level
        let scanner = Scanner::new(repo).reports([ReportLevel::Warning]);
        let reports = scanner.run(repo).unwrap().count();
        assert!(reports > 0);

        // scope
        let scanner = Scanner::new(repo).reports([Scope::Version]);
        let reports = scanner.run(repo).unwrap().count();
        assert!(reports > 0);
    }

    #[test]
    fn repos() {
        let data = test_data();

        // repo with bad metadata
        let repo = data.ebuild_repo("bad").unwrap();
        let path = repo.path();
        let scanner = Scanner::new(repo);
        let expected = glob_reports!("{path}/**/reports.json");
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // empty repo
        let repo = data.ebuild_repo("empty").unwrap();
        let scanner = Scanner::new(repo);
        // no failure with repo target
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, []);
        // no failure with specific target
        let dep = Dep::try_new("nonexistent/pkg").unwrap();
        let reports = scanner.run(&dep).unwrap();
        assert_unordered_eq!(reports, []);

        // overlay repo -- dependent repo is auto-loaded
        let repo = data.ebuild_repo("qa-secondary").unwrap();
        let path = repo.path();
        let scanner = Scanner::new(repo);
        let expected = glob_reports!("{path}/**/reports.json");
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, expected);
    }

    #[traced_test]
    #[test]
    fn skip_check() {
        let data = test_data();
        let repo = data.ebuild_repo("bad").unwrap();
        let path = repo.path();
        let restrict = repo
            .restrict_from_path("eapi/invalid/invalid-9999.ebuild")
            .unwrap();
        let scanner = Scanner::new(repo);
        let reports = scanner.run(restrict).unwrap();
        let expected = glob_reports!("{path}/eapi/invalid/reports.json");
        assert_ordered_eq!(reports, expected);
        assert_logs_re!(format!(".+: skipping due to invalid pkg: eapi/invalid-9999"));
    }

    #[test]
    fn filters() {
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();

        // non-matching filter
        let filter = "cat/pkg".parse().unwrap();
        let scanner = Scanner::new(repo).filters([filter]);
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, []);

        let data = test_data();
        let repo = data.ebuild_repo("gentoo").unwrap();
        let pkgdir = repo.path().join("Header/HeaderInvalid");
        let expected = glob_reports!("{pkgdir}/reports.json");

        // none
        let mut scanner = Scanner::new(repo).reports([ReportKind::HeaderInvalid]);
        let reports: Vec<_> = scanner.run(repo).unwrap().collect();
        assert_unordered_eq!(&reports, &expected);

        for (filters, expected) in [
            (vec!["latest"], &expected[5..]),
            (vec!["!latest"], &expected[..5]),
            (vec!["latest", "!latest"], &[]),
            (vec!["latest-slots"], &[&expected[1..=1], &expected[5..]].concat()),
            (vec!["!latest-slots"], &[&expected[..1], &expected[2..5]].concat()),
            (vec!["live"], &expected[5..]),
            (vec!["!live"], &expected[..5]),
            (vec!["stable"], &expected[..3]),
            (vec!["!stable"], &expected[3..5]),
            (vec!["stable", "latest"], &expected[2..=2]),
            (vec!["masked"], &expected[..1]),
            (vec!["!masked"], &expected[1..]),
            (vec!["slot == '1'"], &expected[2..]),
            (vec!["!slot == '1'"], &expected[..2]),
        ] {
            // apply package filters to scanner
            scanner = scanner.filters(filters.iter().map(|x| x.parse().unwrap()));

            // run scanner in repo scope
            let reports: Vec<_> = scanner.run(repo).unwrap().collect();
            let failed = filters.iter().join(", ");
            assert_unordered_eq!(
                &reports,
                expected,
                format!("repo scope: failed filters: {failed}")
            );

            // run scanner in package scope
            let restrict = repo.restrict_from_path(&pkgdir).unwrap();
            let reports: Vec<_> = scanner.run(restrict).unwrap().collect();
            assert_unordered_eq!(
                &reports,
                expected,
                format!("pkg scope: failed filters: {failed}")
            );
        }
    }

    #[test]
    fn failed() {
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();

        // no reports flagged for failures
        let scanner = Scanner::new(repo);
        scanner.run(repo).unwrap().count();
        assert!(!scanner.failed());

        // missing report variant
        let scanner = scanner.exit([ReportKind::HeaderInvalid]);
        scanner.run(repo).unwrap().count();
        assert!(!scanner.failed());

        // fail on specified report variant
        let scanner = scanner.exit([ReportKind::DependencyDeprecated]);
        scanner.run(repo).unwrap().count();
        assert!(scanner.failed());

        // fail on specified check variant
        let scanner = scanner.exit([CheckKind::Dependency]);
        scanner.run(repo).unwrap().count();
        assert!(scanner.failed());

        // fail on specified report level
        let scanner = scanner.exit([ReportLevel::Warning]);
        scanner.run(repo).unwrap().count();
        assert!(scanner.failed());
    }
}
