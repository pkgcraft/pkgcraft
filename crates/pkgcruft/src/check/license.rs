use std::collections::HashSet;

use dashmap::DashSet;
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::pkg::{ebuild::EbuildPkg, Package};
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::restrict::Scope;

use crate::report::ReportKind::{LicenseDeprecated, LicenseInvalid, LicensesUnused};
use crate::scanner::ReportFilter;
use crate::source::SourceKind;

use super::{CheckKind, EbuildPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::License,
    scope: Scope::Version,
    source: SourceKind::EbuildPkg,
    reports: &[LicenseDeprecated, LicensesUnused, LicenseInvalid],
    context: &[],
};

pub(super) fn create(repo: &EbuildRepo) -> impl EbuildPkgCheck {
    Check {
        deprecated: repo
            .license_groups()
            .get("DEPRECATED")
            .map(|x| x.iter().cloned().collect())
            .unwrap_or_default(),
        missing_categories: ["acct-group", "acct-user", "virtual"]
            .iter()
            .map(|x| x.to_string())
            .collect(),
        unused: repo.metadata().licenses().iter().map(Into::into).collect(),
        repo: repo.clone(),
    }
}

struct Check {
    deprecated: IndexSet<String>,
    missing_categories: HashSet<String>,
    unused: DashSet<String>,
    repo: EbuildRepo,
}

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, filter: &mut ReportFilter) {
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
            if filter.finalize(LicensesUnused) {
                self.unused.remove(&license);
            }
        }
    }

    fn finish(&self, repo: &EbuildRepo, filter: &mut ReportFilter) {
        if !self.unused.is_empty() {
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

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // primary unfixed
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let check_dir = repo.path().join(CHECK);
        let report_dir = repo.path().join("virtual/LicenseInvalid");
        let scanner = Scanner::new(repo).checks([CHECK]);
        let expected =
            glob_reports!("{check_dir}/*/reports.json", "{report_dir}/reports.json");
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
