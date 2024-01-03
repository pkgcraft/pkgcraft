use crossbeam_channel::Sender;
use indexmap::IndexSet;
use pkgcraft::dep::{Dep, Flatten, Version};
use pkgcraft::pkg::ebuild::metadata::Key;
//use pkgcraft::pkg::ebuild::raw::Pkg;
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::pkg::Package;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::traits::Intersects;

use crate::report::{Report, ReportKind};
use crate::source::SourceKind;

use super::{Check, CheckKind, CheckRun, Scope};

pub struct DeprecatedDependency {
    category: String,
    package: String,
    version: Version<String>,
    key: Key,
    dep: Dep<String>,
}

impl std::fmt::Display for DeprecatedDependency {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}/{}-{}: DeprecatedDependency: {}: {}",
            self.category, self.package, self.version, self.key, self.dep
        )
    }
}

impl From<DeprecatedDependency> for Report {
    fn from(value: DeprecatedDependency) -> Self {
        Self::DeprecatedDependency(value)
    }
}

pub(crate) static CHECK: Check = Check {
    kind: CheckKind::Dependency,
    source: SourceKind::EbuildPackage,
    scope: Scope::Package,
    priority: 0,
    reports: &[ReportKind::DeprecatedDependency],
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
    fn run(&self, pkg: &Pkg<'a>, tx: &Sender<Report>) -> crate::Result<()> {
        for key in pkg.eapi().dep_keys() {
            for dep in pkg.dependencies(&[*key]).into_iter_flatten() {
                if dep.blocker().is_none() && self.deprecated(dep) {
                    tx.send(
                        DeprecatedDependency {
                            category: pkg.cpv().category().to_string(),
                            package: pkg.cpv().package().to_string(),
                            version: pkg.cpv().version().clone(),
                            key: *key,
                            dep: dep.clone(),
                        }
                        .into(),
                    )
                    .unwrap();
                }
            }
        }

        Ok(())
    }
}
