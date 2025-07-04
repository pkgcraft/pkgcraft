use std::collections::HashSet;

use itertools::Itertools;
use pkgcraft::pkg::ebuild::EbuildPkg;

use crate::report::ReportKind::PropertiesInvalid;
use crate::scan::ScannerRun;

use super::EbuildPkgCheck;

pub(super) fn create(run: &ScannerRun) -> impl EbuildPkgCheck + 'static {
    Check {
        allowed: run
            .repo
            .trees()
            .flat_map(|r| r.metadata().config.properties_allowed.clone())
            .collect(),
    }
}

struct Check {
    allowed: HashSet<String>,
}

super::register!(Check, super::Check::Properties);

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, run: &ScannerRun) {
        if !self.allowed.is_empty() {
            let vals = pkg
                .properties()
                .iter_flatten()
                .filter(|&x| !self.allowed.contains(x))
                .collect::<HashSet<_>>();

            if !vals.is_empty() {
                let vals = vals.iter().sorted().join(", ");
                PropertiesInvalid.version(pkg).message(vals).report(run);
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
