use std::time::Instant;

use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use pkgcraft::dep::{Cpn, Cpv};
use pkgcraft::pkg::ebuild::{EbuildPkg, EbuildRawPkg};
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::repo::PkgRepository;
use pkgcraft::restrict::Restrict;
use tracing::{debug, trace, warn};

use crate::check::*;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::*;

/// Check runner for synchronous checks.
pub(super) struct SyncCheckRunner {
    runners: IndexMap<SourceKind, CheckRunner>,
}

impl SyncCheckRunner {
    pub(super) fn new(
        scope: Scope,
        repo: &'static EbuildRepo,
        restrict: &Restrict,
        filters: &IndexSet<PkgFilter>,
        checks: &IndexSet<Check>,
    ) -> Self {
        let mut runners = IndexMap::new();

        // TODO: error out instead of skipping checks silently
        // filter checks
        let enabled = checks
            .iter()
            .filter(|c| {
                if !filters.is_empty() && c.filtered() {
                    warn!("check disabled due to filtering: {c}");
                    false
                } else {
                    true
                }
            })
            .filter(|c| {
                if !c.enabled(repo, checks) {
                    warn!("check disabled due to context: {c}");
                    false
                } else {
                    true
                }
            })
            .filter(|c| {
                if c.scope > scope {
                    warn!("check disabled due to {scope} scope: {c}");
                    false
                } else {
                    true
                }
            })
            .copied()
            // sort checks by priority so they run in the correct order
            .sorted();

        for check in enabled {
            runners
                .entry(check.source)
                .or_insert_with(|| {
                    CheckRunner::new(scope, restrict, check.source, repo, filters.clone())
                })
                .add_check(check);
        }

        Self { runners }
    }

    /// Return an iterator of filtered checks.
    pub(super) fn checks<'a, F>(&'a self, filter: F) -> impl Iterator<Item = Check> + 'a
    where
        F: Fn(&Check) -> bool + 'a,
    {
        self.runners.values().flat_map(|r| r.iter()).filter(filter)
    }

    /// Run all check runners in order of priority.
    pub(super) fn run(&self, target: Target, filter: &mut ReportFilter) {
        for (source, runner) in &self.runners {
            match (runner, &target) {
                (CheckRunner::EbuildPkg(r), Target::Cpn(cpn)) => r.run_checks(cpn, filter),
                (CheckRunner::EbuildRawPkg(r), Target::Cpn(cpn)) => r.run_checks(cpn, filter),
                (CheckRunner::Cpn(r), Target::Cpn(cpn)) => r.run_checks(cpn, filter),
                (CheckRunner::Cpv(r), Target::Cpn(cpn)) => r.run_checks(cpn, filter),
                _ => trace!("skipping incompatible target {target} for source: {source:?}"),
            }
        }
    }

    /// Run a specific check.
    pub(super) fn run_check(&self, check: Check, target: Target, filter: &mut ReportFilter) {
        let runner = self
            .runners
            .get(&check.source)
            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
        match (runner, &target) {
            (CheckRunner::EbuildPkg(r), Target::Cpv(cpv)) => r.run_check(&check, cpv, filter),
            (CheckRunner::EbuildPkg(r), Target::Cpn(cpn)) => r.run_pkg_set(&check, cpn, filter),
            (CheckRunner::EbuildRawPkg(r), Target::Cpv(cpv)) => r.run_check(&check, cpv, filter),
            (CheckRunner::EbuildRawPkg(r), Target::Cpn(cpn)) => r.run_pkg_set(&check, cpn, filter),
            (CheckRunner::Cpn(r), Target::Cpn(cpn)) => r.run_check(&check, cpn, filter),
            (CheckRunner::Cpv(r), Target::Cpv(cpv)) => r.run_check(&check, cpv, filter),
            (CheckRunner::Repo(r), Target::Repo(repo)) => r.run_check(&check, repo, filter),
            _ => panic!("incompatible target {target} for check: {check}"),
        }
    }
}

/// Generic check runners.
enum CheckRunner {
    EbuildPkg(EbuildPkgCheckRunner),
    EbuildRawPkg(EbuildRawPkgCheckRunner),
    Cpn(CpnCheckRunner),
    Cpv(CpvCheckRunner),
    Repo(RepoCheckRunner),
}

impl CheckRunner {
    fn new(
        scope: Scope,
        restrict: &Restrict,
        source: SourceKind,
        repo: &'static EbuildRepo,
        filters: IndexSet<PkgFilter>,
    ) -> Self {
        match (scope, source) {
            (Scope::Package | Scope::Version, SourceKind::EbuildPkg) => {
                Self::EbuildPkg(EbuildPkgCheckRunner::new(repo, Some(restrict), filters))
            }
            (_, SourceKind::EbuildPkg) => {
                Self::EbuildPkg(EbuildPkgCheckRunner::new(repo, None, filters))
            }
            (Scope::Package | Scope::Version, SourceKind::EbuildRawPkg) => {
                Self::EbuildRawPkg(EbuildRawPkgCheckRunner::new(repo, Some(restrict), filters))
            }
            (_, SourceKind::EbuildRawPkg) => {
                Self::EbuildRawPkg(EbuildRawPkgCheckRunner::new(repo, None, filters))
            }
            (_, SourceKind::Cpn) => Self::Cpn(CpnCheckRunner::new(repo)),
            (_, SourceKind::Cpv) => Self::Cpv(CpvCheckRunner::new(repo)),
            (_, SourceKind::Repo) => Self::Repo(RepoCheckRunner::new(repo)),
        }
    }

