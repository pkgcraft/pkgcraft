use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use crossbeam_channel::{bounded, Receiver, RecvError, Sender};
use crossbeam_utils::sync::WaitGroup;
use itertools::Itertools;
use pkgcraft::dep::Cpn;
use pkgcraft::repo::{ebuild::EbuildRepo, PkgRepository};
use pkgcraft::restrict::{Restrict, Scope};

use crate::check::Check;
use crate::ignore::Ignore;
use crate::report::{Report, ReportKind, ReportScope};
use crate::runner::{SyncCheckRunner, Target};
use crate::scan::Scanner;

enum ReportOrProcess {
    Report(Report),
    Process(Cpn),
}

impl From<Report> for ReportOrProcess {
    fn from(value: Report) -> Self {
        Self::Report(value)
    }
}

impl From<Cpn> for ReportOrProcess {
    fn from(value: Cpn) -> Self {
        Self::Process(value)
    }
}

enum ReportSender {
    Pkg(Sender<ReportOrProcess>),
    Version(Sender<Report>),
}

impl ReportSender {
    /// Process a single report.
    fn report(&self, report: Report) {
        match self {
            Self::Version(tx) => {
                tx.send(report).ok();
            }
            Self::Pkg(tx) => {
                tx.send(report.into()).ok();
            }
        }
    }

    /// Process all reports for a package.
    fn process(&self, cpn: Cpn) {
        if let Self::Pkg(tx) = self {
            tx.send(cpn.into()).ok();
        }
    }
}

impl From<Sender<ReportOrProcess>> for ReportSender {
    fn from(value: Sender<ReportOrProcess>) -> Self {
        Self::Pkg(value)
    }
}

impl From<Sender<Report>> for ReportSender {
    fn from(value: Sender<Report>) -> Self {
        Self::Version(value)
    }
}

pub(crate) struct ReportFilter {
    enabled: HashSet<ReportKind>,
    exit: HashSet<ReportKind>,
    failed: Arc<AtomicBool>,
    sender: ReportSender,
    force: bool,
    pub(crate) ignore: Ignore,
}

impl ReportFilter {
    fn new<S: Into<ReportSender>>(
        enabled: HashSet<ReportKind>,
        exit: HashSet<ReportKind>,
        scanner: &Scanner,
        tx: S,
    ) -> Self {
        Self {
            enabled,
            exit,
            failed: scanner.failed.clone(),
            sender: tx.into(),
            force: scanner.force,
            ignore: Ignore::new(&scanner.repo),
        }
    }

    /// Conditionally add a report based on filter inclusion.
    pub(crate) fn report(&self, report: Report) {
        let kind = report.kind;
        if self.enabled(kind) && (self.force || !self.ignore.ignored(&report)) {
            if self.exit.contains(&kind) {
                self.failed.store(true, Ordering::Relaxed);
            }

            self.sender.report(report);
        }
    }

    /// Return true if the filter has a report variant enabled.
    pub(crate) fn enabled(&self, kind: ReportKind) -> bool {
        self.enabled.contains(&kind)
    }
}

/// Create a producer thread that sends package targets over a channel to workers.
fn pkg_producer(
    repo: EbuildRepo,
    runner: Arc<SyncCheckRunner>,
    wg: WaitGroup,
    restrict: Restrict,
    scope: Scope,
    tx: Sender<(Option<Check>, Target)>,
    finish_tx: Sender<Check>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        // run non-package checks in parallel
        for check in runner.checks().filter(|c| c.scope() > Scope::Package) {
            tx.send((Some(check), Target::Repo)).ok();
        }

        // parallelize running checks per package
        if runner.checks().any(|c| c.scope() <= Scope::Package) {
            for cpn in repo.iter_cpn_restrict(&restrict) {
                tx.send((None, Target::Cpn(cpn))).ok();
            }
        }

        // wait for all parallelized checks to finish
        drop(tx);
        wg.wait();

        // finalize checks in parallel
        for check in runner.checks().filter(|c| c.finish_check(scope)) {
            finish_tx.send(check).ok();
        }
    })
}

/// Create worker thread that parallelizes check running at a package level.
fn pkg_worker(
    runner: Arc<SyncCheckRunner>,
    wg: WaitGroup,
    filter: Arc<ReportFilter>,
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

        for (check, target) in rx {
            match check {
                Some(check) => runner.run_check(check, &target, &filter),
                None => runner.run_checks(&target, &filter),
            }

            // signal iterator to process results for target package
            if let Target::Cpn(cpn) = target {
                filter.sender.process(cpn);
            }
        }

        // signal the wait group
        drop(wg);

        // finalize checks
        for check in finish_rx {
            runner.finish_check(check, &filter);
        }
    })
}

/// Iterator that parallelizes by package, running in category and repo scope.
#[derive(Debug)]
struct IterPkg {
    rx: Receiver<ReportOrProcess>,
    _producer: thread::JoinHandle<()>,
    _workers: Vec<thread::JoinHandle<()>>,
    cache: HashMap<Cpn, Vec<Report>>,
    reports: VecDeque<Report>,
}

