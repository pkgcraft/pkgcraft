use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::pkg::ebuild::EbuildPkg;
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
};

pub(super) fn create(repo: &'static EbuildRepo) -> impl EbuildPkgCheck {
    Check { repo }
}

struct Check {
    repo: &'static EbuildRepo,
}

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
                .filter_map(Result::ok)
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
    use pkgcraft::test::{assert_unordered_eq, test_data, test_data_patched};

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // primary unfixed
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new(repo).checks([CHECK]);
        let expected = glob_reports!("{dir}/*/reports.json");
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
