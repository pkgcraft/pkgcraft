use std::collections::{HashSet, VecDeque};
use std::thread;

use crossbeam_channel::{unbounded, Receiver, Sender};
use indexmap::IndexSet;
use pkgcraft::dep::Cpv;
use pkgcraft::repo::{ebuild, Repo, Repository};
use pkgcraft::restrict::{dep::Restrict as DepRestrict, Restrict};

use crate::check::{Check, CheckKind, Scope, CHECKS};
use crate::report::{Report, ReportKind, REPORTS};
use crate::runner::CheckRunner;
use crate::source::{self, SourceKind};
use crate::Error;

pub struct Pipeline {
    jobs: usize,
    repo: &'static ebuild::Repo,
    checks: IndexSet<Check>,
    reports: HashSet<ReportKind>,
    restrict: Restrict,
}

impl Pipeline {
    pub fn new(
        jobs: usize,
        checks: &[CheckKind],
        reports: &[ReportKind],
        repo: &Repo,
        restrict: &Restrict,
    ) -> crate::Result<Self> {
        let mut checks: IndexSet<Check> = if checks.is_empty() {
            CHECKS.iter().copied().collect()
        } else {
            checks.iter().map(|k| k.into()).copied().collect()
        };
        checks.sort();

        let reports: HashSet<ReportKind> = if reports.is_empty() {
            REPORTS.iter().copied().collect()
        } else {
            reports.iter().copied().collect()
        };

        // TODO: support checks for non-ebuild repo types?
        let repo = repo.as_ebuild().ok_or_else(|| {
            Error::InvalidValue(format!("unsupported repo format: {}", repo.format()))
        })?;

        // TODO: drop this hack once lifetime handling is improved for thread usage
        let repo = Box::new(repo.clone());
        let repo: &'static ebuild::Repo = Box::leak(repo);

        Ok(Self {
            jobs,
            repo,
            checks,
            reports,
            restrict: restrict.clone(),
        })
    }

    /// Create worker threads that run checks in the pipeline.
    fn create_workers(
        &self,
        report_tx: Sender<Vec<Report>>,
    ) -> (thread::JoinHandle<()>, Vec<thread::JoinHandle<()>>) {
        let (worker_tx, worker_rx) = unbounded();

        let repo = self.repo;
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
        let restrict = self.restrict.clone();
        let pkg_set = !pkg_set_runner.is_empty();
        let producer = thread::spawn(move || {
            let mut prev: Option<Cpv<String>> = None;

            for cpv in repo.iter_cpv_restrict(&restrict) {
                // send versioned restricts for package checks
                let restrict = Restrict::from(&cpv);
                worker_tx.send((Scope::Package, restrict)).unwrap();

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
                    worker_tx.send((Scope::PackageSet, restrict)).unwrap();
                    prev = Some(cpv);
                }
            }
        });

        let workers: Vec<_> = (0..self.jobs)
            .map(|_| {
                let pkg_runner = pkg_runner.clone();
                let raw_pkg_runner = raw_pkg_runner.clone();
                let pkg_set_runner = pkg_set_runner.clone();
                let tx = report_tx.clone();
                let rx = worker_rx.clone();
                thread::spawn(move || {
                    for (scope, restrict) in rx {
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

                        reports.sort();
                        tx.send(reports).unwrap();
                    }
                })
            })
            .collect();

        (producer, workers)
    }

    /*fn create_runners(&self) -> Vec<CheckRunner> {
        vec![]
    }*/
}

impl<'a> IntoIterator for &'a Pipeline {
    type Item = Report;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        let (tx, rx) = unbounded();
        let (_producer, _workers) = self.create_workers(tx);

        Iter {
            rx,
            _producer,
            _workers,
            filter: &self.reports,
            reports: VecDeque::new(),
        }
    }
}

pub struct Iter<'a> {
    rx: Receiver<Vec<Report>>,
    _producer: thread::JoinHandle<()>,
    _workers: Vec<thread::JoinHandle<()>>,
    filter: &'a HashSet<ReportKind>,
    reports: VecDeque<Report>,
}

impl Iterator for Iter<'_> {
    type Item = Report;

    fn next(&mut self) -> Option<Self::Item> {
        self.reports.pop_front().or_else(|| match self.rx.recv() {
            Ok(reports) => {
                self.reports.extend(
                    reports
                        .into_iter()
                        .filter(|r| self.filter.contains(r.kind())),
                );
                self.next()
            }
            Err(_) => None,
        })
    }
}
