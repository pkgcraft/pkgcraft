use std::sync::Arc;

use pkgcraft::dep::Cpn;
use pkgcraft::pkg::ebuild::raw::Pkg;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::repo::Repository;
use pkgcraft::traits::Contains;

use crate::report::ReportKind::PackageOverride;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckContext, CheckKind, RawPackageSetCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Duplicates,
    scope: Scope::Package,
    source: SourceKind::EbuildRaw,
    reports: &[PackageOverride],
    context: &[CheckContext::Optional, CheckContext::Overlay],
    priority: 0,
};

pub(super) fn create(repo: &'static Repo) -> impl RawPackageSetCheck {
    Check {
        repos: repo.masters().collect(),
    }
}

struct Check {
    repos: Vec<Arc<Repo>>,
}

super::register!(Check);

impl RawPackageSetCheck for Check {
    fn run(&self, cpn: &Cpn, _pkgs: &[Pkg], filter: &mut ReportFilter) {
        for repo in &self.repos {
            if repo.contains(cpn) {
                let message = format!("repo: {}", repo.name());
                filter.report(PackageOverride.package(cpn, message));
            }
        }
    }
}
