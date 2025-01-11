use pkgcraft::dep::Cpn;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::pkg::Package;
use pkgcraft::types::OrderedMap;

use crate::iter::ReportFilter;
use crate::report::ReportKind::EapiStale;

use super::EbuildPkgSetCheck;

pub(super) fn create() -> impl EbuildPkgSetCheck {
    Check
}

static CHECK: super::Check = super::Check::EapiStale;

struct Check;

super::register!(Check);

impl EbuildPkgSetCheck for Check {
    fn run(&self, _cpn: &Cpn, pkgs: &[EbuildPkg], filter: &ReportFilter) {
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
                            EapiStale.version(pkg).message(pkg.eapi()).report(filter);
                        }
                    }
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::test::*;

    use crate::scan::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // primary unfixed
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new(repo).checks([CHECK]);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let scanner = Scanner::new(repo).checks([CHECK]);
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, []);
    }
}
