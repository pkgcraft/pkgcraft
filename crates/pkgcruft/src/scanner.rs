use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use crossbeam_channel::{bounded, Receiver, Sender};
use indexmap::IndexSet;
use pkgcraft::dep::Cpn;
use pkgcraft::repo::{ebuild, Repo};
use pkgcraft::restrict::Restrict;
use pkgcraft::utils::bounded_jobs;
use strum::IntoEnumIterator;

use crate::check::Check;
use crate::report::{Report, ReportKind};
use crate::runner::SyncCheckRunner;
use crate::source::PkgFilter;

pub struct Scanner {
    jobs: usize,
    checks: IndexSet<Check>,
    reports: Arc<IndexSet<ReportKind>>,
    exit: Arc<IndexSet<ReportKind>>,
    filters: IndexSet<PkgFilter>,
    failed: Arc<AtomicBool>,
}

impl Default for Scanner {
    fn default() -> Self {
        Self {
            jobs: bounded_jobs(0),
            checks: Check::iter_default().collect(),
            reports: Arc::new(ReportKind::iter().collect()),
            exit: Arc::new(Default::default()),
            filters: Default::default(),
            failed: Arc::new(Default::default()),
        }
    }
}

impl Scanner {
    /// Create a new scanner using the default settings.
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
        I: IntoIterator,
        I::Item: Into<Check>,
    {
        self.checks = values.into_iter().map(Into::into).collect();
        self
    }

    /// Set enabled report variants.
    pub fn reports<I>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = ReportKind>,
    {
        self.reports = Arc::new(values.into_iter().collect());
        self
    }

    /// Set report variants that trigger exit code failures.
    pub fn exit<I>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = ReportKind>,
    {
        self.exit = Arc::new(values.into_iter().collect());
        self
    }

    /// Set package filters for target filtering.
    pub fn filters<I>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = PkgFilter>,
    {
        self.filters = values.into_iter().collect();
        self
    }

    /// Return true if the scanning process failed, false otherwise.
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Relaxed)
    }

    /// Run the scanner returning an iterator of reports.
    pub fn run<I>(&self, repo: &Repo, restricts: I) -> impl Iterator<Item = Report>
    where
        I: IntoIterator,
        I::Item: Into<Restrict>,
    {
        // TODO: Drop this hack once lifetime handling is improved for thread usage.
        // Currently, it's not possible to use std::thread::scope() as the related Scope
        // objects can't be stored without self-referential issues. Also, this is much
        // easier than passing Arc-wrapped repo objects down to checks which then can have
        // more self-referencing issues if they pre-process repo data without cloning it.
        //
        // In addition, note that Box::leak() shouldn't be used internally as that leaks a
        // pointer per call (or require hacks anyway to drop the leaked reference) causing
        // issues with long running services.
        //
        // An alternative would be to externally leak the references outside potential
        // loops with a passed in static reference; however, that makes the API worse and
        // this workaround shouldn't cause any issues to make that pain worth it.
        let repo: &'static Repo = unsafe { std::mem::transmute(repo) };

        let restricts = restricts.into_iter().map(Into::into).collect();
        let (restrict_tx, restrict_rx) = bounded(self.jobs);
        let (reports_tx, reports_rx) = bounded(self.jobs);

        match repo {
            Repo::Ebuild(repo) => {
                let runner = Arc::new(SyncCheckRunner::new(repo, &self.filters, &self.checks));
                let filter = ReportFilter {
                    reports: None,
                    filter: self.reports.clone(),
                    exit: self.exit.clone(),
                    failed: self.failed.clone(),
                    tx: reports_tx,
                };

                Iter {
                    reports_rx,
                    _producer: producer(repo, restricts, restrict_tx),
                    _workers: (0..self.jobs)
                        .map(|_| worker(runner.clone(), filter.clone(), restrict_rx.clone()))
                        .collect(),
                    reports: Default::default(),
                }
            }
            _ => todo!("add support for other repo types"),
        }
    }
}

// TODO: use multiple producers to push restrictions
/// Create a producer thread that sends restrictions over the channel to the workers.
fn producer(
    repo: &'static ebuild::Repo,
    restricts: Vec<Restrict>,
    tx: Sender<Cpn>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for r in restricts {
            for cpn in repo.iter_cpn_restrict(r) {
                tx.send(cpn).ok();
            }
        }
    })
}

#[derive(Clone)]
pub(crate) struct ReportFilter {
    reports: Option<Vec<Report>>,
    filter: Arc<IndexSet<ReportKind>>,
    exit: Arc<IndexSet<ReportKind>>,
    failed: Arc<AtomicBool>,
    tx: Sender<Vec<Report>>,
}

impl ReportFilter {
    /// Conditionally add a report based on filter inclusion.
    pub(crate) fn report(&mut self, report: &Report) {
        if self.filter.contains(report.kind()) {
            if self.exit.contains(report.kind()) {
                self.failed.store(true, Ordering::Relaxed);
            }

            match self.reports.as_mut() {
                Some(reports) => reports.push(report.clone()),
                None => self.reports = Some(vec![report.clone()]),
            }
        }
    }

    /// Sort existing reports and send them to the iterator.
    fn process(&mut self) {
        if let Some(mut reports) = self.reports.take() {
            reports.sort();
            self.tx.send(reports).ok();
        }
    }
}

/// Create worker thread that receives restrictions and send reports over the channel.
fn worker(
    runner: Arc<SyncCheckRunner>,
    mut filter: ReportFilter,
    rx: Receiver<Cpn>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for cpn in rx {
            runner.run(&cpn, &mut filter);
            filter.process();
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
                debug_assert!(!reports.is_empty());
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
            .reports([ReportKind::DependencyDeprecated]);
        let expected = glob_reports!("{repo_path}/Dependency/DependencyDeprecated/reports.json");
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);

        // no checks
        let checks: [Check; 0] = [];
        let scanner = Scanner::new().jobs(1).checks(checks);
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &[]);

        // no reports
        let scanner = Scanner::new().jobs(1).reports([]);
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &[]);

        // non-matching restriction
        let scanner = Scanner::new().jobs(1);
        let dep = Dep::try_new("nonexistent/pkg").unwrap();
        let reports: Vec<_> = scanner.run(repo, [&dep]).collect();
        assert_eq!(&reports, &[]);

        // empty repo
        let repo = TEST_DATA.repo("empty").unwrap();
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
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
            .exit([ReportKind::DependencyDeprecated]);
        scanner.run(repo, [repo]).count();
        assert!(scanner.failed());
    }
}
