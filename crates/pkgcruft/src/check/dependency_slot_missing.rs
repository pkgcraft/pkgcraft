use dashmap::{mapref::one::Ref, DashMap};
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::repo::{ebuild::EbuildRepo, PkgRepository};
use pkgcraft::restrict::Restrict;

use crate::iter::ReportFilter;
use crate::report::ReportKind::DependencySlotMissing;

use super::EbuildPkgCheck;

pub(super) fn create(repo: &EbuildRepo) -> impl EbuildPkgCheck {
    Check {
        repo: repo.clone(),
        dep_slots: Default::default(),
    }
}

static CHECK: super::Check = super::Check::DependencySlotMissing;

struct Check {
    repo: EbuildRepo,
    dep_slots: DashMap<Restrict, Option<String>>,
}

super::register!(Check);

impl Check {
    /// Get the package slots matching a given dependency.
    fn get_dep_slots<R: Into<Restrict>>(&self, dep: R) -> Ref<Restrict, Option<String>> {
        let restrict = dep.into();
        self.dep_slots
            .entry(restrict.clone())
            .or_insert_with(|| {
                let slots = self
                    .repo
                    .iter_restrict(restrict)
                    .filter_map(Result::ok)
                    .map(|pkg| pkg.slot().to_string())
                    .collect::<IndexSet<_>>();
                if slots.len() > 1 {
                    Some(slots.iter().join(", "))
                } else {
                    None
                }
            })
            .downgrade()
    }
}

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, filter: &ReportFilter) {
        for dep in pkg
            .rdepend()
            .intersection(pkg.depend())
            .flat_map(|x| x.iter_flatten())
            .filter(|x| x.blocker().is_none() && x.slot_dep().is_none())
        {
            if let Some(slots) = self.get_dep_slots(dep.no_use_deps()).as_ref() {
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

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, []);
    }
}
