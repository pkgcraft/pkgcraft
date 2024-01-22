use indexmap::IndexMap;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::restrict::Restrict;

use crate::check::{self, Check, CheckKind, CheckRun};
use crate::report::Report;
use crate::source::{self, IterRestrict, SourceKind};

/// Check runner for synchronous checks.
#[derive(Debug, Clone)]
pub(crate) struct SyncCheckRunner<'a> {
    runners: IndexMap<SourceKind, CheckRunner<'a>>,
    repo: &'a Repo,
}

impl<'a> SyncCheckRunner<'a> {
    pub(crate) fn new(repo: &'a Repo) -> Self {
        Self {
            runners: Default::default(),
            repo,
        }
    }

    /// Add checks to the runner.
    ///
    /// This creates new sources and checkrunner variants on the fly. Note that the iterator of
    /// checks should be pre-sorted so the runners get inserted in their running order.
    pub(crate) fn checks<I>(mut self, checks: I) -> Self
    where
        I: IntoIterator<Item = Check>,
    {
        for check in checks {
            let source = check.source();
            self.runners
                .entry(source)
                .or_insert_with(|| source.new_runner(self.repo))
                .add_check(&check);
        }
        self
    }

    /// Run all check runners in order of priority.
    pub(crate) fn run(&self, restrict: &Restrict) -> Vec<Report> {
        let mut reports = vec![];
        for runner in self.runners.values() {
            runner.run(restrict, &mut reports).ok();
        }
        reports
    }
}

/// Generic check runners.
#[derive(Debug, Clone)]
pub(crate) enum CheckRunner<'a> {
    EbuildPkg(EbuildPkgCheckRunner<'a>),
    EbuildRawPkg(EbuildRawPkgCheckRunner<'a>),
}

impl CheckRunner<'_> {
    /// Add a check to the check runner.
    fn add_check(&mut self, check: &Check) {
        match self {
            Self::EbuildPkg(r) => r.add_check(check),
            Self::EbuildRawPkg(r) => r.add_check(check),
        }
    }

    /// Run the check runner for a given restriction.
    fn run(&self, restrict: &Restrict, reports: &mut Vec<Report>) -> crate::Result<()> {
        match self {
            Self::EbuildPkg(r) => r.run(restrict, reports),
            Self::EbuildRawPkg(r) => r.run(restrict, reports),
        }
    }
}

/// Check runner for ebuild package checks.
#[derive(Debug, Clone)]
pub(crate) struct EbuildPkgCheckRunner<'a> {
    pkg_checks: Vec<check::EbuildPkgCheck<'a>>,
    pkgs_checks: Vec<check::EbuildPkgSetCheck<'a>>,
    source: source::EbuildPackage<'a>,
    repo: &'a Repo,
}

impl<'a> EbuildPkgCheckRunner<'a> {
    pub(crate) fn new(repo: &'a Repo) -> Self {
        Self {
            pkg_checks: Default::default(),
            pkgs_checks: Default::default(),
            source: source::EbuildPackage { repo },
            repo,
        }
    }

    /// Add a check to the check runner.
    fn add_check(&mut self, check: &Check) {
        use CheckKind::*;
        match check.kind() {
            EbuildPkg(k) => self.pkg_checks.push(k.to_check(self.repo)),
            EbuildPkgSet(k) => self.pkgs_checks.push(k.to_check(self.repo)),
            _ => panic!("{check} invalid for ebuild pkg check runner"),
        }
    }

    /// Run the check runner for a given restriction.
    fn run<R: Into<Restrict>>(&self, restrict: R, reports: &mut Vec<Report>) -> crate::Result<()> {
        let mut pkgs = vec![];

        for pkg in self.source.iter_restrict(restrict) {
            for check in &self.pkg_checks {
                check.run(&pkg, reports)?;
            }
            pkgs.push(pkg);
        }

        if !pkgs.is_empty() {
            for check in &self.pkgs_checks {
                check.run(&pkgs, reports)?;
            }
        }

        Ok(())
    }
}

/// Check runner for raw ebuild package checks.
#[derive(Debug, Clone)]
pub(crate) struct EbuildRawPkgCheckRunner<'a> {
    pkg_checks: Vec<check::EbuildRawPkgCheck<'a>>,
    source: source::EbuildPackageRaw<'a>,
    repo: &'a Repo,
}

impl<'a> EbuildRawPkgCheckRunner<'a> {
    pub(crate) fn new(repo: &'a Repo) -> Self {
        Self {
            pkg_checks: Default::default(),
            source: source::EbuildPackageRaw { repo },
            repo,
        }
    }

    /// Add a check to the check runner.
    fn add_check(&mut self, check: &Check) {
        use CheckKind::*;
        match check.kind() {
            EbuildRawPkg(k) => self.pkg_checks.push(k.to_check(self.repo)),
            _ => panic!("{check} invalid for ebuild raw pkg check runner"),
        }
    }

    /// Run the check runner for a given restriction.
    fn run<R: Into<Restrict>>(&self, restrict: R, reports: &mut Vec<Report>) -> crate::Result<()> {
        for pkg in self.source.iter_restrict(restrict) {
            for check in &self.pkg_checks {
                check.run(&pkg, reports)?;
            }
        }

        Ok(())
    }
}
