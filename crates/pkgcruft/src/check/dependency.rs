use std::collections::HashSet;

use itertools::Itertools;
use pkgcraft::dep::{Flatten, Operator};
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::pkg::Package;
use pkgcraft::repo::ebuild::Repo;

use crate::report::{Report, ReportKind, VersionReport};
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{Check, CheckKind, CheckRun, EbuildPkgCheckKind};

pub(crate) static CHECK: Check = Check {
    kind: CheckKind::EbuildPkg(EbuildPkgCheckKind::Dependency),
    source: SourceKind::EbuildPackage,
    scope: Scope::Version,
    priority: 0,
    reports: &[
        ReportKind::Version(VersionReport::DeprecatedDependency),
        ReportKind::Version(VersionReport::MissingRevision),
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

impl<'a> CheckRun<&Pkg<'a>> for DependencyCheck<'a> {
    fn run(&self, pkg: &Pkg<'a>, reports: &mut Vec<Report>) -> crate::Result<()> {
        use VersionReport::*;

        for key in pkg.eapi().dep_keys() {
            let mut deprecated = HashSet::new();

            for dep in pkg.dependencies(&[*key]).into_iter_flatten() {
                if self.repo.deprecated(dep).is_some() {
                    // drop use deps since package.deprecated doesn't include them
                    deprecated.insert(dep.no_use_deps());
                }

                if matches!(dep.op(), Some(Operator::Equal)) && dep.revision().is_none() {
                    reports.push(MissingRevision.report(pkg, format!("{key}: {dep}")));
                }
            }

            if !deprecated.is_empty() {
                let msg = format!("{key}: {}", deprecated.iter().sorted().join(", "));
                reports.push(DeprecatedDependency.report(pkg, msg));
            }
        }

        Ok(())
    }
}
