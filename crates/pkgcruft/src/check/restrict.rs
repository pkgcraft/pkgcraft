use std::collections::HashSet;

use itertools::Itertools;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::restrict::Scope;

use crate::report::ReportKind::RestrictInvalid;
use crate::scan::ScannerRun;
use crate::source::SourceKind;

super::register! {
    kind: super::CheckKind::Restrict,
    reports: &[RestrictInvalid],
    scope: Scope::Version,
    sources: &[SourceKind::EbuildPkg],
    context: &[],
    create,
}

pub(super) fn create(run: &ScannerRun) -> crate::Result<super::Runner> {
    Ok(Box::new(Check {
        allowed: run
            .repo
            .trees()
            .flat_map(|r| r.metadata().config.restrict_allowed.clone())
            .collect(),
    }))
}

struct Check {
    allowed: HashSet<String>,
}

impl super::CheckRun for Check {
    fn run_ebuild_pkg(&self, pkg: &EbuildPkg, run: &ScannerRun) {
        if !self.allowed.is_empty() {
            let vals = pkg
                .restrict()
                .iter_flatten()
                .filter(|&x| !self.allowed.contains(x))
                .collect::<HashSet<_>>();

            if !vals.is_empty() {
                let vals = vals.iter().sorted().join(", ");
                RestrictInvalid.version(pkg).message(vals).report(run);
            }
        }
        // TODO: verify USE flags in conditionals
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
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, []);
    }
}
