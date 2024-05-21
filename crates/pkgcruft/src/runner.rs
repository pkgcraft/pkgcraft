use indexmap::IndexMap;
use itertools::Itertools;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::restrict::Restrict;

use crate::check::{self, Check, CheckKind, CheckRun, CheckValue};
use crate::report::Report;
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
    pub(super) fn checks<I>(mut self, checks: I) -> Self
    where
        I: IntoIterator<Item = &'static Check>,
    {
        // sort checks by priority so they run in the correct order
        for check in checks.into_iter().sorted() {
            self.runners
                .entry(check.source)
                .or_insert_with(|| CheckRunner::new(check.source, self.repo))
                .add_check(check.kind);
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
    fn add_check(&mut self, check: CheckKind) {
        match self {
            Self::EbuildPkg(r) => r.add_check(check),
            Self::EbuildRawPkg(r) => r.add_check(check),
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
    pkg_checks: Vec<check::EbuildPkgCheck<'a>>,
    pkg_set_checks: Vec<check::EbuildPkgSetCheck<'a>>,
    source: source::Ebuild<'a>,
    repo: &'a Repo,
}

impl<'a> EbuildPkgCheckRunner<'a> {
    fn new(repo: &'a Repo) -> Self {
        Self {
            pkg_checks: Default::default(),
            pkg_set_checks: Default::default(),
            source: source::Ebuild { repo },
            repo,
        }
    }

    /// Add a check to the check runner.
    fn add_check(&mut self, check: CheckKind) {
        match check.value() {
            CheckValue::Pkg => self.pkg_checks.push(check.ebuild(self.repo)),
            CheckValue::PkgSet => self.pkg_set_checks.push(check.ebuild_pkg_set(self.repo)),
            _ => unreachable!("{check} invalid for ebuild pkg check runner"),
        }
    }

    /// Run the check runner for a given restriction.
    fn run<F: FnMut(Report)>(&self, restrict: &Restrict, mut report: F) {
        let mut pkgs = vec![];

        for pkg in self.source.iter_restrict(restrict) {
            for check in &self.pkg_checks {
                check.run(&pkg, &mut report);
            }
            pkgs.push(pkg);
        }

        if !pkgs.is_empty() {
            for check in &self.pkg_set_checks {
                check.run(&pkgs, &mut report);
            }
        }
    }
}

/// Check runner for raw ebuild package checks.
#[derive(Debug)]
struct EbuildRawPkgCheckRunner<'a> {
    pkg_checks: Vec<check::EbuildRawPkgCheck<'a>>,
    source: source::EbuildRaw<'a>,
    repo: &'a Repo,
}

impl<'a> EbuildRawPkgCheckRunner<'a> {
    fn new(repo: &'a Repo) -> Self {
        Self {
            pkg_checks: Default::default(),
            source: source::EbuildRaw { repo },
            repo,
        }
    }

    /// Add a check to the check runner.
    fn add_check(&mut self, check: CheckKind) {
        match check.value() {
            CheckValue::RawPkg => self.pkg_checks.push(check.ebuild_raw(self.repo)),
            _ => unreachable!("{check} invalid for ebuild raw pkg check runner"),
        }
    }

    /// Run the check runner for a given restriction.
    fn run<F: FnMut(Report)>(&self, restrict: &Restrict, mut report: F) {
        for pkg in self.source.iter_restrict(restrict) {
            for check in &self.pkg_checks {
                check.run(&pkg, &mut report);
            }
        }
    }
}
