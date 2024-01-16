use pkgcraft::dep::{Flatten, Operator};
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::pkg::Package;
use pkgcraft::repo::ebuild::Repo;

use crate::report::{PackageReport, Report, ReportKind};
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{Check, CheckKind, CheckRun};

pub(crate) static CHECK: Check = Check {
    kind: CheckKind::Dependency,
    source: SourceKind::EbuildPackage,
    scope: Scope::Version,
    priority: 0,
    reports: &[
        ReportKind::Package(PackageReport::DeprecatedDependency),
        ReportKind::Package(PackageReport::MissingRevision),
    ],
};

#[derive(Debug, Clone)]
pub(crate) struct DependencyCheck<'a> {
    repo: &'a Repo,
}

impl<'a> DependencyCheck<'a> {
    pub(super) fn new(repo: &'a Repo) -> Self {
        Self { repo }
    }
}

impl<'a> CheckRun<Pkg<'a>> for DependencyCheck<'_> {
    fn run(&self, pkg: &Pkg<'a>, reports: &mut Vec<Report>) -> crate::Result<()> {
        use PackageReport::*;

        for key in pkg.eapi().dep_keys() {
            for dep in pkg.dependencies(&[*key]).into_iter_flatten() {
                if self.repo.deprecated(dep).is_some() {
                    // drop use deps since package.deprecated doesn't include them
                    let msg = format!("{key}: {}", dep.no_use_deps());
                    reports.push(DeprecatedDependency.report(pkg, msg));
                }

                if matches!(dep.op(), Some(Operator::Equal)) && dep.revision().is_none() {
                    reports.push(MissingRevision.report(pkg, format!("{key}: {dep}")));
                }
            }
        }

        Ok(())
    }
}
