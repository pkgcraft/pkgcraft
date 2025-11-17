use std::collections::HashMap;

use itertools::Itertools;
use pkgcraft::dep::{Cpn, Cpv};
use pkgcraft::restrict::Scope;

use crate::report::ReportKind::{EbuildNameInvalid, EbuildVersionsEqual};
use crate::scan::ScannerRun;
use crate::source::SourceKind;

super::register! {
    kind: super::CheckKind::EbuildName,
    reports: &[EbuildNameInvalid, EbuildVersionsEqual],
    scope: Scope::Package,
    sources: &[SourceKind::Cpn],
    context: &[],
    create,
}

pub(super) fn create(_run: &ScannerRun) -> super::Runner {
    Box::new(Check)
}

struct Check;

impl super::CheckRun for Check {
    fn run_cpn(&self, cpn: &Cpn, run: &ScannerRun) {
        let mut cpvs = HashMap::<Cpv, Vec<_>>::new();

        for result in run.repo.cpvs_from_package(cpn.category(), cpn.package()) {
            match result {
                Err(e) => EbuildNameInvalid.package(cpn).message(e).report(run),
                Ok(cpv) => {
                    let version = cpv.version().to_string();
                    cpvs.entry(cpv).or_default().push(version);
                }
            }
        }

        for versions in cpvs.values().filter(|x| x.len() > 1) {
            let versions = versions.iter().sorted().join(", ");
            EbuildVersionsEqual
                .package(cpn)
                .message(format!("versions overlap: {versions}"))
                .report(run);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::env;

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
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, expected);

        // verify scanning in a package with only invalid names
        let dir = dir.join("EbuildNameInvalid");
        env::set_current_dir(&dir).unwrap();
        let expected = glob_reports!("{dir}/reports.json");
        let reports = scanner.run(repo, &dir).unwrap();
        assert_unordered_reports!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, []);
    }
}
