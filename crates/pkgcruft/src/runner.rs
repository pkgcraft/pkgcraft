use std::time::Instant;

use indexmap::{IndexMap, IndexSet};
use itertools::{Either, Itertools};
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::repo::PkgRepository;
use tracing::debug;

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

        // filter checks
        let enabled = checks
            .iter()
            .filter(|c| {
                if !filters.is_empty() && c.scope != Scope::Version {
                    debug!("{c}: disabled due to package filtering");
                    false
                } else {
                    true
                }
            })
            // TODO: replace checks parameter with selected checks once #194 is implemented
            .filter(|c| c.enabled(repo, checks))
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

    /// Run all check runners in order of priority.
    pub(super) fn run(&self, target: &Target, filter: &mut ReportFilter) {
        for runner in self.runners.values() {
            runner.run(target, filter);
        }
    }
}

/// Generic check runners.
// TODO: remove the lint ignore once more variants are added
#[allow(clippy::enum_variant_names)]
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

    /// Add a check to the check runner.
    fn add_check(&mut self, check: Check) {
        match self {
            Self::EbuildPkg(r) => r.add_check(check),
            Self::EbuildRawPkg(r) => r.add_check(check),
            Self::Cpn(r) => r.add_check(check),
            Self::Cpv(r) => r.add_check(check),
        }
    }

    /// Run the check runner for a given restriction.
    fn run(&self, target: &Target, filter: &mut ReportFilter) {
        match self {
            Self::EbuildPkg(r) => r.run(target, filter),
            Self::EbuildRawPkg(r) => r.run(target, filter),
            Self::Cpn(r) => r.run(target, filter),
            Self::Cpv(r) => r.run(target, filter),
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

    /// Run the check runner for a given restriction.
    fn run(&self, target: &Target, filter: &mut ReportFilter) {
        let mut pkgs = vec![];

        for pkg in self.source.iter_restrict(target) {
            for (check, runner) in &self.pkg_checks {
                let now = Instant::now();
                runner.run(&pkg, filter);
                debug!("{check}: {pkg}: {:?}", now.elapsed());
            }

            if !self.pkg_set_checks.is_empty() {
                pkgs.push(pkg);
            }
        }

        if let Target::Cpn(cpn) = target {
            if !pkgs.is_empty() {
                for (check, runner) in &self.pkg_set_checks {
                    let now = Instant::now();
                    runner.run(cpn, &pkgs, filter);
                    debug!("{check}: {cpn}: {:?}", now.elapsed());
                }
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

    /// Run the check runner for a given restriction.
    fn run(&self, target: &Target, filter: &mut ReportFilter) {
        for pkg in self.source.iter_restrict(target) {
            let tree = bash::lazy_parse(pkg.data().as_bytes());
            for (check, runner) in &self.checks {
                let now = Instant::now();
                runner.run(&pkg, &tree, filter);
                debug!("{check}: {pkg}: {:?}", now.elapsed());
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

    /// Run the check runner for a given restriction.
    fn run(&self, target: &Target, filter: &mut ReportFilter) {
        if let Target::Cpn(cpn) = target {
            for (check, runner) in &self.checks {
                let now = Instant::now();
                runner.run(cpn, filter);
                debug!("{check}: {cpn}: {:?}", now.elapsed());
            }
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

    /// Run the check runner for a given restriction.
    fn run(&self, target: &Target, filter: &mut ReportFilter) {
        let cpvs = match target {
            Target::Cpn(cpn) => Either::Left(self.repo.iter_cpv_restrict(cpn)),
            Target::Cpv(cpv) => Either::Right([cpv.clone()].into_iter()),
        };

        for cpv in cpvs {
            for (check, runner) in &self.checks {
                let now = Instant::now();
                runner.run(&cpv, filter);
                debug!("{check}: {cpv}: {:?}", now.elapsed());
            }
        }
    }
}
