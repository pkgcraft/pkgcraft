use pkgcraft::dep::Cpn;
use pkgcraft::pkg::ebuild::EbuildPkg;

use crate::report::ReportKind::LiveOnly;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckContext, CheckKind, EbuildPkgSetCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Live,
    scope: Scope::Package,
    source: SourceKind::EbuildPkg,
    reports: &[LiveOnly],
    context: &[CheckContext::Gentoo],
    priority: 0,
};

pub(super) fn create() -> impl EbuildPkgSetCheck {
    Check
}

struct Check;

super::register!(Check);

impl EbuildPkgSetCheck for Check {
    fn run(&self, cpn: &Cpn, pkgs: &[EbuildPkg], filter: &mut ReportFilter) {
        if pkgs.iter().all(|pkg| pkg.live()) {
            LiveOnly.package(cpn).report(filter);
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{assert_unordered_eq, TEST_DATA, TEST_DATA_PATCHED};

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // gentoo unfixed
        let (pool, repo) = TEST_DATA.repo("gentoo").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new(&pool).checks([CHECK]);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo, repo);
        assert_unordered_eq!(reports, expected);

        // empty repo
        let (_pool, repo) = TEST_DATA.repo("empty").unwrap();
        let reports = scanner.run(repo, repo);
        assert_unordered_eq!(reports, []);

        // gentoo fixed
        let (pool, repo) = TEST_DATA_PATCHED.repo("gentoo").unwrap();
        let scanner = Scanner::new(&pool).checks([CHECK]);
        let reports = scanner.run(repo, repo);
        assert_unordered_eq!(reports, []);
    }

    // TODO: scan with check selected vs unselected in non-gentoo repo once #194 is fixed
}
