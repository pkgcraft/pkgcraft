use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use camino::Utf8PathBuf;
use crossbeam_channel::{bounded, Receiver, RecvError, Sender};
use crossbeam_utils::sync::WaitGroup;
use dashmap::DashMap;
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::dep::Cpn;
use pkgcraft::repo::{ebuild::EbuildRepo, PkgRepository};
use pkgcraft::restrict::{Restrict, Scope};

use crate::check::Check;
use crate::report::{Report, ReportKind, ReportScope, ReportSet};
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

struct IgnorePaths<'a> {
    report: &'a Report,
    scope: Option<Scope>,
}

impl<'a> IgnorePaths<'a> {
    fn new(report: &'a Report) -> Self {
        Self {
            report,
            scope: Some(Scope::Repo),
        }
    }
}

impl Iterator for IgnorePaths<'_> {
    type Item = (Scope, Utf8PathBuf);

    fn next(&mut self) -> Option<Self::Item> {
        self.scope.map(|scope| {
            // construct the relative path to check for ignore files
            let relpath = match (scope, self.report.scope()) {
                (Scope::Category, ReportScope::Category(category)) => category.into(),
                (Scope::Category, ReportScope::Package(cpn)) => cpn.category().into(),
                (Scope::Category, ReportScope::Version(cpv, _)) => cpv.category().into(),
                (Scope::Package, ReportScope::Package(cpn)) => cpn.to_string().into(),
                (Scope::Package, ReportScope::Version(cpv, _)) => cpv.cpn().to_string().into(),
                (Scope::Version, ReportScope::Version(cpv, _)) => cpv.relpath(),
                _ => Default::default(),
            };

            // set the scope to the next lower level
            self.scope = match scope {
                Scope::Repo => Some(Scope::Category),
                Scope::Category => Some(Scope::Package),
                Scope::Package => Some(Scope::Version),
                Scope::Version => None,
            };

            (scope, relpath)
        })
    }
}

pub(crate) struct ReportFilter {
    filter: HashSet<ReportKind>,
    exit: Arc<HashSet<ReportKind>>,
    failed: Arc<AtomicBool>,
    sender: ReportSender,
    force: bool,
    ignore: DashMap<Utf8PathBuf, IndexSet<ReportKind>>,
    default: Arc<IndexSet<ReportKind>>,
    supported: Arc<IndexSet<ReportKind>>,
    repo: EbuildRepo,
}

impl ReportFilter {
    #[allow(clippy::nonminimal_bool)]
    fn new<S: Into<ReportSender>>(
        scope: Scope,
        filtered: bool,
        scanner: &Scanner,
        tx: S,
    ) -> Self {
        Self {
            // TODO: move report filtering into Scanner::run()
            filter: scanner
                .reports
                .iter()
                .filter(|r| {
                    let finalized = r.finalize(scope);
                    (finalized && !filtered)
                        || (filtered && r.scope() <= Scope::Package)
                        || (!finalized && !filtered && scope >= r.scope())
                })
                .copied()
                .collect(),
            exit: scanner.exit.clone(),
            failed: scanner.failed.clone(),
            sender: tx.into(),
            force: scanner.force,
            ignore: Default::default(),
            default: scanner.default.clone(),
            supported: scanner.supported.clone(),
            repo: scanner.repo.clone(),
        }
    }

    /// Determine if a report is ignored via any relevant ignore files.
    fn ignored(&self, report: &Report) -> bool {
        IgnorePaths::new(report).any(|(scope, relpath)| {
            self.ignore
                .entry(relpath.clone())
                .or_insert_with(|| {
                    let path = self.repo.path().join(relpath);
                    if scope == Scope::Version {
                        // TODO: use BufRead to avoid loading the entire ebuild file?
                        let mut ignore = IndexSet::new();
                        for line in fs::read_to_string(path).unwrap_or_default().lines() {
                            let line = line.trim();
                            if let Some(data) = line.strip_prefix("# pkgcruft-ignore: ") {
                                ignore.extend(
                                    data.split_whitespace()
                                        .filter_map(|x| x.parse::<ReportSet>().ok())
                                        .flat_map(|x| {
                                            x.expand(&self.default, &self.supported)
                                        }),
                                )
                            } else if !line.is_empty() && !line.starts_with("#") {
                                break;
                            }
                        }
                        ignore
                    } else {
                        fs::read_to_string(path.join(".pkgcruft-ignore"))
                            .unwrap_or_default()
                            .lines()
                            .filter_map(|x| x.parse::<ReportSet>().ok())
                            .flat_map(|x| x.expand(&self.default, &self.supported))
                            .collect()
                    }
                })
                .contains(&report.kind)
        })
    }

