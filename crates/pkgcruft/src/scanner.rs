use std::collections::{HashSet, VecDeque};
use std::thread;

use crossbeam_channel::{unbounded, Receiver};
use indexmap::IndexSet;
use pkgcraft::repo::{ebuild, Repo, Repository};
use pkgcraft::restrict::Restrict;
use pkgcraft::utils::bounded_jobs;

use crate::check::{Check, CheckKind, CHECKS};
use crate::report::{Report, ReportKind, REPORTS};
use crate::runner::{CheckRunner, CheckRunnerSet};
use crate::source::{self, SourceKind};
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
    pub fn run<I, R>(&self, repo: &Repo, restricts: I) -> crate::Result<Iter>
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

        let mut raw_pkg_runner = CheckRunner::new(source::EbuildPackageRaw { repo });
        let mut pkg_runner = CheckRunnerSet::new(source::EbuildPackage { repo });
        for c in &self.checks {
            match c.source() {
                SourceKind::EbuildPackage => pkg_runner.item_checks.push(c.to_runner(repo)),
                SourceKind::EbuildPackageSet => pkg_runner.set_checks.push(c.to_runner(repo)),
                SourceKind::EbuildPackageRaw => raw_pkg_runner.push(c.to_runner(repo)),
            }
        }

        // send matches to the workers
        let restricts: Vec<_> = restricts.into_iter().map(|r| r.into()).collect();
        let (restrict_tx, restrict_rx) = unbounded();
        // TODO: use multiple producers to push restrictions
        let _producer = thread::spawn(move || {
            for r in restricts {
                for cpn in repo.iter_cpn_restrict(r) {
                    let restrict = Restrict::from(&cpn);
                    restrict_tx.send(restrict).unwrap();
                }
            }
        });

        let (reports_tx, reports_rx) = unbounded();
        let _workers: Vec<_> = (0..self.jobs)
            .map(|_| {
                let filter = self.reports.clone();
                let pkg_runner = pkg_runner.clone();
                let raw_pkg_runner = raw_pkg_runner.clone();
                let reports_tx = reports_tx.clone();
                let restrict_rx = restrict_rx.clone();
                thread::spawn(move || {
                    for restrict in restrict_rx {
                        let mut reports = vec![];

                        if !raw_pkg_runner.is_empty()
                            && raw_pkg_runner.run(&restrict, &mut reports).is_err()
                        {
                            // skip the remaining runners if metadata errors exist
                            continue;
                        }

                        if !pkg_runner.is_empty() {
                            pkg_runner.run(&restrict, &mut reports).ok();
                        }

                        // filter reports
                        reports.retain(|r| filter.contains(r.kind()));

                        // sort and send reports
                        if !reports.is_empty() {
                            reports.sort();
                            reports_tx.send(reports).unwrap();
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

pub struct Iter {
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
