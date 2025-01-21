use pkgcraft::dep::Cpn;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::pkg::Package;
use pkgcraft::types::OrderedMap;

use crate::report::ReportKind::EapiStale;
use crate::scan::ScannerRun;

use super::EbuildPkgSetCheck;

pub(super) fn create() -> impl EbuildPkgSetCheck {
    Check
}

static CHECK: super::Check = super::Check::EapiStale;

struct Check;

super::register!(Check);

impl EbuildPkgSetCheck for Check {
    fn run(&self, _cpn: &Cpn, pkgs: &[EbuildPkg], run: &ScannerRun) {
        pkgs.iter()
            .map(|pkg| (pkg.slot(), pkg))
            .collect::<OrderedMap<_, Vec<_>>>()
            .into_values()
            .for_each(|pkgs| {
                let (live, release): (Vec<_>, Vec<_>) =
                    pkgs.into_iter().partition(|pkg| pkg.live());

                if let Some(latest_release) = release.last() {
                    for pkg in live {
                        if pkg.eapi() < latest_release.eapi() {
                            EapiStale.version(pkg).message(pkg.eapi()).report(run);
                        }
                    }
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::test::{assert_err_re, test_data, test_data_patched};

    use crate::scan::Scanner;
    use crate::test::{assert_unordered_reports, glob_reports};

    use super::*;

    #[test]
    fn check() {
        let scanner = Scanner::new().reports([CHECK]);

        // check requires package scope
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let r = scanner.run(repo, "EapiStale/EapiStale-9999");
        assert_err_re!(r, "EapiStale: check requires package scope");

        // primary unfixed
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
