use pkgcraft::dep::{Dependency, DependencySet};
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::pkg::ebuild::iuse::Iuse;

use crate::report::ReportKind::RestrictMissing;
use crate::scan::ScannerRun;

use super::EbuildPkgCheck;

pub(super) fn create() -> impl EbuildPkgCheck {
    Check {
        restricts: ["test", "!test? ( test )"]
            .iter()
            .map(|s| {
                Dependency::restrict(s)
                    .unwrap_or_else(|e| unreachable!("invalid RESTRICT: {s}: {e}"))
            })
            .collect(),
        iuse: Iuse::try_new("test").unwrap(),
    }
}

struct Check {
    restricts: DependencySet<String>,
    iuse: Iuse,
}

super::register!(Check, super::Check::RestrictTestMissing);

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, run: &ScannerRun) {
        if pkg.iuse().contains(&self.iuse)
            && pkg
                .restrict()
                .intersection(&self.restricts)
                .next()
                .is_none()
        {
            RestrictMissing
                .version(pkg)
                .message(r#"missing RESTRICT="!test? ( test )" with IUSE=test"#)
                .report(run);
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
        let scanner = Scanner::new().reports([CHECK]);

        // primary unfixed
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, []);
    }
}
