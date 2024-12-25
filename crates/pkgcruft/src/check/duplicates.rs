use pkgcraft::dep::Cpn;
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::repo::Repository;
use pkgcraft::restrict::Scope;
use pkgcraft::traits::Contains;

use crate::report::ReportKind::PackageOverride;
use crate::scanner::ReportFilter;
use crate::source::SourceKind;

use super::{CheckContext, CheckKind, CpnCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Duplicates,
    scope: Scope::Package,
    source: SourceKind::Cpn,
    reports: &[PackageOverride],
    context: &[CheckContext::Optional, CheckContext::Overlay],
};

pub(super) fn create(repo: &EbuildRepo) -> impl CpnCheck {
    Check { repo: repo.clone() }
}

struct Check {
    repo: EbuildRepo,
}

impl CpnCheck for Check {
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

#[cfg(test)]
mod tests {
    use pkgcraft::test::*;

    use crate::scanner::Scanner;

    use super::*;

    #[test]
    fn check() {
        let data = test_data();
        let repo = data.ebuild_repo("qa-secondary").unwrap();
        let scanner = Scanner::new(repo).checks([CHECK]);
        let reports: Vec<_> = scanner.run(repo).unwrap().collect();
        assert!(!reports.is_empty());
    }
}
