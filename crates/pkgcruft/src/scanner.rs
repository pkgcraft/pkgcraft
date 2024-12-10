use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::{mem, thread};

use crossbeam_channel::{bounded, Receiver, Sender};
use indexmap::IndexSet;
use itertools::{Either, Itertools};
use pkgcraft::repo::{ebuild::EbuildRepo, PkgRepository};
use pkgcraft::restrict::Restrict;
use pkgcraft::utils::bounded_jobs;
use strum::IntoEnumIterator;
use tracing::info;

use crate::check::Check;
use crate::error::Error;
use crate::report::{Report, ReportKind};
use crate::runner::SyncCheckRunner;
use crate::scope::Scope;
use crate::source::{PkgFilter, Target};

pub struct Scanner {
    jobs: usize,
    checks: IndexSet<Check>,
    reports: Arc<IndexSet<ReportKind>>,
    exit: Arc<IndexSet<ReportKind>>,
    filters: IndexSet<PkgFilter>,
    failed: Arc<AtomicBool>,
    repo: &'static EbuildRepo,
}

impl Scanner {
    /// Create a new scanner.
    pub fn new(repo: &EbuildRepo) -> Self {
        // TODO: drop forced static lifetime once repo handling is improved
        let repo: &'static EbuildRepo = unsafe { mem::transmute(repo) };
        Self {
            jobs: bounded_jobs(0),
            checks: Check::iter_default(Some(repo)).collect(),
            reports: Arc::new(ReportKind::iter().collect()),
            exit: Default::default(),
            filters: Default::default(),
            failed: Default::default(),
            repo,
        }
    }

    /// Set the number of parallel scanner jobs to run.
    pub fn jobs(mut self, jobs: usize) -> Self {
        self.jobs = bounded_jobs(jobs);
        self
    }

    /// Set the checks to run.
    pub fn checks<I>(mut self, values: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<Check>,
    {
        self.checks = values.into_iter().map(Into::into).collect();
        self
    }

    /// Set enabled report variants.
    pub fn reports<I>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = ReportKind>,
    {
        self.reports = Arc::new(values.into_iter().collect());
        self
    }

    /// Set report variants that trigger exit code failures.
    pub fn exit<I>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = ReportKind>,
    {
        self.exit = Arc::new(values.into_iter().collect());
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
    pub fn run<T>(&self, restrict: T) -> crate::Result<impl Iterator<Item = Report>>
    where
        T: Into<Restrict>,
    {
        let restrict = restrict.into();
        let scope = Scope::from(&restrict);
        info!("repo: {}", self.repo);
        info!("scope: {scope}");
        info!("target: {restrict:?}");

        // return early for non-matching restrictions
        if restrict == Restrict::False || self.repo.iter_cpv_restrict(&restrict).next().is_none() {
            return Err(Error::InvalidValue("no matches found".to_string()));
        }

        let runner = Arc::new(SyncCheckRunner::new(
            scope,
            self.repo,
            &restrict,
            &self.filters,
            &self.checks,
        ));
        if scope >= Scope::Category {
            // parallel by package
            let (restrict_tx, restrict_rx) = bounded(self.jobs);
            let (reports_tx, reports_rx) = bounded(self.jobs);
            let filter = ReportFilter {
                reports: Default::default(),
                filter: self.reports.clone(),
                exit: self.exit.clone(),
                failed: self.failed.clone(),
                pkg_tx: Some(reports_tx),
                version_tx: None,
            };

            Ok(Either::Left(IterPkg {
                rx: reports_rx,
                _producer: pkg_producer(self.repo, runner.clone(), restrict, restrict_tx),
                _workers: (0..self.jobs)
                    .map(|_| pkg_worker(runner.clone(), filter.clone(), restrict_rx.clone()))
                    .collect(),
                reports: Default::default(),
            }))
        } else {
            // parallel by check
            if !self.filters.is_empty() {
                return Err(Error::InvalidValue(format!("filters unsupported in {scope} scope")));
            }

            let (restrict_tx, restrict_rx) = bounded(self.jobs);
            let (reports_tx, reports_rx) = bounded(self.jobs);
            let filter = ReportFilter {
                reports: Default::default(),
                filter: self.reports.clone(),
                exit: self.exit.clone(),
                failed: self.failed.clone(),
                pkg_tx: None,
                version_tx: Some(reports_tx),
            };

            Ok(Either::Right(IterVersion {
                rx: reports_rx,
                _producer: version_producer(self.repo, runner.clone(), restrict, restrict_tx),
                _workers: (0..self.jobs)
                    .map(|_| version_worker(runner.clone(), filter.clone(), restrict_rx.clone()))
                    .collect(),
                reports: Default::default(),
            }))
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
}
/// Create a producer thread that sends package targets over a channel to workers.
fn pkg_producer(
    repo: &'static EbuildRepo,
    runner: Arc<SyncCheckRunner>,
    restrict: Restrict,
    tx: Sender<(Option<Check>, Target)>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        // run non-package checks in parallel
        for check in runner.checks(|c| !c.scope.is_pkg()) {
            tx.send((Some(check), Target::Repo(repo))).ok();
        }

        // parallelize per package if relevant checks are selected
        if runner.checks(|c| c.scope.is_pkg()).next().is_some() {
            for cpn in repo.iter_cpn_restrict(&restrict) {
                tx.send((None, Target::Cpn(cpn))).ok();
            }
        }
    })
}

/// Create worker thread that parallelizes check running at a package level.
fn pkg_worker(
    runner: Arc<SyncCheckRunner>,
    mut filter: ReportFilter,
    rx: Receiver<(Option<Check>, Target)>,
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
            if let Some(check) = check {
                runner.run_check(check, target, &mut filter);
            } else {
                runner.run(target, &mut filter);
            }
            filter.process();
        }
    })
}

