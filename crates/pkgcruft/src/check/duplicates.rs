use pkgcraft::dep::Cpn;
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::repo::Repository;
use pkgcraft::traits::Contains;

use crate::iter::ReportFilter;
use crate::report::ReportKind::PackageOverride;

use super::CpnCheck;

pub(super) fn create(repo: &EbuildRepo) -> impl CpnCheck {
    Check { repo: repo.clone() }
}

static CHECK: super::Check = super::Check::Duplicates;

struct Check {
    repo: EbuildRepo,
}

super::register!(Check);

impl CpnCheck for Check {
    fn run(&self, cpn: &Cpn, filter: &ReportFilter) {
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

    use crate::scan::Scanner;

    use super::*;

    #[test]
    fn check() {
        // primary
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let scanner = Scanner::new(repo).reports([CHECK]);
        let r = scanner.run(repo);
        assert_err_re!(r, "requires overlay context");

        // optional check isn't run by default
        let repo = data.ebuild_repo("qa-secondary").unwrap();
        let scanner = Scanner::new(repo);
        let mut reports = scanner.run(repo).unwrap();
        assert!(!reports.any(|r| CHECK.reports().contains(&r.kind)));

        // secondary
        let repo = data.ebuild_repo("qa-secondary").unwrap();
        let scanner = Scanner::new(repo).reports([CHECK]);
        let mut reports = scanner.run(repo).unwrap();
        assert!(reports.any(|r| CHECK.reports().contains(&r.kind)));
    }
}
