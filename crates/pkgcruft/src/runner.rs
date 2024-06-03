use std::sync::Arc;

use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::restrict::Restrict;

use crate::check::{Check, CheckRun, Runner};
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::{self, IterRestrict, SourceKind};

/// Check runner for synchronous checks.
#[derive(Debug)]
pub(super) struct SyncCheckRunner {
    runners: IndexMap<SourceKind, CheckRunner<'static>>,
}

impl SyncCheckRunner {
    pub(super) fn new(repo: &Arc<Repo>, checks: &IndexSet<&'static Check>) -> Self {
        let repo = Box::leak(Box::new(repo.clone()));
        let mut runners = IndexMap::new();

        // filter checks by context
        let checks = checks
            .iter()
            .filter(|c| c.context.iter().all(|x| x.enabled(repo)))
            .copied()
            // sort checks by priority so they run in the correct order
            .sorted();

        for check in checks {
            runners
                .entry(check.source)
                .or_insert_with(|| CheckRunner::new(check.source, repo))
                .add_check(check);
        }

        Self { runners }
    }

    /// Run all check runners in order of priority.
    pub(super) fn run(&self, restrict: &Restrict, filter: &mut ReportFilter) {
        for runner in self.runners.values() {
            runner.run(restrict, filter);
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
    fn add_check(&mut self, check: &'static Check) {
        match self {
            Self::EbuildPkg(r) => r.add_check(check),
            Self::EbuildRawPkg(r) => r.add_check(check),
        }
    }

    /// Run the check runner for a given restriction.
    fn run(&self, restrict: &Restrict, filter: &mut ReportFilter) {
        match self {
            Self::EbuildPkg(r) => r.run(restrict, filter),
            Self::EbuildRawPkg(r) => r.run(restrict, filter),
        }
    }
}

/// Check runner for ebuild package checks.
#[derive(Debug)]
struct EbuildPkgCheckRunner<'a> {
    ver_checks: Vec<Runner<'a>>,
    pkg_checks: Vec<Runner<'a>>,
    source: source::Ebuild<'a>,
    repo: &'a Repo,
}

impl<'a> EbuildPkgCheckRunner<'a> {
    fn new(repo: &'a Repo) -> Self {
        Self {
            ver_checks: Default::default(),
            pkg_checks: Default::default(),
            source: source::Ebuild { repo },
            repo,
        }
    }

    /// Add a check to the check runner.
    fn add_check(&mut self, check: &'static Check) {
        match &check.scope {
            Scope::Version => self.ver_checks.push((check.create)(self.repo)),
            Scope::Package => self.pkg_checks.push((check.create)(self.repo)),
            _ => panic!("unsupported check: {check}"),
        }
    }

    /// Run the check runner for a given restriction.
    fn run(&self, restrict: &Restrict, filter: &mut ReportFilter) {
        let mut pkgs = vec![];

        for pkg in self.source.iter_restrict(restrict) {
            for check in &self.ver_checks {
                check.run(&pkg, filter);
            }

            if !self.pkg_checks.is_empty() {
                pkgs.push(pkg);
            }
        }

        if !pkgs.is_empty() {
            for check in &self.pkg_checks {
                check.run(&pkgs[..], filter);
            }
        }
    }
}

/// Check runner for raw ebuild package checks.
#[derive(Debug)]
struct EbuildRawPkgCheckRunner<'a> {
    ver_checks: Vec<Runner<'a>>,
    source: source::EbuildRaw<'a>,
    repo: &'a Repo,
}

impl<'a> EbuildRawPkgCheckRunner<'a> {
    fn new(repo: &'a Repo) -> Self {
        Self {
            ver_checks: Default::default(),
            source: source::EbuildRaw { repo },
            repo,
        }
    }

    /// Add a check to the check runner.
    fn add_check(&mut self, check: &'static Check) {
        match &check.scope {
            Scope::Version => self.ver_checks.push((check.create)(self.repo)),
            _ => panic!("unsupported check: {check}"),
        }
    }

    /// Run the check runner for a given restriction.
    fn run(&self, restrict: &Restrict, filter: &mut ReportFilter) {
        for pkg in self.source.iter_restrict(restrict) {
            for check in &self.ver_checks {
                check.run(&pkg, filter);
            }
        }
    }
}
