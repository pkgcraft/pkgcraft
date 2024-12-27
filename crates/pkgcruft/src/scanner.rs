use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::{mem, thread};

use crossbeam_channel::{bounded, Receiver, Sender};
use crossbeam_utils::sync::WaitGroup;
use indexmap::IndexSet;
use itertools::{Either, Itertools};
use pkgcraft::repo::{ebuild::EbuildRepo, PkgRepository};
use pkgcraft::restrict::{Restrict, Scope};
use pkgcraft::utils::bounded_jobs;
use strum::IntoEnumIterator;
use tracing::{info, warn};

use crate::check::Check;
use crate::error::Error;
use crate::report::{Report, ReportAlias, ReportKind};
use crate::runner::SyncCheckRunner;
use crate::source::{PkgFilter, Target};

pub struct Scanner {
    jobs: usize,
    default: IndexSet<ReportKind>,
    enabled: Option<IndexSet<Check>>,
    selected: Option<IndexSet<Check>>,
    reports: Arc<IndexSet<ReportKind>>,
    exit: Arc<IndexSet<ReportKind>>,
    filters: IndexSet<PkgFilter>,
    failed: Arc<AtomicBool>,
    repo: EbuildRepo,
}

impl Scanner {
    /// Create a new scanner.
    pub fn new(repo: &EbuildRepo) -> Self {
        Self {
            jobs: bounded_jobs(0),
            default: ReportKind::iter_default(repo).collect(),
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

    /// Set the enabled and selected checks.
    pub fn selected(
        mut self,
        enabled: &IndexSet<ReportKind>,
        selected: &IndexSet<ReportKind>,
    ) -> Self {
        self.enabled = Some(enabled.iter().flat_map(Check::iter_report).collect());
        self.selected = Some(selected.iter().flat_map(Check::iter_report).collect());
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
            (None, None) => {
                (Either::Right(self.default.iter().flat_map(Check::iter_report)), &empty)
            }
        };

        // filter checks -- errors if filtered check is selected
        let checks = enabled
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
            });

        let runner =
            SyncCheckRunner::new(scope, &self.repo, &restrict, &self.filters, checks)?;

        if scope >= Scope::Category {
            Ok(ReportIter::pkg(runner, self, restrict))
        } else {
            Ok(ReportIter::version(runner, self, restrict))
        }
    }
}

#[derive(Clone)]
pub(crate) struct ReportFilter {
    reports: Vec<Report>,
    filter: Arc<IndexSet<ReportKind>>,
    exit: Arc<IndexSet<ReportKind>>,
    failed: Arc<AtomicBool>,
    pkg_tx: Option<Sender<Vec<Report>>>,
    version_tx: Option<Sender<Report>>,
    finalize: bool,
}

impl ReportFilter {
    /// Conditionally add a report based on filter inclusion.
    pub(crate) fn report(&mut self, report: Report) {
        if self.filter.contains(report.kind()) {
            if self.exit.contains(report.kind()) {
                self.failed.store(true, Ordering::Relaxed);
            }

            if let Some(tx) = &self.version_tx {
                tx.send(report).ok();
            } else {
                self.reports.push(report);
            }
        }
    }

    /// Sort existing reports and send them to the iterator.
    fn process(&mut self) {
        if !self.reports.is_empty() {
            if let Some(tx) = &self.pkg_tx {
                self.reports.sort();
                tx.send(mem::take(&mut self.reports)).ok();
            }
        }
    }

    /// Return true if the filter has a report variant enabled.
    pub(crate) fn enabled(&self, kind: ReportKind) -> bool {
        self.filter.contains(&kind)
    }

    /// Return true if post-run finalization should be performed for a report variant.
    pub(crate) fn finalize(&self, kind: ReportKind) -> bool {
        self.finalize && self.enabled(kind)
    }
}

