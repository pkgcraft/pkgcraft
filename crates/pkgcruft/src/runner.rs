use std::fmt;
use std::sync::Arc;
use std::time::Instant;

use indexmap::IndexMap;
use pkgcraft::dep::{Cpn, Cpv};
use pkgcraft::pkg::ebuild::{EbuildPkg, EbuildRawPkg};
use pkgcraft::repo::PkgRepository;
use pkgcraft::restrict::Scope;
use tracing::{debug, warn};

use crate::check::*;
use crate::report::ReportScope;
use crate::scan::ScannerRun;
use crate::source::*;

/// Target to run checks against.
#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub(super) enum Target {
    Cpv(Cpv),
    Cpn(Cpn),
    Category(String),
    Repo,
}

impl From<Cpn> for Target {
    fn from(value: Cpn) -> Self {
        Self::Cpn(value)
    }
}

impl From<Cpv> for Target {
    fn from(value: Cpv) -> Self {
        Self::Cpv(value)
    }
}

impl From<&ReportScope> for Target {
    fn from(value: &ReportScope) -> Self {
        match value {
            ReportScope::Version(cpv, _) => cpv.cpn().clone().into(),
            ReportScope::Package(cpn) => cpn.clone().into(),
            ReportScope::Category(s) => Target::Category(s.to_string()),
            ReportScope::Repo(_) => Target::Repo,
        }
    }
}

/// Check runner for synchronous checks.
#[derive(Default)]
pub(super) struct SyncCheckRunner {
    runners: IndexMap<SourceKind, CheckRunner>,
}

impl SyncCheckRunner {
    pub(super) fn new(run: &Arc<ScannerRun>) -> Self {
        let mut runner = Self::default();

        for check in &run.checks {
            runner.add_check(*check, run);
        }

        runner
    }

    /// Add a check to the runner.
    fn add_check(&mut self, check: Check, run: &ScannerRun) {
        for source in check
            .sources()
            .iter()
            .filter(|source| source.scope() <= run.scope)
            .copied()
        {
            self.runners
                .entry(source)
                .or_insert_with(|| CheckRunner::new(source))
                .add_check(check, run)
        }
    }

    /// Run all checks for a target.
    pub(super) fn run_checks(&self, target: &Target, run: &ScannerRun) {
        for runner in self.runners.values() {
            runner.run_checks(target, run);
        }
    }

    /// Run a check for a target.
    pub(super) fn run_check(&self, check: &Check, target: &Target, run: &ScannerRun) {
        for runner in check.sources().iter().filter_map(|x| self.runners.get(x)) {
            runner.run_check(check, target, run);
        }
    }

    /// Run finalization for a target.
    pub(super) fn finish_target(&self, check: &Check, target: &Target, run: &ScannerRun) {
        for runner in check.sources().iter().filter_map(|x| self.runners.get(x)) {
            runner.finish_target(check, target, run);
        }
    }

    /// Run finalization for a check.
    pub(super) fn finish_check(&self, check: &Check, run: &ScannerRun) {
        for runner in check.sources().iter().filter_map(|x| self.runners.get(x)) {
            runner.finish_check(check, run);
        }
    }
}

/// Generic check runners.
enum CheckRunner {
    EbuildPkg(EbuildPkgCheckRunner),
    EbuildRawPkg(EbuildRawPkgCheckRunner),
    Cpn(CpnCheckRunner),
    Cpv(CpvCheckRunner),
    Category(CategoryCheckRunner),
    Repo(RepoCheckRunner),
}

impl fmt::Debug for CheckRunner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EbuildPkg(_) => write!(f, "EbuildPkgCheckRunner"),
            Self::EbuildRawPkg(_) => write!(f, "EbuildRawPkgCheckRunner"),
            Self::Cpn(_) => write!(f, "CpnCheckRunner"),
            Self::Cpv(_) => write!(f, "CpvCheckRunner"),
            Self::Category(_) => write!(f, "CategoryCheckRunner"),
            Self::Repo(_) => write!(f, "RepoCheckRunner"),
        }
    }
}

