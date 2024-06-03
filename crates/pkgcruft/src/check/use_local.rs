use std::collections::HashSet;

use itertools::Itertools;
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::repo::ebuild::Repo;

use crate::report::ReportKind::{
    UseLocalDescMissing, UseLocalGlobal, UseLocalUnsorted, UseLocalUnused,
};
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

pub(super) static CHECK: super::Check = super::Check {
    name: "UseLocal",
    scope: Scope::Package,
    source: SourceKind::Ebuild,
    reports: &[UseLocalDescMissing, UseLocalGlobal, UseLocalUnused, UseLocalUnsorted],
    context: &[],
    priority: 0,
    create,
};

fn create(repo: &Repo) -> super::Runner {
    super::Runner::UseLocal(Check { repo })
}

#[derive(Debug)]
pub(crate) struct Check<'a> {
    repo: &'a Repo,
}

impl<'a> super::CheckRun<&[Pkg<'a>]> for Check<'a> {
    fn run(&self, pkgs: &[Pkg<'a>], filter: &mut ReportFilter) {
        let local_use = pkgs[0].local_use();
        let sorted_flags = local_use
            .keys()
            .map(|s| s.as_str())
            .sorted()
            .collect::<Vec<_>>();

        let mut unsorted = false;
        for ((flag, desc), sorted) in local_use.iter().zip(&sorted_flags) {
            if desc.is_empty() {
                filter.report(UseLocalDescMissing.package(pkgs, flag));
            }

            if !unsorted && flag != sorted {
                let message = format!("unsorted flag: {flag} (sorted: {sorted})");
                filter.report(UseLocalUnsorted.package(pkgs, message));
                unsorted = true;
            }

            if let Some(global_desc) = self.repo.metadata.use_global().get(flag) {
                if global_desc == desc {
                    filter.report(UseLocalGlobal.package(pkgs, flag));
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
            .collect::<Vec<_>>();

        if !unused.is_empty() {
            let message = unused.iter().join(", ");
            filter.report(UseLocalUnused.package(pkgs, message));
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
        let check_dir = repo.path().join(&CHECK);
        let scanner = Scanner::new().jobs(1).checks([&CHECK]);
        let expected = glob_reports!("{check_dir}/*/reports.json");
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);

        // primary fixed
        let repo = TEST_DATA_PATCHED.repo("qa-primary").unwrap();
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &[]);
    }
}
