use pkgcraft::dep::Cpn;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::restrict::Scope;

use crate::report::ReportKind::LiveOnly;
use crate::scan::ScannerRun;
use crate::source::SourceKind;

use super::Context::Gentoo;

super::register! {
    super::Check {
        kind: super::CheckKind::Live,
        reports: &[LiveOnly],
        scope: Scope::Package,
        sources: &[SourceKind::EbuildPkg],
        context: &[Gentoo],
        create,
    }
}

pub(super) fn create(_run: &ScannerRun) -> super::Runner {
    Box::new(Check)
}

struct Check;

impl super::CheckRun for Check {
    fn run_ebuild_pkg_set(&self, cpn: &Cpn, pkgs: &[EbuildPkg], run: &ScannerRun) {
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