    /// Conditionally add a report based on filter inclusion.
    pub(crate) fn report(&self, report: Report) {
        if self.filter.contains(&report.kind) && (self.force || !self.ignored(&report)) {
            if self.exit.contains(&report.kind) {
                self.failed.store(true, Ordering::Relaxed);
            }

            self.sender.report(report);
        }
    }

    /// Return true if the filter has a report variant enabled.
    pub(crate) fn enabled(&self, kind: ReportKind) -> bool {
        self.filter.contains(&kind)
    }
}

/// Create a producer thread that sends package targets over a channel to workers.
fn pkg_producer(
    repo: EbuildRepo,
    runner: Arc<SyncCheckRunner>,
    wg: WaitGroup,
    scope: Scope,
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
        for check in runner.checks().filter(|c| c.finalize(scope)) {
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
            runner.finish(check, &filter);
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
    scope: Scope,
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
        for check in runner.checks().filter(|c| c.finalize(scope)) {
            finish_tx.send(check).ok();
        }
    })
}

/// Create worker thread that parallelizes check running at a version level.
fn version_worker(
    runner: Arc<SyncCheckRunner>,
    wg: WaitGroup,
    filter: Arc<ReportFilter>,
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
            runner.run_check(check, &target, &filter);
        }

        // signal the wait group
        drop(wg);

        // finalize checks
        for check in finish_rx {
            runner.finish(check, &filter);
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
        scope: Scope,
        checks: I,
        scanner: &Scanner,
        restrict: Restrict,
    ) -> Self
    where
        I: IntoIterator<Item = Check>,
    {
        // determine if any package filtering is enabled
        let filtered = restrict != Restrict::True || !scanner.filters.is_empty();

        if scope >= Scope::Category {
            Self::pkg(scope, checks, scanner, restrict, filtered)
        } else {
            Self::version(scope, checks, scanner, restrict, filtered)
        }
    }

    /// Create an iterator that parallelizes scanning by package.
    fn pkg<I>(
        scope: Scope,
        checks: I,
        scanner: &Scanner,
        restrict: Restrict,
        filtered: bool,
    ) -> Self
    where
        I: IntoIterator<Item = Check>,
    {
        let (targets_tx, targets_rx) = bounded(scanner.jobs);
        let (finish_tx, finish_rx) = bounded(scanner.jobs);
        let (reports_tx, reports_rx) = bounded(scanner.jobs);
        let wg = WaitGroup::new();
        let filter = Arc::new(ReportFilter::new(scope, filtered, scanner, reports_tx));

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
                scope,
                restrict,
                targets_tx,
                finish_tx,
            ),
            cache: Default::default(),
            reports: Default::default(),
        }))
    }

    /// Create an iterator that parallelizes scanning by check.
    fn version<I>(
        scope: Scope,
        checks: I,
        scanner: &Scanner,
        restrict: Restrict,
        filtered: bool,
    ) -> Self
    where
        I: IntoIterator<Item = Check>,
    {
        let (targets_tx, targets_rx) = bounded(scanner.jobs);
        let (finish_tx, finish_rx) = bounded(scanner.jobs);
        let (reports_tx, reports_rx) = bounded(scanner.jobs);
        let wg = WaitGroup::new();
        let filter = Arc::new(ReportFilter::new(scope, filtered, scanner, reports_tx));

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
                scope,
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
