use std::collections::{HashSet, VecDeque};
use std::thread;

use crossbeam_channel::{unbounded, Receiver};
use indexmap::IndexSet;
use pkgcraft::dep::Cpv;
use pkgcraft::repo::{ebuild, Repo, Repository};
use pkgcraft::restrict::{dep::Restrict as DepRestrict, Restrict};
use pkgcraft::utils::bounded_jobs;

use crate::check::{Check, CheckKind, Scope, CHECKS, ENABLED_CHECKS};
use crate::report::{Report, ReportKind, REPORTS};
use crate::runner::CheckRunner;
use crate::source::{self, SourceKind};
use crate::Error;

#[derive(Debug)]
pub struct Scanner {
    jobs: usize,
    checks: IndexSet<Check>,
    filter: HashSet<ReportKind>,
}

impl Default for Scanner {
    fn default() -> Self {
        Self {
            jobs: bounded_jobs(0),
            checks: CHECKS.iter().copied().collect(),
            filter: REPORTS.iter().copied().collect(),
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
            self.checks = ENABLED_CHECKS.clone();
            self.checks.extend(checks.iter().map(Check::from));
            self.checks.sort();
        }
        self
    }

    /// Set the report types to allow.
    pub fn filter(mut self, filter: &[ReportKind]) -> Self {
        if !filter.is_empty() {
            self.filter = ENABLED_CHECKS
                .iter()
                .flat_map(|c| c.reports())
                .copied()
                .collect();
            self.filter.extend(filter.iter().copied());
        }
        self
    }

    /// Run the scanner returning an iterator of reports.
    pub fn run(&self, repo: &Repo, restrict: &Restrict) -> crate::Result<Iter> {
        // TODO: support checks for non-ebuild repo types?
        let repo = repo.as_ebuild().ok_or_else(|| {
            Error::InvalidValue(format!("unsupported repo format: {}", repo.format()))
        })?;

        // TODO: drop this hack once lifetime handling is improved for thread usage
        let repo: &'static ebuild::Repo = Box::leak(Box::new(repo.clone()));

        let mut pkg_runner = CheckRunner::new(source::EbuildPackage { repo });
        let mut raw_pkg_runner = CheckRunner::new(source::EbuildPackageRaw { repo });
        let mut pkg_set_runner = CheckRunner::new(source::EbuildPackageSet { repo });
        for c in &self.checks {
            match c.source() {
                SourceKind::EbuildPackage => pkg_runner.push(c.to_runner(repo)),
                SourceKind::EbuildPackageRaw => raw_pkg_runner.push(c.to_runner(repo)),
                SourceKind::EbuildPackageSet => pkg_set_runner.push(c.to_runner(repo)),
            }
        }

        // send matches to the workers
        let restrict = restrict.clone();
        let (restrict_tx, restrict_rx) = unbounded();
        let pkg_set = !pkg_set_runner.is_empty();
        // TODO: use multiple producers to push restrictions
        let _producer = thread::spawn(move || {
            let mut prev: Option<Cpv<String>> = None;

            for cpv in repo.iter_cpv_restrict(&restrict) {
                // send versioned restricts for package checks
                let restrict = Restrict::from(&cpv);
                restrict_tx.send((Scope::Package, restrict)).unwrap();

                // send unversioned restricts for package set checks
                if pkg_set {
                    if let Some(prev_cpv) = prev.as_ref() {
                        if prev_cpv.category() == cpv.category()
                            && prev_cpv.package() == cpv.package()
                        {
                            continue;
                        }
                    }

                    let restrict = Restrict::and([
                        DepRestrict::category(cpv.category()),
                        DepRestrict::package(cpv.package()),
                    ]);
                    restrict_tx.send((Scope::PackageSet, restrict)).unwrap();
                    prev = Some(cpv);
                }
            }
        });

        let (reports_tx, reports_rx) = unbounded();
        let _workers: Vec<_> = (0..self.jobs)
            .map(|_| {
                let filter = self.filter.clone();
                let pkg_runner = pkg_runner.clone();
                let raw_pkg_runner = raw_pkg_runner.clone();
                let pkg_set_runner = pkg_set_runner.clone();
                let reports_tx = reports_tx.clone();
                let restrict_rx = restrict_rx.clone();
                thread::spawn(move || {
                    for (scope, restrict) in restrict_rx {
                        let mut reports = vec![];

                        match scope {
                            Scope::Package => {
                                if !raw_pkg_runner.is_empty()
                                    && raw_pkg_runner.run(&restrict, &mut reports).is_err()
                                {
                                    // skip the remaining runners if metadata errors exist
                                    continue;
                                }

                                if !pkg_runner.is_empty() {
                                    pkg_runner.run(&restrict, &mut reports).ok();
                                }
                            }
                            Scope::PackageSet => {
                                if !pkg_set_runner.is_empty() {
                                    pkg_set_runner.run(&restrict, &mut reports).ok();
                                }
                            }
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
