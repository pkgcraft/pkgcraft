use std::collections::{HashSet, VecDeque};
use std::thread;

use crossbeam_channel::{unbounded, Receiver};
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

        let mut sync_runner = SyncCheckRunner::new(repo);
        sync_runner.add_checks(self.checks.iter().copied());

        // send matches to the workers
        let restricts: Vec<_> = restricts.into_iter().map(|r| r.into()).collect();
        let (restrict_tx, restrict_rx) = unbounded();
        // TODO: use multiple producers to push restrictions
        let _producer = thread::spawn(move || {
            for r in restricts {
                for cpn in repo.iter_cpn_restrict(r) {
                    restrict_tx
                        .send(Restrict::from(&cpn))
                        .expect("sending restrict failed");
                }
            }
        });

        let (reports_tx, reports_rx) = unbounded();
        let _workers: Vec<_> = (0..self.jobs)
            .map(|_| {
                let filter = self.reports.clone();
                let runner = sync_runner.clone();
                let reports_tx = reports_tx.clone();
                let restrict_rx = restrict_rx.clone();
                thread::spawn(move || {
                    for restrict in restrict_rx {
                        // run checks and filter reports
                        let mut reports = runner.run(&restrict);
                        reports.retain(|r| filter.contains(r.kind()));

                        // sort and send reports
                        if !reports.is_empty() {
                            reports.sort();
                            reports_tx.send(reports).expect("sending report failed");
                        }
                    }
                })
            })
            .collect();

        Ok(Iter {
            reports_rx,
            _producer,
            _workers,
            reports: VecDeque::new(),
        })
    }
}

struct Iter {
    reports_rx: Receiver<Vec<Report>>,
    _producer: thread::JoinHandle<()>,
    _workers: Vec<thread::JoinHandle<()>>,
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
