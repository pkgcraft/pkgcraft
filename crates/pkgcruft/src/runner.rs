use std::sync::Arc;
use std::time::Instant;

use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use pkgcraft::pkg::Package;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::restrict::Restrict;
use tracing::debug;

use crate::bash::Tree;
use crate::check::*;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
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
    Ebuild(EbuildCheckRunner),
    EbuildRaw(EbuildRawCheckRunner),
}

impl CheckRunner {
    fn new(source: SourceKind, repo: &'static Repo) -> Self {
        match source {
            SourceKind::Ebuild => Self::Ebuild(EbuildCheckRunner::new(repo)),
            SourceKind::EbuildRaw => Self::EbuildRaw(EbuildRawCheckRunner::new(repo)),
        }
    }

    /// Add a check to the check runner.
    fn add_check(&mut self, check: Check) {
        match self {
            Self::Ebuild(r) => r.add_check(check),
            Self::EbuildRaw(r) => r.add_check(check),
        }
    }

    /// Run the check runner for a given restriction.
    fn run(&self, restrict: &Restrict, filter: &mut ReportFilter) {
        match self {
            Self::Ebuild(r) => r.run(restrict, filter),
            Self::EbuildRaw(r) => r.run(restrict, filter),
        }
    }
}

/// Check runner for ebuild package checks.
struct EbuildCheckRunner {
    ver_checks: Vec<VersionRunner>,
    pkg_checks: Vec<PackageRunner>,
    source: source::Ebuild,
    repo: &'static Repo,
}

impl EbuildCheckRunner {
    fn new(repo: &'static Repo) -> Self {
        Self {
            ver_checks: Default::default(),
            pkg_checks: Default::default(),
            source: source::Ebuild { repo },
            repo,
        }
    }

    /// Add a check to the check runner.
    fn add_check(&mut self, check: Check) {
        match &check.scope {
            Scope::Version => self.ver_checks.push(check.to_runner(self.repo)),
            Scope::Package => self.pkg_checks.push(check.to_runner(self.repo)),
            _ => unreachable!("unsupported check: {check}"),
        }
    }

    /// Run the check runner for a given restriction.
    fn run(&self, restrict: &Restrict, filter: &mut ReportFilter) {
        let mut pkgs = vec![];

        for pkg in self.source.iter_restrict(restrict) {
            for check in &self.ver_checks {
                let now = Instant::now();
                check.run(&pkg, filter);
                debug!("{check}: {pkg}: {:?}", now.elapsed());
            }

            if !self.pkg_checks.is_empty() {
                pkgs.push(pkg);
            }
        }

        if !pkgs.is_empty() {
            for check in &self.pkg_checks {
                let now = Instant::now();
                check.run(&pkgs[..], filter);
                debug!("{check}: {}: {:?}", pkgs[0].cpn(), now.elapsed());
            }
        }
    }
}

/// Check runner for raw ebuild package checks.
struct EbuildRawCheckRunner {
    ver_checks: Vec<RawVersionRunner>,
    source: source::EbuildRaw,
    repo: &'static Repo,
}

impl EbuildRawCheckRunner {
    fn new(repo: &'static Repo) -> Self {
        Self {
            ver_checks: Default::default(),
            source: source::EbuildRaw { repo },
            repo,
        }
    }

    /// Add a check to the check runner.
    fn add_check(&mut self, check: Check) {
        match &check.scope {
            Scope::Version => self.ver_checks.push(check.to_runner(self.repo)),
            _ => unreachable!("unsupported check: {check}"),
        }
    }

    /// Run the check runner for a given restriction.
    fn run(&self, restrict: &Restrict, filter: &mut ReportFilter) {
        for pkg in self.source.iter_restrict(restrict) {
            let tree = Tree::new(pkg.data().as_bytes());
            for check in &self.ver_checks {
                let now = Instant::now();
                check.run(&pkg, &tree, filter);
                debug!("{check}: {pkg}: {:?}", now.elapsed());
            }
        }
    }
}
