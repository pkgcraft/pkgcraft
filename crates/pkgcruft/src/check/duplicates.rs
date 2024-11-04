use pkgcraft::dep::Cpn;
use pkgcraft::repo::ebuild::EbuildRepo;
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

pub(super) fn create(repo: &'static EbuildRepo) -> impl UnversionedPkgCheck {
    Check { repo }
}

struct Check {
    repo: &'static EbuildRepo,
}

super::register!(Check);

impl UnversionedPkgCheck for Check {
    fn run(&self, cpn: &Cpn, filter: &mut ReportFilter) {
        for repo in self.repo.masters() {
            if repo.contains(cpn) {
                PackageOverride
                    .package(cpn)
                    .message(format!("repo: {}", repo.name()))
                    .report(filter);
            }
        }
    }
}
