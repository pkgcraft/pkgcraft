use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::{mem, thread};

use crossbeam_channel::{bounded, Receiver, Sender};
use crossbeam_utils::sync::WaitGroup;
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::repo::{ebuild::EbuildRepo, PkgRepository};
use pkgcraft::restrict::{Restrict, Scope};

use crate::check::Check;
use crate::report::{Report, ReportKind};
use crate::runner::SyncCheckRunner;
use crate::scan::Scanner;
use crate::source::Target;

#[derive(Clone)]
enum ReportSender {
    Pkg(Sender<Vec<Report>>),
    Version(Sender<Report>),
}

impl From<Sender<Vec<Report>>> for ReportSender {
    fn from(value: Sender<Vec<Report>>) -> Self {
        Self::Pkg(value)
    }
}

impl From<Sender<Report>> for ReportSender {
    fn from(value: Sender<Report>) -> Self {
        Self::Version(value)
    }
}

#[derive(Clone)]
pub(crate) struct ReportFilter {
    scope: Scope,
    reports: Vec<Report>,
    filter: Arc<IndexSet<ReportKind>>,
    exit: Arc<IndexSet<ReportKind>>,
    failed: Arc<AtomicBool>,
    tx: ReportSender,
    finalize: HashSet<ReportKind>,
}

impl ReportFilter {
    fn new<S: Into<ReportSender>>(scope: Scope, scanner: &Scanner, tx: S) -> Self {
        Self {
            scope,
            reports: Default::default(),
            filter: scanner.reports.clone(),
            exit: scanner.exit.clone(),
            failed: scanner.failed.clone(),
            tx: tx.into(),
            finalize: scanner
                .reports
                .iter()
                .filter(|r| scanner.filters.is_empty() && scope >= r.scope())
                .copied()
                .collect(),
        }
    }

    /// Conditionally add a report based on filter inclusion.
    pub(crate) fn report(&mut self, report: Report) {
        if self.filter.contains(report.kind()) {
            if self.exit.contains(report.kind()) {
                self.failed.store(true, Ordering::Relaxed);
            }

            if let ReportSender::Version(tx) = &self.tx {
                tx.send(report).ok();
            } else {
                self.reports.push(report);
            }
        }
    }

    /// Sort existing reports and send them to the iterator.
    fn process(&mut self) {
        if !self.reports.is_empty() {
            if let ReportSender::Pkg(tx) = &self.tx {
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
        self.finalize.contains(&kind)
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
    wg: WaitGroup,
    restrict: Restrict,
    tx: Sender<(Check, Target)>,
    finish_tx: Sender<Check>,
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

        // wait for all parallelized checks to finish
        drop(tx);
        wg.wait();

        // finalize checks in parallel
        for check in runner.checks().filter(|c| c.finalize()) {
            finish_tx.send(check).ok();
        }
    })
}

/// Create worker thread that parallelizes check running at a version level.
fn version_worker(
    runner: Arc<SyncCheckRunner>,
    wg: WaitGroup,
    mut filter: ReportFilter,
    rx: Receiver<(Check, Target)>,
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
            runner.run_check(check, target, &mut filter);
        }

        // signal the wait group
        drop(wg);

        // finalize checks
        for check in finish_rx {
            runner.finish(check, &mut filter);
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
    pub(crate) fn try_new<I>(
        scope: Scope,
        checks: I,
        scanner: &Scanner,
        restrict: Restrict,
    ) -> crate::Result<Self>
    where
        I: IntoIterator<Item = crate::Result<Check>>,
    {
        if scope >= Scope::Category {
            Self::pkg(scope, checks, scanner, restrict)
        } else {
            Self::version(scope, checks, scanner, restrict)
        }
    }

    /// Create an iterator that parallelizes scanning by package.
    fn pkg<I>(
        scope: Scope,
        checks: I,
        scanner: &Scanner,
        restrict: Restrict,
    ) -> crate::Result<Self>
    where
        I: IntoIterator<Item = crate::Result<Check>>,
    {
        let (targets_tx, targets_rx) = bounded(scanner.jobs);
        let (finish_tx, finish_rx) = bounded(scanner.jobs);
        let (reports_tx, reports_rx) = bounded(scanner.jobs);
        let wg = WaitGroup::new();
        let filter = ReportFilter::new(scope, scanner, reports_tx);

        let runner =
            Arc::new(SyncCheckRunner::try_new(scope, scanner, &restrict, checks, &filter)?);

        Ok(Self(ReportIterInternal::Pkg(IterPkg {
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
        })))
    }

    /// Create an iterator that parallelizes scanning by check.
    fn version<I>(
        scope: Scope,
        checks: I,
        scanner: &Scanner,
        restrict: Restrict,
    ) -> crate::Result<Self>
    where
        I: IntoIterator<Item = crate::Result<Check>>,
    {
        let (targets_tx, targets_rx) = bounded(scanner.jobs);
        let (finish_tx, finish_rx) = bounded(scanner.jobs);
        let (reports_tx, reports_rx) = bounded(scanner.jobs);
        let wg = WaitGroup::new();
        let filter = ReportFilter::new(scope, scanner, reports_tx);

        let runner =
            Arc::new(SyncCheckRunner::try_new(scope, scanner, &restrict, checks, &filter)?);

        Ok(Self(ReportIterInternal::Version(IterVersion {
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
        })))
    }
}

impl Iterator for ReportIter {
    type Item = Report;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}
