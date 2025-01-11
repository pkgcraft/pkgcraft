use pkgcraft::dep::Cpn;
use pkgcraft::pkg::ebuild::EbuildPkg;

use crate::iter::ReportFilter;
use crate::report::ReportKind::LiveOnly;

use super::EbuildPkgSetCheck;

pub(super) fn create() -> impl EbuildPkgSetCheck {
    Check
}

static CHECK: super::Check = super::Check::Live;

struct Check;

super::register!(Check);

impl EbuildPkgSetCheck for Check {
    fn run(&self, cpn: &Cpn, pkgs: &[EbuildPkg], filter: &ReportFilter) {
        if pkgs.iter().all(|pkg| pkg.live()) {
            LiveOnly.package(cpn).report(filter);
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::*;

    use crate::scan::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // primary unselected
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let restrict = repo.restrict_from_path(&dir).unwrap();
        let scanner = Scanner::new(repo);
        let reports = scanner.run(&restrict).unwrap();
        assert_unordered_eq!(reports, []);

        // primary unfixed
        let scanner = Scanner::new(repo).checks([CHECK]);
        let expected = glob_reports!("{dir}/*/optional.json");
        let reports = scanner.run(&restrict).unwrap();
        assert_unordered_eq!(reports, expected);

        // gentoo unfixed
        let repo = data.ebuild_repo("gentoo").unwrap();
        let dir = repo.path().join(CHECK);
        let restrict = repo.restrict_from_path(&dir).unwrap();
        let scanner = Scanner::new(repo);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(&restrict).unwrap();
        assert_unordered_eq!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let restrict = repo.restrict_from_path(&dir).unwrap();
        let scanner = Scanner::new(repo).checks([CHECK]);
        let reports = scanner.run(restrict).unwrap();
        assert_unordered_eq!(reports, []);

        // gentoo fixed
        let repo = data.ebuild_repo("gentoo").unwrap();
        let scanner = Scanner::new(repo).checks([CHECK]);
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, []);
    }
}
