use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::pkg::ebuild::{EbuildPackage, EbuildPkg};
use pkgcraft::repo::{ebuild::EbuildRepo, PkgRepository};

use crate::report::ReportKind::DependencySlotMissing;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, EbuildPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::DependencySlotMissing,
    scope: Scope::Version,
    source: SourceKind::EbuildPkg,
    reports: &[DependencySlotMissing],
    context: &[],
    priority: 0,
};

pub(super) fn create(repo: &'static EbuildRepo) -> impl EbuildPkgCheck {
    Check { repo }
}

struct Check {
    repo: &'static EbuildRepo,
}

super::register!(Check);

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, filter: &mut ReportFilter) {
        for dep in pkg
            .rdepend()
            .intersection(pkg.depend())
            .flat_map(|x| x.iter_flatten())
            .filter(|x| x.blocker().is_none() && x.slot_dep().is_none())
        {
            // TODO: use cached lookup instead of searching for each dep
            let slots = self
                .repo
                .iter_restrict(dep.no_use_deps())
                .map(|pkg| pkg.slot().to_string())
                .collect::<IndexSet<_>>();
            if slots.len() > 1 {
                let slots = slots.iter().join(", ");
                DependencySlotMissing
                    .version(pkg)
                    .message(format!("{dep} matches multiple slots: {slots}"))
                    .report(filter);
            }
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
        let (pool, repo) = TEST_DATA.repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new(&pool).checks([CHECK]);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo, repo);
        assert_unordered_eq!(reports, expected);

        // primary fixed
        let (pool, repo) = TEST_DATA_PATCHED.repo("qa-primary").unwrap();
        let scanner = Scanner::new(&pool).checks([CHECK]);
        let reports = scanner.run(repo, repo);
        assert_unordered_eq!(reports, []);
    }
}
