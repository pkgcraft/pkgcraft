use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use crossbeam_channel::{unbounded, Receiver, Sender};
use indexmap::IndexSet;
use pkgcraft::repo::{ebuild, Repo};
use pkgcraft::restrict::Restrict;
use pkgcraft::utils::bounded_jobs;
use strum::IntoEnumIterator;

use crate::check::Check;
use crate::report::{Report, ReportKind};
use crate::runner::SyncCheckRunner;

#[derive(Debug)]
pub struct Scanner {
    jobs: usize,
    checks: IndexSet<Check>,
    reports: IndexSet<ReportKind>,
    exit: IndexSet<ReportKind>,
    failed: Arc<AtomicBool>,
}

impl Default for Scanner {
    fn default() -> Self {
        Self {
            jobs: bounded_jobs(0),
            checks: Check::iter().collect(),
            reports: ReportKind::iter().collect(),
            exit: Default::default(),
            failed: Arc::new(Default::default()),
        }
    }
}

impl Scanner {
    /// Create a new scanner with all checks enabled.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the number of parallel scanner jobs to run.
    pub fn jobs(mut self, jobs: usize) -> Self {
        self.jobs = bounded_jobs(jobs);
        self
    }

    /// Set the checks to run.
    pub fn checks<I>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = Check>,
    {
        self.checks = values.into_iter().collect();
        self
    }

    /// Set enabled report variants.
    pub fn reports<I, T>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<ReportKind>,
    {
        self.reports = values.into_iter().map(Into::into).collect();
        self
    }

    /// Set report variants that trigger exit code failures.
    pub fn exit<I, T>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<ReportKind>,
    {
        self.exit = values.into_iter().map(Into::into).collect();
        self
    }

    /// Return true if the scanning process failed, false otherwise.
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Relaxed)
    }

    /// Run the scanner returning an iterator of reports.
    pub fn run<I, R>(&self, repo: &Repo, restricts: I) -> impl Iterator<Item = Report>
    where
        I: IntoIterator<Item = R>,
        R: Into<Restrict>,
    {
        match repo {
            Repo::Ebuild(r) => {
                // TODO: drop this hack once lifetime handling is improved for thread usage
                let repo: &'static ebuild::Repo = Box::leak(Box::new(r.clone()));

                let sync_runner = SyncCheckRunner::new(repo).checks(self.checks.iter().copied());
                let (restrict_tx, restrict_rx) = unbounded();
                let (reports_tx, reports_rx) = unbounded();
                let runner = Arc::new(sync_runner);
                let filter = Arc::new(self.reports.clone());
                let exit = Arc::new(self.exit.clone());

                Iter {
                    reports_rx,
                    _producer: Producer::new(repo, restricts, restrict_tx),
                    _workers: Workers::new(
                        self.jobs,
                        &runner,
                        &filter,
                        &exit,
                        &self.failed,
                        &restrict_rx,
                        &reports_tx,
                    ),
                    reports: VecDeque::new(),
                }
            }
            _ => todo!("add support for other repo types"),
        }
    }
}

// TODO: use multiple producers to push restrictions
/// Restriction producer thread that helps parallelize check running.
struct Producer {
    _thread: thread::JoinHandle<()>,
}

impl Producer {
    /// Create a producer that sends restrictions over the channel to the workers.
    fn new<I, R>(repo: &'static ebuild::Repo, restricts: I, tx: Sender<Restrict>) -> Self
    where
        I: IntoIterator<Item = R>,
        R: Into<Restrict>,
    {
        let restricts: Vec<_> = restricts.into_iter().map(|r| r.into()).collect();
        Self {
            _thread: thread::spawn(move || {
                for r in restricts {
                    for cpn in repo.iter_cpn_restrict(r) {
                        tx.send(Restrict::from(&cpn)).ok();
                    }
                }
            }),
        }
    }
}

/// Worker threads that parallelize check running.
struct Workers {
    _threads: Vec<thread::JoinHandle<()>>,
}

impl Workers {
    /// Create workers that receive restrictions and send reports over the channel.
    fn new(
        jobs: usize,
        runner: &Arc<SyncCheckRunner<'static>>,
        filter: &Arc<IndexSet<ReportKind>>,
        exit: &Arc<IndexSet<ReportKind>>,
        failed: &Arc<AtomicBool>,
        rx: &Receiver<Restrict>,
        tx: &Sender<Vec<Report>>,
    ) -> Self {
        Self {
            _threads: (0..jobs)
                .map(|_| {
                    let runner = runner.clone();
                    let filter = filter.clone();
                    let exit = exit.clone();
                    let failed = failed.clone();
                    let rx = rx.clone();
                    let tx = tx.clone();
                    thread::spawn(move || {
                        for restrict in rx {
                            let mut reports = vec![];

                            // report processing callback
                            let mut report = |report: Report| {
                                if filter.contains(report.kind()) {
                                    if exit.contains(report.kind()) {
                                        failed.store(true, Ordering::Relaxed);
                                    }
                                    reports.push(report);
                                }
                            };

                            // run checks
                            runner.run(&restrict, &mut report);

                            // sort and send reports
                            if !reports.is_empty() {
                                reports.sort();
                                tx.send(reports).ok();
                            }
                        }
                    })
                })
                .collect(),
        }
    }
}

struct Iter {
    reports_rx: Receiver<Vec<Report>>,
    _producer: Producer,
    _workers: Workers,
    reports: VecDeque<Report>,
}

impl Iterator for Iter {
    type Item = Report;

    fn next(&mut self) -> Option<Self::Item> {
        self.reports.pop_front().or_else(|| {
            self.reports_rx.recv().ok().and_then(|reports| {
                self.reports.extend(reports);
                self.next()
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::dep::Dep;
    use pkgcraft::repo::Repository;
    use pkgcraft::test::TEST_DATA;
    use pretty_assertions::assert_eq;

    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn run() {
        let repo = TEST_DATA.repo("qa-primary").unwrap();
        let repo_path = repo.path();

        // repo level
        let scanner = Scanner::new().jobs(1);
        let expected = glob_reports!("{repo_path}/**/reports.json");
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);

        // specific checks
        let scanner = Scanner::new().jobs(1).checks([Check::Dependency]);
        let expected = glob_reports!("{repo_path}/Dependency/**/reports.json");
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);

        // specific reports
        let scanner = Scanner::new()
            .jobs(1)
            .reports([ReportKind::DeprecatedDependency]);
        let expected = glob_reports!("{repo_path}/Dependency/DeprecatedDependency/reports.json");
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);

        // non-matching restriction
        let scanner = Scanner::new().jobs(1);
        let dep = Dep::try_new("nonexistent/pkg").unwrap();
        assert!(scanner.run(repo, [&dep]).next().is_none());
    }
}
