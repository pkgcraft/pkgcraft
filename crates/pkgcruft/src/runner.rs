use std::sync::Arc;

use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::restrict::Restrict;

use crate::check::{Check, CheckKind, CheckRun};
use crate::report::Report;
use crate::scope::Scope;
use crate::source::{self, IterRestrict, SourceKind};

/// Check runner for synchronous checks.
#[derive(Debug)]
pub(super) struct SyncCheckRunner {
    runners: IndexMap<SourceKind, CheckRunner<'static>>,
}

impl SyncCheckRunner {
    pub(super) fn new(repo: &Arc<Repo>, checks: &IndexSet<CheckKind>) -> Self {
        let repo = Box::leak(Box::new(repo.clone()));
        let mut runners = IndexMap::new();

        // filter checks by context
        let checks = checks
            .iter()
            .filter(|c| c.context().iter().all(|x| x.enabled(repo)))
            .copied()
            // sort checks by priority so they run in the correct order
            .sorted_by(CheckKind::prioritized);

        for check in checks {
            runners
                .entry(check.source())
                .or_insert_with(|| CheckRunner::new(check.source(), repo))
                .add_check(check.create(repo));
        }

        Self { runners }
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
    fn add_check(&mut self, check: Check<'a>) {
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
    checks: IndexMap<Scope, Vec<Check<'a>>>,
    source: source::Ebuild<'a>,
}

impl<'a> EbuildPkgCheckRunner<'a> {
    fn new(repo: &'a Repo) -> Self {
        Self {
            checks: Default::default(),
            source: source::Ebuild { repo },
        }
    }

    /// Add a check to the check runner.
    fn add_check(&mut self, check: Check<'a>) {
        self.checks
            .entry(check.kind().scope())
            .or_default()
            .push(check);
    }

    /// Run the check runner for a given restriction.
    fn run<F: FnMut(Report)>(&self, restrict: &Restrict, mut report: F) {
        let mut pkg_set = self
            .checks
            .get(&Scope::Package)
            .map(|checks| (checks, vec![]));

        for pkg in self.source.iter_restrict(restrict) {
            if let Some(checks) = self.checks.get(&Scope::Version) {
                for check in checks {
                    check.run(&pkg, &mut report);
                }
            }

            if let Some((_, pkgs)) = &mut pkg_set {
                pkgs.push(pkg);
            }
        }

        if let Some((checks, pkgs)) = pkg_set {
            if !pkgs.is_empty() {
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
}

impl<'a> EbuildRawPkgCheckRunner<'a> {
    fn new(repo: &'a Repo) -> Self {
        Self {
            checks: Default::default(),
            source: source::EbuildRaw { repo },
        }
    }

    /// Add a check to the check runner.
    fn add_check(&mut self, check: Check<'a>) {
        self.checks
            .entry(check.kind().scope())
            .or_default()
            .push(check);
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