impl CheckRunner {
    fn new(source: SourceKind) -> Self {
        match source {
            SourceKind::EbuildPkg => Self::EbuildPkg(Default::default()),
            SourceKind::EbuildRawPkg => Self::EbuildRawPkg(Default::default()),
            SourceKind::Cpn => Self::Cpn(Default::default()),
            SourceKind::Cpv => Self::Cpv(Default::default()),
            SourceKind::Category => Self::Category(Default::default()),
            SourceKind::Repo => Self::Repo(Default::default()),
        }
    }

    fn add_check(&mut self, check: Check, run: &ScannerRun) {
        match self {
            Self::EbuildPkg(r) => r.add_check(check, run),
            Self::EbuildRawPkg(r) => r.add_check(check, run),
            Self::Cpn(r) => r.add_check(check, run),
            Self::Cpv(r) => r.add_check(check, run),
            Self::Category(r) => r.add_check(check, run),
            Self::Repo(r) => r.add_check(check, run),
        }
    }

    fn run_checks(&self, target: &Target, run: &ScannerRun) {
        match (self, target) {
            (Self::EbuildPkg(r), Target::Cpn(cpn)) => r.run_checks(cpn, run),
            (Self::EbuildRawPkg(r), Target::Cpn(cpn)) => r.run_checks(cpn, run),
            (Self::Cpn(r), Target::Cpn(cpn)) => r.run_checks(cpn, run),
            (Self::Cpv(r), Target::Cpn(cpn)) => r.run_checks(cpn, run),
            _ => (),
        }
    }

    fn run_check(&self, check: &Check, target: &Target, run: &ScannerRun) {
        match (self, target) {
            (Self::EbuildPkg(r), Target::Cpv(cpv)) => r.run_pkg(check, cpv, run),
            (Self::EbuildPkg(r), Target::Cpn(cpn)) => r.run_pkg_set(check, cpn, run),
            (Self::EbuildRawPkg(r), Target::Cpv(cpv)) => r.run_pkg(check, cpv, run),
            (Self::EbuildRawPkg(r), Target::Cpn(cpn)) => r.run_pkg_set(check, cpn, run),
            (Self::Cpn(r), Target::Cpn(cpn)) => r.run_check(check, cpn, run),
            (Self::Cpv(r), Target::Cpv(cpv)) => r.run_check(check, cpv, run),
            (Self::Category(r), Target::Category(cat)) => r.run_check(check, cat, run),
            (Self::Repo(r), Target::Repo) => r.run_check(check, run),
            _ => (),
        }
    }

    fn finish_target(&self, check: &Check, target: &Target, run: &ScannerRun) {
        match (self, target) {
            (Self::Cpn(r), Target::Cpn(cpn)) => r.finish_target(check, cpn, run),
            (Self::Cpv(r), Target::Cpv(cpv)) => r.finish_target(check, cpv, run),
            (Self::Category(r), Target::Category(cat)) => r.finish_target(check, cat, run),
            _ => (),
        }
    }

    fn finish_check(&self, check: &Check, run: &ScannerRun) {
        match self {
            Self::EbuildPkg(r) => r.finish_check(check, run),
            Self::EbuildRawPkg(r) => r.finish_check(check, run),
            Self::Cpn(r) => r.finish_check(check, run),
            Self::Cpv(r) => r.finish_check(check, run),
            Self::Category(r) => r.finish_check(check, run),
            _ => (),
        }
    }
}

