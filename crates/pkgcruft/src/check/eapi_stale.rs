use pkgcraft::dep::Cpn;
use pkgcraft::pkg::ebuild::{EbuildPackage, EbuildPkg};
use pkgcraft::pkg::Package;
use pkgcraft::types::OrderedMap;

use crate::report::ReportKind::EapiStale;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, EbuildPkgSetCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::EapiStale,
    scope: Scope::Package,
    source: SourceKind::EbuildPkg,
    reports: &[EapiStale],
    context: &[],
    priority: 0,
};

pub(super) fn create() -> impl EbuildPkgSetCheck {
    Check
}

struct Check;

super::register!(Check);

impl EbuildPkgSetCheck for Check {
    fn run(&self, _cpn: &Cpn, pkgs: &[EbuildPkg], filter: &mut ReportFilter) {
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
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{assert_unordered_eq, TEST_DATA, TEST_DATA_PATCHED};

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // primary unfixed
        let repo = TEST_DATA.repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new().checks([CHECK]);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // primary fixed
        let repo = TEST_DATA_PATCHED.repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, []);
    }
}
