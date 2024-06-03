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
    runners: IndexMap<SourceKind, CheckRunner<'static>>,
}

impl SyncCheckRunner {
    pub(super) fn new(repo: &Arc<Repo>, checks: &IndexSet<Check>) -> Self {
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
struct EbuildPkgCheckRunner<'a> {
    ver_checks: Vec<Box<dyn VersionCheckRun + Send + Sync + 'a>>,
    pkg_checks: Vec<Box<dyn PackageCheckRun + Send + Sync + 'a>>,
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
    #[rustfmt::skip]
    fn add_check(&mut self, check: Check) {
        match check.name {
            "Dependency" => self.ver_checks.push(Box::new(dependency::Check::new(self.repo))),
            "DependencySlotMissing" => self.ver_checks.push(Box::new(dependency_slot_missing::Check::new(self.repo))),
            "EapiStale" => self.pkg_checks.push(Box::new(eapi_stale::Check)),
            "EapiStatus" => self.ver_checks.push(Box::new(eapi_status::Check::new(self.repo))),
            "Keywords" => self.ver_checks.push(Box::new(keywords::Check::new(self.repo))),
            "KeywordsDropped" => self.pkg_checks.push(Box::new(keywords_dropped::Check::new(self.repo))),
            "LiveOnly" => self.pkg_checks.push(Box::new(live_only::Check)),
            "RestrictTestMissing" => self.ver_checks.push(Box::new(restrict_test_missing::Check::new())),
            "UnstableOnly" => self.pkg_checks.push(Box::new(unstable_only::Check::new(self.repo))),
            "UseLocal" => self.pkg_checks.push(Box::new(use_local::Check::new(self.repo))),
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
struct EbuildRawPkgCheckRunner<'a> {
    ver_checks: Vec<Box<dyn RawVersionCheckRun + Send + Sync + 'a>>,
    source: source::EbuildRaw<'a>,
}

impl<'a> EbuildRawPkgCheckRunner<'a> {
    fn new(repo: &'a Repo) -> Self {
        Self {
            ver_checks: Default::default(),
            source: source::EbuildRaw { repo },
        }
    }

    /// Add a check to the check runner.
    #[rustfmt::skip]
    fn add_check(&mut self, check: Check) {
        match check.name {
            "Metadata" => self.ver_checks.push(Box::new(metadata::Check)),
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