/// Create generic package check runners.
macro_rules! make_pkg_check_runner {
    ($pkg_check_runner:ident, $pkg_runner:ty, $pkg_set_runner:ty, $source:ty, $pkg:ty) => {
        /// Check runner for package checks.
        #[derive(Default)]
        struct $pkg_check_runner {
            pkg_checks: IndexMap<Check, $pkg_runner>,
            pkg_set_checks: IndexMap<Check, $pkg_set_runner>,
            source: std::sync::OnceLock<$source>,
            cache: std::sync::OnceLock<PkgCache<$pkg>>,
        }

        impl $pkg_check_runner {
            fn source(&self, run: &ScannerRun) -> &$source {
                self.source.get_or_init(|| <$source>::new(run))
            }

            fn cache(&self, run: &ScannerRun) -> &PkgCache<$pkg> {
                self.cache
                    .get_or_init(|| PkgCache::new(self.source(run), run))
            }

            fn add_check(&mut self, check: Check, run: &ScannerRun) {
                if check.scope() == Scope::Version {
                    self.pkg_checks.insert(check, check.to_runner(run));
                } else {
                    self.pkg_set_checks.insert(check, check.to_runner(run));
                }
            }

            fn run_checks(&self, cpn: &Cpn, run: &ScannerRun) {
                let source = self.source(run);
                let mut pkgs = Ok(vec![]);

                for result in source.iter_restrict(cpn) {
                    match result {
                        Ok(pkg) => {
                            for (check, runner) in &self.pkg_checks {
                                let now = Instant::now();
                                runner.run(&pkg, run);
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

                match &pkgs {
                    Ok(pkgs) => {
                        if !pkgs.is_empty() {
                            for (check, runner) in &self.pkg_set_checks {
                                let now = Instant::now();
                                runner.run(cpn, pkgs, run);
                                debug!("{check}: {cpn}: {:?}", now.elapsed());
                            }
                        }
                    }
                    Err(e) => warn!("skipping {source} set checks due to {e}"),
                }
            }

            /// Run a check for a [`Cpv`].
            fn run_pkg(&self, check: &Check, cpv: &Cpv, run: &ScannerRun) {
                match self.cache(run).get_pkg(cpv) {
                    Some(Ok(pkg)) => {
                        let runner = self
                            .pkg_checks
                            .get(check)
                            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
                        let now = Instant::now();
                        runner.run(pkg, run);
                        debug!("{check}: {cpv}: {:?}", now.elapsed());
                    }
                    Some(Err(e)) => warn!("{check}: skipping due to {e}"),
                    None => warn!("{check}: skipping due to filtered pkg: {cpv}"),
                }
            }

            /// Run a check for a [`Cpn`].
            fn run_pkg_set(&self, check: &Check, cpn: &Cpn, run: &ScannerRun) {
                match self.cache(run).get_pkgs() {
                    Ok(pkgs) => {
                        if !pkgs.is_empty() {
                            let runner = self
                                .pkg_set_checks
                                .get(check)
                                .unwrap_or_else(|| unreachable!("unknown check: {check}"));
                            let now = Instant::now();
                            runner.run(cpn, pkgs, run);
                            debug!("{check}: {cpn}: {:?}", now.elapsed());
                        }
                    }
                    Err(e) => warn!("{check}: skipping due to {e}"),
                }
            }

            fn finish_check(&self, check: &Check, run: &ScannerRun) {
                let now = Instant::now();
                if check.scope() == Scope::Version {
                    let runner = self
                        .pkg_checks
                        .get(check)
                        .unwrap_or_else(|| unreachable!("unknown check: {check}"));
                    runner.finish_check(run);
                } else {
                    let runner = self
                        .pkg_set_checks
                        .get(check)
                        .unwrap_or_else(|| unreachable!("unknown check: {check}"));
                    runner.finish_check(run);
                }
                debug!("{check}: finish: {:?}", now.elapsed());
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

/// Check runner for [`Cpn`] objects.
#[derive(Default)]
struct CpnCheckRunner {
    checks: IndexMap<Check, CpnRunner>,
}

impl CpnCheckRunner {
    fn add_check(&mut self, check: Check, run: &ScannerRun) {
        self.checks.insert(check, check.to_runner(run));
    }

    fn run_checks(&self, cpn: &Cpn, run: &ScannerRun) {
        for (check, runner) in &self.checks {
            let now = Instant::now();
            runner.run(cpn, run);
            debug!("{check}: {cpn}: {:?}", now.elapsed());

            // run finalize methods for a target
            if check.finish_target() {
                self.finish_target(check, cpn, run);
            }
        }
    }

    fn run_check(&self, check: &Check, cpn: &Cpn, run: &ScannerRun) {
        let runner = self
            .checks
            .get(check)
            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
        let now = Instant::now();
        runner.run(cpn, run);
        debug!("{check}: {cpn}: {:?}", now.elapsed());
    }

    fn finish_target(&self, check: &Check, cpn: &Cpn, run: &ScannerRun) {
        let runner = self
            .checks
            .get(check)
            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
        let now = Instant::now();
        runner.finish_target(cpn, run);
        debug!("{check}: {cpn}: finish target: {:?}", now.elapsed());
    }

    fn finish_check(&self, check: &Check, run: &ScannerRun) {
        let runner = self
            .checks
            .get(check)
            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
        let now = Instant::now();
        runner.finish_check(run);
        debug!("{check}: finish: {:?}", now.elapsed());
    }
}

/// Check runner for [`Cpv`] objects.
#[derive(Default)]
struct CpvCheckRunner {
    checks: IndexMap<Check, CpvRunner>,
}

impl CpvCheckRunner {
    fn add_check(&mut self, check: Check, run: &ScannerRun) {
        self.checks.insert(check, check.to_runner(run));
    }

    fn run_checks(&self, cpn: &Cpn, run: &ScannerRun) {
        for cpv in run.repo.iter_cpv_restrict(cpn) {
            for (check, runner) in &self.checks {
                let now = Instant::now();
                runner.run(&cpv, run);
                debug!("{check}: {cpv}: {:?}", now.elapsed());

                // run finalize methods for a target
                if check.finish_target() {
                    self.finish_target(check, &cpv, run);
                }
            }
        }
    }

    fn run_check(&self, check: &Check, cpv: &Cpv, run: &ScannerRun) {
        let runner = self
            .checks
            .get(check)
            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
        let now = Instant::now();
        runner.run(cpv, run);
        debug!("{check}: {cpv}: {:?}", now.elapsed());
    }

    fn finish_target(&self, check: &Check, cpv: &Cpv, run: &ScannerRun) {
        let runner = self
            .checks
            .get(check)
            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
        let now = Instant::now();
        runner.finish_target(cpv, run);
        debug!("{check}: {cpv}: finish target: {:?}", now.elapsed());
    }

    fn finish_check(&self, check: &Check, run: &ScannerRun) {
        let runner = self
            .checks
            .get(check)
            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
        let now = Instant::now();
        runner.finish_check(run);
        debug!("{check}: finish: {:?}", now.elapsed());
    }
}

/// Check runner for category targets.
#[derive(Default)]
struct CategoryCheckRunner {
    checks: IndexMap<Check, CategoryRunner>,
}

impl CategoryCheckRunner {
    fn add_check(&mut self, check: Check, run: &ScannerRun) {
        self.checks.insert(check, check.to_runner(run));
    }

    fn run_check(&self, check: &Check, category: &str, run: &ScannerRun) {
        let runner = self
            .checks
            .get(check)
            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
        let now = Instant::now();
        runner.run(category, run);
        debug!("{check}: {category} {:?}", now.elapsed());
    }

    fn finish_target(&self, check: &Check, category: &str, run: &ScannerRun) {
        let runner = self
            .checks
            .get(check)
            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
        let now = Instant::now();
        runner.finish_target(category, run);
        debug!("{check}: {category}: finish target: {:?}", now.elapsed());
    }

    fn finish_check(&self, check: &Check, run: &ScannerRun) {
        let runner = self
            .checks
            .get(check)
            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
        let now = Instant::now();
        runner.finish_check(run);
        debug!("{check}: finish: {:?}", now.elapsed());
    }
}

/// Check runner for repo targets.
#[derive(Default)]
struct RepoCheckRunner {
    checks: IndexMap<Check, RepoRunner>,
}

impl RepoCheckRunner {
    fn add_check(&mut self, check: Check, run: &ScannerRun) {
        self.checks.insert(check, check.to_runner(run));
    }

    fn run_check(&self, check: &Check, run: &ScannerRun) {
        let runner = self
            .checks
            .get(check)
            .unwrap_or_else(|| unreachable!("unknown check: {check}"));
        let now = Instant::now();
        runner.run(run);
        debug!("{check}: {} {:?}", run.repo, now.elapsed());
    }
}