    /// Return the iterator of registered checks.
    fn iter(&self) -> Box<dyn Iterator<Item = Check> + '_> {
        match self {
            Self::EbuildPkg(r) => Box::new(r.iter()),
            Self::EbuildRawPkg(r) => Box::new(r.iter()),
            Self::Cpn(r) => Box::new(r.iter()),
            Self::Cpv(r) => Box::new(r.iter()),
            Self::Repo(r) => Box::new(r.iter()),
        }
    }

    /// Add a check to the check runner.
    fn add_check(&mut self, check: Check) {
        match self {
            Self::EbuildPkg(r) => r.add_check(check),
            Self::EbuildRawPkg(r) => r.add_check(check),
            Self::Cpn(r) => r.add_check(check),
            Self::Cpv(r) => r.add_check(check),
            Self::Repo(r) => r.add_check(check),
        }
    }
}

/// Create generic package check runners.
macro_rules! make_pkg_check_runner {
    ($pkg_check_runner:ident, $pkg_runner:ty, $pkg_set_runner:ty, $source:ty, $pkg:ty) => {
        /// Check runner for package checks.
        struct $pkg_check_runner {
            pkg_checks: IndexMap<Check, $pkg_runner>,
            pkg_set_checks: IndexMap<Check, $pkg_set_runner>,
            source: $source,
            cache: PkgCache<$pkg>,
            repo: &'static EbuildRepo,
        }

        impl $pkg_check_runner {
            fn new(
                repo: &'static EbuildRepo,
                restrict: Option<&Restrict>,
                filters: IndexSet<PkgFilter>,
            ) -> Self {
                let source = <$source>::new(repo, filters);
                // create pkg cache when running in pkg or version scope
                let cache = if let Some(restrict) = restrict {
                    PkgCache::new(&source, restrict)
                } else {
                    Default::default()
                };

                Self {
                    pkg_checks: Default::default(),
                    pkg_set_checks: Default::default(),
                    source,
                    cache,
                    repo,
                }
            }

            /// Add a check to the check runner.
            fn add_check(&mut self, check: Check) {
                match &check.scope {
                    Scope::Version => {
                        self.pkg_checks.insert(check, check.to_runner(self.repo));
                    }
                    Scope::Package => {
                        self.pkg_set_checks
                            .insert(check, check.to_runner(self.repo));
                    }
                    _ => unreachable!("unsupported check: {check}"),
                }
            }

            /// Return the iterator of registered checks.
            fn iter(&self) -> impl Iterator<Item = Check> + '_ {
                self.pkg_checks
                    .keys()
                    .chain(self.pkg_set_checks.keys())
                    .cloned()
            }

            /// Run all checks for a Cpn.
            fn run_checks(&self, cpn: &Cpn, filter: &mut ReportFilter) {
                let source = &self.source;
                let mut pkgs = Ok(vec![]);

                for result in source.iter_restrict(cpn) {
                    match result {
                        Ok(pkg) => {
                            for (check, runner) in &self.pkg_checks {
                                let now = Instant::now();
                                runner.run(&pkg, filter);
                                debug!("{check}: {pkg}: {:?}", now.elapsed());
                            }

                            if !self.pkg_set_checks.is_empty() {
                                if let Ok(pkgs) = pkgs.as_mut() {
                                    pkgs.push(pkg);
                                }
                            }
                        }
                        Err(e) => pkgs = Err(e),
                    }
                }

                // TODO: Replace is_empty() usage with debug_assert!() once iter_cpn_restrict()
                // ignores empty pkgs.
                match &pkgs {
                    Ok(pkgs) if !pkgs.is_empty() => {
                        for (check, runner) in &self.pkg_set_checks {
                            let now = Instant::now();
                            runner.run(cpn, pkgs, filter);
                            debug!("{check}: {cpn}: {:?}", now.elapsed());
                        }
                    }
                    Ok(_) => (),
                    Err(e) => warn!("skipping {source} set checks due to {e}"),
                }
            }

            /// Run a check for a Cpn.
            fn run_pkg_set(&self, check: &Check, cpn: &Cpn, filter: &mut ReportFilter) {
                match self.cache.get_pkgs() {
                    Ok(pkgs) => {
                        debug_assert!(!pkgs.is_empty(), "no matching packages: {cpn}");
                        let runner = self
                            .pkg_set_checks
                            .get(check)
                            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
                        let now = Instant::now();
                        runner.run(cpn, pkgs, filter);
                        debug!("{check}: {cpn}: {:?}", now.elapsed());
                    }
                    Err(e) => warn!("{check}: skipping due to {e}"),
                }
            }

            /// Run a check for a Cpv.
            fn run_check(&self, check: &Check, cpv: &Cpv, filter: &mut ReportFilter) {
                match self.cache.get_pkg(cpv) {
                    Some(Ok(pkg)) => {
                        let now = Instant::now();
                        let runner = self
                            .pkg_checks
                            .get(check)
                            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
                        runner.run(pkg, filter);
                        debug!("{check}: {cpv}: {:?}", now.elapsed());
                    }
                    Some(Err(e)) => warn!("{check}: skipping due to {e}"),
                    None => warn!("{check}: skipping due to filtered pkg: {cpv}"),
                }
            }
        }
    };
}

