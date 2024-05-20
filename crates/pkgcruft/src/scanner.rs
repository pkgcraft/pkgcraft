use std::collections::VecDeque;
use std::sync::Arc;
use std::thread;

use crossbeam_channel::{unbounded, Receiver, Sender};
use indexmap::IndexSet;
use pkgcraft::repo::{ebuild, Repo};
use pkgcraft::restrict::Restrict;
use pkgcraft::utils::bounded_jobs;
use strum::IntoEnumIterator;

use crate::check::{Check, CheckKind};
use crate::report::{Report, ReportKind};
use crate::runner::SyncCheckRunner;

#[derive(Debug)]
pub struct Scanner {
    jobs: usize,
    checks: IndexSet<&'static Check>,
    reports: IndexSet<ReportKind>,
}

impl Default for Scanner {
    fn default() -> Self {
        Self {
            jobs: bounded_jobs(0),
            checks: CheckKind::iter().map(Into::into).collect(),
            reports: ReportKind::iter().collect(),
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
    pub fn checks<I, T>(mut self, checks: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<&'static Check>,
    {
        self.checks = checks.into_iter().map(Into::into).collect();
        self
    }

    /// Set enabled report variants.
    pub fn reports<I, T>(mut self, reports: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<ReportKind>,
    {
        self.reports = reports.into_iter().map(Into::into).collect();
        self
    }

    /// Run the scanner returning an iterator of reports.
    pub fn run<I, R>(&self, repo: &Repo, restricts: I) -> impl Iterator<Item = Report>
    where
        I: IntoIterator<Item = R>,
        R: Into<Restrict>,
    {
        let checks = self.checks.iter().copied();
        match repo {
            Repo::Ebuild(r) => {
                // TODO: drop this hack once lifetime handling is improved for thread usage
                let repo: &'static ebuild::Repo = Box::leak(Box::new(r.clone()));

                let sync_runner = SyncCheckRunner::new(repo).checks(checks);
                let (restrict_tx, restrict_rx) = unbounded();
                let (reports_tx, reports_rx) = unbounded();
                let runner = Arc::new(sync_runner);
                let filter = Arc::new(self.reports.clone());

                Iter {
                    reports_rx,
                    _producer: Producer::new(repo, restricts, restrict_tx),
                    _workers: Workers::new(self.jobs, &runner, &filter, &restrict_rx, &reports_tx),
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

    use crate::check::CheckKind;
    use crate::report::ReportKind;
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
        let scanner = Scanner::new().jobs(1).checks([CheckKind::Dependency]);
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
