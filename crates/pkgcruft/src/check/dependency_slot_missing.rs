use dashmap::{DashMap, mapref::one::Ref};
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::repo::{EbuildRepo, PkgRepository};
use pkgcraft::restrict::{Restrict, Scope};

use crate::report::ReportKind::DependencySlotMissing;
use crate::scan::ScannerRun;
use crate::source::SourceKind;

super::register! {
    kind: super::CheckKind::DependencySlotMissing,
    reports: &[DependencySlotMissing],
    scope: Scope::Version,
    sources: &[SourceKind::EbuildPkg],
    context: &[],
    create,
}

pub(super) fn create(_run: &ScannerRun) -> crate::Result<super::Runner> {
    Ok(Box::new(Check { dep_slots: Default::default() }))
}

struct Check {
    dep_slots: DashMap<Restrict, Option<String>>,
}

impl Check {
    /// Get the package slots for a dependency if more than one exist.
    fn get_slots<R: Into<Restrict>>(
        &self,
        repo: &EbuildRepo,
        dep: R,
    ) -> Ref<'_, Restrict, Option<String>> {
        let restrict = dep.into();
        self.dep_slots
            .entry(restrict.clone())
            .or_insert_with(|| {
                let slots = repo
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

impl super::CheckRun for Check {
    fn run_ebuild_pkg(&self, pkg: &EbuildPkg, run: &ScannerRun) {
        for dep in pkg
            .rdepend()
            .intersection(pkg.depend())
            .flat_map(|x| x.iter_flatten())
            .filter(|x| x.blocker().is_none() && x.slot_dep().is_none())
        {
            if let Some(slots) = self.get_slots(&run.repo, dep.no_use_deps()).as_ref() {
                DependencySlotMissing
                    .version(pkg)
                    .message(format!("{dep} matches multiple slots: {slots}"))
                    .report(run);
            }
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
