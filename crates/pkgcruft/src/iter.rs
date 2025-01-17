use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::thread;

use crossbeam_channel::{bounded, Receiver, RecvError, Sender};
use crossbeam_utils::sync::WaitGroup;
use itertools::Itertools;
use pkgcraft::dep::Cpn;
use pkgcraft::repo::PkgRepository;
use pkgcraft::restrict::Scope;

use crate::check::Check;
use crate::report::{Report, ReportScope};
use crate::runner::{CheckRunner, SyncCheckRunner, Target};
use crate::scan::ScannerRun;

#[derive(Debug)]
pub(crate) enum ReportOrProcess {
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

#[derive(Debug)]
pub(crate) enum ReportSender {
    Pkg(Sender<ReportOrProcess>),
    Version(Sender<Report>),
}

impl ReportSender {
    /// Process a single report.
    pub(crate) fn report(&self, report: Report) {
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

/// Create a producer thread that sends package targets over a channel to workers.
fn pkg_producer(
    run: Arc<ScannerRun>,
    wg: WaitGroup,
    tx: Sender<(Option<Check>, Target)>,
    finish_tx: Sender<Check>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        // parallelize running checks per package
        if run.checks.iter().any(|c| c.scope() <= Scope::Package) {
            for cpn in run.repo.iter_cpn_restrict(&run.restrict) {
                tx.send((None, Target::Cpn(cpn))).ok();
            }
        }

        // run non-package checks in parallel
        for check in run.checks.iter().filter(|c| c.scope() > Scope::Package) {
            tx.send((Some(*check), Target::Repo)).ok();
        }

        // wait for all parallelized checks to finish
        drop(tx);
        wg.wait();

        // finalize checks in parallel
        for check in run.checks.iter().filter(|c| c.finish_check(run.scope)) {
            finish_tx.send(*check).ok();
        }
    })
}

/// Create worker thread that parallelizes check running at a package level.
fn pkg_worker(
    run: Arc<ScannerRun>,
    runner: Arc<SyncCheckRunner>,
    wg: WaitGroup,
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
            if let Target::Cpn(cpn) = target {
                runner.run_checks(&cpn, &run);
                // signal iterator to process results for target package
                run.sender().process(cpn);
            } else if let Some(check) = check {
                runner.run_check(&check, &target, &run);
            }
        }

        // signal the wait group
        drop(wg);

        // finalize checks
        for check in finish_rx {
            runner.finish_check(&check, &run);
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
    run: Arc<ScannerRun>,
    wg: WaitGroup,
    tx: Sender<(Check, Target)>,
    finish_tx: Sender<(Check, Target)>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for cpv in run.repo.iter_cpv_restrict(&run.restrict) {
            for check in run.checks.iter().filter(|c| c.scope() == Scope::Version) {
                tx.send((*check, Target::Cpv(cpv.clone()))).ok();
            }
        }

        for cpn in run.repo.iter_cpn_restrict(&run.restrict) {
            for check in run.checks.iter().filter(|c| c.scope() == Scope::Package) {
                tx.send((*check, Target::Cpn(cpn.clone()))).ok();
            }
        }

        // wait for all parallelized checks to finish
        drop(tx);
        wg.wait();

        for cpv in run.repo.iter_cpv_restrict(&run.restrict) {
            for check in run.checks.iter().filter(|c| c.finish_target()) {
                finish_tx.send((*check, Target::Cpv(cpv.clone()))).ok();
            }
        }

        for cpn in run.repo.iter_cpn_restrict(&run.restrict) {
            for check in run.checks.iter().filter(|c| c.finish_target()) {
                finish_tx.send((*check, Target::Cpn(cpn.clone()))).ok();
            }
        }
    })
}

/// Create worker thread that parallelizes check running at a version level.
fn version_worker(
    run: Arc<ScannerRun>,
    runner: Arc<SyncCheckRunner>,
    wg: WaitGroup,
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
            runner.run_check(&check, &target, &run);
        }

        // signal the wait group
        drop(wg);

        // run finalize methods for targets
        for (check, target) in finish_rx {
            runner.finish_target(&check, &target, &run);
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
    pub(crate) fn new(run: Arc<ScannerRun>) -> Self {
        if run.scope >= Scope::Category {
            Self::pkg(run)
        } else {
            Self::version(run)
        }
    }

    /// Create an iterator that parallelizes scanning by package.
    fn pkg(run: Arc<ScannerRun>) -> Self {
        let (targets_tx, targets_rx) = bounded(run.jobs);
        let (finish_tx, finish_rx) = bounded(run.jobs);
        let (reports_tx, reports_rx) = bounded(run.jobs);
        run.sender
            .set(reports_tx.into())
            .expect("failed setting sender");
        let wg = WaitGroup::new();
        let runner = Arc::new(SyncCheckRunner::new(&run));

        Self(ReportIterInternal::Pkg(IterPkg {
            rx: reports_rx,
            _workers: (0..run.jobs)
                .map(|_| {
                    pkg_worker(
                        run.clone(),
                        runner.clone(),
                        wg.clone(),
                        targets_rx.clone(),
                        finish_rx.clone(),
                    )
                })
                .collect(),
            _producer: pkg_producer(run.clone(), wg, targets_tx, finish_tx),
            cache: Default::default(),
            reports: Default::default(),
        }))
    }

    /// Create an iterator that parallelizes scanning by check.
    fn version(run: Arc<ScannerRun>) -> Self {
        let (targets_tx, targets_rx) = bounded(run.jobs);
        let (finish_tx, finish_rx) = bounded(run.jobs);
        let (reports_tx, reports_rx) = bounded(run.jobs);
        run.sender
            .set(reports_tx.into())
            .expect("failed setting sender");
        let wg = WaitGroup::new();
        let runner = Arc::new(SyncCheckRunner::new(&run));

        Self(ReportIterInternal::Version(IterVersion {
            rx: reports_rx,
            _workers: (0..run.jobs)
                .map(|_| {
                    version_worker(
                        run.clone(),
                        runner.clone(),
                        wg.clone(),
                        targets_rx.clone(),
                        finish_rx.clone(),
                    )
                })
                .collect(),
            _producer: version_producer(run.clone(), wg, targets_tx, finish_tx),
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
