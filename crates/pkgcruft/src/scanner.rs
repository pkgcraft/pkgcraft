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

use crate::check::CheckKind;
use crate::report::{Report, ReportKind};
use crate::runner::SyncCheckRunner;

#[derive(Debug)]
pub struct Scanner {
    jobs: usize,
    checks: IndexSet<CheckKind>,
    reports: IndexSet<ReportKind>,
    exit: IndexSet<ReportKind>,
    failed: Arc<AtomicBool>,
}

impl Default for Scanner {
    fn default() -> Self {
        Self {
            jobs: bounded_jobs(0),
            checks: CheckKind::iter().collect(),
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
        I: IntoIterator<Item = CheckKind>,
    {
        self.checks = values.into_iter().collect();
        self
    }

    /// Set enabled report variants.
    pub fn reports<I>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = ReportKind>,
    {
        self.reports = values.into_iter().collect();
        self
    }

    /// Set report variants that trigger exit code failures.
    pub fn exit<I>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = ReportKind>,
    {
        self.exit = values.into_iter().collect();
        self
    }

    /// Return true if the scanning process failed, false otherwise.
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Relaxed)
    }

    /// Run the scanner returning an iterator of reports.
    pub fn run<'a, I, R>(&self, repo: &'a Repo, restricts: I) -> impl Iterator<Item = Report> + 'a
    where
        I: IntoIterator<Item = R>,
        R: Into<Restrict>,
    {
        match repo {
            Repo::Ebuild(r) => {
                let (restrict_tx, restrict_rx) = unbounded();
                let (reports_tx, reports_rx) = unbounded();
                let runner = Arc::new(SyncCheckRunner::new(r, &self.checks));
                let filter = Arc::new(self.reports.clone());
                let exit = Arc::new(self.exit.clone());

                Iter {
                    reports_rx,
                    _producer: producer(r.clone(), restricts, restrict_tx),
                    _workers: (0..self.jobs)
                        .map(|_| {
                            worker(
                                runner.clone(),
                                filter.clone(),
                                exit.clone(),
                                self.failed.clone(),
                                restrict_rx.clone(),
                                reports_tx.clone(),
                            )
                        })
                        .collect(),
                    reports: VecDeque::new(),
                }
            }
            _ => todo!("add support for other repo types"),
        }
    }
}

// TODO: use multiple producers to push restrictions
/// Create a producer thread that sends restrictions over the channel to the workers.
fn producer<I, R>(
    repo: Arc<ebuild::Repo>,
    restricts: I,
    tx: Sender<Restrict>,
) -> thread::JoinHandle<()>
where
    I: IntoIterator<Item = R>,
    R: Into<Restrict>,
{
    let restricts: Vec<_> = restricts.into_iter().map(Into::into).collect();
    thread::spawn(move || {
        for r in restricts {
            for cpn in repo.iter_cpn_restrict(r) {
                tx.send(Restrict::from(&cpn)).ok();
            }
        }
    })
}

/// Create worker thread that receives restrictions and send reports over the channel.
fn worker(
    runner: Arc<SyncCheckRunner>,
    filter: Arc<IndexSet<ReportKind>>,
    exit: Arc<IndexSet<ReportKind>>,
    failed: Arc<AtomicBool>,
    rx: Receiver<Restrict>,
    tx: Sender<Vec<Report>>,
) -> thread::JoinHandle<()> {
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

        // no checks
        let scanner = Scanner::new().jobs(1).checks([]);
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &[]);

        // specific checks
        let scanner = Scanner::new().jobs(1).checks([CheckKind::Dependency]);
        let expected = glob_reports!("{repo_path}/dependency/**/reports.json");
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);

        // specific reports
        let scanner = Scanner::new()
            .jobs(1)
            .reports([ReportKind::DeprecatedDependency]);
        let expected = glob_reports!("{repo_path}/dependency/deprecated-dependency/reports.json");
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);

        // no reports
        let scanner = Scanner::new().jobs(1).reports([]);
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &[]);

        // non-matching restriction
        let scanner = Scanner::new().jobs(1);
        let dep = Dep::try_new("nonexistent/pkg").unwrap();
        let reports: Vec<_> = scanner.run(repo, [&dep]).collect();
        assert_eq!(&reports, &[]);
    }

    #[test]
    fn failed() {
        let repo = TEST_DATA.repo("qa-primary").unwrap();

        // no reports flagged for failures
        let scanner = Scanner::new().jobs(1);
        scanner.run(repo, [repo]).count();
        assert!(!scanner.failed());

        // fail on specified report variant
        let scanner = Scanner::new()
            .jobs(1)
            .exit([ReportKind::DeprecatedDependency]);
        scanner.run(repo, [repo]).count();
        assert!(scanner.failed());
    }
}
