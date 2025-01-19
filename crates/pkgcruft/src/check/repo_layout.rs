use camino::Utf8Path;
use dashmap::DashSet;
use itertools::Itertools;
use pkgcraft::dep::Cpn;
use pkgcraft::files::is_ebuild;
use pkgcraft::macros::build_path;
use pkgcraft::repo::{ebuild::EbuildRepo, PkgRepository};

use crate::report::ReportKind::{RepoCategoriesUnused, RepoCategoryEmpty, RepoPackageEmpty};
use crate::scan::ScannerRun;

use super::{CpnCheck, RepoCheck};

pub(super) fn create(run: &ScannerRun) -> Check {
    let empty_categories = if run.enabled(RepoPackageEmpty) {
        run.repo.categories().iter().map(Into::into).collect()
    } else {
        Default::default()
    };

    Check {
        repo: run.repo.clone(),
        empty_categories,
    }
}

static CHECK: super::Check = super::Check::RepoLayout;

pub(super) struct Check {
    repo: EbuildRepo,
    empty_categories: DashSet<String>,
}

super::register!(Check);

/// Determine if an ebuild file exists in a directory path.
fn find_ebuild(path: &Utf8Path) -> bool {
    path.read_dir_utf8()
        .map(|entries| entries.filter_map(Result::ok).any(|e| is_ebuild(&e)))
        .unwrap_or(false)
}

impl CpnCheck for Check {
    fn run(&self, cpn: &Cpn, run: &ScannerRun) {
        let (category, package) = (cpn.category(), cpn.package());
        let path = build_path!(&self.repo, category, package);
        if !find_ebuild(&path) {
            RepoPackageEmpty.package(cpn).report(run);
        } else {
            self.empty_categories.remove(category);
        }
    }

    fn finish_check(&self, _repo: &EbuildRepo, run: &ScannerRun) {
        for category in self.empty_categories.iter() {
            RepoCategoryEmpty.category(category.to_string()).report(run);
        }
    }
}

impl RepoCheck for Check {
    fn run(&self, repo: &EbuildRepo, run: &ScannerRun) {
        let unused = repo
            .metadata()
            .categories()
            .iter()
            .filter(|x| !repo.path().join(x).is_dir())
            .join(", ");
        if !unused.is_empty() {
            RepoCategoriesUnused.repo(repo).message(unused).report(run);
        }
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
        let scanner = Scanner::new().reports([CHECK]);

        // primary unfixed
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path();
        let expected = glob_reports!("{dir}/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, []);
    }
}
