use std::thread;

use crossbeam_channel::{unbounded, Receiver, Sender};
use pkgcraft::dep::Cpv;
use pkgcraft::repo::{ebuild, Repo};
use pkgcraft::restrict::{dep::Restrict as DepRestrict, Restrict};

use crate::check::{Check, CheckKind, Scope, CHECKS};
use crate::report::Report;
use crate::runner::CheckRunner;
use crate::source::{self, SourceKind};

pub struct Pipeline {
    jobs: usize,
    repo: Repo,
    checks: Vec<Check>,
    restrict: Restrict,
}

impl Pipeline {
    pub fn new(jobs: usize, checks: &[CheckKind], repo: &Repo, restrict: &Restrict) -> Self {
        let mut checks: Vec<Check> = if checks.is_empty() {
            CHECKS.iter().copied().collect()
        } else {
            checks.iter().map(|k| k.into()).copied().collect()
        };
        checks.sort();

        Self {
            jobs,
            repo: repo.clone(),
            checks,
            restrict: restrict.clone(),
        }
    }

    /// Create worker threads that run checks in the pipeline.
    fn create_workers(
        &self,
        report_tx: Sender<Report>,
    ) -> (thread::JoinHandle<()>, Vec<thread::JoinHandle<()>>) {
        let (worker_tx, worker_rx) = unbounded();

        // TODO: support checks for non-ebuild repo types?
        let repo = self
            .repo
            .as_ebuild()
            .expect("currently only ebuild repos are supported");

        // TODO: drop this hack once lifetime handling is improved for thread usage
        let repo = Box::new(repo.clone());
        let repo: &'static ebuild::Repo = Box::leak(repo);

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
                        match scope {
                            Scope::Package => {
                                if !raw_pkg_runner.is_empty()
                                    && raw_pkg_runner.run(&restrict, &tx).is_err()
                                {
                                    // skip the remaining runners if metadata errors exist
                                    continue;
                                }

                                if !pkg_runner.is_empty() {
                                    pkg_runner.run(&restrict, &tx).ok();
                                }
                            }
                            Scope::PackageSet => {
                                if !pkg_set_runner.is_empty() {
                                    pkg_set_runner.run(&restrict, &tx).ok();
                                }
                            }
                        }
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

impl IntoIterator for &Pipeline {
    type Item = Report;
    type IntoIter = Iter;

    fn into_iter(self) -> Self::IntoIter {
        let (tx, rx) = unbounded();
        let (_producer, _workers) = self.create_workers(tx);

        Iter { rx, _producer, _workers }
    }
}

pub struct Iter {
    rx: Receiver<Report>,
    _producer: thread::JoinHandle<()>,
    _workers: Vec<thread::JoinHandle<()>>,
}

impl Iterator for Iter {
    type Item = Report;

    fn next(&mut self) -> Option<Self::Item> {
        self.rx.recv().ok()
    }
}
