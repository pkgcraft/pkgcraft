use std::collections::HashSet;

use dashmap::DashSet;
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::pkg::{Package, ebuild::EbuildPkg};
use pkgcraft::restrict::Scope;

use crate::report::ReportKind::{LicenseDeprecated, LicenseInvalid, LicensesUnused};
use crate::scan::ScannerRun;
use crate::source::SourceKind;

super::register! {
    kind: super::CheckKind::License,
    reports: &[LicenseDeprecated, LicensesUnused, LicenseInvalid],
    scope: Scope::Version,
    sources: &[SourceKind::EbuildPkg],
    context: &[],
    create,
}

pub(super) fn create(run: &ScannerRun) -> super::Runner {
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

    Box::new(Check {
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
    })
}

struct Check {
    deprecated: IndexSet<String>,
    missing_categories: HashSet<String>,
    unused: DashSet<String>,
}

impl super::CheckRun for Check {
    fn run_ebuild_pkg(&self, pkg: &EbuildPkg, run: &ScannerRun) {
        let licenses: IndexSet<_> = pkg.license().iter_flatten().cloned().collect();
        let allowed_missing = self.missing_categories.contains(pkg.category());
        if licenses.is_empty() {
            if !allowed_missing {
                LicenseInvalid.version(pkg).message("missing").report(run);
            }
        } else if allowed_missing {
            LicenseInvalid.version(pkg).message("unneeded").report(run);
        } else {
            let deprecated = licenses.intersection(&self.deprecated).sorted().join(", ");
            if !deprecated.is_empty() {
                LicenseDeprecated
                    .version(pkg)
                    .message(deprecated)
                    .report(run);
            }
        }

        for license in licenses {
            if !run.repo.licenses().contains(&license) {
                LicenseInvalid
                    .version(pkg)
                    .message(format!("nonexistent: {license}"))
                    .report(run);
            }

            // mangle values for post-run finalization
            if run.enabled(LicensesUnused) {
                self.unused.remove(&license);
            }
        }
    }

    fn finish(&self, run: &ScannerRun) {
        if run.enabled(LicensesUnused) && !self.unused.is_empty() {
            let unused = self
                .unused
                .iter()
                .map(|x| x.to_string())
                .sorted()
                .join(", ");
            LicensesUnused.repo(&run.repo).message(unused).report(run);
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::test::{test_data, test_data_patched};

    use crate::scan::Scanner;
    use crate::test::{assert_unordered_reports, glob_reports};

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
        assert_unordered_reports!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, []);
    }
}
