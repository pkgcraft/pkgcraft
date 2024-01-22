use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::thread;

use crossbeam_channel::{unbounded, Receiver, Sender};
use indexmap::IndexSet;
use pkgcraft::repo::{ebuild, Repo, Repository};
use pkgcraft::restrict::Restrict;
use pkgcraft::utils::bounded_jobs;

use crate::check::{Check, CheckKind, CHECKS};
use crate::report::{Report, ReportKind, REPORTS};
use crate::runner::SyncCheckRunner;
use crate::Error;

#[derive(Debug)]
pub struct Scanner {
    jobs: usize,
    checks: IndexSet<Check>,
    reports: HashSet<ReportKind>,
}

impl Default for Scanner {
    fn default() -> Self {
        Self {
            jobs: bounded_jobs(0),
            checks: CHECKS.iter().copied().collect(),
            reports: REPORTS.iter().copied().collect(),
        }
    }
}

impl Scanner {
    /// Create a new scanner using the default settings.
    pub fn new() -> Self {
        Scanner::default()
    }

    /// Set the number of parallel scanner jobs to run.
    pub fn jobs(mut self, jobs: usize) -> Self {
        self.jobs = bounded_jobs(jobs);
        self
    }

    /// Set the checks to run.
    pub fn checks(mut self, checks: &[CheckKind]) -> Self {
        if !checks.is_empty() {
            self.checks = checks.iter().map(Check::from).collect();
            self.checks.sort();
        }
        self
    }

    /// Set the report types to allow.
    pub fn reports(mut self, reports: &[ReportKind]) -> Self {
        if !reports.is_empty() {
            self.reports = reports.iter().copied().collect();
        }
        self
    }

    /// Run the scanner returning an iterator of reports.
    pub fn run<I, R>(
        &self,
        repo: &Repo,
        restricts: I,
    ) -> crate::Result<impl Iterator<Item = Report>>
    where
        I: IntoIterator<Item = R>,
        R: Into<Restrict>,
    {
        // TODO: support checks for non-ebuild repo types?
        let repo = repo.as_ebuild().ok_or_else(|| {
            Error::InvalidValue(format!("unsupported repo format: {}", repo.format()))
        })?;

        // TODO: drop this hack once lifetime handling is improved for thread usage
        let repo: &'static ebuild::Repo = Box::leak(Box::new(repo.clone()));

        let sync_runner = SyncCheckRunner::new(repo).checks(self.checks.iter().copied());
        let (restrict_tx, restrict_rx) = unbounded();
        let (reports_tx, reports_rx) = unbounded();
        let runner = Arc::new(sync_runner);
        let filter = Arc::new(self.reports.clone());

        Ok(Iter {
            reports_rx,
            _producer: Producer::new(repo, restricts, restrict_tx),
            _workers: Workers::new(self.jobs, &runner, &filter, &restrict_rx, &reports_tx),
            reports: VecDeque::new(),
        })
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
                        tx.send(Restrict::from(&cpn))
                            .expect("sending restrict failed");
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
        filter: &Arc<HashSet<ReportKind>>,
        rx: &Receiver<Restrict>,
        tx: &Sender<Vec<Report>>,
    ) -> Self {
        Self {
            _threads: (0..jobs)
                .map(|_| {
                    let runner = runner.clone();
                    let filter = filter.clone();
                    let rx = rx.clone();
                    let tx = tx.clone();
                    thread::spawn(move || {
                        for restrict in rx {
                            // run checks and filter reports
                            let mut reports = runner.run(&restrict);
                            reports.retain(|r| filter.contains(r.kind()));

                            // sort and send reports
                            if !reports.is_empty() {
                                reports.sort();
                                tx.send(reports).expect("sending reports failed");
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
    use pkgcraft::repo::Repository;
    use pkgcraft::test::TEST_DATA;

    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn run() {
        let repo = TEST_DATA.config().repos.get("qa-primary").unwrap();
        let repo_path = repo.path();
        let restrict = repo.restrict_from_path(repo_path).unwrap();
        let scanner = Scanner::new().jobs(1);
        let expected: Vec<_> = glob_reports(format!("{repo_path}/**/reports.json")).collect();
        let reports: Vec<_> = scanner.run(repo, [&restrict]).unwrap().collect();
        assert_eq!(&reports, &expected);
    }
}
