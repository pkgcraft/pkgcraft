use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::{iter, mem, thread};

use crossbeam_channel::{bounded, Receiver, Sender};
use indexmap::IndexSet;
use pkgcraft::repo::{ebuild::EbuildRepo, PkgRepository, Repo};
use pkgcraft::restrict::Restrict;
use pkgcraft::utils::bounded_jobs;
use strum::IntoEnumIterator;
use tracing::info;

use crate::check::Check;
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
}

impl Default for Scanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Scanner {
    /// Create a new scanner.
    pub fn new() -> Self {
        Self {
            jobs: bounded_jobs(0),
            checks: Check::iter_default().collect(),
            reports: Arc::new(ReportKind::iter().collect()),
            exit: Arc::new(Default::default()),
            filters: Default::default(),
            failed: Arc::new(Default::default()),
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
    pub fn run<T>(&self, repo: &Repo, restrict: T) -> Box<dyn Iterator<Item = Report>>
    where
        T: Into<Restrict>,
    {
        // TODO: Drop this hack once lifetime handling is improved for thread usage.
        // Currently, it's not possible to use std::thread::scope() as the related Scope
        // objects can't be stored without self-referential issues. Also, this is much
        // easier than passing Arc-wrapped repo objects down to checks which then can have
        // more self-referencing issues if they pre-process repo data without cloning it.
        //
        // In addition, note that Box::leak() shouldn't be used internally as that leaks a
        // pointer per call (or require hacks anyway to drop the leaked reference) causing
        // issues with long running services.
        //
        // An alternative would be to externally leak the references outside potential
        // loops with a passed in static reference; however, that makes the API worse and
        // this workaround shouldn't cause any issues to make that pain worth it.
        let repo: &'static Repo = unsafe { mem::transmute(repo) };

        // return early for static, non-matching restriction
        let restrict = restrict.into();
        if restrict == Restrict::False {
            return Box::new(iter::empty());
        }

        let scan_scope = Scope::from(&restrict);
        info!("scan scope: {scan_scope}");

        match repo {
            Repo::Ebuild(repo) => {
                let runner = Arc::new(SyncCheckRunner::new(repo, &self.filters, &self.checks));
                if scan_scope >= Scope::Category {
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

                    Box::new(IterPkg {
                        reports_rx,
                        _producer: pkg_producer(repo, restrict, restrict_tx),
                        _workers: (0..self.jobs)
                            .map(|_| {
                                pkg_worker(runner.clone(), filter.clone(), restrict_rx.clone())
                            })
                            .collect(),
                        reports: Default::default(),
                    })
                } else {
                    // parallel by check
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

                    Box::new(IterVersion {
                        reports_rx,
                        _producer: version_producer(repo, runner.clone(), restrict, restrict_tx),
                        _workers: (0..self.jobs)
                            .map(|_| {
                                version_worker(runner.clone(), filter.clone(), restrict_rx.clone())
                            })
                            .collect(),
                        reports: Default::default(),
                        finished: false,
                    })
                }
            }
            _ => todo!("add support for other repo types"),
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
    restrict: Restrict,
    tx: Sender<Target>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for cpn in repo.iter_cpn_restrict(&restrict) {
            tx.send(Target::Cpn(cpn)).ok();
        }
    })
}

/// Create worker thread that parallelizes check running at a package level.
fn pkg_worker(
    runner: Arc<SyncCheckRunner>,
    mut filter: ReportFilter,
    rx: Receiver<Target>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for target in rx {
            runner.run(target, &mut filter);
            filter.process();
        }
    })
}

struct IterPkg {
    reports_rx: Receiver<Vec<Report>>,
    _producer: thread::JoinHandle<()>,
    _workers: Vec<thread::JoinHandle<()>>,
    reports: VecDeque<Report>,
}

impl Iterator for IterPkg {
    type Item = Report;

