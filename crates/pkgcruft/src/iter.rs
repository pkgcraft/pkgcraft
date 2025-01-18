use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::{mem, thread};

use crossbeam_channel::{bounded, Receiver, RecvError, Sender};
use crossbeam_utils::sync::WaitGroup;
use itertools::Itertools;
use pkgcraft::repo::PkgRepository;
use pkgcraft::restrict::Scope;

use crate::check::Check;
use crate::report::Report;
use crate::runner::{CheckRunner, SyncCheckRunner, Target};
use crate::scan::ScannerRun;

/// Item sent to the report iterator for processing.
#[derive(Debug)]
pub(crate) enum Item {
    Report(Report),
    Process(Target, usize),
    Finish(thread::ThreadId),
}

#[derive(Debug)]
pub(crate) struct ReportSender(Sender<Item>);

impl ReportSender {
    /// Send a single report.
    pub(crate) fn report(&self, report: Report) {
        self.0.send(Item::Report(report)).ok();
    }

    /// Direct the iterator to process all reports for a target.
    fn process(&self, target: Target, id: usize) {
        self.0.send(Item::Process(target, id)).ok();
    }

    /// Notify the iterator that a worker thread finished.
    fn finish(&self) {
        self.0.send(Item::Finish(thread::current().id())).ok();
    }
}

/// Create a producer thread that sends package targets over a channel to workers.
fn pkg_producer(
    run: Arc<ScannerRun>,
    wg: WaitGroup,
    tx: Sender<(Option<Check>, Target, usize)>,
    finish_tx: Sender<Check>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        // run non-package checks in parallel
        for check in run.checks.iter().filter(|c| c.scope() > Scope::Package) {
            tx.send((Some(*check), Target::Repo, 0)).ok();
        }

        // parallelize running checks per package
        if run.checks.iter().any(|c| c.scope() <= Scope::Package) {
            for (id, cpn) in run.repo.iter_cpn_restrict(&run.restrict).enumerate() {
                tx.send((None, cpn.into(), id)).ok();
            }
        }

        // wait for all parallelized checks to finish
        drop(tx);
        wg.wait();

        // finalize checks in parallel
        for check in run.checks.iter().filter(|c| c.finish_check(run.scope)) {
            finish_tx.send(*check).ok();
        }

        // signal iterator on thread completion
        run.sender().finish();
    })
}

/// Create worker thread that parallelizes check running at a package level.
fn pkg_worker(
    run: Arc<ScannerRun>,
    runner: Arc<SyncCheckRunner>,
    wg: WaitGroup,
    rx: Receiver<(Option<Check>, Target, usize)>,
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

        for (check, target, id) in rx {
            if let Target::Cpn(cpn) = &target {
                runner.run_checks(cpn, &run);
                // signal iterator to process reports for target package
                run.sender().process(target, id);
            } else if let Some(check) = check {
                runner.run_check(&check, &target, &run);
            }
        }

        // signal the finish wait group
        drop(wg);

        // finalize checks
        for check in finish_rx {
            runner.finish_check(&check, &run);
        }

        // signal iterator on thread completion
        run.sender().finish();
    })
}

/// Create a producer thread that sends checks with targets over a channel to workers.
fn version_producer(
    run: Arc<ScannerRun>,
    wg: WaitGroup,
    tx: Sender<(Check, Target)>,
    finish_tx: Sender<(Check, Target)>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let cpvs: Vec<_> = run.repo.iter_cpv_restrict(&run.restrict).collect();
        let cpns: Vec<_> = run.repo.iter_cpn_restrict(&run.restrict).collect();

        for cpv in &cpvs {
            for check in run.checks.iter().filter(|c| c.scope() == Scope::Version) {
                tx.send((*check, cpv.clone().into())).ok();
            }
        }

        for cpn in &cpns {
            for check in run.checks.iter().filter(|c| c.scope() == Scope::Package) {
                tx.send((*check, cpn.clone().into())).ok();
            }
        }

        // wait for all parallelized checks to finish
        drop(tx);
        wg.wait();

        for cpv in &cpvs {
            for check in run.checks.iter().filter(|c| c.finish_target()) {
                finish_tx.send((*check, cpv.clone().into())).ok();
            }
        }

        for cpn in &cpns {
            for check in run.checks.iter().filter(|c| c.finish_target()) {
                finish_tx.send((*check, cpn.clone().into())).ok();
            }
        }

        // signal iterator on thread completion
        run.sender().finish();
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

        // signal the finish wait group
        drop(wg);

        // run finalize methods for targets
        for (check, target) in finish_rx {
            runner.finish_target(&check, &target, &run);
        }

        // signal iterator on thread completion
        run.sender().finish();
    })
}

