use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::pkg::{ebuild::Pkg, Package};
use pkgcraft::repo::ebuild::Repo;

use crate::report::ReportKind::{LicenseDeprecated, LicenseMissing, LicenseUnneeded};
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, VersionCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::License,
    scope: Scope::Version,
    source: SourceKind::Ebuild,
    reports: &[LicenseDeprecated, LicenseMissing, LicenseUnneeded],
    context: &[],
    priority: 0,
};

pub(super) fn create(repo: &'static Repo) -> impl VersionCheck {
    Check {
        deprecated: repo
            .license_groups()
            .get("DEPRECATED")
            .map(|x| x.iter().collect())
            .unwrap_or_default(),
        unlicensed_categories: ["acct-group", "acct-user", "virtual"]
            .iter()
            .map(|x| x.to_string())
            .collect(),
    }
}

struct Check {
    deprecated: IndexSet<&'static String>,
    unlicensed_categories: IndexSet<String>,
}

super::register!(Check);

impl VersionCheck for Check {
    fn run(&self, pkg: &Pkg, filter: &mut ReportFilter) {
        let licenses: IndexSet<_> = pkg.license().iter_flatten().collect();
        if licenses.is_empty() {
            if !self.unlicensed_categories.contains(pkg.category()) {
                filter.report(LicenseMissing.version(pkg, ""));
            }
        } else if self.unlicensed_categories.contains(pkg.category()) {
            filter.report(LicenseUnneeded.version(pkg, ""));
        } else {
            let deprecated = licenses.intersection(&self.deprecated).sorted().join(", ");
            if !deprecated.is_empty() {
                filter.report(LicenseDeprecated.version(pkg, deprecated));
            }
        }
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
        let check_dir = repo.path().join(CHECK);
        let report_dir = repo.path().join("virtual/LicenseUnneeded");
        let scanner = Scanner::new().jobs(1).checks([CHECK]);
        let expected = glob_reports!("{check_dir}/*/reports.json", "{report_dir}/reports.json");
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);

        // primary fixed
        let repo = TEST_DATA_PATCHED.repo("qa-primary").unwrap();
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &[]);
    }
}
