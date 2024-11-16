use std::collections::HashSet;

use itertools::Itertools;
use pkgcraft::dep::Cpn;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::repo::ebuild::EbuildRepo;

use crate::report::ReportKind::{
    UseLocalDescMissing, UseLocalGlobal, UseLocalUnsorted, UseLocalUnused,
};
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, EbuildPkgSetCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::UseLocal,
    scope: Scope::Package,
    source: SourceKind::EbuildPkg,
    reports: &[UseLocalDescMissing, UseLocalGlobal, UseLocalUnused, UseLocalUnsorted],
    context: &[],
    priority: 0,
};

pub(super) fn create(repo: &'static EbuildRepo) -> impl EbuildPkgSetCheck {
    Check { repo }
}

struct Check {
    repo: &'static EbuildRepo,
}

super::register!(Check);

impl EbuildPkgSetCheck for Check {
    fn run(&self, cpn: &Cpn, pkgs: &[EbuildPkg], filter: &mut ReportFilter) {
        let metadata = self.repo.metadata().pkg(cpn);
        let local_use = metadata.local_use();
        let sorted_flags = local_use
            .keys()
            .map(|s| s.as_str())
            .sorted()
            .collect::<Vec<_>>();

        let mut unsorted = false;
        for ((flag, desc), sorted) in local_use.iter().zip(&sorted_flags) {
            if desc.is_empty() {
                UseLocalDescMissing
                    .package(cpn)
                    .message(flag)
                    .report(filter);
            }

            if !unsorted && flag != sorted {
                unsorted = true;
                UseLocalUnsorted
                    .package(cpn)
                    .message(format!("unsorted flag: {flag} (sorted: {sorted})"))
                    .report(filter);
            }

            if let Some(global_desc) = self.repo.metadata().use_global().get(flag) {
                if global_desc == desc {
                    UseLocalGlobal.package(cpn).message(flag).report(filter);
                }
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
            UseLocalUnused.package(cpn).message(unused).report(filter);
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{assert_unordered_eq, TEST_DATA, TEST_DATA_PATCHED};

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // primary unfixed
        let repo = TEST_DATA.repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new().checks([CHECK]);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // primary fixed
        let repo = TEST_DATA_PATCHED.repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, []);
    }
}