/// Iterator of reports.
#[derive(Debug)]
pub struct ReportIter {
    rx: Receiver<Item>,
    threads: HashMap<thread::ThreadId, thread::JoinHandle<()>>,
    id: usize,
    sort: bool,
    target_cache: HashMap<Target, Vec<Report>>,
    id_cache: HashMap<usize, Vec<Report>>,
    reports: VecDeque<Report>,
}

impl ReportIter {
    pub(crate) fn new(run: Arc<ScannerRun>) -> Self {
        // inject report sender into aggregate type
        let (reports_tx, reports_rx) = bounded(run.jobs);
        run.sender
            .set(ReportSender(reports_tx))
            .expect("failed setting sender");

        let wg = WaitGroup::new();
        let runner = Arc::new(SyncCheckRunner::new(&run));

        // create worker and producer threads depending on run scope
        let mut threads = HashMap::new();
        if run.scope >= Scope::Category {
            let (targets_tx, targets_rx) = bounded(run.jobs);
            let (finish_tx, finish_rx) = bounded(run.jobs);
            threads.extend(
                (0..run.jobs)
                    .map(|_| {
                        pkg_worker(
                            run.clone(),
                            runner.clone(),
                            wg.clone(),
                            targets_rx.clone(),
                            finish_rx.clone(),
                        )
                    })
                    .map(|h| (h.thread().id(), h)),
            );
            let producer = pkg_producer(run.clone(), wg, targets_tx, finish_tx);
            threads.insert(producer.thread().id(), producer);
        } else {
            let (targets_tx, targets_rx) = bounded(run.jobs);
            let (finish_tx, finish_rx) = bounded(run.jobs);
            threads.extend(
                (0..run.jobs)
                    .map(|_| {
                        version_worker(
                            run.clone(),
                            runner.clone(),
                            wg.clone(),
                            targets_rx.clone(),
                            finish_rx.clone(),
                        )
                    })
                    .map(|h| (h.thread().id(), h)),
            );
            let producer = version_producer(run.clone(), wg, targets_tx, finish_tx);
            threads.insert(producer.thread().id(), producer);
        }

        Self {
            rx: reports_rx,
            threads,
            id: Default::default(),
            sort: run.sort,
            target_cache: Default::default(),
            id_cache: Default::default(),
            reports: Default::default(),
        }
    }

    /// Process items from the reports channel.
    fn receive(&mut self) -> Result<(), RecvError> {
        self.rx.recv().map(|value| match value {
            Item::Report(report) => {
                self.target_cache
                    .entry(report.scope().into())
                    .or_default()
                    .push(report);
            }
            Item::Process(target, id) => {
                let mut reports = self.target_cache.remove(&target).unwrap_or_default();
                reports.sort();
                if self.sort {
                    self.id_cache.insert(id, reports);
                } else if !reports.is_empty() {
                    self.reports.extend(reports);
                }
            }
            Item::Finish(id) => {
                let thread = self
                    .threads
                    .remove(&id)
                    .unwrap_or_else(|| panic!("unknown thread: {id:?}"));
                thread.join().unwrap();

                // flush remaining cached reports when all threads are complete
                if self.threads.is_empty() {
                    self.reports
                        .extend(self.target_cache.values_mut().flat_map(mem::take).sorted());
                }
            }
        })
    }
}

impl Iterator for ReportIter {
    type Item = Report;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.sort {
                if let Some(reports) = self.id_cache.remove(&self.id) {
                    self.id += 1;
                    if reports.is_empty() {
                        continue;
                    }
                    self.reports.extend(reports);
                }
            }

            if let Some(report) = self.reports.pop_front() {
                return Some(report);
            } else if self.receive().is_err() {
                return None;
            }
        }
    }
}
