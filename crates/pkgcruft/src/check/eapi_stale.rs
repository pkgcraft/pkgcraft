use pkgcraft::pkg::ebuild::{EbuildPackage, Pkg};
use pkgcraft::pkg::Package;
use pkgcraft::traits::Contains;
use pkgcraft::types::{OrderedMap, OrderedSet};

use crate::report::ReportKind::EapiStale;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, PackageCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::EapiStale,
    scope: Scope::Package,
    source: SourceKind::Ebuild,
    reports: &[EapiStale],
    context: &[],
    priority: 0,
};

pub(super) fn create() -> impl PackageCheck {
    Check
}

struct Check;

super::register!(Check);

impl PackageCheck for Check {
    fn run(&self, pkgs: &[Pkg], filter: &mut ReportFilter) {
        pkgs.iter()
            .map(|pkg| (pkg.slot(), pkg))
            .collect::<OrderedMap<_, OrderedSet<_>>>()
            .values()
            .for_each(|pkgs| {
                let (live, release): (Vec<&Pkg>, Vec<&Pkg>) = pkgs
                    .into_iter()
                    .partition(|pkg| pkg.properties().contains("live"));

                if let Some(latest_release) = release.last() {
                    for pkg in live {
                        if pkg.eapi() < latest_release.eapi() {
                            filter.report(EapiStale.version(pkg, pkg.eapi()));
                        }
                    }
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{TEST_DATA, TEST_DATA_PATCHED};
    use pretty_assertions::assert_eq;

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // primary unfixed
        let repo = TEST_DATA.repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new().jobs(1).checks([CHECK]);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);

        // primary fixed
        let repo = TEST_DATA_PATCHED.repo("qa-primary").unwrap();
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &[]);
    }
}
