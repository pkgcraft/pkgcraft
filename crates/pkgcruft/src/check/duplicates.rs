use std::sync::Arc;

use pkgcraft::dep::Cpn;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::repo::Repository;
use pkgcraft::traits::Contains;

use crate::report::ReportKind::PackageOverride;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckContext, CheckKind, UnversionedPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Duplicates,
    scope: Scope::Package,
    source: SourceKind::UnversionedPkg,
    reports: &[PackageOverride],
    context: &[CheckContext::Optional, CheckContext::Overlay],
    priority: 0,
};

pub(super) fn create(repo: &'static Repo) -> impl UnversionedPkgCheck {
    Check {
        repos: repo.masters().collect(),
    }
}

struct Check {
    repos: Vec<Arc<Repo>>,
}

super::register!(Check);

impl UnversionedPkgCheck for Check {
    fn run(&self, cpn: &Cpn, filter: &mut ReportFilter) {
        for repo in &self.repos {
            if repo.contains(cpn) {
                let message = format!("repo: {}", repo.name());
                filter.report(PackageOverride.package(cpn, message));
            }
        }
    }
}
