use std::collections::HashSet;

use itertools::Itertools;
use pkgcraft::dep::Cpn;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::restrict::Scope;

use crate::report::ReportKind::{
    UseLocalDescMissing, UseLocalGlobal, UseLocalUnsorted, UseLocalUnused,
};
use crate::scan::ScannerRun;
use crate::source::SourceKind;

super::register! {
    super::Check {
        kind: super::CheckKind::UseLocal,
        reports: &[UseLocalDescMissing, UseLocalGlobal, UseLocalUnsorted, UseLocalUnused],
        scope: Scope::Package,
        sources: &[SourceKind::EbuildPkg],
        context: &[],
        create,
    }
}

pub(super) fn create(_run: &ScannerRun) -> super::Runner {
    Box::new(Check)
}

struct Check;

impl super::CheckRun for Check {
    fn run_ebuild_pkg_set(&self, cpn: &Cpn, pkgs: &[EbuildPkg], run: &ScannerRun) {
        let metadata = run.repo.metadata().pkg_metadata(cpn);
        let local_use = metadata.local_use();
        let sorted_flags = local_use
            .keys()
            .map(|s| s.as_str())
            .sorted()
            .collect::<Vec<_>>();

        let mut unsorted = false;
        for ((flag, desc), sorted) in local_use.iter().zip(&sorted_flags) {
            if desc.is_empty() {
                UseLocalDescMissing.package(cpn).message(flag).report(run);
            }

            if !unsorted && flag != sorted {
                unsorted = true;
                UseLocalUnsorted
                    .package(cpn)
                    .message(format!("unsorted flag: {flag} (sorted: {sorted})"))
                    .report(run);
            }

            if let Some(global_desc) = run.repo.metadata().use_global().get(flag)
                && global_desc == desc
            {
                UseLocalGlobal.package(cpn).message(flag).report(run);
            }
        }

        let used = pkgs
            .iter()
            .flat_map(|pkg| pkg.iuse())
            .map(|iuse| iuse.flag())
            .collect::<HashSet<_>>();
        let unused = sorted_flags
            .iter()
            .filter(|&x| !used.contains(x))
            .join(", ");

        if !unused.is_empty() {
            UseLocalUnused.package(cpn).message(unused).report(run);
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
