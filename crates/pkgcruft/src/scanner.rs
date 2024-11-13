use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::{iter, thread};

use crossbeam_channel::{bounded, Receiver, Sender};
use indexmap::IndexSet;
use pkgcraft::repo::{ebuild::EbuildRepo, PkgRepository, Repo};
use pkgcraft::restrict::Restrict;
use pkgcraft::utils::bounded_jobs;
use strum::IntoEnumIterator;

use crate::check::Check;
use crate::error::Error;
use crate::report::{Report, ReportKind};
use crate::runner::SyncCheckRunner;
use crate::source::{PkgFilter, Target};

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
    pub fn run<T>(
        &self,
        repo: &Repo,
        restrict: T,
    ) -> crate::Result<Box<dyn Iterator<Item = Report>>>
    where
        T: Into<Restrict>,
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

        // return early for static, non-matching restriction
        let restrict = restrict.into();
        if restrict == Restrict::False {
            return Ok(Box::new(iter::empty()));
        }

        let (restrict_tx, restrict_rx) = bounded(self.jobs);
        let (reports_tx, reports_rx) = bounded(self.jobs);
        let filter = ReportFilter {
            reports: None,
            filter: self.reports.clone(),
            exit: self.exit.clone(),
            failed: self.failed.clone(),
            tx: reports_tx,
        };

        match repo {
            Repo::Ebuild(repo) => {
                // force target metadata regen
                let mut regen = repo
                    .metadata()
                    .cache()
                    .regen()
                    .jobs(self.jobs)
                    .progress(false);
                // TODO: use parallel Cpv restriction iterator
                // skip repo level targets that needlessly slow down regen
                if restrict != Restrict::True {
                    regen = regen.targets(repo.iter_cpv_restrict(&restrict));
                }
                regen
                    .run(repo)
                    .map_err(|e| Error::InvalidValue(format!("metadata generation failed: {e}")))?;

                // run checks
                let runner = Arc::new(SyncCheckRunner::new(repo, &self.filters, &self.checks));
                Ok(Box::new(Iter {
                    reports_rx,
                    _producer: producer(repo, restrict, restrict_tx),
                    _workers: (0..self.jobs)
                        .map(|_| worker(runner.clone(), filter.clone(), restrict_rx.clone()))
                        .collect(),
                    reports: Default::default(),
                }))
            }
            _ => todo!("add support for other repo types"),
        }
    }
}

/// Create a producer thread that sends targets over a channel to workers.
fn producer(
    repo: &'static EbuildRepo,
    restrict: Restrict,
    tx: Sender<Target>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut unversioned = false;

        for cpn in repo.iter_cpn_restrict(&restrict) {
            unversioned = true;
            tx.send(Target::Cpn(cpn)).ok();
        }

        if !unversioned {
            for cpv in repo.iter_cpv_restrict(&restrict) {
                tx.send(Target::Cpv(cpv)).ok();
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
    pub(crate) fn report(&mut self, report: Report) {
        if self.filter.contains(report.kind()) {
            if self.exit.contains(report.kind()) {
                self.failed.store(true, Ordering::Relaxed);
            }

            match self.reports.as_mut() {
                Some(reports) => reports.push(report),
                None => self.reports = Some(vec![report]),
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
    rx: Receiver<Target>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for target in rx {
            runner.run(&target, &mut filter);
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
    use pkgcraft::test::{assert_ordered_eq, TEST_DATA};

    use crate::check::CheckKind;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn run() {
        let repo = TEST_DATA.repo("qa-primary").unwrap();
        let repo_path = repo.path();

        // repo target
        let scanner = Scanner::new().jobs(1);
        let expected = glob_reports!("{repo_path}/**/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_ordered_eq!(reports, expected);

        // category target
        let scanner = Scanner::new().jobs(1);
        let expected = glob_reports!("{repo_path}/Keywords/**/reports.json");
        let restrict = repo.restrict_from_path("Keywords").unwrap();
        let reports = scanner.run(repo, restrict).unwrap();
        assert_ordered_eq!(reports, expected);

        // package target
        let scanner = Scanner::new().jobs(1);
        let expected = glob_reports!("{repo_path}/Keywords/KeywordsLive/**/reports.json");
        let restrict = repo.restrict_from_path("Keywords/KeywordsLive").unwrap();
        let reports = scanner.run(repo, restrict).unwrap();
        assert_ordered_eq!(reports, expected);

        // version target
        let scanner = Scanner::new().jobs(1);
        let expected = glob_reports!("{repo_path}/Keywords/KeywordsLive/**/reports.json");
        let restrict = repo
            .restrict_from_path("Keywords/KeywordsLive/KeywordsLive-9999.ebuild")
            .unwrap();
        let reports = scanner.run(repo, restrict).unwrap();
        assert_ordered_eq!(reports, expected);

        // specific checks
        let scanner = Scanner::new().jobs(1).checks([CheckKind::Dependency]);
        let expected = glob_reports!("{repo_path}/Dependency/**/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_ordered_eq!(reports, expected);

        // specific reports
        let scanner = Scanner::new()
            .jobs(1)
            .reports([ReportKind::DependencyDeprecated]);
        let expected = glob_reports!("{repo_path}/Dependency/DependencyDeprecated/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_ordered_eq!(reports, expected);

        // no checks
        let checks: [Check; 0] = [];
        let scanner = Scanner::new().jobs(1).checks(checks);
        let reports = scanner.run(repo, repo).unwrap();
        assert_ordered_eq!(reports, []);

        // no reports
        let scanner = Scanner::new().jobs(1).reports([]);
        let reports = scanner.run(repo, repo).unwrap();
        assert_ordered_eq!(reports, []);

        // non-matching restriction
        let scanner = Scanner::new().jobs(1);
        let dep = Dep::try_new("nonexistent/pkg").unwrap();
        let reports = scanner.run(repo, &dep).unwrap();
        assert_ordered_eq!(reports, []);

        // empty repo
        let repo = TEST_DATA.repo("empty").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_ordered_eq!(reports, []);
    }

    #[test]
    fn failed() {
        let repo = TEST_DATA.repo("qa-primary").unwrap();

        // no reports flagged for failures
        let scanner = Scanner::new().jobs(1);
        scanner.run(repo, repo).unwrap().count();
        assert!(!scanner.failed());

        // fail on specified report variant
        let scanner = Scanner::new()
            .jobs(1)
            .exit([ReportKind::DependencyDeprecated]);
        scanner.run(repo, repo).unwrap().count();
        assert!(scanner.failed());
    }
}