impl IterPkg {
    /// Process items from the reports channel.
    fn receive(&mut self) -> Result<(), RecvError> {
        self.rx.recv().map(|value| match value {
            ReportOrProcess::Report(report) => {
                let cached = report.kind.scope() <= Scope::Package;
                match report.scope() {
                    ReportScope::Version(cpv, _) if cached => {
                        self.cache
                            .entry(cpv.cpn().clone())
                            .or_default()
                            .push(report);
                    }
                    ReportScope::Package(cpn) if cached => {
                        self.cache.entry(cpn.clone()).or_default().push(report);
                    }
                    _ => self.reports.push_back(report),
                }
            }
            ReportOrProcess::Process(cpn) => {
                if let Some(mut reports) = self.cache.remove(&cpn) {
                    reports.sort();
                    self.reports.extend(reports);
                }
            }
        })
    }
}

impl Iterator for IterPkg {
    type Item = Report;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(report) = self.reports.pop_front() {
                return Some(report);
            } else if self.receive().is_err() {
                return None;
            }
        }
    }
}

/// Create a producer thread that sends checks with targets over a channel to workers.
fn version_producer(
    repo: EbuildRepo,
    runner: Arc<SyncCheckRunner>,
    wg: WaitGroup,
    restrict: Restrict,
    tx: Sender<(Check, Target)>,
    finish_tx: Sender<(Check, Target)>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for cpv in repo.iter_cpv_restrict(&restrict) {
            for check in runner.checks().filter(|c| c.scope() == Scope::Version) {
                tx.send((check, Target::Cpv(cpv.clone()))).ok();
            }
        }

        for cpn in repo.iter_cpn_restrict(&restrict) {
            for check in runner.checks().filter(|c| c.scope() == Scope::Package) {
                tx.send((check, Target::Cpn(cpn.clone()))).ok();
            }
        }

        // wait for all parallelized checks to finish
        drop(tx);
        wg.wait();

        for cpv in repo.iter_cpv_restrict(&restrict) {
            for check in runner.checks().filter(|c| c.finish_target()) {
                finish_tx.send((check, Target::Cpv(cpv.clone()))).ok();
            }
        }

        for cpn in repo.iter_cpn_restrict(&restrict) {
            for check in runner.checks().filter(|c| c.finish_target()) {
                finish_tx.send((check, Target::Cpn(cpn.clone()))).ok();
            }
        }
    })
}

/// Create worker thread that parallelizes check running at a version level.
fn version_worker(
    runner: Arc<SyncCheckRunner>,
    wg: WaitGroup,
    filter: Arc<ReportFilter>,
    rx: Receiver<(Check, Target)>,
    finish_rx: Receiver<(Check, Target)>,
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
            runner.run_check(check, &target, &filter);
        }

        // signal the wait group
        drop(wg);

        // run finalize methods for targets
        for (check, target) in finish_rx {
            runner.finish_target(check, &target, &filter);
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
    pub(crate) fn new<I>(
        enabled: HashSet<ReportKind>,
        exit: HashSet<ReportKind>,
        scope: Scope,
        checks: I,
        scanner: &Scanner,
        restrict: Restrict,
    ) -> Self
    where
        I: IntoIterator<Item = Check>,
    {
        if scope >= Scope::Category {
            Self::pkg(enabled, exit, scope, checks, scanner, restrict)
        } else {
            Self::version(enabled, exit, scope, checks, scanner, restrict)
        }
    }

    /// Create an iterator that parallelizes scanning by package.
    fn pkg<I>(
        enabled: HashSet<ReportKind>,
        exit: HashSet<ReportKind>,
        scope: Scope,
        checks: I,
        scanner: &Scanner,
        restrict: Restrict,
    ) -> Self
    where
        I: IntoIterator<Item = Check>,
    {
        let (targets_tx, targets_rx) = bounded(scanner.jobs);
        let (finish_tx, finish_rx) = bounded(scanner.jobs);
        let (reports_tx, reports_rx) = bounded(scanner.jobs);
        let wg = WaitGroup::new();
        let filter = Arc::new(ReportFilter::new(enabled, exit, scanner, reports_tx));

        let runner =
            Arc::new(SyncCheckRunner::new(scope, scanner, &restrict, checks, &filter));

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
                scope,
                targets_tx,
                finish_tx,
            ),
            cache: Default::default(),
            reports: Default::default(),
        }))
    }

    /// Create an iterator that parallelizes scanning by check.
    fn version<I>(
        enabled: HashSet<ReportKind>,
        exit: HashSet<ReportKind>,
        scope: Scope,
        checks: I,
        scanner: &Scanner,
        restrict: Restrict,
    ) -> Self
    where
        I: IntoIterator<Item = Check>,
    {
        let (targets_tx, targets_rx) = bounded(scanner.jobs);
        let (finish_tx, finish_rx) = bounded(scanner.jobs);
        let (reports_tx, reports_rx) = bounded(scanner.jobs);
        let wg = WaitGroup::new();
        let filter = Arc::new(ReportFilter::new(enabled, exit, scanner, reports_tx));

        let runner =
            Arc::new(SyncCheckRunner::new(scope, scanner, &restrict, checks, &filter));

        Self(ReportIterInternal::Version(IterVersion {
            rx: reports_rx,
            _workers: (0..scanner.jobs)
                .map(|_| {
                    version_worker(
                        runner.clone(),
                        wg.clone(),
                        filter.clone(),
                        targets_rx.clone(),
                        finish_rx.clone(),
                    )
                })
                .collect(),
            _producer: version_producer(
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
}

impl Iterator for ReportIter {
    type Item = Report;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}
