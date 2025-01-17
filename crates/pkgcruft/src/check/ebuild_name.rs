use std::collections::HashMap;

use itertools::Itertools;
use pkgcraft::dep::{Cpn, Cpv};
use pkgcraft::repo::ebuild::EbuildRepo;

use crate::iter::ReportFilter;
use crate::report::ReportKind::{EbuildNameInvalid, EbuildVersionsEqual};
use crate::scan::ScannerRun;

use super::CpnCheck;

pub(super) fn create(run: &ScannerRun) -> impl CpnCheck {
    Check { repo: run.repo.clone() }
}

static CHECK: super::Check = super::Check::EbuildName;

struct Check {
    repo: EbuildRepo,
}

super::register!(Check);

impl CpnCheck for Check {
    fn run(&self, cpn: &Cpn, filter: &ReportFilter) {
        let mut cpvs = HashMap::<Cpv, Vec<_>>::new();

        for result in self.repo.cpvs_from_package(cpn.category(), cpn.package()) {
            match result {
                Err(e) => EbuildNameInvalid.package(cpn).message(e).report(filter),
                Ok(cpv) => {
                    let version = cpv.version().to_string();
                    cpvs.entry(cpv).or_default().push(version);
                }
            }
        }

        for versions in cpvs.values() {
            if versions.len() > 1 {
                let versions = versions.iter().sorted().join(", ");
                EbuildVersionsEqual
                    .package(cpn)
                    .message(format!("versions overlap: {versions}"))
                    .report(filter);
            }
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
        let scanner = Scanner::new().reports([CHECK]);

        // primary unfixed
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // verify scanning in a package with only invalid names
        let dir = dir.join("EbuildNameInvalid");
        env::set_current_dir(&dir).unwrap();
        let expected = glob_reports!("{dir}/reports.json");
        let reports = scanner.run(repo, &dir).unwrap();
        assert_unordered_eq!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, []);
    }
}