    fn next(&mut self) -> Option<Self::Item> {
        self.reports.pop_front().or_else(|| {
            self.reports_rx.recv().ok().and_then(|reports| {
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
            for check in runner.checks(Scope::Version) {
                tx.send((check, Target::Cpv(cpv.clone()))).ok();
            }
        }

        // TODO: re-use object sets generated by versioned checks
        for cpn in repo.iter_cpn_restrict(&restrict) {
            for check in runner.checks(Scope::Package) {
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
    thread::spawn(move || {
        for (check, target) in rx {
            runner.run_check(check, target, &mut filter);
        }
    })
}

struct IterVersion {
    reports_rx: Receiver<Report>,
    _producer: thread::JoinHandle<()>,
    _workers: Vec<thread::JoinHandle<()>>,
    reports: Vec<Report>,
    finished: bool,
}

impl Iterator for IterVersion {
    type Item = Report;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.finished {
            self.reports.extend(&self.reports_rx);
            self.reports.sort_by(|r1, r2| r2.cmp(r1));
            self.finished = true;
        }

        self.reports.pop()
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::dep::Dep;
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{assert_unordered_eq, test_data};

    use crate::check::CheckKind;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn run() {
        let data = test_data();
        let repo = data.repo("qa-primary").unwrap();
        let repo_path = repo.path();

        // repo target
        let scanner = Scanner::new();
        let expected = glob_reports!("{repo_path}/**/reports.json");
        let reports = scanner.run(repo, repo);
        assert_unordered_eq!(reports, expected);

        // category target
        let scanner = Scanner::new();
        let expected = glob_reports!("{repo_path}/Keywords/**/reports.json");
        let restrict = repo.restrict_from_path("Keywords").unwrap();
        let reports = scanner.run(repo, restrict);
        assert_unordered_eq!(reports, expected);

        // package target
        let scanner = Scanner::new();
        let expected = glob_reports!("{repo_path}/Keywords/KeywordsLive/**/reports.json");
        let restrict = repo.restrict_from_path("Keywords/KeywordsLive").unwrap();
        let reports = scanner.run(repo, restrict);
        assert_unordered_eq!(reports, expected);

        // version target
        let scanner = Scanner::new();
        let expected = glob_reports!("{repo_path}/Keywords/KeywordsLive/**/reports.json");
        let restrict = repo
            .restrict_from_path("Keywords/KeywordsLive/KeywordsLive-9999.ebuild")
            .unwrap();
        let reports = scanner.run(repo, restrict);
        assert_unordered_eq!(reports, expected);

        // specific checks
        let scanner = Scanner::new().checks([CheckKind::Dependency]);
        let expected = glob_reports!("{repo_path}/Dependency/**/reports.json");
        let reports = scanner.run(repo, repo);
        assert_unordered_eq!(reports, expected);

        // specific reports
        let scanner = Scanner::new().reports([ReportKind::DependencyDeprecated]);
        let expected = glob_reports!("{repo_path}/Dependency/DependencyDeprecated/reports.json");
        let reports = scanner.run(repo, repo);
        assert_unordered_eq!(reports, expected);

        // no checks
        let checks: [Check; 0] = [];
        let scanner = Scanner::new().checks(checks);
        let reports = scanner.run(repo, repo);
        assert_unordered_eq!(reports, []);

        // no reports
        let scanner = Scanner::new().reports([]);
        let reports = scanner.run(repo, repo);
        assert_unordered_eq!(reports, []);

        // non-matching restriction
        let scanner = Scanner::new();
        let dep = Dep::try_new("nonexistent/pkg").unwrap();
        let reports = scanner.run(repo, &dep);
        assert_unordered_eq!(reports, []);

        // repo with bad metadata
        let repo = data.repo("bad").unwrap();
        let repo_path = repo.path();
        let expected = glob_reports!("{repo_path}/**/reports.json");
        let reports = scanner.run(repo, repo);
        assert_unordered_eq!(reports, expected);

        // empty repo
        let repo = data.repo("empty").unwrap();
        let reports = scanner.run(repo, repo);
        assert_unordered_eq!(reports, []);
    }

    #[test]
    fn failed() {
        let data = test_data();
        let repo = data.repo("qa-primary").unwrap();

        // no reports flagged for failures
        let scanner = Scanner::new();
        scanner.run(repo, repo).count();
        assert!(!scanner.failed());

        // fail on specified report variant
        let scanner = Scanner::new().exit([ReportKind::DependencyDeprecated]);
        scanner.run(repo, repo).count();
        assert!(scanner.failed());
    }
}
