use std::time::Instant;

use indexmap::{IndexMap, IndexSet};
use pkgcraft::dep::{Cpn, Cpv};
use pkgcraft::pkg::ebuild::{EbuildPkg, EbuildRawPkg};
use pkgcraft::repo::PkgRepository;
use pkgcraft::restrict::Scope;
use tracing::warn;

use crate::check::CheckRunner;
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

impl Target {
    /// Run finalization for a target.
    fn finish(&self, runner: &CheckRunner, run: &ScannerRun) {
        match self {
            Self::Cpn(cpn) => runner.finish_cpn(cpn, run),
            Self::Cpv(cpv) => runner.finish_cpv(cpv, run),
            Self::Category(cat) => runner.finish_category(cat, run),
            _ => (),
        }
    }
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
    runners: IndexMap<SourceKind, GenericCheckRunner>,
}

impl SyncCheckRunner {
    pub(super) fn new(run: &ScannerRun) -> Self {
        let mut runner = Self::default();

        for r in &run.runners {
            runner.add_runner(r, run);
        }

        runner
    }

    /// Add a check to the runner.
    fn add_runner(&mut self, runner: &CheckRunner, run: &ScannerRun) {
        for source in runner
            .check
            .sources
            .iter()
            .filter(|source| source.scope() <= run.scope)
            .copied()
        {
            self.runners
                .entry(source)
                .or_insert_with(|| GenericCheckRunner::new(source, run))
                .add_runner(runner.clone())
        }
    }

    /// Run all checks for a target.
    pub(super) fn run_checks(&self, target: &Target, run: &ScannerRun) {
        for runner in self.runners.values() {
            runner.run_checks(target, run);
        }
    }

    /// Run a check for a target.
    pub(super) fn run(&self, runner: &CheckRunner, target: &Target, run: &ScannerRun) {
        for r in runner
            .check
            .sources
            .iter()
            .filter_map(|x| self.runners.get(x))
        {
            r.run(runner, target, run);
        }
    }

    /// Run finalization for a target.
    pub(super) fn finish_target(
        &self,
        runner: &CheckRunner,
        target: &Target,
        run: &ScannerRun,
    ) {
        let now = Instant::now();
        target.finish(runner, run);
        *run.stats.entry(runner.check).or_default() += now.elapsed();
    }

    /// Run finalization for a check.
    ///
    /// This is only run once even if a check has multiple source variants.
    pub(super) fn finish_check(&self, runner: &CheckRunner, run: &ScannerRun) {
        let now = Instant::now();
        runner.finish(run);
        *run.stats.entry(runner.check).or_default() += now.elapsed();
    }
}

/// Generic check runners.
enum GenericCheckRunner {
    EbuildPkg(EbuildPkgCheckRunner),
    EbuildRawPkg(EbuildRawPkgCheckRunner),
    Cpn(CpnCheckRunner),
    Cpv(CpvCheckRunner),
    Category,
    Repo,
}

impl GenericCheckRunner {
    fn new(source: SourceKind, run: &ScannerRun) -> Self {
        match source {
            SourceKind::EbuildPkg => Self::EbuildPkg(EbuildPkgCheckRunner::new(run)),
            SourceKind::EbuildRawPkg => Self::EbuildRawPkg(EbuildRawPkgCheckRunner::new(run)),
            SourceKind::Cpn => Self::Cpn(Default::default()),
            SourceKind::Cpv => Self::Cpv(Default::default()),
            SourceKind::Category => Self::Category,
            SourceKind::Repo => Self::Repo,
        }
    }

