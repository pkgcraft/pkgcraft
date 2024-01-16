use pkgcraft::repo::ebuild::Repo;
use pkgcraft::restrict::Restrict;

use crate::check::{self, Check, CheckKind, CheckRun};
use crate::report::Report;
use crate::source::{self, IterRestrict};

#[derive(Debug, Clone)]
pub(crate) struct EbuildPkgCheckRunner {
    checks: Vec<check::EbuildPkgCheck<'static>>,
    set_checks: Vec<check::EbuildPkgSetCheck<'static>>,
    source: source::EbuildPackage<'static>,
    repo: &'static Repo,
}

impl EbuildPkgCheckRunner {
    pub(crate) fn new(repo: &'static Repo) -> Self {
        Self {
            checks: Default::default(),
            set_checks: Default::default(),
            source: source::EbuildPackage { repo },
            repo,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.checks.is_empty() && self.set_checks.is_empty()
    }

    pub(crate) fn add_check(&mut self, check: &Check) {
        use CheckKind::*;
        match check.kind() {
            EbuildPkg(k) => self.checks.push(k.to_check(self.repo)),
            EbuildPkgSet(k) => self.set_checks.push(k.to_check(self.repo)),
            _ => panic!("{check} invalid for ebuild pkg check runner"),
        }
    }

    pub(crate) fn run<R: Into<Restrict>>(
        &self,
        restrict: R,
        reports: &mut Vec<Report>,
    ) -> crate::Result<()> {
        let mut pkgs = vec![];

        for pkg in self.source.iter_restrict(restrict) {
            for check in &self.checks {
                check.run(&pkg, reports)?;
            }
            pkgs.push(pkg);
        }

        if !pkgs.is_empty() {
            for check in &self.set_checks {
                check.run(&pkgs, reports)?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct EbuildRawPkgCheckRunner {
    checks: Vec<check::EbuildRawPkgCheck<'static>>,
    source: source::EbuildPackageRaw<'static>,
    repo: &'static Repo,
}

impl EbuildRawPkgCheckRunner {
    pub(crate) fn new(repo: &'static Repo) -> Self {
        Self {
            checks: Default::default(),
            source: source::EbuildPackageRaw { repo },
            repo,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.checks.is_empty()
    }

    pub(crate) fn add_check(&mut self, check: &Check) {
        use CheckKind::*;
        match check.kind() {
            EbuildRawPkg(k) => self.checks.push(k.to_check(self.repo)),
            _ => panic!("{check} invalid for ebuild raw pkg check runner"),
        }
    }

    pub(crate) fn run<R: Into<Restrict>>(
        &self,
        restrict: R,
        reports: &mut Vec<Report>,
    ) -> crate::Result<()> {
        for pkg in self.source.iter_restrict(restrict) {
            for check in &self.checks {
                check.run(&pkg, reports)?;
            }
        }

        Ok(())
    }
}