/// Create a producer thread that sends package targets over a channel to workers.
fn pkg_producer(
    repo: EbuildRepo,
    runner: Arc<SyncCheckRunner>,
    wg: WaitGroup,
    restrict: Restrict,
    tx: Sender<(Option<Check>, Target)>,
    finish_tx: Sender<Check>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        // run non-package checks in parallel
        for check in runner.checks().filter(|c| c.scope > Scope::Package) {
            tx.send((Some(check), Target::Repo)).ok();
        }

        // return if no package checks are selected
        if !runner.checks().any(|c| c.scope <= Scope::Package) {
            return;
        }

        // parallelize running checks per package
        for cpn in repo.iter_cpn_restrict(&restrict) {
            tx.send((None, Target::Cpn(cpn))).ok();
        }

        // return if scanning run doesn't support check finalization
        if !runner.finalize() {
            return;
        }

        // wait for all parallelized checks to finish
        drop(tx);
        wg.wait();

        // finalize checks in parallel
        for check in runner.checks().filter(|c| c.finalize()) {
            finish_tx.send(check).ok();
        }
    })
}

/// Create worker thread that parallelizes check running at a package level.
fn pkg_worker(
    runner: Arc<SyncCheckRunner>,
    wg: WaitGroup,
    mut filter: ReportFilter,
    rx: Receiver<(Option<Check>, Target)>,
    finish_rx: Receiver<Check>,
) -> thread::JoinHandle<()> {
    // hack to force log capturing for tests to work in threads
    // https://github.com/dbrgn/tracing-test/issues/23
    #[cfg(test)]
    let thread_span = tracing::debug_span!("thread").or_current();

    thread::spawn(move || {
        // hack to force log capturing for tests to work in threads
        // https://github.com/dbrgn/tracing-test/issues/23
        #[cfg(test)]
        let _entered = thread_span.clone().entered();

        // run checks across packages in parallel
        for (check, target) in rx {
            if let Some(check) = check {
                runner.run_check(check, target, &mut filter);
            } else {
                runner.run_checks(target, &mut filter);
            }
            filter.process();
        }

        // signal the wait group
        drop(wg);

        // finalize checks
        for check in finish_rx {
            runner.finish(check, &mut filter);
            filter.process();
        }
    })
}

/// Iterator that parallelizes by package, running in category and repo scope.
#[derive(Debug)]
struct IterPkg {
    rx: Receiver<Vec<Report>>,
    _producer: thread::JoinHandle<()>,
    _workers: Vec<thread::JoinHandle<()>>,
    reports: VecDeque<Report>,
}

impl Iterator for IterPkg {
    type Item = Report;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(report) = self.reports.pop_front() {
                return Some(report);
            } else if let Ok(reports) = self.rx.recv() {
                debug_assert!(!reports.is_empty());
                self.reports.extend(reports);
            } else {
                return None;
            }
        }
    }
}

/// Create a producer thread that sends checks with targets over a channel to workers.
fn version_producer(
    repo: EbuildRepo,
    runner: Arc<SyncCheckRunner>,
    restrict: Restrict,
    tx: Sender<(Check, Target)>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for cpv in repo.iter_cpv_restrict(&restrict) {
            for check in runner.checks().filter(|c| c.scope == Scope::Version) {
                tx.send((check, Target::Cpv(cpv.clone()))).ok();
            }
        }

        for cpn in repo.iter_cpn_restrict(&restrict) {
            for check in runner.checks().filter(|c| c.scope == Scope::Package) {
                tx.send((check, Target::Cpn(cpn.clone()))).ok();
            }
        }
    })
}

