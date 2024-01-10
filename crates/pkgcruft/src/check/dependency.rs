use indexmap::IndexSet;
use pkgcraft::dep::{Dep, Flatten};
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::pkg::Package;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::traits::Intersects;

use crate::report::{PackageReport, Report, ReportKind};
use crate::source::SourceKind;

use super::{Check, CheckKind, CheckRun, Scope};

pub(crate) static CHECK: Check = Check {
    kind: CheckKind::Dependency,
    source: SourceKind::EbuildPackage,
    scope: Scope::Package,
    priority: 0,
    reports: &[ReportKind::Package(PackageReport::DeprecatedDependency)],
};

#[derive(Debug, Clone)]
pub(crate) struct DependencyCheck<'a> {
    pkg_deprecated: &'a IndexSet<Dep<String>>,
}

impl<'a> DependencyCheck<'a> {
    pub(super) fn new(repo: &'a Repo) -> Self {
        Self {
            pkg_deprecated: repo.metadata().pkg_deprecated(),
        }
    }

    fn deprecated(&self, dep: &Dep<String>) -> bool {
        self.pkg_deprecated.iter().any(|x| dep.intersects(x))
    }
}

impl<'a> CheckRun<Pkg<'a>> for DependencyCheck<'_> {
    fn run(&self, pkg: &Pkg<'a>, reports: &mut Vec<Report>) -> crate::Result<()> {
        use PackageReport::*;

        for key in pkg.eapi().dep_keys() {
            for dep in pkg.dependencies(&[*key]).into_iter_flatten() {
                if dep.blocker().is_none() && self.deprecated(dep) {
                    reports.push(DeprecatedDependency.report(pkg, format!("{key}: {dep}")));
                }
            }
        }

        Ok(())
    }
}
