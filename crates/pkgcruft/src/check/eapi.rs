use pkgcraft::pkg::{ebuild::Pkg, Package};
use pkgcraft::repo::ebuild::Repo;

use crate::report::{
    Report, ReportKind,
    VersionReport::{EapiBanned, EapiDeprecated},
};
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{Check, CheckKind, CheckRun, EbuildPkgCheckKind};

pub(crate) static CHECK: Check = Check {
    kind: CheckKind::EbuildPkg(EbuildPkgCheckKind::Eapi),
    source: SourceKind::Ebuild,
    scope: Scope::Package,
    priority: 0,
    reports: &[ReportKind::Version(EapiBanned), ReportKind::Version(EapiDeprecated)],
};

#[derive(Debug)]
pub(crate) struct EapiCheck<'a> {
    repo: &'a Repo,
}

impl<'a> EapiCheck<'a> {
    pub(super) fn new(repo: &'a Repo) -> Self {
        Self { repo }
    }
}

impl<'a> CheckRun<&Pkg<'a>> for EapiCheck<'a> {
    fn run(&self, pkg: &Pkg<'a>, reports: &mut Vec<Report>) {
        if self
            .repo
            .metadata()
            .config()
            .eapis_deprecated()
            .contains(pkg.eapi().as_ref())
        {
            reports.push(EapiDeprecated.report(pkg, pkg.eapi()));
        } else if self
            .repo
            .metadata()
            .config()
            .eapis_banned()
            .contains(pkg.eapi().as_ref())
        {
            reports.push(EapiBanned.report(pkg, pkg.eapi()));
        }
    }
}