/// Create worker thread that parallelizes check running at a version level.
fn version_worker(
    runner: Arc<SyncCheckRunner>,
    mut filter: ReportFilter,
    rx: Receiver<(Check, Target)>,
) -> thread::JoinHandle<()> {
    // hack to force log capturing for tests to work in threads
    // https://github.com/dbrgn/tracing-test/issues/23
    #[cfg(test)]
    let thread_span = tracing::debug_span!("thread").or_current();

    thread::spawn(move || {
        // hack to force log capturing for tests to work in threads
        // https://github.com/dbrgn/tracing-test/issues/23
        #[cfg(test)]
        let _entered = thread_span.clone().entered();

        for (check, target) in rx {
            runner.run_check(check, target, &mut filter);
        }
    })
}

/// Iterator that parallelizes by check, running in version and package scope.
#[derive(Debug)]
struct IterVersion {
    rx: Receiver<Report>,
    _producer: thread::JoinHandle<()>,
    _workers: Vec<thread::JoinHandle<()>>,
    reports: Option<Vec<Report>>,
}

impl Iterator for IterVersion {
    type Item = Report;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(reports) = self.reports.as_mut() {
                return reports.pop();
            } else {
                self.reports = Some(self.rx.iter().sorted_by(|a, b| b.cmp(a)).collect());
            }
        }
    }
}

/// Encapsulating iterator supporting varying scanning target parallelism.
#[derive(Debug)]
enum ReportIterInternal {
    Pkg(IterPkg),
    Version(IterVersion),
}

impl Iterator for ReportIterInternal {
    type Item = Report;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Pkg(iter) => iter.next(),
            Self::Version(iter) => iter.next(),
        }
    }
}

/// Iterator of reports.
#[derive(Debug)]
pub struct ReportIter(ReportIterInternal);

impl ReportIter {
    /// Create an iterator that parallelizes scanning by package.
    fn pkg(runner: SyncCheckRunner, scanner: &Scanner, restrict: Restrict) -> Self {
        let runner = Arc::new(runner);
        let (targets_tx, targets_rx) = bounded(scanner.jobs);
        let (finish_tx, finish_rx) = bounded(scanner.jobs);
        let (reports_tx, reports_rx) = bounded(scanner.jobs);
        let wg = WaitGroup::new();
        let filter = ReportFilter {
            reports: Default::default(),
            filter: scanner.reports.clone(),
            exit: scanner.exit.clone(),
            failed: scanner.failed.clone(),
            pkg_tx: Some(reports_tx),
            version_tx: None,
            finalize: runner.finalize(),
        };

        Self(ReportIterInternal::Pkg(IterPkg {
            rx: reports_rx,
            _workers: (0..scanner.jobs)
                .map(|_| {
                    pkg_worker(
                        runner.clone(),
                        wg.clone(),
                        filter.clone(),
                        targets_rx.clone(),
                        finish_rx.clone(),
                    )
                })
                .collect(),
            _producer: pkg_producer(
                scanner.repo.clone(),
                runner.clone(),
                wg,
                restrict,
                targets_tx,
                finish_tx,
            ),
            reports: Default::default(),
        }))
    }

    /// Create an iterator that parallelizes scanning by check.
    fn version(runner: SyncCheckRunner, scanner: &Scanner, restrict: Restrict) -> Self {
        let runner = Arc::new(runner);
        let (targets_tx, targets_rx) = bounded(scanner.jobs);
        let (reports_tx, reports_rx) = bounded(scanner.jobs);
        let filter = ReportFilter {
            reports: Default::default(),
            filter: scanner.reports.clone(),
            exit: scanner.exit.clone(),
            failed: scanner.failed.clone(),
            pkg_tx: None,
            version_tx: Some(reports_tx),
            finalize: runner.finalize(),
        };

        Self(ReportIterInternal::Version(IterVersion {
            rx: reports_rx,
            _workers: (0..scanner.jobs)
                .map(|_| version_worker(runner.clone(), filter.clone(), targets_rx.clone()))
                .collect(),
            _producer: version_producer(
                scanner.repo.clone(),
                runner.clone(),
                restrict,
                targets_tx,
            ),
            reports: Default::default(),
        }))
    }
}

