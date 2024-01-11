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

    /// Scan a repo's deprecated package list returning the first match for a dependency.
    fn deprecated(&self, dep: &Dep<String>) -> Option<&Dep<String>> {
        if dep.blocker().is_none() {
            if let Some(pkg) = self.pkg_deprecated.iter().find(|x| x.intersects(dep)) {
                match (pkg.slot_dep(), dep.slot_dep()) {
                    // deprecated pkg matches all slots
                    (None, _) => return Some(pkg),
                    // deprecated slot dep matches the dependency
                    (Some(s1), Some(s2)) if s1.slot() == s2.slot() => return Some(pkg),
                    // TODO: query slot cache for remaining mismatched variants?
                    _ => return None,
                }
            }
        }
        None
    }
}

impl<'a> CheckRun<Pkg<'a>> for DependencyCheck<'_> {
    fn run(&self, pkg: &Pkg<'a>, reports: &mut Vec<Report>) -> crate::Result<()> {
        use PackageReport::*;

        for key in pkg.eapi().dep_keys() {
            for dep in pkg.dependencies(&[*key]).into_iter_flatten() {
                if self.deprecated(dep).is_some() {
                    reports.push(DeprecatedDependency.report(pkg, format!("{key}: {dep}")));
                }
            }
        }

        Ok(())
    }
}