    fn add_runner(&mut self, runner: CheckRunner) {
        match self {
            Self::EbuildPkg(r) => r.add_runner(runner),
            Self::EbuildRawPkg(r) => r.add_runner(runner),
            Self::Cpn(r) => r.add_runner(runner),
            Self::Cpv(r) => r.add_runner(runner),
            Self::Category => (),
            Self::Repo => (),
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

    fn run(&self, runner: &CheckRunner, target: &Target, run: &ScannerRun) {
        let now = Instant::now();
        match (self, target) {
            (Self::EbuildPkg(r), Target::Cpv(cpv)) => r.run_pkg(runner, cpv, run),
            (Self::EbuildPkg(r), Target::Cpn(cpn)) => r.run_pkg_set(runner, cpn, run),
            (Self::EbuildRawPkg(r), Target::Cpv(cpv)) => r.run_pkg(runner, cpv, run),
            (Self::EbuildRawPkg(r), Target::Cpn(cpn)) => r.run_pkg_set(runner, cpn, run),
            (Self::Cpn(_), Target::Cpn(cpn)) => runner.run_cpn(cpn, run),
            (Self::Cpv(_), Target::Cpv(cpv)) => runner.run_cpv(cpv, run),
            (Self::Category, Target::Category(cat)) => runner.run_category(cat, run),
            (Self::Repo, Target::Repo) => runner.run_repo(run),
            _ => (),
        }
        *run.stats.entry(runner.check).or_default() += now.elapsed();
    }
}

/// Create generic package check runners.
macro_rules! make_pkg_check_runner {
    ($pkg_check_runner:ident, $source:ty, $pkg:ty) => {
        /// Check runner for package checks.
        struct $pkg_check_runner {
            pkg_runners: IndexSet<CheckRunner>,
            pkg_set_runners: IndexSet<CheckRunner>,
            source: $source,
            cache: std::sync::OnceLock<PkgCache<$pkg>>,
        }

        impl $pkg_check_runner {
            fn new(run: &ScannerRun) -> Self {
                Self {
                    pkg_runners: Default::default(),
                    pkg_set_runners: Default::default(),
                    source: <$source>::new(run),
                    cache: Default::default(),
                }
            }

            fn cache(&self, run: &ScannerRun) -> &PkgCache<$pkg> {
                self.cache.get_or_init(|| PkgCache::new(&self.source, run))
            }

            fn add_runner(&mut self, runner: CheckRunner) {
                if runner.check.scope == Scope::Version {
                    self.pkg_runners.insert(runner);
                } else {
                    self.pkg_set_runners.insert(runner);
                }
            }

            fn run_checks(&self, cpn: &Cpn, run: &ScannerRun) {
                let source = &self.source;
                let mut pkgs = Ok(vec![]);

                for result in source.iter_restrict(cpn) {
                    match result {
                        Ok(pkg) => {
                            for runner in &self.pkg_runners {
                                let now = Instant::now();
                                source.run_pkg(runner, &pkg, run);
                                *run.stats.entry(runner.check).or_default() += now.elapsed();
                            }

                            if !self.pkg_set_runners.is_empty()
                                && let Ok(pkgs) = pkgs.as_mut()
                            {
                                pkgs.push(pkg);
                            }
                        }
                        Err(e) => pkgs = Err(e),
                    }
                }

                match &pkgs {
                    Ok(pkgs) => {
                        if !pkgs.is_empty() {
                            for runner in &self.pkg_set_runners {
                                let now = Instant::now();
                                source.run_pkg_set(runner, cpn, pkgs, run);
                                *run.stats.entry(runner.check).or_default() += now.elapsed();
                            }
                        }
                    }
                    Err(e) => warn!("skipping {source} set checks due to {e}"),
                }
            }

            /// Run a check for a [`Cpv`].
            fn run_pkg(&self, runner: &CheckRunner, cpv: &Cpv, run: &ScannerRun) {
                match self.cache(run).get_pkg(cpv) {
                    Some(Ok(pkg)) => self.source.run_pkg(runner, pkg, run),
                    Some(Err(e)) => warn!("{runner}: skipping due to {e}"),
                    None => warn!("{runner}: skipping due to filtered pkg: {cpv}"),
                }
            }

            /// Run a check for a [`Cpn`].
            fn run_pkg_set(&self, runner: &CheckRunner, cpn: &Cpn, run: &ScannerRun) {
                match self.cache(run).get_pkgs() {
                    Ok(pkgs) => {
                        if !pkgs.is_empty() {
                            self.source.run_pkg_set(runner, cpn, pkgs, run);
                        }
                    }
                    Err(e) => warn!("{runner}: skipping due to {e}"),
                }
            }
        }
    };
}

// Check runner for ebuild package checks.
make_pkg_check_runner!(EbuildPkgCheckRunner, EbuildPkgSource, EbuildPkg);

// Check runner for raw ebuild package checks.
make_pkg_check_runner!(EbuildRawPkgCheckRunner, EbuildRawPkgSource, EbuildRawPkg);

/// Check runner for [`Cpn`] objects.
#[derive(Default)]
struct CpnCheckRunner {
    runners: IndexSet<CheckRunner>,
}

impl CpnCheckRunner {
    fn add_runner(&mut self, runner: CheckRunner) {
        self.runners.insert(runner);
    }

    fn run_checks(&self, cpn: &Cpn, run: &ScannerRun) {
        for runner in &self.runners {
            let now = Instant::now();
            runner.run_cpn(cpn, run);

            // run finalize methods for a target
            if runner.check.finish_target() {
                runner.finish_cpn(cpn, run);
            }

            *run.stats.entry(runner.check).or_default() += now.elapsed();
        }
    }
}

/// Check runner for [`Cpv`] objects.
#[derive(Default)]
struct CpvCheckRunner {
    runners: IndexSet<CheckRunner>,
}

impl CpvCheckRunner {
    fn add_runner(&mut self, runner: CheckRunner) {
        self.runners.insert(runner);
    }

    fn run_checks(&self, cpn: &Cpn, run: &ScannerRun) {
        for cpv in run.repo.iter_cpv_restrict(cpn) {
            for runner in &self.runners {
                let now = Instant::now();
                runner.run_cpv(&cpv, run);

                // run finalize methods for a target
                if runner.check.finish_target() {
                    runner.finish_cpv(&cpv, run);
                }

                *run.stats.entry(runner.check).or_default() += now.elapsed();
            }
        }
    }
}