impl Iterator for ReportIter {
    type Item = Report;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::dep::Dep;
    use pkgcraft::repo::Repository;
    use pkgcraft::test::*;
    use tracing_test::traced_test;

    use crate::check::CheckKind;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn run() {
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let repo_path = repo.path();

        // repo target
        let scanner = Scanner::new(repo);
        let expected = glob_reports!("{repo_path}/**/reports.json");
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // category target
        let expected = glob_reports!("{repo_path}/Keywords/**/reports.json");
        let restrict = repo.restrict_from_path("Keywords").unwrap();
        let reports = scanner.run(restrict).unwrap();
        assert_unordered_eq!(reports, expected);

        // package target
        let expected = glob_reports!("{repo_path}/Dependency/DependencyInvalid/reports.json");
        let restrict = repo
            .restrict_from_path("Dependency/DependencyInvalid")
            .unwrap();
        let reports = scanner.run(restrict).unwrap();
        assert_ordered_eq!(reports, expected);

        // version target
        let expected = glob_reports!("{repo_path}/Whitespace/WhitespaceInvalid/reports.json");
        let restrict = repo
            .restrict_from_path("Whitespace/WhitespaceInvalid/WhitespaceInvalid-0.ebuild")
            .unwrap();
        let reports = scanner.run(restrict).unwrap();
        assert_ordered_eq!(reports, expected);

        // specific checks
        let scanner = Scanner::new(repo).checks([CheckKind::Dependency]);
        let expected = glob_reports!("{repo_path}/Dependency/**/reports.json");
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // specific reports
        let scanner = Scanner::new(repo).reports([ReportKind::DependencyDeprecated]);
        let expected =
            glob_reports!("{repo_path}/Dependency/DependencyDeprecated/reports.json");
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // no checks
        let checks: [Check; 0] = [];
        let scanner = Scanner::new(repo).checks(checks);
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, []);

        // no reports
        let kinds: [ReportKind; 0] = [];
        let scanner = Scanner::new(repo).reports(kinds);
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, []);

        // non-matching filter
        let filter = "cat/pkg".parse().unwrap();
        let scanner = Scanner::new(repo).filters([filter]);
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, []);

        // non-matching restriction doesn't raise error
        let scanner = Scanner::new(repo);
        let dep = Dep::try_new("nonexistent/pkg").unwrap();
        let reports = scanner.run(&dep).unwrap();
        assert_unordered_eq!(reports, []);

        // repo with bad metadata
        let repo = data.ebuild_repo("bad").unwrap();
        let repo_path = repo.path();
        let scanner = Scanner::new(repo);
        let expected = glob_reports!("{repo_path}/**/reports.json");
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // empty repo
        let repo = data.ebuild_repo("empty").unwrap();
        let scanner = Scanner::new(repo);
        // no failure with repo target
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, []);
        // no failure with specific target
        let reports = scanner.run(&dep).unwrap();
        assert_unordered_eq!(reports, []);

        // overlay repo -- dependent repo is auto-loaded
        let repo = data.ebuild_repo("qa-secondary").unwrap();
        let repo_path = repo.path();
        let scanner = Scanner::new(repo);
        let expected = glob_reports!("{repo_path}/**/reports.json");
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, expected);
    }

    #[traced_test]
    #[test]
    fn skip_check() {
        let data = test_data();
        let repo = data.ebuild_repo("bad").unwrap();
        let repo_path = repo.path();
        let restrict = repo
            .restrict_from_path("eapi/invalid/invalid-9999.ebuild")
            .unwrap();
        let scanner = Scanner::new(repo);
        let reports = scanner.run(restrict).unwrap();
        let expected = glob_reports!("{repo_path}/eapi/invalid/reports.json");
        assert_ordered_eq!(reports, expected);
        assert_logs_re!(format!(".+: skipping due to invalid pkg: eapi/invalid-9999"));
    }

    #[test]
    fn filters() {
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
    }
}
