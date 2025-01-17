use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::restrict::{Restrict, Scope, TryIntoRestrict};
use pkgcraft::utils::bounded_jobs;
use tracing::{info, warn};

use crate::check::Check;
use crate::error::Error;
use crate::ignore::Ignore;
use crate::iter::{ReportIter, ReportSender};
use crate::report::{Report, ReportKind, ReportSet, ReportTarget};
use crate::source::PkgFilter;

/// Scanner builder.
#[derive(Debug, Default)]
pub struct Scanner {
    jobs: usize,
    force: bool,
    reports: IndexSet<ReportTarget>,
    exit: IndexSet<ReportSet>,
    filters: IndexSet<PkgFilter>,
    failed: Arc<AtomicBool>,
}

impl Scanner {
    /// Create a new scanner.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the number of parallel scanner jobs to run.
    pub fn jobs(mut self, value: usize) -> Self {
        self.jobs = value;
        self
    }

    /// Configure if ignore directives are respected.
    pub fn force(mut self, value: bool) -> Self {
        self.force = value;
        self
    }

    /// Set the report set targets.
    pub fn reports<I>(mut self, values: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<ReportTarget>,
    {
        self.reports = values.into_iter().map(Into::into).collect();
        self
    }

    /// Set report variants that trigger exit code failures.
    pub fn exit<I>(mut self, values: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<ReportSet>,
    {
        self.exit = values.into_iter().map(Into::into).collect();
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
    pub fn run<T>(&self, repo: &EbuildRepo, value: T) -> crate::Result<ReportIter>
    where
        T: TryIntoRestrict<EbuildRepo>,
    {
        let mut run = ScannerRun::new(self, repo, value)?;

        // expand report sets into enabled and selected reports
        let defaults = ReportKind::defaults(repo);
        let supported = ReportKind::supported(repo, run.scope);
        let (enabled, selected) = if self.reports.is_empty() {
            (defaults.clone(), Default::default())
        } else {
            ReportTarget::collapse(&self.reports, &defaults, &supported)?
        };

        // expand exit sets
        run.exit = self
            .exit
            .iter()
            .flat_map(|x| x.expand(&defaults, &supported))
            .collect();

        // determine if any filtering is enabled
        let pkg_filtering = !self.filters.is_empty();

        // determine enabled reports -- errors if incompatible report is selected
        run.enabled = enabled
            .iter()
            .copied()
            .map(|report| {
                if let Some(scope) = report.scoped(run.scope) {
                    Err(Error::ReportInit(report, format!("requires {scope} scope")))
                } else if pkg_filtering && report.finish_check(run.scope) {
                    Err(Error::ReportInit(report, "requires no package filtering".to_string()))
                } else {
                    Ok(report)
                }
            })
            .filter(|result| {
                if let Err(Error::ReportInit(report, msg)) = &result {
                    if !selected.contains(report) {
                        warn!("skipping {report} report: {msg}");
                        return false;
                    }
                }
                true
            })
            .try_collect()?;

        // determine enabled checks -- errors if incompatible check is selected
        let selected = Check::iter_report(&selected).collect();
        run.checks = Check::iter_report(&enabled)
            .unique()
            .sorted()
            .map(|check| {
                if pkg_filtering && check.filtered() {
                    Err(Error::CheckInit(check, "requires no package filtering".to_string()))
                } else if let Some(context) = check.skipped(repo, &selected) {
                    Err(Error::CheckInit(check, format!("requires {context} context")))
                } else if let Some(scope) = check.scoped(run.scope) {
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

        Ok(ReportIter::new(Arc::new(run)))
    }
}

/// Conglomeration of scanning run data.
pub(crate) struct ScannerRun {
    pub(crate) repo: EbuildRepo,
    pub(crate) restrict: Restrict,
    pub(crate) scope: Scope,
    force: bool,
    pub(crate) jobs: usize,
    pub(crate) filters: IndexSet<PkgFilter>,
    pub(crate) checks: IndexSet<Check>,
    pub(crate) ignore: Ignore,
    enabled: HashSet<ReportKind>,
    exit: HashSet<ReportKind>,
    failed: Arc<AtomicBool>,
    pub(crate) sender: OnceLock<ReportSender>,
}

impl ScannerRun {
    /// Create new aggregate data for a scanning run.
    fn new<T>(scanner: &Scanner, repo: &EbuildRepo, value: T) -> crate::Result<Self>
    where
        T: TryIntoRestrict<EbuildRepo>,
    {
        let restrict = value.try_into_restrict(repo)?;
        let scope = Scope::from(&restrict);

        info!("repo: {repo}");
        info!("scope: {scope}");
        info!("target: {restrict:?}");

        Ok(Self {
            repo: repo.clone(),
            restrict,
            scope,
            force: scanner.force,
            jobs: bounded_jobs(scanner.jobs),
            filters: scanner.filters.clone(),
            checks: Default::default(),
            ignore: Ignore::new(repo),
            enabled: Default::default(),
            exit: Default::default(),
            failed: scanner.failed.clone(),
            sender: Default::default(),
        })
    }

    pub(crate) fn sender(&self) -> &ReportSender {
        self.sender.get().expect("failed getting sender")
    }

    /// Conditionally add a report based on filter inclusion.
    pub(crate) fn report(&self, report: Report) {
        let kind = report.kind;
        if self.enabled(kind) && (self.force || !self.ignore.ignored(&report)) {
            if self.exit.contains(&kind) {
                self.failed.store(true, Ordering::Relaxed);
            }
            self.sender().report(report);
        }
    }

    /// Return true if the run has a report variant enabled.
    pub(crate) fn enabled(&self, kind: ReportKind) -> bool {
        self.enabled.contains(&kind)
    }
}

#[cfg(test)]
mod tests {
    use camino::Utf8Path;
    use pkgcraft::test::*;
    use tracing_test::traced_test;

    use crate::check::{Check, CheckContext};
    use crate::report::ReportLevel;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn targets() {
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let path = repo.path();
        let scanner = Scanner::new();

        // repo
        let expected = glob_reports!("{path}/**/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // category
        let expected = glob_reports!("{path}/Keywords/*/reports.json");
        let reports = scanner.run(repo, Utf8Path::new("Keywords")).unwrap();
        assert_unordered_eq!(reports, expected);

        // package
        let expected = glob_reports!("{path}/Dependency/DependencyInvalid/reports.json");
        let reports = scanner
            .run(repo, Utf8Path::new("Dependency/DependencyInvalid"))
            .unwrap();
        assert_ordered_eq!(reports, expected);

        // version
        let expected = glob_reports!("{path}/Whitespace/WhitespaceInvalid/reports.json");
        let reports = scanner.run(repo, "Whitespace/WhitespaceInvalid-0").unwrap();
        assert_ordered_eq!(reports, expected);

        // non-matching restriction doesn't raise error unlike `pkgcruft scan`
        let reports = scanner.run(repo, "nonexistent/pkg").unwrap();
        assert_unordered_eq!(reports, []);
    }

    #[test]
    fn reports() {
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let path = repo.path();

        // no explicit reports uses default set
        let scanner = Scanner::new();
        let reports = scanner.run(repo, repo).unwrap().count();
        assert!(reports > 0);

        // all
        let scanner = Scanner::new().reports([ReportSet::All]);
        let reports = scanner.run(repo, repo).unwrap().count();
        assert!(reports > 0);

        // finalized
        let scanner = Scanner::new().reports([ReportSet::Finalize]);
        let reports = scanner.run(repo, repo).unwrap().count();
        assert!(reports > 0);

        // check
        let scanner = Scanner::new().reports([Check::Dependency]);
        let expected = glob_reports!("{path}/Dependency/**/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // filter failure
        let latest = "latest".parse().unwrap();
        let scanner = Scanner::new().reports([Check::Filesdir]).filters([latest]);
        let result = scanner.run(repo, repo);
        assert_err_re!(result, "Filesdir: check requires no package filtering");

        // context failure
        let scanner = Scanner::new().reports([Check::PythonUpdate]);
        let result = scanner.run(repo, repo);
        assert_err_re!(result, "PythonUpdate: check requires gentoo-inherited context");

        // scope failure
        let scanner = Scanner::new().reports([Check::Filesdir]);
        let result = scanner.run(repo, "Filesdir/FilesUnused-0");
        assert_err_re!(result, "FilesUnused: report requires package scope");

        // context
        let scanner = Scanner::new().reports([CheckContext::Optional]);
        let reports = scanner.run(repo, repo).unwrap().count();
        assert!(reports > 0);

        // level
        let scanner = Scanner::new().reports([ReportLevel::Warning]);
        let reports = scanner.run(repo, repo).unwrap().count();
        assert!(reports > 0);

        // report
        let scanner = Scanner::new().reports([ReportKind::DependencyDeprecated]);
        let reports = scanner.run(repo, repo).unwrap().count();
        assert!(reports > 0);

        // scope
        let scanner = Scanner::new().reports([Scope::Version]);
        let reports = scanner.run(repo, repo).unwrap().count();
        assert!(reports > 0);
    }

    #[test]
    fn repos() {
        let data = test_data();
        let scanner = Scanner::new();

        // repo with bad metadata
        let repo = data.ebuild_repo("bad").unwrap();
        let path = repo.path();
        let expected = glob_reports!("{path}/**/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // empty repo
        let repo = data.ebuild_repo("empty").unwrap();
        // no failure with repo target
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, []);
        // no failure with specific target
        let reports = scanner.run(repo, "nonexistent/pkg").unwrap();
        assert_unordered_eq!(reports, []);

        // overlay repo -- dependent repo is auto-loaded
        let repo = data.ebuild_repo("qa-secondary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, []);
    }

    #[traced_test]
    #[test]
    fn skip_check() {
        let data = test_data();
        let repo = data.ebuild_repo("bad").unwrap();
        let path = repo.path();
        let scanner = Scanner::new();
        let reports = scanner.run(repo, "eapi/invalid-9999").unwrap();
        let expected = glob_reports!("{path}/eapi/invalid/reports.json");
        assert_ordered_eq!(reports, expected);
        assert_logs_re!(format!(".+: skipping due to invalid pkg: eapi/invalid-9999"));
    }

    #[test]
    fn filters() {
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();

        // verify finalized reports aren't triggered with filters
        let reports: Vec<_> = Scanner::new()
            .filters(["live", "!live"].iter().map(|x| x.parse().unwrap()))
            .run(repo, repo)
            .unwrap()
            .collect();
        assert_unordered_eq!(&reports, &[]);

        let repo = data.ebuild_repo("gentoo").unwrap();
        let pkgdir = repo.path().join("Header/HeaderInvalid");
        let expected = glob_reports!("{pkgdir}/reports.json");

        // none
        let mut scanner = Scanner::new().reports([ReportKind::HeaderInvalid]);
        let reports: Vec<_> = scanner.run(repo, repo).unwrap().collect();
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
            let reports: Vec<_> = scanner.run(repo, repo).unwrap().collect();
            let failed = filters.iter().join(", ");
            assert_unordered_eq!(
                &reports,
                expected,
                format!("repo scope: failed filters: {failed}")
            );

            // run scanner in package scope
            let reports: Vec<_> = scanner.run(repo, pkgdir.as_path()).unwrap().collect();
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
        let scanner = Scanner::new();
        scanner.run(repo, repo).unwrap().count();
        assert!(!scanner.failed());

        // missing report variant
        let scanner = scanner.exit([ReportKind::HeaderInvalid]);
        scanner.run(repo, repo).unwrap().count();
        assert!(!scanner.failed());

        // fail on specified report variant
        let scanner = scanner.exit([ReportKind::DependencyDeprecated]);
        scanner.run(repo, repo).unwrap().count();
        assert!(scanner.failed());

        // fail on specified check variant
        let scanner = scanner.exit([Check::Dependency]);
        scanner.run(repo, repo).unwrap().count();
        assert!(scanner.failed());

        // fail on specified report level
        let scanner = scanner.exit([ReportLevel::Warning]);
        scanner.run(repo, repo).unwrap().count();
        assert!(scanner.failed());
    }
}