struct IterPkg {
    rx: Receiver<Vec<Report>>,
    _producer: thread::JoinHandle<()>,
    _workers: Vec<thread::JoinHandle<()>>,
    reports: VecDeque<Report>,
}

impl Iterator for IterPkg {
    type Item = Report;

    fn next(&mut self) -> Option<Self::Item> {
        self.reports.pop_front().or_else(|| {
            self.rx.recv().ok().and_then(|reports| {
                debug_assert!(!reports.is_empty());
                self.reports.extend(reports);
                self.next()
            })
        })
    }
}

/// Create a producer thread that sends checks with targets over a channel to workers.
fn version_producer(
    repo: &'static EbuildRepo,
    runner: Arc<SyncCheckRunner>,
    restrict: Restrict,
    tx: Sender<(Check, Target)>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for cpv in repo.iter_cpv_restrict(&restrict) {
            for check in runner.checks(|c| c.scope == Scope::Version) {
                tx.send((check, Target::Cpv(cpv.clone()))).ok();
            }
        }

        for cpn in repo.iter_cpn_restrict(&restrict) {
            for check in runner.checks(|c| c.scope == Scope::Package) {
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

struct IterVersion {
    rx: Receiver<Report>,
    _producer: thread::JoinHandle<()>,
    _workers: Vec<thread::JoinHandle<()>>,
    reports: Option<Vec<Report>>,
}

impl Iterator for IterVersion {
    type Item = Report;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(reports) = self.reports.as_mut() {
            reports.pop()
        } else {
            self.reports = Some(self.rx.iter().sorted_by(|a, b| b.cmp(a)).collect());
            self.next()
        }
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
        let expected = glob_reports!("{repo_path}/Dependency/DependencyDeprecated/reports.json");
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // no checks
        let checks: [Check; 0] = [];
        let scanner = Scanner::new(repo).checks(checks);
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, []);

        // no reports
        let scanner = Scanner::new(repo).reports([]);
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, []);

        // non-matching restriction
        let scanner = Scanner::new(repo);
        let dep = Dep::try_new("nonexistent/pkg").unwrap();
        assert!(scanner.run(&dep).is_err());

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
        assert!(scanner.run(repo).is_err());

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
        assert_logs_re!(format!(".+: skipping due to invalid pkg: eapi/invalid-9999$"));
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
