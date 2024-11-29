use std::time::Instant;

use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use pkgcraft::dep::{Cpn, Cpv};
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::repo::PkgRepository;
use tracing::{debug, trace, warn};

use crate::bash;
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
        repo: &'static EbuildRepo,
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
                    warn!("{c}: disabled due to package filtering");
                    false
                } else {
                    true
                }
            })
            .filter(|c| {
                if !c.enabled(repo, checks) {
                    warn!("{c}: disabled due to scan context");
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
                .or_insert_with(|| CheckRunner::new(check.source, repo, filters.clone()))
                .add_check(check);
        }

        Self { runners }
    }

    /// Return the iterator of registered checks for a given [`Scope`].
    pub(super) fn checks(&self, scope: Scope) -> impl Iterator<Item = Check> + '_ {
        self.runners
            .values()
            .flat_map(|r| r.iter())
            .filter(move |c| c.scope == scope)
    }

    /// Run all check runners in order of priority.
    pub(super) fn run(&self, target: Target, filter: &mut ReportFilter) {
        for (source, runner) in &self.runners {
            match (runner, &target) {
                (CheckRunner::EbuildPkg(r), Target::Cpn(cpn)) => r.run_cpn(cpn, filter),
                (CheckRunner::EbuildRawPkg(r), Target::Cpn(cpn)) => r.run_cpn(cpn, filter),
                (CheckRunner::Cpn(r), Target::Cpn(cpn)) => r.run_cpn(cpn, filter),
                (CheckRunner::Cpv(r), Target::Cpn(cpn)) => r.run_cpn(cpn, filter),
                _ => trace!("skipping incompatible target {target} for source: {source:?}"),
            }
        }
    }

    /// Run a specific check.
    pub(super) fn run_check(&self, check: Check, target: Target, filter: &mut ReportFilter) {
        if let Some(runner) = self.runners.get(&check.source) {
            match (runner, &target) {
                (CheckRunner::EbuildPkg(r), Target::Cpv(cpv)) => r.run_cpv(&check, cpv, filter),
                (CheckRunner::EbuildPkg(r), Target::Cpn(cpn)) => r.run_pkg_set(&check, cpn, filter),
                (CheckRunner::EbuildRawPkg(r), Target::Cpv(cpv)) => r.run_cpv(&check, cpv, filter),
                (CheckRunner::Cpv(r), Target::Cpv(cpv)) => r.run_cpv(&check, cpv, filter),
                _ => panic!("incompatible target {target} for check: {check}"),
            }
        }
    }
}

/// Generic check runners.
enum CheckRunner {
    EbuildPkg(EbuildPkgCheckRunner),
    EbuildRawPkg(EbuildRawPkgCheckRunner),
    Cpn(CpnCheckRunner),
    Cpv(CpvCheckRunner),
}

impl CheckRunner {
    fn new(source: SourceKind, repo: &'static EbuildRepo, filters: IndexSet<PkgFilter>) -> Self {
        match source {
            SourceKind::EbuildPkg => Self::EbuildPkg(EbuildPkgCheckRunner::new(repo, filters)),
            SourceKind::EbuildRawPkg => {
                Self::EbuildRawPkg(EbuildRawPkgCheckRunner::new(repo, filters))
            }
            SourceKind::Cpn => Self::Cpn(CpnCheckRunner::new(repo)),
            SourceKind::Cpv => Self::Cpv(CpvCheckRunner::new(repo)),
        }
    }

    /// Return the iterator of registered checks.
    fn iter(&self) -> Box<dyn Iterator<Item = Check> + '_> {
        match self {
            Self::EbuildPkg(r) => Box::new(r.iter()),
            Self::EbuildRawPkg(r) => Box::new(r.iter()),
            Self::Cpn(r) => Box::new(r.iter()),
            Self::Cpv(r) => Box::new(r.iter()),
        }
    }

    /// Add a check to the check runner.
    fn add_check(&mut self, check: Check) {
        match self {
            Self::EbuildPkg(r) => r.add_check(check),
            Self::EbuildRawPkg(r) => r.add_check(check),
            Self::Cpn(r) => r.add_check(check),
            Self::Cpv(r) => r.add_check(check),
        }
    }
}

/// Check runner for ebuild package checks.
struct EbuildPkgCheckRunner {
    pkg_checks: IndexMap<Check, EbuildPkgRunner>,
    pkg_set_checks: IndexMap<Check, EbuildPkgSetRunner>,
    source: EbuildPkgSource,
    repo: &'static EbuildRepo,
}

