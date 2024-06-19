use std::sync::Arc;

use pkgcraft::pkg::ebuild::raw::Pkg;
use pkgcraft::pkg::Package;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::repo::Repository;
use pkgcraft::traits::Contains;

use crate::report::ReportKind::PackageOverride;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckContext, CheckKind, RawPackageCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Duplicates,
    scope: Scope::Package,
    source: SourceKind::EbuildRaw,
    reports: &[PackageOverride],
    context: &[CheckContext::Optional, CheckContext::Overlay],
    priority: 0,
};

pub(super) fn create(repo: &'static Repo) -> impl RawPackageCheck {
    Check {
        repos: repo.masters().collect(),
    }
}

struct Check {
    repos: Vec<Arc<Repo>>,
}

super::register!(Check);

impl RawPackageCheck for Check {
    fn run(&self, pkgs: &[Pkg], filter: &mut ReportFilter) {
        let cpn = pkgs[0].cpn();
        for repo in &self.repos {
            if repo.contains(cpn) {
                let message = format!("repo: {}", repo.name());
                filter.report(PackageOverride.package(pkgs, message));
            }
        }
    }
}
