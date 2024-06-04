use std::sync::Arc;

use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::restrict::Restrict;

use crate::check::*;
use crate::scanner::ReportFilter;
use crate::source::{self, IterRestrict, SourceKind};

/// Check runner for synchronous checks.
pub(super) struct SyncCheckRunner {
    runners: IndexMap<SourceKind, CheckRunner>,
}

impl SyncCheckRunner {
    pub(super) fn new(repo: &Arc<Repo>, checks: &IndexSet<Check>) -> Self {
        let repo = Box::leak(Box::new(repo.clone()));
        let mut runners = IndexMap::new();

        // filter checks by context
        let enabled = checks
            .iter()
            // TODO: replace checks parameter with selected checks once #194 is implemented
            .filter(|c| c.enabled(repo, checks))
            .copied()
            // sort checks by priority so they run in the correct order
            .sorted();

        for check in enabled {
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
enum CheckRunner {
    EbuildPkg(EbuildPkgCheckRunner),
    EbuildRawPkg(EbuildRawPkgCheckRunner),
}

impl CheckRunner {
    fn new(source: SourceKind, repo: &'static Repo) -> Self {
        match source {
            SourceKind::Ebuild => Self::EbuildPkg(EbuildPkgCheckRunner::new(repo)),
            SourceKind::EbuildRaw => Self::EbuildRawPkg(EbuildRawPkgCheckRunner::new(repo)),
        }
    }

    /// Add a check to the check runner.
    fn add_check(&mut self, check: Check) {
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
struct EbuildPkgCheckRunner {
    ver_checks: Vec<Box<dyn VersionCheck + Send + Sync>>,
    pkg_checks: Vec<Box<dyn PackageCheck + Send + Sync>>,
    source: source::Ebuild,
    repo: &'static Repo,
}

impl EbuildPkgCheckRunner {
    fn new(repo: &'static Repo) -> Self {
        Self {
            ver_checks: Default::default(),
            pkg_checks: Default::default(),
            source: source::Ebuild { repo },
            repo,
        }
    }

    /// Add a check to the check runner.
    #[rustfmt::skip]
    fn add_check(&mut self, check: Check) {
        match &check.kind {
            CheckKind::Dependency => self.ver_checks.push(Box::new(dependency::Check::new(self.repo))),
            CheckKind::DependencySlotMissing => self.ver_checks.push(Box::new(dependency_slot_missing::Check::new(self.repo))),
            CheckKind::EapiStale => self.pkg_checks.push(Box::new(eapi_stale::Check)),
            CheckKind::EapiStatus => self.ver_checks.push(Box::new(eapi_status::Check::new(self.repo))),
            CheckKind::Keywords => self.ver_checks.push(Box::new(keywords::Check::new(self.repo))),
            CheckKind::KeywordsDropped => self.pkg_checks.push(Box::new(keywords_dropped::Check::new(self.repo))),
            CheckKind::LiveOnly => self.pkg_checks.push(Box::new(live_only::Check)),
            CheckKind::RestrictTestMissing => self.ver_checks.push(Box::new(restrict_test_missing::Check::new())),
            CheckKind::UnstableOnly => self.pkg_checks.push(Box::new(unstable_only::Check::new(self.repo))),
            CheckKind::UseLocal => self.pkg_checks.push(Box::new(use_local::Check::new(self.repo))),
            _ => unreachable!("unsupported check: {check}"),
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
struct EbuildRawPkgCheckRunner {
    ver_checks: Vec<Box<dyn RawVersionCheck + Send + Sync>>,
    source: source::EbuildRaw,
}

impl EbuildRawPkgCheckRunner {
    fn new(repo: &'static Repo) -> Self {
        Self {
            ver_checks: Default::default(),
            source: source::EbuildRaw { repo },
        }
    }

    /// Add a check to the check runner.
    #[rustfmt::skip]
    fn add_check(&mut self, check: Check) {
        match &check.kind {
            CheckKind::Metadata => self.ver_checks.push(Box::new(metadata::Check)),
            _ => unreachable!("unsupported check: {check}"),
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
