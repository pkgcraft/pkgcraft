use pkgcraft::dep::Cpn;
use pkgcraft::pkg::ebuild::EbuildPkg;

use crate::report::ReportKind::LiveOnly;
use crate::scan::ScannerRun;

use super::EbuildPkgSetCheck;

pub(super) fn create() -> impl EbuildPkgSetCheck {
    Check
}

struct Check;

super::register!(Check, super::Check::Live);

impl EbuildPkgSetCheck for Check {
    fn run(&self, cpn: &Cpn, pkgs: &[EbuildPkg], run: &ScannerRun) {
        if pkgs.iter().all(|pkg| pkg.live()) {
            LiveOnly.package(cpn).report(run);
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::test::{test_data, test_data_patched};

    use crate::scan::Scanner;
    use crate::test::{assert_unordered_reports, glob_reports};

    use super::*;

    #[test]
    fn check() {
        // primary unselected
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new();
        let reports = scanner.run(repo, &dir).unwrap();
        assert_unordered_reports!(reports, []);

        // primary unfixed
        let scanner = Scanner::new().reports([CHECK]);
        let expected = glob_reports!("{dir}/*/optional.json");
        let reports = scanner.run(repo, &dir).unwrap();
        assert_unordered_reports!(reports, expected);

        // gentoo unfixed
        let repo = data.ebuild_repo("gentoo").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new();
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo, &dir).unwrap();
        assert_unordered_reports!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new().reports([CHECK]);
        let reports = scanner.run(repo, &dir).unwrap();
        assert_unordered_reports!(reports, []);

        // gentoo fixed
        let repo = data.ebuild_repo("gentoo").unwrap();
        let scanner = Scanner::new().reports([CHECK]);
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, []);
    }
}