// Check runner for ebuild package checks.
make_pkg_check_runner!(
    EbuildPkgCheckRunner,
    EbuildPkgRunner,
    EbuildPkgSetRunner,
    EbuildPkgSource,
    EbuildPkg
);

// Check runner for raw ebuild package checks.
make_pkg_check_runner!(
    EbuildRawPkgCheckRunner,
    EbuildRawPkgRunner,
    EbuildRawPkgSetRunner,
    EbuildRawPkgSource,
    EbuildRawPkg
);

/// Check runner for Cpn objects.
struct CpnCheckRunner {
    checks: IndexMap<Check, CpnRunner>,
    repo: &'static EbuildRepo,
}

impl CpnCheckRunner {
    fn new(repo: &'static EbuildRepo) -> Self {
        Self {
            checks: Default::default(),
            repo,
        }
    }

    /// Add a check to the check runner.
    fn add_check(&mut self, check: Check) {
        match &check.scope {
            Scope::Package => {
                self.checks.insert(check, check.to_runner(self.repo));
            }
            _ => unreachable!("unsupported check: {check}"),
        }
    }

    /// Return the iterator of registered checks.
    fn iter(&self) -> impl Iterator<Item = Check> + '_ {
        self.checks.keys().cloned()
    }

    /// Run all checks for a Cpn.
    fn run_checks(&self, cpn: &Cpn, filter: &mut ReportFilter) {
        for (check, runner) in &self.checks {
            let now = Instant::now();
            runner.run(cpn, filter);
            debug!("{check}: {cpn}: {:?}", now.elapsed());
        }
    }

    /// Run a check for a Cpn.
    fn run_check(&self, check: &Check, cpn: &Cpn, filter: &mut ReportFilter) {
        let runner = self
            .checks
            .get(check)
            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
        let now = Instant::now();
        runner.run(cpn, filter);
        debug!("{check}: {cpn}: {:?}", now.elapsed());
    }
}

/// Check runner for Cpv objects.
struct CpvCheckRunner {
    checks: IndexMap<Check, CpvRunner>,
    repo: &'static EbuildRepo,
}

impl CpvCheckRunner {
    fn new(repo: &'static EbuildRepo) -> Self {
        Self {
            checks: Default::default(),
            repo,
        }
    }

    /// Add a check to the check runner.
    fn add_check(&mut self, check: Check) {
        match &check.scope {
            Scope::Version => {
                self.checks.insert(check, check.to_runner(self.repo));
            }
            _ => unreachable!("unsupported check: {check}"),
        }
    }

    /// Return the iterator of registered checks.
    fn iter(&self) -> impl Iterator<Item = Check> + '_ {
        self.checks.keys().cloned()
    }

    /// Run all checks for a Cpn.
    fn run_checks(&self, cpn: &Cpn, filter: &mut ReportFilter) {
        for cpv in self.repo.iter_cpv_restrict(cpn) {
            for (check, runner) in &self.checks {
                let now = Instant::now();
                runner.run(&cpv, filter);
                debug!("{check}: {cpv}: {:?}", now.elapsed());
            }
        }
    }

    /// Run a check for a Cpv.
    fn run_check(&self, check: &Check, cpv: &Cpv, filter: &mut ReportFilter) {
        let runner = self
            .checks
            .get(check)
            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
        let now = Instant::now();
        runner.run(cpv, filter);
        debug!("{check}: {cpv}: {:?}", now.elapsed());
    }
}

/// Check runner for Repo objects.
struct RepoCheckRunner {
    checks: IndexMap<Check, RepoRunner>,
    repo: &'static EbuildRepo,
}

impl RepoCheckRunner {
    fn new(repo: &'static EbuildRepo) -> Self {
        Self {
            checks: Default::default(),
            repo,
        }
    }

    /// Add a check to the check runner.
    fn add_check(&mut self, check: Check) {
        match &check.scope {
            Scope::Repo => {
                self.checks.insert(check, check.to_runner(self.repo));
            }
            _ => unreachable!("unsupported check: {check}"),
        }
    }

    /// Return the iterator of registered checks.
    fn iter(&self) -> impl Iterator<Item = Check> + '_ {
        self.checks.keys().cloned()
    }

    /// Run a check for a repo.
    fn run_check(&self, check: &Check, repo: &EbuildRepo, filter: &mut ReportFilter) {
        let runner = self
            .checks
            .get(check)
            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
        let now = Instant::now();
        runner.run(repo, filter);
        debug!("{check}: {repo}: {:?}", now.elapsed());
    }
}
