use indexmap::IndexMap;
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
    /// This creates new sources and checkrunner variants on the fly. Note that the iterator of
    /// checks should be pre-sorted so the runners get inserted in their running order.
    pub(super) fn checks<I>(mut self, checks: I) -> Self
    where
        I: IntoIterator<Item = &'static Check>,
    {
        for check in checks {
            self.runners
                .entry(check.source)
                .or_insert_with(|| CheckRunner::new(check.source, self.repo))
                .add_check(check.kind);
        }
        self
    }

    /// Run all check runners in order of priority.
    pub(super) fn run(&self, restrict: &Restrict) -> Vec<Report> {
        let mut reports = vec![];
        for runner in self.runners.values() {
            runner.run(restrict, &mut reports);
        }
        reports
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
    fn run(&self, restrict: &Restrict, reports: &mut Vec<Report>) {
        match self {
            Self::EbuildPkg(r) => r.run(restrict, reports),
            Self::EbuildRawPkg(r) => r.run(restrict, reports),
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
    fn add_check(&mut self, kind: CheckKind) {
        match kind.value() {
            CheckValue::Pkg => self.pkg_checks.push(kind.ebuild(self.repo)),
            CheckValue::PkgSet => self.pkg_set_checks.push(kind.ebuild_pkg_set(self.repo)),
            _ => unreachable!("{kind} invalid for ebuild pkg check runner"),
        }
    }

    /// Run the check runner for a given restriction.
    fn run<R: Into<Restrict>>(&self, restrict: R, reports: &mut Vec<Report>) {
        let mut pkgs = vec![];

        for pkg in self.source.iter_restrict(restrict) {
            for check in &self.pkg_checks {
                check.run(&pkg, reports);
            }
            pkgs.push(pkg);
        }

        if !pkgs.is_empty() {
            for check in &self.pkg_set_checks {
                check.run(&pkgs, reports);
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
    fn add_check(&mut self, kind: CheckKind) {
        match kind.value() {
            CheckValue::RawPkg => self.pkg_checks.push(kind.ebuild_raw(self.repo)),
            _ => unreachable!("{kind} invalid for ebuild raw pkg check runner"),
        }
    }

    /// Run the check runner for a given restriction.
    fn run<R: Into<Restrict>>(&self, restrict: R, reports: &mut Vec<Report>) {
        for pkg in self.source.iter_restrict(restrict) {
            for check in &self.pkg_checks {
                check.run(&pkg, reports);
            }
        }
    }
}
