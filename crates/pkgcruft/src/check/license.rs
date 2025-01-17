use std::collections::HashSet;

use dashmap::DashSet;
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::pkg::{ebuild::EbuildPkg, Package};
use pkgcraft::repo::ebuild::EbuildRepo;

use crate::iter::ReportFilter;
use crate::report::ReportKind::{LicenseDeprecated, LicenseInvalid, LicensesUnused};
use crate::scan::ScannerRun;

use super::EbuildPkgCheck;

pub(super) fn create(run: &ScannerRun) -> impl EbuildPkgCheck {
    let unused = if run.enabled(LicensesUnused) {
        run.repo
            .metadata()
            .licenses()
            .iter()
            .map(Into::into)
            .collect()
    } else {
        Default::default()
    };

    Check {
        deprecated: run
            .repo
            .license_groups()
            .get("DEPRECATED")
            .map(|x| x.iter().cloned().collect())
            .unwrap_or_default(),
        missing_categories: ["acct-group", "acct-user", "virtual"]
            .iter()
            .map(|x| x.to_string())
            .collect(),
        unused,
        repo: run.repo.clone(),
    }
}

static CHECK: super::Check = super::Check::License;

struct Check {
    deprecated: IndexSet<String>,
    missing_categories: HashSet<String>,
    unused: DashSet<String>,
    repo: EbuildRepo,
}

super::register!(Check);

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, filter: &ReportFilter) {
        let licenses: IndexSet<_> = pkg.license().iter_flatten().cloned().collect();
        let allowed_missing = self.missing_categories.contains(pkg.category());
        if licenses.is_empty() {
            if !allowed_missing {
                LicenseInvalid
                    .version(pkg)
                    .message("missing")
                    .report(filter);
            }
        } else if allowed_missing {
            LicenseInvalid
                .version(pkg)
                .message("unneeded")
                .report(filter);
        } else {
            let deprecated = licenses.intersection(&self.deprecated).sorted().join(", ");
            if !deprecated.is_empty() {
                LicenseDeprecated
                    .version(pkg)
                    .message(deprecated)
                    .report(filter);
            }
        }

        for license in licenses {
            if !self.repo.licenses().contains(&license) {
                LicenseInvalid
                    .version(pkg)
                    .message(format!("nonexistent: {license}"))
                    .report(filter);
            }

            // mangle values for post-run finalization
            if filter.enabled(LicensesUnused) {
                self.unused.remove(&license);
            }
        }
    }

    fn finish_check(&self, repo: &EbuildRepo, filter: &ReportFilter) {
        if filter.enabled(LicensesUnused) && !self.unused.is_empty() {
            let unused = self
                .unused
                .iter()
                .map(|x| x.to_string())
                .sorted()
                .join(", ");
            LicensesUnused.repo(repo).message(unused).report(filter);
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
        let dir = repo.path().join(CHECK);
        let unneeded = repo.path().join("virtual/LicenseInvalid");
        let expected = glob_reports!("{dir}/**/reports.json", "{unneeded}/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, []);
    }
}
