use indexmap::IndexMap;
use itertools::Itertools;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::restrict::Restrict;

use crate::check::{Check, CheckKind, CheckRun};
use crate::report::Report;
use crate::scope::Scope;
use crate::source::{self, IterRestrict, SourceKind};

/// Check runner for synchronous checks.
#[derive(Debug)]
pub(super) struct SyncCheckRunner<'a> {
    runners: IndexMap<SourceKind, CheckRunner<'a>>,
    repo: &'a Repo,
}

impl<'a> SyncCheckRunner<'a> {
    pub(super) fn new(repo: &'a Repo) -> Self {
        Self {
            runners: Default::default(),
            repo,
        }
    }

    /// Add checks to the runner.
    ///
    /// This creates new sources and checkrunner variants on the fly.
    pub(super) fn checks<I>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = CheckKind>,
    {
        // sort checks by priority so they run in the correct order
        for kind in values.into_iter().sorted_by(CheckKind::prioritized) {
            self.runners
                .entry(kind.source())
                .or_insert_with(|| CheckRunner::new(kind.source(), self.repo))
                .add_check(kind);
        }
        self
    }

    /// Run all check runners in order of priority.
    pub(super) fn run<F: FnMut(Report)>(&self, restrict: &Restrict, mut report: F) {
        for runner in self.runners.values() {
            runner.run(restrict, &mut report);
        }
    }
}

/// Generic check runners.
#[derive(Debug)]
enum CheckRunner<'a> {
    EbuildPkg(EbuildPkgCheckRunner<'a>),
    EbuildRawPkg(EbuildRawPkgCheckRunner<'a>),
}

impl<'a> CheckRunner<'a> {
    fn new(source: SourceKind, repo: &'a Repo) -> Self {
        match source {
            SourceKind::Ebuild => Self::EbuildPkg(EbuildPkgCheckRunner::new(repo)),
            SourceKind::EbuildRaw => Self::EbuildRawPkg(EbuildRawPkgCheckRunner::new(repo)),
        }
    }

    /// Add a check to the check runner.
    fn add_check(&mut self, kind: CheckKind) {
        match self {
            Self::EbuildPkg(r) => r.add_check(kind),
            Self::EbuildRawPkg(r) => r.add_check(kind),
        }
    }

    /// Run the check runner for a given restriction.
    fn run<F: FnMut(Report)>(&self, restrict: &Restrict, report: F) {
        match self {
            Self::EbuildPkg(r) => r.run(restrict, report),
            Self::EbuildRawPkg(r) => r.run(restrict, report),
        }
    }
}

/// Check runner for ebuild package checks.
#[derive(Debug)]
struct EbuildPkgCheckRunner<'a> {
    checks: IndexMap<Scope, Vec<Check<'a>>>,
    source: source::Ebuild<'a>,
    repo: &'a Repo,
}

impl<'a> EbuildPkgCheckRunner<'a> {
    fn new(repo: &'a Repo) -> Self {
        Self {
            checks: Default::default(),
            source: source::Ebuild { repo },
            repo,
        }
    }

    /// Add a check to the check runner.
    fn add_check(&mut self, kind: CheckKind) {
        self.checks
            .entry(kind.scope())
            .or_default()
            .push(Check::new(kind, self.repo));
    }

    /// Run the check runner for a given restriction.
    fn run<F: FnMut(Report)>(&self, restrict: &Restrict, mut report: F) {
        let mut pkgs = vec![];

        for pkg in self.source.iter_restrict(restrict) {
            if let Some(checks) = self.checks.get(&Scope::Version) {
                for check in checks {
                    check.run(&pkg, &mut report);
                }
            }
            pkgs.push(pkg);
        }

        if !pkgs.is_empty() {
            if let Some(checks) = self.checks.get(&Scope::Package) {
                for check in checks {
                    check.run(&pkgs[..], &mut report);
                }
            }
        }
    }
}

/// Check runner for raw ebuild package checks.
#[derive(Debug)]
struct EbuildRawPkgCheckRunner<'a> {
    checks: IndexMap<Scope, Vec<Check<'a>>>,
    source: source::EbuildRaw<'a>,
    repo: &'a Repo,
}

impl<'a> EbuildRawPkgCheckRunner<'a> {
    fn new(repo: &'a Repo) -> Self {
        Self {
            checks: Default::default(),
            source: source::EbuildRaw { repo },
            repo,
        }
    }

    /// Add a check to the check runner.
    fn add_check(&mut self, kind: CheckKind) {
        self.checks
            .entry(kind.scope())
            .or_default()
            .push(Check::new(kind, self.repo));
    }

    /// Run the check runner for a given restriction.
    fn run<F: FnMut(Report)>(&self, restrict: &Restrict, mut report: F) {
        for pkg in self.source.iter_restrict(restrict) {
            if let Some(checks) = self.checks.get(&Scope::Version) {
                for check in checks {
                    check.run(&pkg, &mut report);
                }
            }
        }
    }
}