impl EbuildPkgCheckRunner {
    fn new(repo: &'static EbuildRepo, filters: IndexSet<PkgFilter>) -> Self {
        Self {
            pkg_checks: Default::default(),
            pkg_set_checks: Default::default(),
            source: EbuildPkgSource::new(repo, filters),
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

    /// Run the check runner for a given Cpn.
    fn run_cpn(&self, cpn: &Cpn, filter: &mut ReportFilter) {
        let mut pkgs = vec![];

        for pkg in self.source.iter_restrict(cpn) {
            for (check, runner) in &self.pkg_checks {
                let now = Instant::now();
                runner.run(&pkg, filter);
                debug!("{check}: {pkg}: {:?}", now.elapsed());
            }

            if !self.pkg_set_checks.is_empty() {
                pkgs.push(pkg);
            }
        }

        // TODO: Consider skipping package set checks if an error is returned during
        // iteration, for example if any package throws a MetadataError the package level
        // checks will be missing that package and thus may be incorrect.
        if !pkgs.is_empty() {
            for (check, runner) in &self.pkg_set_checks {
                let now = Instant::now();
                runner.run(cpn, &pkgs, filter);
                debug!("{check}: {cpn}: {:?}", now.elapsed());
            }
        }
    }

    /// Run a specific check for a given package set.
    fn run_pkg_set(&self, check: &Check, cpn: &Cpn, filter: &mut ReportFilter) {
        if let Some(runner) = self.pkg_set_checks.get(check) {
            let pkgs: Vec<_> = self.source.iter_restrict(cpn).collect();
            let now = Instant::now();
            runner.run(cpn, &pkgs, filter);
            debug!("{check}: {cpn}: {:?}", now.elapsed());
        }
    }

    /// Run a specific check for a given Cpv.
    fn run_cpv(&self, check: &Check, cpv: &Cpv, filter: &mut ReportFilter) {
        if let Some(runner) = self.pkg_checks.get(check) {
            for pkg in self.source.iter_restrict(cpv) {
                let now = Instant::now();
                runner.run(&pkg, filter);
                debug!("{check}: {cpv}: {:?}", now.elapsed());
            }
        }
    }
}

/// Check runner for raw ebuild package checks.
struct EbuildRawPkgCheckRunner {
    checks: IndexMap<Check, EbuildRawPkgRunner>,
    source: EbuildRawPkgSource,
    repo: &'static EbuildRepo,
}

impl EbuildRawPkgCheckRunner {
    fn new(repo: &'static EbuildRepo, filters: IndexSet<PkgFilter>) -> Self {
        Self {
            checks: Default::default(),
            source: EbuildRawPkgSource::new(repo, filters),
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

    /// Run the check runner for a given Cpn.
    fn run_cpn(&self, cpn: &Cpn, filter: &mut ReportFilter) {
        for pkg in self.source.iter_restrict(cpn) {
            let tree = bash::lazy_parse(pkg.data().as_bytes());
            for (check, runner) in &self.checks {
                let now = Instant::now();
                runner.run(&pkg, &tree, filter);
                debug!("{check}: {pkg}: {:?}", now.elapsed());
            }
        }
    }

    /// Run a specific check for a given Cpv.
    fn run_cpv(&self, check: &Check, cpv: &Cpv, filter: &mut ReportFilter) {
        if let Some(runner) = self.checks.get(check) {
            for pkg in self.source.iter_restrict(cpv) {
                let tree = bash::lazy_parse(pkg.data().as_bytes());
                let now = Instant::now();
                runner.run(&pkg, &tree, filter);
                debug!("{check}: {cpv}: {:?}", now.elapsed());
            }
        }
    }
}

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

    /// Run the check runner for a given Cpn.
    fn run_cpn(&self, cpn: &Cpn, filter: &mut ReportFilter) {
        for (check, runner) in &self.checks {
            let now = Instant::now();
            runner.run(cpn, filter);
            debug!("{check}: {cpn}: {:?}", now.elapsed());
        }
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

    /// Run the check runner for a given Cpn.
    fn run_cpn(&self, cpn: &Cpn, filter: &mut ReportFilter) {
        for cpv in self.repo.iter_cpv_restrict(cpn) {
            for (check, runner) in &self.checks {
                let now = Instant::now();
                runner.run(&cpv, filter);
                debug!("{check}: {cpv}: {:?}", now.elapsed());
            }
        }
    }

    /// Run a specific check for a given Cpv.
    fn run_cpv(&self, check: &Check, cpv: &Cpv, filter: &mut ReportFilter) {
        if let Some(runner) = self.checks.get(check) {
            let now = Instant::now();
            runner.run(cpv, filter);
            debug!("{check}: {cpv}: {:?}", now.elapsed());
        }
    }
}
