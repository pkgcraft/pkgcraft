use pkgcraft::pkg::ebuild::{EbuildPackage, Pkg};
use pkgcraft::pkg::Package;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::traits::Contains;
use pkgcraft::types::{OrderedMap, OrderedSet};

use crate::report::{
    Report,
    ReportKind::{self, EapiStale},
};

pub(super) static REPORTS: &[ReportKind] = &[EapiStale];

#[derive(Debug)]
pub(crate) struct Check<'a> {
    _repo: &'a Repo,
}

impl<'a> Check<'a> {
    pub(super) fn new(_repo: &'a Repo) -> Self {
        Self { _repo }
    }
}

impl<'a> super::CheckRun<&[Pkg<'a>]> for Check<'a> {
    fn run<F: FnMut(Report)>(&self, pkgs: &[Pkg<'a>], mut report: F) {
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
                            report(EapiStale.version(pkg, pkg.eapi()));
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

    use crate::check::CheckKind::EapiStale;
    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    #[test]
    fn check() {
        let repo = TEST_DATA.repo("qa-primary").unwrap();
        let check_dir = repo.path().join(EapiStale);
        let scanner = Scanner::new().jobs(1).checks([EapiStale]);
        let expected = glob_reports!("{check_dir}/*/reports.json");
        assert!(!expected.is_empty());

        // check dir restriction
        let restrict = repo.restrict_from_path(&check_dir).unwrap();
        let reports: Vec<_> = scanner.run(repo, [&restrict]).collect();
        assert_eq!(&reports, &expected);

        // repo restriction
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);
    }

    #[test]
    fn patched() {
        let repo = TEST_DATA_PATCHED.repo("qa-primary").unwrap();
        let scanner = Scanner::new().jobs(1).checks([EapiStale]);
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &[]);
    }
}
