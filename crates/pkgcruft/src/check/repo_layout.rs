use camino::Utf8Path;
use dashmap::DashSet;
use itertools::Itertools;
use pkgcraft::dep::Cpn;
use pkgcraft::files::is_ebuild;
use pkgcraft::macros::build_path;
use pkgcraft::repo::PkgRepository;

use crate::report::ReportKind::{RepoCategoriesUnused, RepoCategoryEmpty, RepoPackageEmpty};
use crate::scan::ScannerRun;

use super::{CategoryCheck, CpnCheck};

pub(super) fn create(run: &ScannerRun) -> Check {
    let empty_categories = if run.enabled(RepoPackageEmpty) {
        run.repo.categories().iter().map(Into::into).collect()
    } else {
        Default::default()
    };

    let unused = if run.enabled(RepoCategoriesUnused) {
        run.repo
            .metadata()
            .categories()
            .iter()
            .map(Into::into)
            .collect()
    } else {
        Default::default()
    };

    Check { empty_categories, unused }
}

static CHECK: super::Check = super::Check::RepoLayout;

pub(super) struct Check {
    empty_categories: DashSet<String>,
    unused: DashSet<String>,
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
        let path = build_path!(&run.repo, category, package);
        if !find_ebuild(&path) {
            RepoPackageEmpty.package(cpn).report(run);
        } else {
            self.empty_categories.remove(category);
        }
    }

    fn finish_check(&self, run: &ScannerRun) {
        for category in self.empty_categories.iter() {
            RepoCategoryEmpty.category(category.to_string()).report(run);
        }
    }
}

impl CategoryCheck for Check {
    fn run(&self, category: &str, _run: &ScannerRun) {
        self.unused.remove(category);
    }

    fn finish_check(&self, run: &ScannerRun) {
        if run.enabled(RepoCategoriesUnused) && !self.unused.is_empty() {
            let unused = self
                .unused
                .iter()
                .map(|x| x.to_string())
                .sorted()
                .join(", ");
            RepoCategoriesUnused
                .repo(&run.repo)
                .message(unused)
                .report(run);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use pkgcraft::test::*;

    use crate::scan::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();

        // verify scanning in empty package
        let pkg_dir = repo.path().join("CategoryEmpty/PackageEmpty");
        env::set_current_dir(&pkg_dir).unwrap();
        let expected = glob_reports!("{pkg_dir}/reports.json");
        let scanner = Scanner::new();
        let reports = scanner.run(repo, &pkg_dir).unwrap();
        assert_unordered_eq!(reports, expected);

        // primary unfixed
        let dir = repo.path();
        let expected = glob_reports!("{dir}/reports.json", "{pkg_dir}/reports.json");
        let scanner = Scanner::new().reports([CHECK]);
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, []);
    }
}
